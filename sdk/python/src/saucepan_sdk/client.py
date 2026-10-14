"""Explicit client context. Resource methods are thin mappings to the Rust CLI."""
from dataclasses import replace
from collections.abc import Sequence
from typing import Literal, cast

from .errors import TransportError
from .transport import Call, CliTransport, Config, Payload, RunOptions, Transport
from .utils.process import RunResult
from .utils.payloads import MAX_INTEGER
from .models import claim as c, composition as co, group as g, pin as p, policy as po, source as s


def _options(**values: str | int | Sequence[str] | Payload | None) -> list[str | Payload]:
    result: list[str | Payload] = []
    for name, value in values.items():
        if value is None:
            continue
        if isinstance(value, int) and (isinstance(value, bool) or abs(value) > MAX_INTEGER):
            raise TransportError('invalid_input')
        flag = '--' + name.replace('_', '-')
        if isinstance(value, (list, tuple)):
            for item in value:
                result.extend([flag, str(item)])
        else:
            result.extend([flag, value if isinstance(value, Payload) else str(value)])
    return result


class Client:
    def __init__(self, *, executable: str = "saucepan", root: str | None = None,
                 timeout: float | None = 120, capture_limit: int = 16 * 1024 * 1024,
                 input_limit: int = 16 * 1024 * 1024, credential_file: str | None = None,
                 transport: Transport | None = None):
        self._config = Config(executable=executable, root=root, timeout=timeout,
                              capture_limit=capture_limit, input_limit=input_limit, credential_file=credential_file)
        self._transport = transport if transport is not None else CliTransport()

    def for_app(self, app: str, *, credential_file: str | None = None) -> "Client":
        client = Client(transport=self._transport)
        client._config = replace(self._config, app=app, credential_file=credential_file)
        return client

    def _identity(self, app: str) -> str:
        if self._config.app is not None and app != self._config.app:
            raise TransportError("context_mismatch")
        return app

    def _json(self, argv: Sequence[str | Payload], *, required: tuple[str, ...] = (),
              mutates: bool = False, array: bool = False, bounded: bool = False,
              policies: list[po.Policy] | None = None) -> object:
        return self._transport.json(self._config, Call(argv, mutates, 'array' if array else 'object',
            required, 1024 * 1024 if bounded else None, policies))

    def _run(self, argv: Sequence[str | Payload], command: Sequence[str], *,
             options: RunOptions | None, mutates: bool = False, bounded: bool = False,
             policies: list[po.Policy] | None = None) -> RunResult:
        if not command or isinstance(command, str):
            raise TransportError('invalid_input')
        return self._transport.run(self._config, Call([*argv, '--', *command], mutates,
            payload_limit=1024 * 1024 if bounded else None, policies=policies), options or RunOptions())

    def init(self, *, integrity: Literal['basic', 'keyring'] | None = None) -> s.InitResult:
        return cast(s.InitResult, self._json(['init', *_options(integrity=integrity)], mutates=True, required=('root', 'integrity')))

    def register_application(self, request: c.ApplicationRegistration, *, credential_out: str | None = None) -> c.Application:
        return cast(c.Application, self._json(['register-application', Payload(request), *_options(credential_out=credential_out)], mutates=True, required=('uuid', 'revision')))

    def configure_application(self, app: str, request: c.ApplicationConfiguration, *, credential_out: str | None = None) -> c.Application:
        return cast(c.Application, self._json(['configure-application', self._identity(app), Payload(request), *_options(credential_out=credential_out)], mutates=True, required=('uuid', 'revision')))

    def register_source(self, source: s.Source) -> s.SourceRecord:
        return cast(s.SourceRecord, self._json(['register-source', Payload(source)], mutates=True, required=('uuid', 'source')))

    def inspect(self, uuid: str) -> s.Record:
        return cast(s.Record, self._json(['inspect', uuid], required=('uuid', 'type')))

    def query_sources(self, query: s.SourceQuery | None = None) -> s.SourceQueryResult:
        return cast(s.SourceQueryResult, self._json(['query-sources', *([] if query is None else [Payload(query)])], required=('generation', 'sources')))

    def query_claims(self, query: c.ClaimQuery) -> c.QueryResult:
        self._identity(query['app'])
        return cast(c.QueryResult, self._json(['query-claims', Payload(query)], required=('app', 'revision', 'claims')))

    def discover_claims(self, query: c.DiscoveryQuery) -> list[c.QueryResult]:
        return cast(list[c.QueryResult], self._json(['discover-claims', Payload(query)], array=True))

    def claims(self, app: str) -> list[c.ClaimView]:
        return cast(list[c.ClaimView], self._json(['claims', self._identity(app)], array=True))

    def set_source_visibility(self, source: str, visibility: Literal['public', 'private']) -> s.SourceRecord:
        return cast(s.SourceRecord, self._json(['set-source-visibility', source, visibility], mutates=True, required=('uuid',)))

    def register_policies(self, policies: list[po.Policy]) -> list[str]:
        return cast(list[str], self._json(['register-policies', Payload(policies)], mutates=True, array=True))

    def policy(self, uuid: str) -> po.Policy:
        return cast(po.Policy, self._json(['policy', uuid], required=('uuid', 'type')))

    def adopt_app_policies(self, app: str, expected_revision: int, policy_refs: list[str]) -> po.ApplicationAdoption:
        return cast(po.ApplicationAdoption, self._json(['adopt-app-policies', self._identity(app), *_options(expected_revision=expected_revision, policy=policy_refs)], mutates=True, required=('app', 'revision', 'policy_refs')))

    def adopt_source_policies(self, source: str, expected_revision: int, policy_refs: list[str]) -> s.SourceRecord:
        return cast(s.SourceRecord, self._json(['adopt-source-policies', source, *_options(expected_revision=expected_revision, policy=policy_refs)], mutates=True, required=('uuid', 'policy_revision')))

    def adopt_claim_policies(self, app: str, claim: str, expected_revision: int, policy_refs: list[str]) -> c.ClaimAdoption:
        return cast(c.ClaimAdoption, self._json(['adopt-claim-policies', self._identity(app), claim, *_options(expected_revision=expected_revision, policy=policy_refs)], mutates=True, required=('app', 'revision', 'claim')))

    def acquire(self, request: c.AcquireRequest, *, app: str | None = None, scope: str | None = None,
                context: dict[str, str] | None = None) -> c.Acquired | c.Claimed:
        # A bound application is propagated into acquire's explicit --app, never
        # silently treated as a local acquisition. Scope remains caller-supplied.
        identity = app if app is not None else self._config.app
        if identity is not None:
            self._identity(identity)
        if (identity is not None and scope is None) or (identity is None and (scope is not None or context is not None)):
            raise TransportError('invalid_input')
        return cast(c.Acquired | c.Claimed, self._json(['acquire', Payload(request), *_options(app=identity, scope=scope, context=None if context is None else Payload(context))], mutates=True,
                    required=('claim', 'view', 'acquired') if identity is not None else ('entry', 'directory')))

    def ensure_claim(self, app: str, request: c.EnsureRequest) -> c.Claimed:
        return cast(c.Claimed, self._json(['ensure-claim', self._identity(app), Payload(request)], mutates=True, required=('claim', 'view', 'acquired')))

    def update_claim(self, app: str, claim: str) -> c.Claimed:
        return cast(c.Claimed, self._json(['update-claim', self._identity(app), claim], mutates=True, required=('claim', 'view', 'acquired')))

    def recover_claim(self, app: str, claim: str) -> c.Claimed:
        return cast(c.Claimed, self._json(['recover-claim', self._identity(app), claim], mutates=True, required=('claim', 'view', 'acquired')))

    def release_claim(self, app: str, claim: str) -> c.ReleaseResult:
        return cast(c.ReleaseResult, self._json(['release-claim', self._identity(app), claim], mutates=True, required=('released',)))

    def set_claim_context(self, app: str, claim: str, context: dict[str, str], expected_revision: int) -> c.Claim:
        return cast(c.Claim, self._json(['set-claim-context', self._identity(app), claim, Payload(context), *_options(expected_revision=expected_revision)], mutates=True, required=('uuid', 'context')))

    def retrieve(self, request: co.Retrieval, *, scope: str | None = None, policies: list[po.Policy] | None = None) -> co.RetrievalReceipt:
        return cast(co.RetrievalReceipt, self._json(['retrieve', Payload(request), *_options(scope=scope)], mutates=True, policies=policies, required=('request', 'descriptor')))

    def acquire_retrieved(self, request: co.AcquireRetrievedRequest, *, scope: str | None = None, policies: list[po.Policy] | None = None) -> co.AcquiredDescriptor:
        return cast(co.AcquiredDescriptor, self._json(['acquire-retrieved', Payload(request), *_options(scope=scope)], mutates=True, policies=policies, required=('ownership', 'path')))

    def source_path(self, retrieval: str, *, scope: str | None = None) -> co.SourcePath:
        return cast(co.SourcePath, self._json(['source-path', retrieval, *_options(scope=scope)], required=('path',)))

    def path(self, uuid: str, *, verification: Literal['metadata', 'full'] | None = None, policy_refs: list[str] | None = None) -> co.ResolvedResource:
        return cast(co.ResolvedResource, self._json(['path', uuid, *_options(verification=verification, policy=policy_refs)], required=('entry_uuid', 'directory', 'verification')))

    def verify(self, uuid: str) -> co.ResolvedResource:
        return cast(co.ResolvedResource, self._json(['verify', uuid], required=('entry_uuid', 'directory', 'verification')))

    def reclaim(self) -> c.ReclaimReport:
        return cast(c.ReclaimReport, self._json(['reclaim'], mutates=True, required=('reclaimed', 'deferred', 'pending')))

    def purge_retrieval(self, retrieval: str) -> co.PurgeResult:
        return cast(co.PurgeResult, self._json(['purge-retrieval', retrieval], mutates=True, required=('purged',)))

    def register_group(self, request: g.GroupDefinition) -> g.GroupRecord:
        return cast(g.GroupRecord, self._json(['register-group', Payload(request)], mutates=True, bounded=True, required=('uuid', 'revision', 'definition')))

    def show_group(self, uuid: str) -> g.GroupRecord:
        return cast(g.GroupRecord, self._json(['show-group', uuid], required=('uuid', 'revision', 'definition')))

    def list_groups(self, *, key: str | None = None) -> list[g.GroupRecord]:
        return cast(list[g.GroupRecord], self._json(['list-groups', *_options(key=key)], array=True))

    def replace_group(self, uuid: str, request: g.GroupDefinition, expected_revision: int) -> g.GroupRecord:
        return cast(g.GroupRecord, self._json(['replace-group', uuid, Payload(request), *_options(expected_revision=expected_revision)], mutates=True, bounded=True, required=('uuid', 'revision', 'definition')))

    def remove_group(self, uuid: str, expected_revision: int) -> g.RemoveResult:
        return cast(g.RemoveResult, self._json(['remove-group', uuid, *_options(expected_revision=expected_revision)], mutates=True, required=('removed',)))

    def prepare_group(self, uuid: str, *, expected_revision: int | None = None, pin: p.GroupPinRequest | None = None) -> p.PreparedGroup:
        return cast(p.PreparedGroup, self._json(['prepare-group', uuid, *_options(expected_revision=expected_revision, pin=None if pin is None else Payload(pin))], mutates=True, bounded=True, required=('group', 'pin', 'resources', 'stale')))

    def pin_outputs(self, request: p.PinRequest) -> p.PinnedOutputs:
        return cast(p.PinnedOutputs, self._json(['pin-outputs', Payload(request)], mutates=True, bounded=True, required=('pin', 'paths')))

    def pin_group(self, uuid: str, expected_revision: int, pin: p.GroupPinRequest) -> p.PinRecord:
        return cast(p.PinRecord, self._json(['pin-group', uuid, *_options(expected_revision=expected_revision, pin=Payload(pin))], mutates=True, bounded=True, required=('uuid', 'state')))

    def pin(self, uuid: str) -> p.PinInspection:
        return cast(p.PinInspection, self._json(['pin', uuid], required=('uuid', 'state', 'outputs')))

    def pins(self, *, key: str | None = None) -> list[p.PinInspection]:
        return cast(list[p.PinInspection], self._json(['pins', *_options(key=key)], array=True))

    def unpin(self, uuid: str) -> p.PinRecord:
        return cast(p.PinRecord, self._json(['unpin', uuid], mutates=True, required=('uuid', 'state')))

    def run(self, uuid: str, command: Sequence[str], *, verification: Literal['metadata', 'full'] | None = None,
            policy_refs: list[str] | None = None, options: RunOptions | None = None) -> RunResult:
        return self._run(['run', uuid, *_options(verification=verification, policy=policy_refs)], command, options=options)

    def run_claim(self, app: str, claim: str, command: Sequence[str], *, relative: str | None = None,
                  verification: Literal['metadata', 'full'] | None = None, policy_refs: list[str] | None = None,
                  options: RunOptions | None = None) -> RunResult:
        return self._run(['run-claim', self._identity(app), claim, *_options(relative=relative, verification=verification, policy=policy_refs)], command, options=options)

    def run_group(self, request: g.GroupRequest, command: Sequence[str], *, policies: list[po.Policy] | None = None,
                  options: RunOptions | None = None) -> RunResult:
        return self._run(['run-group', Payload(request)], command, options=options, policies=policies, bounded=True)

    def run_registered_group(self, uuid: str, command: Sequence[str], *, options: RunOptions | None = None) -> RunResult:
        return self._run(['run-registered-group', uuid], command, options=options)

    def prepare_run_group(self, uuid: str, command: Sequence[str], *, expected_revision: int | None = None,
                          pin: p.GroupPinRequest | None = None, options: RunOptions | None = None) -> RunResult:
        return self._run(['prepare-run-group', uuid, *_options(expected_revision=expected_revision, pin=None if pin is None else Payload(pin))], command, options=options, mutates=True, bounded=True)

    def run_retrieved(self, request: co.Retrieval, command: Sequence[str], *, scope: str | None = None,
                      policies: list[po.Policy] | None = None, options: RunOptions | None = None) -> RunResult:
        return self._run(['run-retrieved', Payload(request), *_options(scope=scope)], command, options=options, policies=policies, mutates=True)

    def run_acquired(self, request: co.CompositionRequest, command: Sequence[str], *, scope: str | None = None,
                     policies: list[po.Policy] | None = None, options: RunOptions | None = None) -> RunResult:
        return self._run(['run-acquired', Payload(request), *_options(scope=scope)], command, options=options, policies=policies, mutates=True)
