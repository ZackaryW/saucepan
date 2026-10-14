# Durable output pins

[← Saucepan guide](../README.md#resource-lifetime-groups-and-pins)

A pin keeps an exact set of output UUIDs available until explicitly released. It survives the process that created it and does not follow a moving claim. Ordinary acquisition, claims, and invocation leases do not create pins.

Register this immutable policy through `saucepan register-policies pin-policy.json`:

```json
[{"uuid":"33333333-3333-4333-8333-333333333333","type":"pin_policy","retention":"until_released"}]
```

After acquiring an output, put its actual UUID in `pin-a.json`:

```json
{"registration_key":"power-eagle/session-42/plugin/revision-a","outputs":["22222222-2222-4222-8222-222222222222"],"policy_refs":["33333333-3333-4333-8333-333333333333"]}
```

Run `saucepan pin-outputs pin-a.json`. The response contains the durable pin receipt and a path per output. The caller may exit; consumers can continue reading those exact paths. Paths are read-only by contract: editing a cached file is corruption, not a new revision.

When upgrading, acquire B, pin B with a new registration key, switch consumers to B, then run `saucepan unpin PIN_A_UUID` only after every consumer of A is finished. Reclaim can then remove A under its revision policy. A second independent pin on A still protects it. `history: none` does not override pins.

`saucepan pin PIN_UUID` inspects a receipt; `saucepan pins --key KEY` finds receipts by registration key. An equivalent retry returns the same active pin; different targets under the same key conflict. Released receipts remain inspectable and cannot be reactivated. Repeated unpin is harmless; an unknown UUID is an error.

Inspection adds an `outputs` map to the receipt: each UUID reports its exact `path`, recorded `state`, and current metadata-level `available` flag. Missing payloads remain inspectable as unavailable; this flag is not a full file-content verification. Native `pin`/`pins` return `PinInspection`, separating the saved `pin` from these live observations.

Use `--as-app APP_UUID` and, when enabled, `--credential-file FILE` for app-owned pins. Admission requires a real readable owned claim and the `pin` action. Inspection uses its captured source and scope even after that claim moves or is released. The independent `unpin` action permits cleanup without permitting new retention. Current credentials and source/scope restrictions still apply. Trusted local administration can inspect and release orphaned pins after app uninstall; uninstall itself never releases them.

Pin requests accept exactly one registered `pin_policy`. It is not an app/source default or an acquisition policy. Unknown fields, duplicate outputs, more than 128 targets, and CLI payloads over 1 MiB are rejected. Multiple targets are admitted together: verification failure creates no partial pin. Reclaim verifies the pin catalog before deleting any outputs and refuses corrupted metadata.

The native equivalents are `Store::pin_outputs`, `pin`, `pins`, and `unpin`, also exposed by `AppSession`. No new crate is needed: verified metadata transactions, ordered native output guards, and existing policy fences provide the persistence and concurrency contracts.

For [registered groups](resource-groups.md), `prepare-group GROUP_UUID --expected-revision N --pin FILE` publishes the generation and explicit pin together. `pin-group GROUP_UUID --expected-revision N --pin FILE` retains an existing generation. The file contains only `registration_key` and registered `policy_refs`. Group receipts preserve the original generation for retries after claim movement; their `stale` member set records permitted fallback without inventing a successful update check.
