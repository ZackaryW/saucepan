// Test driver only; public SDK APIs remain explicit typed methods.
import { readFileSync } from 'node:fs';
import { Client, SaucepanError, TransportError } from '../../ts/dist/index.js';
import { NodeTransport } from '../../ts/dist/transport.js';
const input = JSON.parse(readFileSync(0, 'utf8'));
const camel = name => name.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
class FixtureTransport extends NodeTransport {
  run(config,call,options) { return super.run(config,input.api ? {...call,argv:['--test-github-api',input.api,...call.argv]}:call,options); }
}
let client = new Client({ executable: input.binary, root: input.root, transport:new FixtureTransport() });
if (input.app) client = client.forApp(input.app);
const options = {};
for (const [key, value] of Object.entries(input.options)) {
  if (key === 'options') {
    Object.assign(options, value);
    if (typeof options.timeout === 'number') options.timeout *= 1000;
  } else options[camel(key)] = value;
}
try {
  const result = await client[camel(input.name)](...input.args, ...(Object.keys(options).length ? [options] : []));
  if (result && 'exitCode' in result) {
    console.log(JSON.stringify({ result: { exit_code: result.exitCode, signal: result.signal,
      stdout: result.stdout === null ? null : new TextDecoder().decode(result.stdout),
      stderr: result.stderr === null ? null : new TextDecoder().decode(result.stderr) } }));
  } else console.log(JSON.stringify({ result }));
} catch (error) {
  if (error instanceof SaucepanError) console.log(JSON.stringify({ error: { code: error.code, message: error.remoteMessage, details: error.details } }));
  else if (error instanceof TransportError) console.log(JSON.stringify({ transport: error.kind, indeterminate: error.indeterminate }));
  else throw error;
}
