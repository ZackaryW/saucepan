// Copied to the installed consumer directory; no checkout/private imports.
import { copyFileSync, mkdirSync, existsSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import assert from 'node:assert/strict';
import { Client } from 'saucepan-sdk';
const [binary,shim,root]=process.argv.slice(2);
const client=new Client({executable:binary,root});
assert.ok((await client.profiles()).includes('managed'));
const profile=await client.profile('explicit-update',{forInstall:true});
const explained=await client.explainPolicies({operation:'retrieve',request:{
  source:{kind:'inline',source:{type:'url',url:'http://127.0.0.1:1/unused'}},selector:{type:'url'},
  policy_refs:profile.retrieval_policy_refs}},{policies:profile.catalog});
assert.equal(explained.axes.refresh.origin,'request'); assert.ok(!existsSync(root));
await client.init(); await client.registerPolicies(profile.catalog);
const folder=join(dirname(root),'payload'); mkdirSync(folder); copyFileSync(binary,join(folder,basename(binary)));
const source=await client.registerSource({type:'local',path:folder});
const request={source_uuid:source.uuid,selector:{type:'local'},selection:{bin:[basename(binary)]},policy_refs:profile.combined_policy_refs};
const probe={args:['--version'],stdout_contains:'saucepan ',timeout_seconds:10};
const installed=await client.installApplication('Packaged SDK',request,probe,shim,{scope:'package/tool',shim:'packaged-tool'});
assert.equal(installed.exit_code,0);
const {app,claim,candidate:entry}=installed.result;
assert.equal((await client.updateApplication(app,claim,probe)).exit_code,0);
const failed=await client.updateApplication(app,claim,{...probe,stdout_contains:'impossible-package-probe'});
assert.equal(failed.exit_code,2); assert.equal(failed.result.status,'failed');
assert.equal((await client.rollback(app,claim,entry,probe)).exit_code,0);
assert.ok((await client.operations({app})).some(r=>r.operation===failed.result.operation));
assert.equal((await client.reconcile({operation:failed.result.operation})).exit_code,2);
assert.equal((await client.uninstall(app)).exit_code,0);
console.log(JSON.stringify({package:'typescript',profile_digest:profile.digest,install:'complete',failed_probe:failed.result.status,rollback:'complete',uninstall:'complete'}));
