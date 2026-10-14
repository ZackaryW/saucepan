# Group leases

A group retains already prepared outputs for one consumer. It does not retrieve,
update, recover, register an application or create claims. Prepare resources first.

An output-only request needs no application registration or credentials:

```json
{
  "members": [
    {"name": "command", "target": {"type": "output", "uuid": "11111111-1111-4111-8111-111111111111"}},
    {"name": "hooks", "target": {"type": "output", "uuid": "22222222-2222-4222-8222-222222222222"}, "relative": "hooks"}
  ]
}
```

Use real output UUIDs from acquisition receipts. A claim target uses
`{"type":"claim","uuid":"33333333-3333-4333-8333-333333333333"}` and requires
an explicit app session. Every member is checked under the same session, including
direct output targets. An unavailable or denied member fails the whole admission.

There must be 1–128 members with unique case-sensitive names matching
`[A-Za-z][A-Za-z0-9_-]{0,63}`. Names label results; they do not grant scope access.
Optional `relative` values identify directories within an output's manifest.
Two names may refer to the same output with different directory views.
Unknown fields and invalid paths are rejected. CLI input is limited to 1 MiB.

## Optional boundaries

Add `policy_refs` containing registered or caller-catalog policy UUIDs. Only access
policies and one path verification policy are accepted. For example, a portable
catalog can contain:

```json
[
  {
    "uuid": "44444444-4444-4444-8444-444444444444",
    "type": "access_policy",
    "actions": ["lease"],
    "scopes": ["project-a"]
  },
  {
    "uuid": "55555555-5555-4555-8555-555555555555",
    "type": "verification_policy",
    "policy_target": "path",
    "mode": "full"
  }
]
```

Reference these UUIDs in the request. Scope restrictions require claim targets;
direct output targets have no scope witness. Multiple access policies intersect.
An omitted dimension is unrestricted; an empty list denies all values of that
dimension. The boundary also applies to trusted local requests. It never expands
registered permissions or supplies identity. Caller catalogs cannot shadow
registered UUIDs, and changing a policy requires a new UUID and explicit adoption.

Group verification is a minimum: `metadata` cannot weaken a member's existing
`full` requirement. Shared outputs are checked once at the strongest required mode.
Descriptors report the actual mode and policy provenance. These are informational
paths, not authorization tokens or serialized lifetime guards.

This is a cooperative lifetime and policy boundary, not a filesystem sandbox.
Persistent groups, automatic retrieval and atomic multi-resource updates are
separate future changes.
