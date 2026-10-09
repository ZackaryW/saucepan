# Claim context

Context is a plain map of caller-defined string labels on a claim. It never
changes output identity, deduplication, source refresh, or retention. Two projects
can have distinct claims and contexts while sharing the same cached output.

Native callers use `Store::acquire_for_with_context(app, request, context, options)`
to publish a new claim and its labels together. `acquire_for` creates empty
context. Failed acquisition does not publish partial labels. Repeated acquisition
creates another claim; labels do not enforce uniqueness or provide an upsert.

`Store::set_claim_context(app, claim, context, expected_revision)` replaces the
whole map and returns the committed claim. Read the application's revision first.
A stale revision fails, including for a replacement equal to current labels.
An equal replacement at the expected revision is a no-op; `{}` clears labels.
Changed context increments the application's revision without moving its claim or
binding. Missing/removing applications and missing claims fail.

Keys contain 1–128 UTF-8 bytes; values contain 0–4096 bytes; each context has at
most 64 entries. Values must be strings. Dotted keys are literal. Paths, case,
whitespace, and Unicode are preserved: the consumer owns canonical project names.
Omitting context means empty metadata in the current contract, not a general
schema compatibility or legacy migration promise.

Primitive updates preserve context present at commit time, including a concurrent
context edit. Managed updates and rollback preserve labels but conflict if a
context edit changes their captured application revision. Deactivation and
recovery preserve labels. Release/uninstall remove them with the claim; reclaim
keeps them alongside the claim's unavailable output. Managed installation and
bootstrap start with empty context; use explicit replacement to label them.
