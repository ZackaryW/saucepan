# Registered resource groups

A group saves resource intent under an explicit local or app owner. Registration checks sources, policy references and authority without retrieving anything. It creates no claims or pins.

Example local definition (replace the source UUID with a registered source):

```json
{"registration_key":"zush/project-a","members":[{"name":"command","target":{"type":"acquire","source_uuid":"11111111-1111-4111-8111-111111111111","selector":{"type":"ref","reference":"main"},"selection":{"folder":"commands/build"},"policy_refs":[]}}],"policy_refs":[]}
```

Example app-owned definition:

```json
{"registration_key":"plugins/project-a","members":[{"name":"plugin","target":{"type":"acquire","source_uuid":"11111111-1111-4111-8111-111111111111","selector":{"type":"ref","reference":"main"},"selection":{"folder":"plugins/example"},"claim":{"scope":"project-a","context":{"consumer":"plugins"}},"policy_refs":[]}}],"policy_refs":[]}
```

Native callers use `Store::register_group` for local groups or `AppSession::register_group` for app groups. `group(uuid)` inspects a record and `groups(Some(key))` finds owner-visible registrations by literal key. Equivalent registrations converge on one UUID, independent of member order; divergent definitions under that owner's key conflict. Another app can use the same key independently.

`replace_group(uuid, expected_revision, definition)` changes a definition and clears its selection. `remove_group(uuid, expected_revision)` removes only registration metadata. Neither deletes claims, pins or payloads. App uninstall retires its groups in the existing metadata transaction while leaving explicit output pins intact. Current source/scope permissions apply using saved registration witnesses, including cleanup after claim release.

Members can reference existing `output` UUIDs, owned `claim` UUIDs, or registered-source `acquire` intents. Each group has 1–128 uniquely named members; optional relative views must stay inside the selected output. Local acquire intents omit `claim`; app intents supply an explicit scope. There is no inferred application or synthetic public scope.

Group `policy_refs` accept only access restrictions and a path-verification floor. Acquisition policy references remain on their member. Pin policies are supplied only to explicit pin operations. All persistent references must already be registered. Temporary, no-store and caller-directory acquisition remain standalone operations and cannot become persistent group intents.

Preparation and consumer launch are separate operations; registration alone performs neither. Preparation may leave complete unselected cache entries after a failure. Atomicity covers final selections and metadata, not provider downloads or external consumer activation.

`prepare_group(uuid, expected_revision, options)` returns the selected record and a `GroupLease` held continuously through publication. Existing manual members stay selected; due update policies refresh acquisition results. Fixed outputs never move. Unavailable claim targets obey their saved report-unavailable, restore-exact, or refresh policy. Compatible Git subpaths share one exact retrieval; incompatible provider requests release their source guards before the next retrieval.

Preparation matches the complete scoped claim set first. Compatible fresh parent/child requests share the containing claim independently of input order; distinct scopes remain independent. Existing narrower ownership cannot be silently broadened, and context or adopted policy changes require their existing explicit operations.

Complete immutable candidate outputs can be published individually, but claims, successful-check records and the group generation are committed in one metadata transaction. An app revision increments once. Candidates stay guarded throughout. A failed member or changed group/app/policy observation preserves the prior selection and check timestamps. Each selection records exact output and source provenance. Permitted stale fallback is reported separately in the returned `stale` member set; it does not create successful-check evidence.

`open_group(uuid)` is read-only. It refuses a generation whose live claim witness has moved or disappeared, and never repairs it implicitly. A previously returned lease continues protecting its exact old outputs even after replacement or removal.

`prepare_group_with_pin(uuid, expected_revision, options, pin_request)` adds an explicit durable pin to that same final transaction. `pin_group(uuid, expected_revision, pin_request)` pins an already prepared generation. A group pin request contains only `registration_key` and `policy_refs`; targets come from the exact selected generation. Register its `pin_policy` first. Older pins remain independent. The same key and original expected revision return the durable original receipt after a lost response, without another retrieval or `new_entry`; different inputs or a released key conflict. A post-journal error may already have committed: inspect the group revision or retry the same pin receipt before starting a new operation.

These native operations are available on `Store` and `AppSession`. Neither ordinary preparation nor read-only opening creates durable retention. See [output pins](output-pins.md) for the switch-to-B/release-A lifetime protocol.

## CLI flow

Initialize a fresh store, register sources/policies, and save one of the definitions above with the returned UUIDs. The CLI defaults to the user-home `.saucepan`; use `--root` for isolated fixtures. App ownership is opt-in through `--as-app APP_UUID`, with `--credential-file` only when that app requires it.

```text
saucepan register-group group.json
saucepan list-groups --key zush/project-a
saucepan show-group GROUP_UUID
saucepan prepare-group GROUP_UUID --expected-revision 1
saucepan run-registered-group GROUP_UUID -- PROGRAM ARGUMENTS
saucepan prepare-run-group GROUP_UUID -- PROGRAM ARGUMENTS
saucepan replace-group GROUP_UUID replacement.json --expected-revision N
saucepan remove-group GROUP_UUID --expected-revision N
```

Preparation returns `group`, `resources`, optional `pin`, and a `stale` member-name set. It releases transient native guards on CLI exit; use a run command or an explicit pin for subsequent use. Both launch commands set `SAUCEPAN_RESOURCE_GROUP_FILE`, preserve literal arguments and child exit status, and use the same guarded launch path as `run-group`. The descriptor is valid for that launch lifetime. An app-owned command needs its app flags on every invocation; failure never falls back to local administration.

For durable retention, save this group pin request after registering its policy:

```json
{"registration_key":"power-eagle/session-42/plugin-set/a","policy_refs":["33333333-3333-4333-8333-333333333333"]}
```

```text
saucepan prepare-group GROUP_UUID --expected-revision N --pin pin.json
saucepan pin-group GROUP_UUID --expected-revision N --pin another-pin.json
```

The first operation prepares and pins in one transaction. The second pins the existing generation. `prepare-run-group` also accepts `--pin` with an explicit expected revision. Change the pin key for a new generation or independent consumer; repeat the original key/revision to reconcile a lost response. The JSON never accepts caller-supplied output overrides on a group pin.

The executable fixtures are `cargo test --test resource_group_cli` and `cargo test --test resource_groups`. They build fresh local/app definitions with real UUIDs, exercise a command/hooks/service-shaped lifetime and pin receipts, and preserve historical `mock/`. Detached service supervision, activation rollback, and runtimes that close inherited handles remain consumer responsibilities; persistent pins are available when an in-process host cannot retain a launcher guard.
