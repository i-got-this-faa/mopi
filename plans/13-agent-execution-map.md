# Agent Execution Map

Overall Status: IN PROGRESS  
Current Owner: OpenCode  
Blocked By: None  
Last Updated: 2026-04-27

## Objective

Provide a concrete parallelization map so multiple agents can work without stepping on the same contracts or waiting on avoidable dependencies.

## Phase Order

### Phase 0

- [x] Finish `plans/01-workspace-and-foundation.md`.
- [x] Keep `plans/00-master-tracker.md` and this file updated.

### Phase 1

These can proceed in parallel after workspace scaffold stabilizes.

- [ ] `plans/02-daemon-and-ipc.md`
- [ ] `plans/03-config-and-policy.md`
- [ ] initial test-support scaffold from `plans/11-testing-benchmarks-and-hardening.md`

### Phase 2

These can proceed in parallel once config contracts are stable.

- [x] `plans/04-crawl-and-discovery.md` (Watcher and Traversal DONE)
- [x] text/config portions of `plans/05-extraction-and-normalization.md` (DONE, plus Office/PDF)

### Phase 3

These should start once crawl and basic extraction contracts exist.

- [x] `plans/06-storage-and-indexing.md` (Core DONE)
- [ ] model benchmark harness work from `plans/07-embeddings-and-vector-search.md`

### Phase 4

These can proceed together once storage and initial daemon query surfaces exist.

- [ ] remaining `plans/07-embeddings-and-vector-search.md`
- [ ] `plans/08-query-ranking-and-filters.md`
- [x] `plans/09-cli-lssctl.md` (Daemon side DONE)

### Phase 5

- [ ] office and PDF parts of `plans/05-extraction-and-normalization.md`
- [ ] `plans/10-lssi-gui.md`

### Phase 6

- [ ] `plans/11-testing-benchmarks-and-hardening.md`
- [ ] `plans/12-observability-release-and-maintenance.md`

## Shared Contracts That Need Coordination

- [ ] Config schema
- [ ] IPC request and response envelope
- [ ] search result shape
- [ ] file identity and alias-path model
- [ ] chunk metadata shape
- [ ] DB migration strategy

## Suggested Agent Ownership Boundaries

- [ ] Agent A: workspace, shared types, and common conventions
- [ ] Agent B: daemon, IPC, and client bootstrap
- [ ] Agent C: config, policy, and path rules
- [ ] Agent D: crawl, change detection, and file identity
- [ ] Agent E: extraction and normalization
- [ ] Agent F: storage and indexing
- [ ] Agent G: embeddings, ANN, and semantic retrieval
- [ ] Agent H: query parser, ranking, and snippets
- [ ] Agent I: CLI and diagnostics surface
- [ ] Agent J: GUI and interaction model
- [ ] Agent K: testing, benchmarks, and hardening
- [ ] Agent L: release, service, and operational docs

## Merge Risk Areas

- [ ] Shared `types` crate churn.
- [ ] IPC protocol churn.
- [ ] Storage schema churn.
- [ ] Query result shape churn.
- [ ] Config schema churn.

## Rules For Parallel Work

- [ ] Do not change shared contracts without updating all dependent plan files and implementations.
- [ ] Land infrastructure before high-level feature branches depend on it.
- [ ] Keep the daemon as the source of truth for search and indexing behavior.
- [ ] Prefer additive schema changes until the storage layer stabilizes.

## Completion Criteria

- [ ] All subsystem files are either complete or explicitly blocked with named dependencies.
- [ ] The master tracker accurately reflects subsystem status.
- [ ] No major contract surface remains ambiguous.
