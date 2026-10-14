import { TransportError } from '../errors.js';

/** JSON-compatible data only: no toJSON callbacks, dates, bigint or coercion. */
function portable(value: unknown, seen = new Set<object>()): void {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return;
  if (typeof value === 'number') {
    if (!Number.isFinite(value) || Math.abs(value) > Number.MAX_SAFE_INTEGER) throw new Error();
    return;
  }
  if (typeof value !== 'object' || seen.has(value)) throw new Error();
  if (!Array.isArray(value) && Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) throw new Error();
  seen.add(value);
  for (const item of Object.values(value)) portable(item, seen);
  seen.delete(value);
}

export function encode(value: unknown, limit: number): Uint8Array {
  try {
    portable(value);
    const result = new TextEncoder().encode(JSON.stringify(value));
    if (result.byteLength > limit) throw new TransportError('input_limit');
    return result;
  } catch (error) {
    if (error instanceof TransportError) throw error;
    throw new TransportError('invalid_input');
  }
}

export function decode(data: Uint8Array): unknown {
  try {
    const text = new TextDecoder('utf-8', { fatal: true }).decode(data);
    // Examine numeric tokens before JSON.parse can round integer identities.
    for (const match of text.matchAll(/"(?:\\.|[^"\\])*"|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)/g)) {
      const token = match[1];
      if (token === undefined) continue;
      const number = Number(token);
      if (!Number.isFinite(number) || Math.abs(number) > Number.MAX_SAFE_INTEGER) throw new Error();
      if (/^-?\d+$/.test(token) && (BigInt(token) > BigInt(Number.MAX_SAFE_INTEGER) || BigInt(token) < -BigInt(Number.MAX_SAFE_INTEGER))) throw new Error();
    }
    const value: unknown = JSON.parse(text);
    portable(value);
    return value;
  } catch {
    throw new TransportError('protocol', true);
  }
}
