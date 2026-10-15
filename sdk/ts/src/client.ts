import type { Call, Config, RunOptions, RunResult, Transport } from './contracts.js';
import { TransportError } from './errors.js';
import { Payload } from './contracts.js';
import type * as c from './models/claim.js';
import type * as co from './models/composition.js';
import type * as g from './models/group.js';
import type * as p from './models/pin.js';
import type * as po from './models/policy.js';
import type * as s from './models/source.js';
import type * as st from './models/setup.js';

function flags(values: Record<string, string | number | readonly string[] | Payload | undefined>): (string | Payload)[] {
  const argv: (string | Payload)[] = [];
  for (const [name, value] of Object.entries(values)) {
    if (value === undefined) continue;
    if (typeof value === 'number' && !Number.isSafeInteger(value)) throw new TransportError('invalid_input');
    const flag = '--' + name.replaceAll('_', '-');
    for (const item of Array.isArray(value) ? value : [value]) argv.push(flag, item instanceof Payload ? item : String(item));
  }
  return argv;
}

class DefaultTransport implements Transport {
  async json(config: Config, call: Call): Promise<unknown> {
    const { NodeTransport } = await import('./transport.js');
    return new NodeTransport().json(config, call);
  }
  async run(config: Config, call: Call, options: RunOptions): Promise<RunResult> {
    const { NodeTransport } = await import('./transport.js');
    return new NodeTransport().run(config, call, options);
  }
}

export interface ClientOptions {
  executable?: string;
  root?: string;
  credentialFile?: string;
  /** JSON operation deadline in milliseconds; null explicitly disables it. */
  timeout?: number | null;
  captureLimit?: number;
  inputLimit?: number;
  transport?: Transport;
}
export interface PathOptions { verification?: 'metadata' | 'full'; policyRefs?: string[]; }
export interface CompositionOptions { scope?: string; policies?: po.Policy[]; }
export interface PrepareOptions { expectedRevision?: number; pin?: p.GroupPinRequest; }

export class Client {
  private config: Config;
  private readonly transport: Transport;
  constructor(options: ClientOptions = {}) {
    this.config = Object.freeze({ executable: options.executable ?? 'saucepan', root: options.root,
      credentialFile: options.credentialFile, timeout: options.timeout === undefined ? 120_000 : options.timeout,
      captureLimit: options.captureLimit ?? 16 * 1024 * 1024, inputLimit: options.inputLimit ?? 16 * 1024 * 1024 });
    this.transport = options.transport ?? new DefaultTransport();
  }
  forApp(app: string, options: { credentialFile?: string } = {}): Client {
    const client = new Client({ transport: this.transport });
    client.config = Object.freeze({ ...this.config, app, credentialFile: options.credentialFile });
    return client;
  }
  private identity(app: string): string {
    if (this.config.app !== undefined && this.config.app !== app) throw new TransportError('context_mismatch');
    return app;
  }
  profiles(): Promise<st.ProfileName[]> { return this.json(['profiles'], { shape: 'array' }); }
  profile(name?: st.ProfileName, options: { forInstall?: boolean; catalog?: po.Policy[]; retrievalPolicyRefs?: string[]; acquisitionPolicyRefs?: string[] } = {}): Promise<st.ProfileExpansion> {
    return this.json(['profile', ...(name === undefined ? [] : [name]), ...(options.forInstall ? ['--for-install'] : []),
      ...flags({ catalog: options.catalog === undefined ? undefined : new Payload(options.catalog), retrieval_policy: options.retrievalPolicyRefs, acquisition_policy: options.acquisitionPolicyRefs })],
      { required: ['name', 'digest', 'catalog', 'retrieval_policy_refs', 'acquisition_policy_refs', 'combined_policy_refs', 'for_install'] });
  }
  explainPolicies(request: st.ExplainRequest, options: { policies?: po.Policy[] } = {}): Promise<st.PolicyExplanation> {
    if (request.operation === 'claim') this.identity(request.app);
    return this.json(['explain-policies', new Payload(request)], { policies: options.policies, required: ['axes', 'restrictions'] });
  }
  installApplication(name: string, request: c.AcquireRequest, probe: st.Probe, builtShim: string, options: { scope: string; shim: string; operation?: string }): Promise<st.LifecycleResult<st.Operation>> {
    return this.json(['install', name, new Payload(request), new Payload(probe), builtShim,
      ...flags({ scope: options.scope, shim: options.shim, operation: options.operation })], { mutates: true, lifecycle: 'operation' });
  }
  bootstrap(request: c.AcquireRequest, builtShim: string, options: { operation?: string } = {}): Promise<st.LifecycleResult<st.Operation>> {
    return this.json(['bootstrap', new Payload(request), builtShim, ...flags({ operation: options.operation })], { mutates: true, lifecycle: 'operation' });
  }
  updateApplication(app: string, claim: string, probe: st.Probe, options: { operation?: string } = {}): Promise<st.LifecycleResult<st.Operation>> {
    return this.json(['update-app', this.identity(app), claim, new Payload(probe), ...flags({ operation: options.operation })], { mutates: true, lifecycle: 'operation' });
  }
  selfUpdate(app: string, claim: string, options: { operation?: string } = {}): Promise<st.LifecycleResult<st.Operation>> {
    return this.json(['self-update', this.identity(app), claim, ...flags({ operation: options.operation })], { mutates: true, lifecycle: 'operation' });
  }
  rollback(app: string, claim: string, entry: string, probe: st.Probe, options: { operation?: string } = {}): Promise<st.LifecycleResult<st.Operation>> {
    return this.json(['rollback', this.identity(app), claim, entry, new Payload(probe), ...flags({ operation: options.operation })], { mutates: true, lifecycle: 'operation' });
  }
  uninstall(app: string, options: { leaveUnmanagedShims?: boolean; operation?: string } = {}): Promise<st.LifecycleResult<st.Operation>> {
    return this.json(['uninstall', this.identity(app), ...(options.leaveUnmanagedShims ? ['--leave-unmanaged-shims'] : []), ...flags({ operation: options.operation })], { mutates: true, lifecycle: 'operation' });
  }
  operations(options: { app?: string } = {}): Promise<st.Operation[]> {
    return this.json(['operations', ...flags({ app: options.app === undefined ? undefined : this.identity(options.app) })], { lifecycle: 'inspection' });
  }
  reconcile(options: { operation?: string } = {}): Promise<st.LifecycleResult<st.Operation[]>> {
    return this.json(['reconcile', ...flags({ operation: options.operation })], { mutates: true, lifecycle: 'reconcile' });
  }
  private async json<T>(argv: readonly (string | Payload)[], call: Omit<Call, 'argv'> = {}): Promise<T> {
    return await this.transport.json(this.config, { ...call, argv }) as T;
  }
  private execute(argv: readonly (string | Payload)[], command: readonly string[], options: RunOptions = {}, call: Omit<Call, 'argv'> = {}): Promise<RunResult> {
    if (!Array.isArray(command) || command.length === 0) throw new TransportError('invalid_input');
    return this.transport.run(this.config, { ...call, argv: [...argv, '--', ...command] }, options);
  }
  querySources(query?: s.SourceQuery): Promise<s.SourceQueryResult> {
    return this.json(['query-sources', ...(query === undefined ? [] : [new Payload(query)])], { required: ['generation', 'sources'] });
  }
  init(options: { integrity?: 'basic' | 'keyring' } = {}): Promise<s.InitResult> {
    return this.json(['init', ...flags({ integrity: options.integrity })], { mutates: true, required: ['root', 'integrity'] });
  }
  registerApplication(request: c.ApplicationRegistration, options: { credentialOut?: string } = {}): Promise<c.Application> {
    return this.json(['register-application', new Payload(request), ...flags({ credential_out: options.credentialOut })], { mutates: true, required: ['uuid', 'revision'] });
  }
  configureApplication(app: string, request: c.ApplicationConfiguration, options: { credentialOut?: string } = {}): Promise<c.Application> {
    return this.json(['configure-application', this.identity(app), new Payload(request), ...flags({ credential_out: options.credentialOut })], { mutates: true, required: ['uuid', 'revision'] });
  }
  registerSource(source: s.Source): Promise<s.SourceRecord> {
    return this.json(['register-source', new Payload(source)], { mutates: true, required: ['uuid', 'source'] });
  }
  inspect(uuid: string): Promise<s.IndexRecord> {
    return this.json(['inspect', uuid], { required: ['uuid', 'type'] });
  }
  queryClaims(query: c.ClaimQuery): Promise<c.QueryResult> {
    this.identity(query.app);
    return this.json(['query-claims', new Payload(query)], { required: ['app', 'revision', 'claims'] });
  }
  discoverClaims(query: c.DiscoveryQuery): Promise<c.QueryResult[]> {
    return this.json(['discover-claims', new Payload(query)], { shape: 'array' });
  }
  claims(app: string): Promise<c.ClaimView[]> {
    return this.json(['claims', this.identity(app)], { shape: 'array' });
  }
  setSourceVisibility(source: string, visibility: 'public' | 'private'): Promise<s.SourceRecord> {
    return this.json(['set-source-visibility', source, visibility], { mutates: true, required: ['uuid'] });
  }
  registerPolicies(policies: po.Policy[]): Promise<string[]> {
    return this.json(['register-policies', new Payload(policies)], { mutates: true, shape: 'array' });
  }
  policy(uuid: string): Promise<po.Policy> {
    return this.json(['policy', uuid], { required: ['uuid', 'type'] });
  }
  adoptAppPolicies(app: string, expectedRevision: number, policyRefs: string[]): Promise<po.ApplicationAdoption> {
    return this.json(['adopt-app-policies', this.identity(app), ...flags({ expected_revision: expectedRevision, policy: policyRefs })], { mutates: true, required: ['app', 'revision', 'policy_refs'] });
  }
  adoptSourcePolicies(source: string, expectedRevision: number, policyRefs: string[]): Promise<s.SourceRecord> {
    return this.json(['adopt-source-policies', source, ...flags({ expected_revision: expectedRevision, policy: policyRefs })], { mutates: true, required: ['uuid', 'policy_revision'] });
  }
  adoptClaimPolicies(app: string, claim: string, expectedRevision: number, policyRefs: string[]): Promise<c.ClaimAdoption> {
    return this.json(['adopt-claim-policies', this.identity(app), claim, ...flags({ expected_revision: expectedRevision, policy: policyRefs })], { mutates: true, required: ['app', 'revision', 'claim'] });
  }
  acquire(request: c.AcquireRequest, options: { app?: string; scope?: string; context?: Record<string, string> } = {}): Promise<c.Acquired | c.Claimed> {
    const app = options.app ?? this.config.app;
    if (app !== undefined) this.identity(app);
    if ((app !== undefined && options.scope === undefined) || (app === undefined && (options.scope !== undefined || options.context !== undefined))) throw new TransportError('invalid_input');
    return this.json(['acquire', new Payload(request), ...flags({ app, scope: options.scope, context: options.context === undefined ? undefined : new Payload(options.context) })],
      { mutates: true, required: app === undefined ? ['entry', 'directory'] : ['claim', 'view', 'acquired'] });
  }
  ensureClaim(app: string, request: c.EnsureRequest): Promise<c.Claimed> {
    return this.json(['ensure-claim', this.identity(app), new Payload(request)], { mutates: true, required: ['claim', 'view', 'acquired'] });
  }
  updateClaim(app: string, claim: string): Promise<c.Claimed> {
    return this.json(['update-claim', this.identity(app), claim], { mutates: true, required: ['claim', 'view', 'acquired'] });
  }
  recoverClaim(app: string, claim: string): Promise<c.Claimed> {
    return this.json(['recover-claim', this.identity(app), claim], { mutates: true, required: ['claim', 'view', 'acquired'] });
  }
  releaseClaim(app: string, claim: string): Promise<c.ReleaseResult> {
    return this.json(['release-claim', this.identity(app), claim], { mutates: true, required: ['released'] });
  }
  setClaimContext(app: string, claim: string, context: Record<string, string>, expectedRevision: number): Promise<c.Claim> {
    return this.json(['set-claim-context', this.identity(app), claim, new Payload(context), ...flags({ expected_revision: expectedRevision })], { mutates: true, required: ['uuid', 'context'] });
  }
  retrieve(request: co.Retrieval, options: CompositionOptions = {}): Promise<co.RetrievalReceipt> {
    return this.json(['retrieve', new Payload(request), ...flags({ scope: options.scope })], { mutates: true, policies: options.policies, required: ['request', 'descriptor'] });
  }
  acquireRetrieved(request: co.AcquireRetrievedRequest, options: CompositionOptions = {}): Promise<co.AcquiredDescriptor> {
    return this.json(['acquire-retrieved', new Payload(request), ...flags({ scope: options.scope })], { mutates: true, policies: options.policies, required: ['ownership', 'path'] });
  }
  sourcePath(retrieval: string, options: { scope?: string } = {}): Promise<co.SourcePath> {
    return this.json(['source-path', retrieval, ...flags({ scope: options.scope })], { required: ['path'] });
  }
  path(uuid: string, options: PathOptions = {}): Promise<co.ResolvedResource> {
    return this.json(['path', uuid, ...flags({ verification: options.verification, policy: options.policyRefs })], { required: ['entry_uuid', 'directory', 'verification'] });
  }
  verify(uuid: string): Promise<co.ResolvedResource> {
    return this.json(['verify', uuid], { required: ['entry_uuid', 'directory', 'verification'] });
  }
  reclaim(): Promise<c.ReclaimReport> {
    return this.json(['reclaim'], { mutates: true, required: ['reclaimed', 'deferred', 'pending'] });
  }
  purgeRetrieval(retrieval: string): Promise<co.PurgeResult> {
    return this.json(['purge-retrieval', retrieval], { mutates: true, required: ['purged'] });
  }
  registerGroup(request: g.GroupDefinition): Promise<g.GroupRecord> {
    return this.json(['register-group', new Payload(request)], { mutates: true, payloadLimit: 1024 * 1024, required: ['uuid', 'revision', 'definition'] });
  }
  showGroup(uuid: string): Promise<g.GroupRecord> {
    return this.json(['show-group', uuid], { required: ['uuid', 'revision', 'definition'] });
  }
  listGroups(options: { key?: string } = {}): Promise<g.GroupRecord[]> {
    return this.json(['list-groups', ...flags({ key: options.key })], { shape: 'array' });
  }
  replaceGroup(uuid: string, request: g.GroupDefinition, expectedRevision: number): Promise<g.GroupRecord> {
    return this.json(['replace-group', uuid, new Payload(request), ...flags({ expected_revision: expectedRevision })], { mutates: true, payloadLimit: 1024 * 1024, required: ['uuid', 'revision', 'definition'] });
  }
  removeGroup(uuid: string, expectedRevision: number): Promise<g.RemoveResult> {
    return this.json(['remove-group', uuid, ...flags({ expected_revision: expectedRevision })], { mutates: true, required: ['removed'] });
  }
  prepareGroup(uuid: string, options: PrepareOptions = {}): Promise<p.PreparedGroup> {
    return this.json(['prepare-group', uuid, ...flags({ expected_revision: options.expectedRevision, pin: options.pin === undefined ? undefined : new Payload(options.pin) })],
      { mutates: true, payloadLimit: 1024 * 1024, required: ['group', 'pin', 'resources', 'stale'] });
  }
  pinOutputs(request: p.PinRequest): Promise<p.PinnedOutputs> {
    return this.json(['pin-outputs', new Payload(request)], { mutates: true, payloadLimit: 1024 * 1024, required: ['pin', 'paths'] });
  }
  pinGroup(uuid: string, expectedRevision: number, pin: p.GroupPinRequest): Promise<p.PinRecord> {
    return this.json(['pin-group', uuid, ...flags({ expected_revision: expectedRevision, pin: new Payload(pin) })], { mutates: true, payloadLimit: 1024 * 1024, required: ['uuid', 'state'] });
  }
  pin(uuid: string): Promise<p.PinInspection> {
    return this.json(['pin', uuid], { required: ['uuid', 'state', 'outputs'] });
  }
  pins(options: { key?: string } = {}): Promise<p.PinInspection[]> {
    return this.json(['pins', ...flags({ key: options.key })], { shape: 'array' });
  }
  unpin(uuid: string): Promise<p.PinRecord> {
    return this.json(['unpin', uuid], { mutates: true, required: ['uuid', 'state'] });
  }
  run(uuid: string, command: readonly string[], options: PathOptions & RunOptions = {}): Promise<RunResult> {
    return this.execute(['run', uuid, ...flags({ verification: options.verification, policy: options.policyRefs })], command, options);
  }
  runClaim(app: string, claim: string, command: readonly string[], options: PathOptions & RunOptions & { relative?: string } = {}): Promise<RunResult> {
    return this.execute(['run-claim', this.identity(app), claim, ...flags({ relative: options.relative, verification: options.verification, policy: options.policyRefs })], command, options);
  }
  runGroup(request: g.GroupRequest, command: readonly string[], options: RunOptions & { policies?: po.Policy[] } = {}): Promise<RunResult> {
    return this.execute(['run-group', new Payload(request)], command, options, { payloadLimit: 1024 * 1024, policies: options.policies });
  }
  runRegisteredGroup(uuid: string, command: readonly string[], options: RunOptions = {}): Promise<RunResult> {
    return this.execute(['run-registered-group', uuid], command, options);
  }
  prepareRunGroup(uuid: string, command: readonly string[], options: PrepareOptions & RunOptions = {}): Promise<RunResult> {
    return this.execute(['prepare-run-group', uuid, ...flags({ expected_revision: options.expectedRevision, pin: options.pin === undefined ? undefined : new Payload(options.pin) })], command, options, { mutates: true, payloadLimit: 1024 * 1024 });
  }
  runRetrieved(request: co.Retrieval, command: readonly string[], options: CompositionOptions & RunOptions = {}): Promise<RunResult> {
    return this.execute(['run-retrieved', new Payload(request), ...flags({ scope: options.scope })], command, options, { mutates: true, policies: options.policies });
  }
  runAcquired(request: co.CompositionRequest, command: readonly string[], options: CompositionOptions & RunOptions = {}): Promise<RunResult> {
    return this.execute(['run-acquired', new Payload(request), ...flags({ scope: options.scope })], command, options, { mutates: true, policies: options.policies });
  }
}
