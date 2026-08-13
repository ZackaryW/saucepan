## 1. Mature approved utilities

- [x] 1.1 Add focused failing SDK tests for optional-ref bucket registration, bucket refresh, return values, and cache invalidation
- [x] 1.2 Implement `Workspace.add_bucket(..., reference=None)` and `Workspace.refresh_bucket(...)` through the existing mutation seam
- [x] 1.3 Add focused failing import/contract checks for the planned root and SDK Behave lifecycle support modules
- [x] 1.4 Implement the shared root and SDK Behave lifecycle support signatures from `utility-plan.md`
- [x] 1.5 Run focused utility verification independently and remove the disposable utility plan

## 2. Wire capability-owned public behavior

- [x] 2.1 Add each root capability's delegating `environment.py`, `support.py`, and thin `steps/bindings.py`
- [x] 2.2 Add the SDK capability's delegating lifecycle files and thin public-SDK bindings
- [x] 2.3 Prove a relevant undefined-step RED, then run every capability-owned Behave root independently to GREEN

## 3. Restore repository quality gates

- [x] 3.1 Apply the current Rust formatter without semantic edits
- [x] 3.2 Resolve all Clippy warnings with the smallest behavior-preserving changes
- [x] 3.3 Update SDK documentation for optional-ref bucket registration and bucket refresh

## 4. Reconcile specifications and complete verification

- [x] 4.1 Reconcile mature behavior into canonical OpenSpec specifications without losing existing requirements
- [x] 4.2 Run lock, supported-interpreter, format, lint, focused and complete test, native BDD, and clean package-build gates
- [x] 4.3 Record all ZPP stage outcomes and verify no retained OpenLease successor cohort exists
