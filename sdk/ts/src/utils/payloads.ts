import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { encode } from './json.js';

export async function payloadFiles<T>(values: ReadonlyMap<string, unknown>, limit: number,
  use: (paths: ReadonlyMap<string, string>) => Promise<T>): Promise<T> {
  const encoded = [...values].map(([name, value]) => [name, encode(value, limit)] as const);
  const directory = await mkdtemp(join(tmpdir(), 'saucepan-sdk-'));
  try {
    const paths = new Map<string, string>();
    for (const [index, [name, data]] of encoded.entries()) {
      const path = join(directory, `${index}.json`);
      await writeFile(path, data, { flag: 'wx', mode: 0o600 });
      paths.set(name, path);
    }
    return await use(paths);
  } finally {
    // directory came directly from mkdtemp under the OS temp root.
    await rm(directory, { recursive: true, force: true });
  }
}
