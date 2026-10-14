export class TransportError extends Error {
  constructor(public readonly kind: string, public readonly started = false, public indeterminate = false) {
    super(`Saucepan transport: ${kind}`);
    this.name = 'TransportError';
  }
}

export class SaucepanError extends Error {
  constructor(public readonly code: string, public readonly remoteMessage: string, public readonly details: Record<string, string>) {
    super(`Saucepan: ${code}`);
    this.name = 'SaucepanError';
  }
}
