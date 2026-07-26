## 1. Preserve Index and Identity Compatibility

- [x] 1.1 Add fail-first tests proving legacy GitHub index entries deserialize without revision fields and absent optional fields are omitted when serialized.
- [x] 1.2 Extend GitHub index entries with optional requested-ref and resolved-commit metadata while preserving existing manifest-name lookup behavior.
- [x] 1.3 Add fail-first tests for same-origin replacement and conflicts from different same-type or cross-type origins.
- [x] 1.4 Replace discriminant-only upsert checks with source-origin comparison that preserves the existing entry on conflict.

## 2. Normalize and Resolve GitHub Targets

- [x] 2.1 Add fail-first unit tests for strict `owner/repo` recognition, raw-Git HTTPS normalization, and pass-through of explicit URLs and filesystem paths.
- [x] 2.2 Implement minimal GitHub slug normalization without a new parser dependency or changes to the existing workspace directory mapping.
- [x] 2.3 Add fail-first Git integration tests for default-branch installs and branch, tag, and commit `--ref` installs with recorded resolved commits.
- [x] 2.4 Extend the install CLI and Git fetch result to carry optional requested refs and resolved commits into the GitHub index entry.
- [x] 2.5 Add fail-first update tests proving stored branches advance, tags resolve consistently, commit SHAs remain pinned, and ref metadata refreshes.
- [x] 2.6 Implement ref fetching, documented resolution order, detached checkout, and `HEAD` resolution while preserving no-ref pull behavior.

## 3. Use Native Authentication and Stable Errors

- [x] 3.1 Add fail-first tests that raw Git with a configured token warns once and omits ineffective username/password injection, while gh receives `GITHUB_TOKEN`.
- [x] 3.2 Implement backend-specific native authentication behavior while retaining existing SSL-key forwarding and configuration parsing.
- [x] 3.3 Add fail-first tests for no-source configuration errors, known misses, Git/gh source failures, successful fallback, and mixed failure aggregation.
- [x] 3.4 Construct and propagate `ConfigError`, `NotFound`, and `SourceError` at their source boundaries and return compact per-source context after fallback.

## 4. Add Safe Uninstall

- [x] 4.1 Add BDD scenarios and fail-first integration tests for uninstalling GitHub, custom-Git, local, missing-checkout, and unknown entries.
- [x] 4.2 Add unit tests that managed checkout deletion is contained beneath the expected source directory and refuses an escaping path.
- [x] 4.3 Implement `uninstall <manifest-name>` CLI dispatch, index removal, managed-checkout deletion, local-path preservation, and not-found behavior.

## 5. Documentation and Verification

- [x] 5.1 Update README examples to distinguish install targets from manifest-name operations and document shorthand, refs, native authentication, exit categories, and uninstall.
- [x] 5.2 Update install and update BDD scenarios to cover the accepted behavior while keeping marketplace and repository-root boundaries explicit.
- [x] 5.3 Run formatting, the full Cargo test suite, and OpenSpec validation; resolve all failures without adding deferred capabilities.
