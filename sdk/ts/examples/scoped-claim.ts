import { Client } from '../src/index.js';

export async function ensureProject(client: Client, app: string, source: string, scope: string): Promise<string> {
  const result = await client.forApp(app).ensureClaim(app, {
    source_uuid: source, selector: {type:'ref',reference:'HEAD'},
    scope, selection: {folder:'skills'},
  });
  return result.claim.uuid;
}

export async function projectSelection(client: Client, app: string, scope: string): Promise<string[]> {
  const result = await client.forApp(app).queryClaims({app,scope,where:[]});
  return result.claims.map(view => view.claim.entry_uuid);
}
