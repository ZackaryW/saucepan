import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, access } from 'node:fs/promises';
import { Client, TransportError, SaucepanError } from '../dist/index.js';
import { NodeTransport } from '../dist/transport.js';
import { invoke } from '../dist/utils/process.js';
import { payloadFiles } from '../dist/utils/payloads.js';
import { decode, encode } from '../dist/utils/json.js';

test('literal Unicode argv, both streams and raw status', async () => {
  const args = ['雪 space', '"quoted"', '$(echo bad); & | >'];
  const result = await invoke(process.execPath, ['-e', 'console.log(JSON.stringify(process.argv.slice(1))); process.stderr.write("e".repeat(100000)); process.exitCode=7', ...args], { capture: true });
  assert.equal(result.exitCode, 7);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(result.stdout)), args);
  assert.equal(result.stderr.byteLength, 100000);
});

test('capture limits, timeout, cancellation, missing executable', async () => {
  await assert.rejects(invoke(process.execPath, ['-e', 'process.stdout.write("x".repeat(100000))'], { capture: true, captureLimit: 10 }), { kind: 'output_limit', started: true });
  await assert.rejects(invoke(process.execPath, ['-e', 'setTimeout(()=>{},20000)'], { timeout: 100 }), { kind: 'timeout', started: true });
  await assert.rejects(invoke('missing-saucepan-sdk-test-executable', []), { kind: 'missing_executable', started: false });
  const abort = new AbortController();
  const promise = invoke(process.execPath, ['-e', 'setTimeout(()=>{},20000)'], { signal: abort.signal });
  setTimeout(() => abort.abort(), 100);
  await assert.rejects(promise, { kind: 'cancelled', started: true });
});

test('closed concurrent payloads and cleanup on failure', async () => {
  const value = { unicode: '雪', empty: [], explicit: null };
  const paths = [];
  await Promise.all([1, 2].map(() => payloadFiles(new Map([['x', value]]), 1024, async files => {
    const path = files.get('x'); paths.push(path);
    assert.deepEqual(JSON.parse(await readFile(path, 'utf8')), value);
  })));
  assert.notEqual(paths[0], paths[1]);
  for (const path of paths) await assert.rejects(access(path));
  let failed;
  await assert.rejects(payloadFiles(new Map([['x', {}]]), 1024, async files => {
    failed = files.get('x'); throw new Error('fixture');
  }), /fixture/);
  await assert.rejects(access(failed));
  for (const value of [undefined, NaN, Infinity, 2 ** 53, new Date(), { x: undefined }, 1n]) assert.throws(() => encode(value, 1024), { kind: 'invalid_input' });
  assert.throws(() => encode('x'.repeat(2000), 1024), { kind: 'input_limit' });
});

test('UTF-8 and integer validation occurs before rounding', () => {
  for (const text of ['{"revision":9007199254740993}', '{"n":1e999}', 'null garbage', '{"n":-9007199254740993}']) {
    assert.throws(() => decode(new TextEncoder().encode(text)), { kind: 'protocol' });
  }
  assert.throws(() => decode(Uint8Array.of(255)), { kind: 'protocol' });
  assert.deepEqual(decode(new TextEncoder().encode('{"text":"9007199254740993","revision":9007199254740991}')), { text: '9007199254740993', revision: Number.MAX_SAFE_INTEGER });
});

test('JSON envelope and uncertain malformed mutation, never child diagnostics', async () => {
  class Harness extends NodeTransport {
    constructor(script) { super(); this.script = script; }
    run() { return invoke(process.execPath, ['-e', this.script], { capture: true }); }
  }
  const config = { executable: 'unused', timeout: 1000, inputLimit: 1024, captureLimit: 1024 };
  await assert.rejects(new Harness('console.log("not JSON")').json(config, { argv: ['init'], mutates: true }), { kind: 'protocol', indeterminate: true });
  await assert.rejects(new Harness('console.log("{}")').json(config, { argv: ['init'], required: ['root'] }), { kind: 'protocol' });
  await assert.rejects(new Harness('console.error(JSON.stringify({error:{code:"revision_conflict",message:"safe",details:{member:"a"}}})); process.exitCode=1').json(config, { argv: ['init'] }), error => error instanceof SaucepanError && error.code === 'revision_conflict' && error.details.member === 'a');
});

test('inert client and injected transport', async () => {
  let calls = 0;
  const transport = { json(config, call) { calls++; assert.deepEqual(call.argv, ['query-sources']); return Promise.resolve({generation: 0, sources: []}); }, run() { calls++; return Promise.resolve({}); } };
  const client = new Client({ executable: 'missing', transport });
  client.forApp('explicit');
  assert.equal(calls, 0);
  assert.deepEqual(await client.querySources(), {generation: 0, sources: []});
  assert.equal(calls, 1);
});

test('client import has no eager filesystem or process module access', async () => {
  const url = new URL('../dist/client.js', import.meta.url).href;
  const script = `import { register } from 'node:module';
register('data:text/javascript,'+encodeURIComponent('export async function resolve(s,c,n){if(["node:fs","node:fs/promises","node:child_process"].includes(s))throw Error("eager Node import");return n(s,c)}'), import.meta.url);
const {Client}=await import(${JSON.stringify(url)}); new Client({transport:{}});`;
  const result = await invoke(process.execPath, ['--input-type=module', '-e', script], { capture: true });
  assert.equal(result.exitCode, 0, new TextDecoder().decode(result.stderr));
});
