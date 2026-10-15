// Root-free preview: node profiles.mjs /absolute/path/to/saucepan
import { Client } from 'saucepan-sdk';
const client = new Client({ executable: process.argv[2] });
console.log(await client.profiles());
const profile = await client.profile('temporary');
console.log(await client.explainPolicies({ operation: 'retrieve', request: {
  source: { kind: 'inline', source: { type: 'url', url: 'https://example.invalid/archive.zip' } },
  selector: { type: 'url' }, policy_refs: profile.retrieval_policy_refs,
} }, { policies: profile.catalog }));
// Actual temporary use goes through runRetrieved, which holds native lifetime.
