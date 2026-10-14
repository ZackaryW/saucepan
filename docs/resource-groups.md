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
