import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

for (const provider of ['local', 'git']) test(`POSIX shell SDK forwards ${provider} content, JSON, proof and failure status`, () => {
  const root = mkdtempSync(join(tmpdir(), 'ss-'));
  try {
    const input = join(root, 'input with spaces');
    mkdirSync(input);
    writeFileSync(join(input, 'asset.txt'), 'hello');
    let source = { provider: 'local', path: input };
    if (provider === 'git') {
      mkdirSync(join(input, 'shared'));
      writeFileSync(join(input, 'shared/asset.txt'), 'hello');
      const git = (...args) => execFileSync('git', ['-C', input, '-c', 'user.name=Shell SDK tests', '-c', 'user.email=sdk@example.invalid', '-c', 'core.hooksPath=/dev/null', ...args], { stdio: 'pipe' });
      git('init', '-b', 'main'); git('add', '.');
      const oid = execFileSync('git', ['-C', input, 'hash-object', '-w', '--stdin'], { input: 'shared/asset.txt', encoding: 'utf8' }).trim();
      git('update-index', '--add', '--cacheinfo', `120000,${oid},asset.txt`);
      git('commit', '-m', 'symlink fixture');
      source = { provider: 'git', origin: input, reference: 'main' };
    }
    writeFileSync(join(root, 'recipe.json'), JSON.stringify({ source }));
    writeFileSync(join(root, 'settings.json'), JSON.stringify({ retain_snapshots: true, verify_content: true, allow_local_fallback: false }));
    const binary = process.env.SAUCEPAN_TEST_BINARY || fileURLToPath(new URL('../../../target/debug/' + (process.platform === 'win32' ? 'saucepan.exe' : 'saucepan'), import.meta.url));
    execFileSync(process.env.SAUCEPAN_TEST_SHELL || 'sh', [
      fileURLToPath(new URL('./contract.sh', import.meta.url)).replaceAll('\\', '/'),
      fileURLToPath(new URL('../saucepan.sh', import.meta.url)).replaceAll('\\', '/'),
      root.replaceAll('\\', '/'),
    ], { env: { ...process.env, SAUCEPAN_BIN: binary.replaceAll('\\', '/') }, stdio: 'pipe' });
    const json = name => JSON.parse(readFileSync(join(root, name), 'utf8'));
    assert.equal(json('proof.json').app, 'app with spaces;$(literal)');
    const acquired = json('acquired.json');
    assert.equal(readFileSync(join(acquired.directory, 'asset.txt'), 'utf8'), 'hello');
    assert.deepEqual(Object.keys(json('view.json').entries), [acquired.artifact.id]);
    assert.deepEqual(json('verified.json'), { verified: true });
    assert.equal(json('configured-view.json').settings.verify_content, true);
    assert.equal(readFileSync(join(root, 'wrong.out'), 'utf8'), '');
    assert.ok(readFileSync(join(root, 'wrong.err'), 'utf8').length);
    assert.ok(json('executable.json').includes('.saucepan'));
    execFileSync(process.env.SAUCEPAN_TEST_SHELL || 'sh', [
      fileURLToPath(new URL('./content.sh', import.meta.url)).replaceAll('\\', '/'),
      fileURLToPath(new URL('../saucepan.sh', import.meta.url)).replaceAll('\\', '/'),
      root.replaceAll('\\', '/'), acquired.artifact.id, acquired.artifact.source_id, acquired.artifact.snapshot_id,
    ], { env: { ...process.env, SAUCEPAN_BIN: binary.replaceAll('\\', '/') }, stdio: 'pipe' });
    assert.equal(json('path.json'), acquired.directory);
    assert.equal(json('history.json').current.id, acquired.artifact.snapshot_id);
    assert.equal(json('snapshot.json').artifact.id, acquired.artifact.id);
    assert.equal(readFileSync(join(root, 'mirror with spaces/asset.txt'), 'utf8'), 'hello');
    assert.deepEqual(json('ordinary.json').entries, {});
  } finally { rmSync(root, { recursive: true, force: true }); }
});
