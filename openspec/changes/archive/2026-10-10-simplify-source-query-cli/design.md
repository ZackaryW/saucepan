# Design

## Context

The implemented `add-source-discovery` change supplies `SourceQuery`, private-default source publication, anonymous `Store::query_sources`, app-session policy checks and deterministic metadata views. It is not synchronized into main specs yet. `Commands::QuerySources` currently requires a `PathBuf`; execution always reads JSON from that file. Docs start with application registration before showing anonymous usage. See proposal.md for motivation.

The current `SourceType` is a serde enum, and CLI input belongs in `src/cli`, not `core/query` or provider models. Existing `tests/source_query_cli.rs` already checks native/CLI parity, real keyring credentials and ensure/lease behavior.

## Goals / Non-Goals

**Goals:** Make the anonymous, file-free path explicit; lower filter-entry effort while keeping query and policy semantics identical across CLI inputs.

**Non-Goals:** Change public/private defaults, broaden existing source visibility, provide anonymous ownership, remove claim scopes, infer app IDs, add query aliases/DSLs/stdin formats, search working-directory ancestors, redesign claim-query inputs or migrate schemas. Default user-home root selection was explicitly added by the user before implementation.

## Decisions

1. **No arguments means the empty query.** `saucepan query-sources` constructs `SourceQuery::default()`. Do not manufacture `{}` on disk or register a synthetic application. The no-app path already exists; preserve it and make it the leading documented path.
2. **Five flags map to the existing fields.** Add `--uuid`, `--type`, `--repo`, `--origin`, `--url`. Use exact snake_case type values (especially `github_release`) and the existing native validation for semantic bounds. Use a small CLI-local `SourceQueryArgs` if that keeps the main dispatch readable. Avoid adding clap derives to the domain/query model just for CLI conversion.
3. **Keep a single optional positional file.** Retain `query-sources query.json` for scripts. Do not add an additional `--file` spelling, inline JSON or stdin handling in this bounded change. Clap conflicts reject file-plus-filter arguments before I/O; repeated scalar flags are invalid. This avoids both silent precedence and a breaking removal of the existing CLI form; it does not imply legacy persisted-schema support.
4. **Identity remains orthogonal to input.** Existing global `--as-app` chooses the app session; without it, execution remains anonymous. All input forms converge on `Store::query_sources`. Never retry an app query anonymously on error. Existing credential-file behavior stays unchanged.
5. **Preserve publication defaults.** The two usability corrections do not by themselves authorize exposing previously private definitions. This plan retains the implemented private default and explains publishing sources for anonymous lookup. A broader default change requires an explicit product decision and an updated plan, not an assumption during implementation.
6. **Extend the pending source-query capability.** This delta only adds uniquely named CLI requirements to the same `source-query` path. It does not duplicate or edit the four requirements owned by `add-source-discovery`. When separately authorized, sync/archive the base change before this extension. No other change artifacts or main specs are edited now.
7. **One coherent implementation checkpoint.** Input conversion, executable tests and the updated first-use docs form one small behavior slice. Reuse clap, serde and the existing test fixture; no new portable helper or external dependency is warranted. Run focused tests, format/Clippy and strict OpenSpec validation, inspect the diff and checkpoint explicit implementation paths only when applying.
8. **Resolve root once at the CLI boundary.** Explicit `--root` wins, including relative paths relative to the working directory. Otherwise use `std::env::home_dir()` and append `.saucepan` (USERPROFILE on Windows, HOME on Unix with the standard OS fallback). Reject an unavailable, empty or relative home with `invalid_request` and guidance to pass `--root`; never fall back to the working directory. All commands share this resolver. `init` alone creates a new root; other commands encountering a missing directory report `unsupported_store` with initialization guidance. Existing corrupt or unsupported roots retain their errors and are never initialized automatically. Native APIs and shim-carried roots remain explicit. Tests override only child-process home environments to disposable directories and never touch the real home store.

## Risks / Trade-offs

- Anonymous searches can return nothing despite registered private sources → explain visibility and the existing publication command; never silently escalate.
- Multiple input mechanisms can drift → construct the same `SourceQuery` and test exact output parity for all five filters and empty input.
- CLI enum naming defaults can differ from serde → explicitly test `github_release` and rejection of unsupported spellings.
- Synchronized specs lag behind the implemented base → document the dependency and preserve archive order rather than duplicating capability names.

## Migration Plan

No data migration. Existing file-based source-query commands continue to work. Direct flags and empty-input queries require the rebuilt CLI. Documentation should lead with the new form and retain a short saved-query example. No core access-policy or storage changes are needed.
