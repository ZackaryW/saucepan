// Explicit isolated install: manager shim root source-folder executable-name
import { Client } from 'saucepan-sdk';
const [binary, shim, root, sourceFolder, executableName] = process.argv.slice(2);
const client = new Client({ executable: binary, root });
await client.init(); // Basic integrity, no keyring or credential setup.
const profile = await client.profile('explicit-update', { forInstall: true });
await client.registerPolicies(profile.catalog);
const source = await client.registerSource({ type: 'local', path: sourceFolder });
const request = { source_uuid: source.uuid, selector: { type: 'local' },
  selection: { bin: [executableName] }, policy_refs: profile.combined_policy_refs };
const probe = { args: ['--version'], stdout_contains: 'saucepan ', timeout_seconds: 10 };
const installed = await client.installApplication('Example tool', request, probe, shim, { scope: 'example/tool', shim: 'example-tool' });
console.log(installed);
if (installed.exit_code !== 0) throw Error('Inspect this operation before explicitly retrying or reconciling.');
console.log(await client.operations({ app: installed.result.app }));
// Explicit example cleanup, not automatic SDK compensation.
console.log(await client.uninstall(installed.result.app));
