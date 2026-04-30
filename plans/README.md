# Mopi Planning Suite

Purpose: these files are the execution plans and the completion trackers for the initial build of `mopi`.

This suite is written for parallel agent execution. Each subsystem file is detailed enough to let an agent pick up work, implement against a stable contract, and update progress without needing to reconstruct the overall architecture from chat history.

## Status Conventions

- `Overall Status`: `NOT STARTED`, `IN PROGRESS`, `BLOCKED`, `DONE`
- Checklists use Markdown task boxes:
- `[ ]` not started
- `[x]` completed

When work is blocked, do not mark items complete. Add a short blocker note inside the relevant file.

## Update Protocol

1. Read `plans/00-master-tracker.md` before starting.
2. Read the subsystem file you are about to work from.
3. Update the subsystem file first.
4. Update `plans/00-master-tracker.md` second.
5. Do not mark a section done until its acceptance criteria and verification steps are satisfied.
6. If you change a contract that other workstreams depend on, update all affected plan files in the same change.

## Execution Rules For Agents

- Prefer the smallest correct implementation that satisfies the acceptance criteria.
- Keep shared contracts stable: config schema, IPC message shapes, storage schema, and search result types should not drift independently.
- Preserve soft metadata filter behavior by default. `filetype:rs` and `name:main` should bias results rather than hard-exclude them unless a strict mode is explicitly introduced.
- Optimize for warm-query latency and indexing throughput, but do not bypass safety checks around traversal, parser limits, or corrupted document handling.
- `mopid` owns indexes and model state. `kiwi` and `mopictl` are clients.

## Fixed Product Decisions

- Implementation language: Rust
- Topology: `mopid` daemon + `kiwi` GUI + `mopictl` CLI
- Embeddings: fully local model runtime
- Config: single global config rooted at `XDG_CONFIG_HOME/mopi`
- Search relevance: content-first, with filename/path and metadata boosts
- Filters: soft by default
- Hidden files: ignored by default
- Index policy: whitelist and blacklist modes must both exist
- Symlinks: fully followed, with cycle and duplication protection
- PDFs: speed-first extraction, no OCR in v1

## File Map

- `plans/00-master-tracker.md`: master tracker, milestones, dependency map, global acceptance criteria
- `plans/01-workspace-and-foundation.md`: workspace layout, crate scaffold, shared conventions
- `plans/02-daemon-and-ipc.md`: `mopid`, socket protocol, request lifecycle, streaming search
- `plans/03-config-and-policy.md`: XDG config, policy schema, validation, defaults
- `plans/04-crawl-and-discovery.md`: directory traversal, ignore rules, symlink following, change detection
- `plans/05-extraction-and-normalization.md`: text/config/docx/odt/pdf extraction pipeline
- `plans/06-storage-and-indexing.md`: SQLite, Tantivy, vector metadata coordination, recovery
- `plans/07-embeddings-and-vector-search.md`: model runtime, chunk embedding, ANN index
- `plans/08-query-ranking-and-filters.md`: query grammar, hybrid retrieval, reranking, snippets
- `plans/09-cli-mopictl.md`: CLI commands, admin workflows, JSON output
- `plans/10-kiwi-gui.md`: Wayland launcher UI, streaming UX, actions
- `plans/11-testing-benchmarks-and-hardening.md`: fixtures, adversarial tests, benchmarks, fuzzing
- `plans/12-observability-release-and-maintenance.md`: logging, stats, systemd user service, packaging, docs
- `plans/13-agent-execution-map.md`: recommended agent sequencing and parallelization boundaries

## Completion Standard

The project is not complete when code merely compiles. The project is complete when:

- the subsystem checklist is finished
- the subsystem acceptance criteria are satisfied
- verification commands pass
- the master tracker is updated
- downstream blockers are removed or explicitly documented
