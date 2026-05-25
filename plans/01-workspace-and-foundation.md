# Workspace And Foundation

Overall Status: DONE  
Current Owner: OpenCode  
Blocked By: None  
Last Updated: 2026-04-27

## Objective

Create the Rust workspace, crate boundaries, shared conventions, and baseline tooling so all other agents can implement against a stable project skeleton.

## Scope

- workspace root files
- binary crate scaffolding
- shared crate scaffolding
- baseline linting, formatting, testing, and logging conventions
- crate dependency boundaries
- shared result and error patterns

## Deliverables

- [x] Root `Cargo.toml` workspace manifest.
- [x] Binary crates for `lssd`, `lssctl`, and `lssi`.
- [x] Shared crates for types, config, crawl, extraction, indexing, query, ranking, and IPC.
- [x] Baseline README with developer bootstrap instructions.
- [x] Formatting and lint configuration.
- [x] Common tracing and error-handling setup.

## Recommended Workspace Layout

- `bin/lssd`
- `bin/lssctl`
- `bin/lssi`
- `crates/types`
- `crates/config`
- `crates/crawl`
- `crates/extract`
- `crates/index-meta`
- `crates/index-lexical`
- `crates/index-vector`
- `crates/embed`
- `crates/query`
- `crates/rank`
- `crates/ipc`
- `crates/test-support`
- `fixtures/`
- `scripts/`
- `plans/`

## Detailed Checklist

- [x] Create the root workspace manifest with explicit members.
- [x] Create a consistent package naming convention for all crates.
- [x] Add a pinned Rust toolchain file.
- [x] Add `.gitignore` entries for build outputs, indexes, caches, and model artifacts.
- [x] Add `rustfmt` configuration if non-default behavior is required.
- [x] Add `clippy` configuration or project lint policy.
- [x] Decide and document the async runtime strategy.
- [x] Decide and document the error-stack strategy.
- [x] Add a shared `types` crate for request, response, result, and domain identifiers.
- [x] Add a shared tracing initialization utility or crate-level helper.
- [x] Add a `test-support` crate for fixture setup, temp dirs, daemon harnesses, and corpus generation.
- [x] Document crate dependency rules so feature work does not create circular references.
- [x] Define where model artifacts, indexes, DB files, and logs will live in XDG data/cache/runtime directories.

## Dependency Rules

- `types` must not depend on application crates.
- `config` may depend on `types`, but not on UI crates.
- `ipc` may depend on `types`, but not on GUI code.
- `lssd` may depend on everything except `lssi` and `lssctl`.
- `lssctl` and `lssi` should depend on `ipc`, `types`, and lightweight shared helpers only.
- Extractors must not depend on GUI or CLI crates.

## Acceptance Criteria

- [x] `cargo check --workspace` succeeds.
- [x] `cargo fmt --all --check` succeeds.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` succeeds.
- [x] `cargo test --workspace` succeeds, even if early tests are only scaffold smoke tests.
- [x] Every crate has a clear responsibility and no obvious circular design pressure.
- [x] Future agents can add code to a subsystem without first restructuring the workspace.

## Verification

- [x] Run `cargo check --workspace`.
- [x] Run `cargo fmt --all --check`.
- [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Run `cargo test --workspace`.

## Notes And Risks

- Do not prematurely split tiny crates if the boundary is purely theoretical. Keep the layout modular, but not fragmented without purpose.
- The workspace is the backbone for every downstream plan file. Do not mark this workstream done while crate ownership remains ambiguous.
