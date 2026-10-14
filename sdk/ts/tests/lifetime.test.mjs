import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, access, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { Client } from '../dist/index.js';

const binary = process.env.SAUCEPAN_TEST_BINARY ?? fileURLToPath(new URL(`../../../target/debug/saucepan${process.platform === 'win32' ? '.exe' : ''}`, import.meta.url));
const exists = path => access(path).then(() => true, () => false);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

test('public context conflicts are rejected before spawn', () => {
  const client = new Client({ executable: 'missing-sdk-binary' }).forApp('a');
  assert.throws(() => client.claims('b'), {kind: 'context_mismatch'});
  assert.throws(() => client.queryClaims({app: 'b', where: []}), {kind: 'context_mismatch'});
  assert.throws(() => client.acquire({source_uuid:'s', selector:{type:'local'}}, {app:'b',scope:'s'}), {kind:'context_mismatch'});
});

test('native run guards reclaim, cancellation leaves explicit pin intact', async () => {
  const base = await mkdtemp(join(tmpdir(), 'saucepan-node-lifetime-'));
  try {
    const client = new Client({ executable: binary, root: join(base, 'root') });
    await client.init();
    const folder = join(base, 'source'); await mkdir(folder);
    await writeFile(join(folder, 'file'), 'A');
    const source = await client.registerSource({type:'local',path:folder});
    const history = randomUUID(), pinPolicy = randomUUID();
    await client.registerPolicies([{uuid:history,type:'revision_policy',history:'none'}, {uuid:pinPolicy,type:'pin_policy',retention:'until_released'}]);
    const request = {source_uuid:source.uuid,selector:{type:'local'},policy_refs:[history]};
    const a = await client.acquire(request);
    const ready = join(base,'ready'), release = join(base,'release');
    const child = [process.execPath,'-e','const fs=require("fs"); fs.writeFileSync(process.argv[1],"");const t=setInterval(()=>{if(fs.existsSync(process.argv[2]))clearInterval(t)},20)',ready,release];
    const run = client.run(a.entry.uuid, child, { capture:true, timeout:10000 });
    try {
      const deadline = Date.now()+5000;
      while (!await exists(ready)) { assert.ok(Date.now()<deadline); await sleep(20); }
      await writeFile(join(folder,'file'),'B');
      await client.acquire(request); await client.reclaim();
      assert.ok(await exists(a.directory));
    } finally { await writeFile(release,''); }
    assert.equal((await run).exitCode,0);
    await client.reclaim(); assert.equal(await exists(a.directory),false);
    const b = await client.acquire(request);
    const pin = await client.pinOutputs({registration_key:'retained',outputs:[b.entry.uuid],policy_refs:[pinPolicy]});
    const abort = new AbortController();
    const pending = client.run(b.entry.uuid, [process.execPath,'-e','setTimeout(()=>{},500)'], {capture:true,signal:abort.signal});
    setTimeout(()=>abort.abort(),200);
    await assert.rejects(pending,{kind:'cancelled'});
    assert.equal((await client.pin(pin.pin.uuid)).state,'active');
    await sleep(600); // surviving child lifetime is intentionally independent
    await client.unpin(pin.pin.uuid);
  } finally { await rm(base,{recursive:true,force:true}); }
});
