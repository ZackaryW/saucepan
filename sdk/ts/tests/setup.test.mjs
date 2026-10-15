import test from 'node:test';
import assert from 'node:assert/strict';
import { Client, Payload } from '../dist/index.js';
import { NodeTransport } from '../dist/transport.js';
const receipt = status => ({ operation:'operation', app:'app', claim:'claim', fingerprint:'hash', status, changed:false,
  action:{type:'update'}, pending:[], leftovers:[], shims:[], candidate:null, desired:null, desired_registry:null, source_policy:null, reason:null, expected_revision:null });
const config = { executable:'unused',timeout:1000,inputLimit:1024,captureLimit:1024 };
class Harness extends NodeTransport {
  constructor(value, code, stderr='') { super(); Object.assign(this,{value,code,stderr}); }
  async run() { return {exitCode:this.code,signal:null,stdout:new TextEncoder().encode(JSON.stringify(this.value)),stderr:new TextEncoder().encode(this.stderr)}; }
}
test('lifecycle receipts, inspection and mixed reconciliation preserve status', async () => {
  for (const status of ['complete','preparing','commit_intent','cleanup_pending','failed','conflict']) {
    const code=status==='complete'?0:2;
    assert.deepEqual(await new Harness(receipt(status),code).json(config,{argv:['update-app'],mutates:true,lifecycle:'operation'}),{exit_code:code,result:receipt(status)});
  }
  const mixed=[receipt('complete'),receipt('failed')];
  assert.deepEqual((await new Harness(mixed,2).json(config,{argv:['reconcile'],lifecycle:'reconcile'})).result,mixed);
  assert.deepEqual(await new Harness(mixed,0).json(config,{argv:['operations'],lifecycle:'inspection'}),mixed);
  assert.deepEqual((await new Harness([],0).json(config,{argv:['reconcile'],lifecycle:'reconcile'})).result,[]);
});
test('malformed/status-inconsistent mutations remain indeterminate; domain errors remain errors', async () => {
  for (const [value,code] of [[receipt('complete'),2],[receipt('failed'),0],[{},2],[[receipt('failed')],2],[{...receipt('complete'),changed:'yes'},0],[{...receipt('failed'),candidate:[]},2]]) {
    await assert.rejects(new Harness(value,code).json(config,{argv:['uninstall'],mutates:true,lifecycle:'operation'}),{kind:'protocol',indeterminate:true});
  }
  await assert.rejects(new Harness(null,1,'{"error":{"code":"denied","message":"denied","details":{}}}').json(config,{argv:['uninstall'],lifecycle:'operation'}),{code:'denied'});
  await assert.rejects(new Harness(receipt('failed'),2).json(config,{argv:['query-sources']}),{kind:'protocol'});
});
test('literal mappings, omitted defaults, explicit identity and missing manager', async () => {
  const calls=[];
  const client=new Client({executable:'missing',transport:{async json(config,call){calls.push({config,call}); return {};}}});
  assert.equal(calls.length,0);
  await client.profile(); assert.deepEqual(calls.at(-1).call.argv,['profile']);
  await client.profile('managed',{forInstall:true,catalog:[],retrievalPolicyRefs:['r'],acquisitionPolicyRefs:['a']});
  assert.deepEqual(calls.at(-1).call.argv.slice(0,4),['profile','managed','--for-install','--catalog']);
  assert.ok(calls.at(-1).call.argv[4] instanceof Payload);
  await client.installApplication('name;雪',{}, {},'shim space',{scope:'project',shim:'tool',operation:'op'});
  assert.deepEqual(calls.at(-1).call.argv.slice(4),['shim space','--scope','project','--shim','tool','--operation','op']);
  const bound=client.forApp('a'); await bound.updateApplication('a','claim',{});
  assert.equal(calls.at(-1).config.app,'a'); const before=calls.length;
  for(const fn of [()=>bound.selfUpdate('b','c'),()=>bound.operations({app:'b'}),()=>bound.explainPolicies({operation:'claim',app:'b',claim:'c'})]) assert.throws(fn,{kind:'context_mismatch'});
  assert.equal(calls.length,before);
  await assert.rejects(new Client({executable:'missing-saucepan-sdk-setup'}).bootstrap({},'shim'),{kind:'missing_executable',indeterminate:false});
});
