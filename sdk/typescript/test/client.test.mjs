import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, readdir } from 'node:fs/promises';
import { tmpdir, homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { execFileSync } from 'node:child_process';
import { Saucepan, SaucepanError, sharedExecutablePath } from '../dist/index.js';

const actualBinary = process.env.SAUCEPAN_TEST_BINARY || resolve(fileURLToPath(new URL('../../../target/debug/', import.meta.url)), process.platform === 'win32' ? 'saucepan.exe' : 'saucepan');
async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), 'st-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const options = { binary: actualBinary, testStore: { root: join(root, 'store'), key: '07'.repeat(32) } };
  const store = new Saucepan(options);
  await store.init();
  return { root, store, options };
}
const checked = { retain_snapshots: true, verify_content: true, allow_local_fallback: false };

test('central lifecycle, typed settings, history and independent mirrors', async t => {
  const { root, store } = await fixture(t);
  const proof = await store.register('app-a');
  const app = store.forApp(proof);
  const other = store.forApp(await store.register('app-b'));
  const input = join(root, 'input with spaces');
  await mkdir(input);
  await writeFile(join(input, 'asset.txt'), 'first');
  const recipe = { source: { provider: 'local', path: input } };
  const first = await app.acquire(recipe);
  assert.equal(await readFile(join(first.directory, 'asset.txt'), 'utf8'), 'first');
  assert.equal(await app.path(first.artifact.id), first.directory);
  assert.equal(await other.path(first.artifact.id), null);
  assert.deepEqual((await other.view()).entries, {});
  await app.configure({ settings: checked });
  await writeFile(join(input, 'asset.txt'), 'second');
  const second = await app.acquire(recipe);
  assert.equal(second.content_verified, true);
  const history = await app.history(first.artifact.source_id);
  assert.equal(history.current.id, second.artifact.snapshot_id);
  assert.equal(history.history[0].id, first.artifact.snapshot_id);
  const previous = await app.snapshot(first.artifact.source_id, first.artifact.snapshot_id);
  assert.equal(await readFile(join(previous.directory, 'asset.txt'), 'utf8'), 'first');
  const mirror = join(root, 'mirror ; literal');
  await app.mirror(first.artifact.id, mirror);
  await writeFile(join(mirror, 'asset.txt'), 'edited');
  assert.equal(await readFile(join(first.directory, 'asset.txt'), 'utf8'), 'first');
  await assert.rejects(app.mirror(first.artifact.id, mirror), error => error instanceof SaucepanError && error.exitCode === 1);
  const view = await app.view();
  assert.deepEqual(await app.verify(view), { verified: true });
  await assert.rejects(app.verify({ ...view, app: 'app-b' }), SaucepanError);
  assert.equal(await app.sharedExecutable(), sharedExecutablePath());
});

test('literal arguments, stable caller-owned marker, concurrency and failed-request cleanup', async t => {
  const { root, store, options } = await fixture(t);
  const name = 'app spaces;$(never-execute)&quote"';
  const proof = await store.register(name);
  assert.equal(proof.app, name);
  await store.register('-literal-app');
  assert.equal((await store.forApp('-literal-app').view()).app, '-literal-app');
  const marker = join(root, 'marker with spaces.json');
  const bytes = JSON.stringify(proof);
  await writeFile(marker, bytes);
  const app = new Saucepan({ ...options, marker });
  await app.configure({ settings: checked, filters: { source_ids: [], providers: ['local'] } });
  await Promise.all(Array.from({ length: 4 }, async () => app.verify(await app.view())));
  assert.equal(await readFile(marker, 'utf8'), bytes);
  const before = (await readdir(tmpdir())).filter(name => name.startsWith('saucepan-request-'));
  await assert.rejects(app.acquire({ source: { provider: 'unsupported' } }), SaucepanError);
  const after = (await readdir(tmpdir())).filter(name => name.startsWith('saucepan-request-'));
  assert.deepEqual(after, before);
  const bad = { ...proof, token: proof.token.map(byte => byte ^ 1) };
  await assert.rejects(store.forApp(bad).view(), error => error.exitCode === 1 && error.stderr.length > 0);
  assert.deepEqual(await store.forApp(name).verify(await app.view()).catch(e => e.exitCode), 1);
});

test('URL downloads and timeout failures use the same core', async t => {
  const { store, options } = await fixture(t);
  const app = store.forApp(await store.register('url-app'));
  const server = createServer((request, response) => {
    if (request.url === '/slow') return;
    response.end('downloaded');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => { server.closeAllConnections(); server.close(); });
  const url = `http://127.0.0.1:${server.address().port}`;
  const acquired = await app.acquire({ source: { provider: 'url', url, download: { format: 'file', name: 'asset.txt' } } });
  assert.equal(await readFile(join(acquired.directory, 'asset.txt'), 'utf8'), 'downloaded');
  const impatient = new Saucepan({ ...options, app: 'url-app', timeoutMs: 100 });
  await assert.rejects(impatient.acquire({ source: { provider: 'url', url: url + '/slow', download: { format: 'file', name: 'asset.txt' } } }), e => e instanceof SaucepanError && e.killed);
});

test('default path and missing executable errors never download a binary', async () => {
  assert.equal(sharedExecutablePath(), join(homedir(), '.saucepan', 'bin', process.platform === 'win32' ? 'saucepan.exe' : 'saucepan'));
  const client = new Saucepan({ binary: join(tmpdir(), 'missing-saucepan-binary') });
  await assert.rejects(client.view(), e => e instanceof SaucepanError && e.code === 'ENOENT' && e.exitCode === null);
  assert.throws(() => new Saucepan({ testStore: { root: 'test' } }), /key/i);
});

test('Git folders share one source and retain the requested ref', async t => {
  const { root, store } = await fixture(t);
  const repo = join(root, 'git input');
  await mkdir(join(repo, 'a'), { recursive: true });
  await mkdir(join(repo, 'b'));
  await writeFile(join(repo, 'a/file'), 'A');
  await writeFile(join(repo, 'b/file'), 'B');
  const git = (...args) => execFileSync('git', ['-C', repo, '-c', 'user.name=SDK tests', '-c', 'user.email=sdk@example.invalid', '-c', 'core.hooksPath=/dev/null', ...args], { stdio: 'pipe' });
  git('init', '-b', 'main'); git('add', '.');
  for (const [name, target] of [['a/linked', '../b/file'], ['alias', 'b']]) {
    const oid = execFileSync('git', ['-C', repo, 'hash-object', '-w', '--stdin'], { input: target, encoding: 'utf8' }).trim();
    git('update-index', '--add', '--cacheinfo', `120000,${oid},${name}`);
  }
  git('commit', '-m', 'fixture');
  const app = store.forApp(await store.register('git-app'));
  const source = { provider: 'git', origin: repo, reference: 'main' };
  const a = await app.acquire({ source, folder: 'a' });
  const b = await app.acquire({ source, folder: 'b' });
  assert.equal(a.artifact.source_id, b.artifact.source_id);
  assert.equal(a.artifact.snapshot_id, b.artifact.snapshot_id);
  assert.equal(await readFile(join(a.directory, 'file'), 'utf8'), 'A');
  assert.equal(await readFile(join(a.directory, 'linked'), 'utf8'), 'B');
  assert.equal(await readFile(join(b.directory, 'file'), 'utf8'), 'B');
  const alias = await app.acquire({ source, folder: 'alias' });
  assert.equal(alias.artifact.snapshot_id, b.artifact.snapshot_id);
  assert.equal(await readFile(join(alias.directory, 'file'), 'utf8'), 'B');
  await app.verify(await app.view());
  const historical = await app.snapshot(a.artifact.source_id, a.artifact.snapshot_id, 'b');
  assert.equal(historical.artifact.id, b.artifact.id);
});
