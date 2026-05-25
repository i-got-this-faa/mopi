# Master Tracker

Overall Status: IN PROGRESS  
Current Owner: OpenCode  
Blocked By: None  
Last Updated: 2026-04-27

## Mission

Build a secure, blazing fast, accurate, modular local semantic search engine for local files with these mandatory capabilities:

- search by name and content
- support plain text and common config formats
- support `docx`, `odt`, and `pdf`
- run embeddings fully locally
- expose a daemon, GUI, and CLI

## Fixed Architecture

- Language: Rust
- Binaries: `lssd`, `lssi`, `lssctl`
- Lexical search: embedded index, content and name aware
- Semantic search: fully local embeddings plus embedded ANN library
- Metadata/state: SQLite
- Config: single global XDG config under `XDG_CONFIG_HOME/lss`
- Filters: soft by default
- Hidden files: ignored by default
- Policies: whitelist and blacklist modes
- Symlinks: fully followed with loop and duplicate suppression
- PDF strategy: speed-first extraction

## Global Success Criteria

- [x] `lssd` serves queries over a local Unix socket without remote network exposure.
- [ ] Warm lexical queries return interactively fast on realistic corpora.
- [ ] Warm hybrid queries return interactively fast on realistic corpora.
- [ ] `lssi` shows progressive search results quickly enough to feel launcher-like.
- [ ] Name and content both materially influence ranking.
- [ ] `filetype:` and `name:` work as soft metadata filters by default.
- [x] Plain text and config files index correctly.
- [ ] `docx`, `odt`, and `pdf` files index correctly.
- [x] Hidden files are excluded by default.
- [x] Whitelist and blacklist policies are both implemented and tested.
- [x] Symlink loops and duplicate-target paths do not create runaway indexing.
- [ ] Extraction failures never crash the daemon.
- [ ] Missing embedding model yields graceful lexical-only behavior.
- [ ] Storage can recover cleanly from interrupted indexing work.

## Performance Targets

These are target gates, not suggestions. If they are missed, the relevant subsystem is not done.

- [ ] Warm daemon lexical query P50 under 60 ms on the reference development machine.
- [ ] Warm daemon hybrid query P50 under 200 ms on the reference development machine.
- [ ] `lssi` first visible results under 50 ms after a typical keystroke on a warm index.
- [ ] Query embedding time stays within the interactive budget on the reference model.
- [ ] Indexing unchanged files avoids expensive re-extraction and re-embedding.
- [ ] PDF extraction path remains speed-first and does not block the full pipeline on slow documents.

## Workstream Index

| Workstream | Plan File | Status | Depends On |
| --- | --- | --- | --- |
| Workspace and foundation | `plans/01-workspace-and-foundation.md` | DONE | None |
| Daemon and IPC | `plans/02-daemon-and-ipc.md` | IN PROGRESS | 01 |
| Config and policy | `plans/03-config-and-policy.md` | IN PROGRESS | 01 |
| Crawl and discovery | `plans/04-crawl-and-discovery.md` | IN PROGRESS | 01, 03 |
| Extraction and normalization | `plans/05-extraction-and-normalization.md` | IN PROGRESS | 01, 03, 04 |
| Storage and indexing | `plans/06-storage-and-indexing.md` | IN PROGRESS | 01, 02, 03, 04, 05 |
| Embeddings and vector search | `plans/07-embeddings-and-vector-search.md` | DONE | 01, 05, 06 |
| Query, ranking, filters | `plans/08-query-ranking-and-filters.md` | IN PROGRESS | 02, 05, 06, 07 |
| CLI | `plans/09-cli-lssctl.md` | NOT STARTED | 02, 03, 06, 08 |
| GUI | `plans/10-lssi-gui.md` | IN PROGRESS | 02, 08 |
| Testing, benchmarks, hardening | `plans/11-testing-benchmarks-and-hardening.md` | NOT STARTED | 01-10 |
| Observability, release, maintenance | `plans/12-observability-release-and-maintenance.md` | NOT STARTED | 01-11 |
| Agent execution map | `plans/13-agent-execution-map.md` | IN PROGRESS | None |

## Milestones

### M0 Foundation

- [x] Workspace layout exists with the three binaries and shared crates.
- [x] Common error, logging, config-path, and test conventions are documented and scaffolded.
- [x] The master tracker and agent execution map are kept current.

### M1 Lexical Search Baseline

- [x] Config loads from XDG paths.
- [x] Crawl and policy engine walks allowed roots, ignores dotfiles, and handles symlink loops safely.
- [x] Plain text and config extraction work. (Plus Office/PDF)
- [x] SQLite and Tantivy integration work. (Core schema DONE)
- [x] `lssd` can answer lexical name-and-content queries.
- [x] `lssctl query` is usable end-to-end.

### M2 Semantic Hybrid Search

- [x] Local embedding runtime is wired into the daemon.
- [x] Chunking and vector indexing work.
- [x] Hybrid retrieval and reranking work.
- [ ] Soft metadata filters influence ranking.
- [x] Lexical-only fallback exists when the model is unavailable.

### M3 Document Coverage And GUI

- [ ] `docx` extraction works.
- [ ] `odt` extraction works.
- [ ] Fast `pdf` extraction works.
- [ ] `lssi` streams results from `lssd` with launcher-style UX.

### M4 Hardening And Release Readiness

- [ ] Adversarial tests cover traversal, parser failure, and corrupted files.
- [ ] Benchmarks exist and are tracked.
- [ ] Stats, logs, and diagnostics are exposed.
- [ ] Packaging and a user-service deployment path exist.
- [ ] User-facing documentation and sample config exist.

## Cross-Stream Contracts That Must Stay Stable

- [ ] Config schema changes are reflected in config docs, CLI flows, and daemon reload behavior.
- [ ] IPC request and response changes are reflected in `lssctl` and `lssi` clients.
- [ ] Storage schema changes include migration and recovery notes.
- [ ] Search result types always include enough information for filename, path, snippet, and explanation display.
- [ ] File identity rules stay consistent across crawler, storage, and indexers.
- [ ] Alias-path handling for symlinked files stays consistent across crawl, indexing, and ranking.

## Global Risks

- Symlink-following can explode traversal cost or create duplicate records if file identity is not designed correctly.
- PDF extraction can dominate indexing time if slow or malformed files are not bounded.
- Overly strict metadata filters can reduce recall and violate the product goal of content-first relevance.
- Embedding runtime choice can sink memory budget or interactive latency if selected without benchmark gates.
- Partial-index commit failures can corrupt cross-store consistency unless journaling and recovery are designed early.

## Release Gate

Before the initial release candidate, every subsystem file must have:

- [ ] `Overall Status: DONE`
- [ ] no unresolved blockers
- [ ] acceptance criteria checked
- [ ] verification section completed
