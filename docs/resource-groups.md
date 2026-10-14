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
