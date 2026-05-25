# Observability, Release, And Maintenance

Overall Status: NOT STARTED  
Current Owner: UNASSIGNED  
Blocked By: Core implementation across workstreams  
Last Updated: 2026-04-27

## Objective

Provide the operational surfaces required to run, inspect, package, and maintain `lss` as a local desktop service and toolset.

## Scope

- logs and tracing
- daemon stats and diagnostics
- packaging and install layout
- user service setup
- docs and sample config
- schema and index migrations

## Logging And Stats Checklist

- [ ] Add structured tracing across daemon, indexing, extraction, and query paths.
- [ ] Expose daemon stats through IPC and `lssctl`.
- [ ] Record extractor failures and indexing failures in inspectable form.
- [ ] Make performance counters visible enough to debug latency and throughput problems.

## Packaging Checklist

- [ ] Define install locations for binaries, config sample, and service files.
- [ ] Provide a `systemd --user` service for `lssd`.
- [ ] Document how indexes and model files are stored under XDG paths.
- [ ] Document upgrade and migration behavior.
- [ ] Decide how model artifacts are provisioned or downloaded locally.

## Maintenance Checklist

- [ ] Implement DB schema migration flow.
- [ ] Implement index format versioning or rebuild detection.
- [ ] Add `doctor` diagnostics for missing model files, broken config, bad socket state, and corrupted indexes.
- [ ] Add a user-facing troubleshooting guide.

## Documentation Checklist

- [ ] Root README with architecture overview and quickstart.
- [ ] Sample global config.
- [ ] CLI usage docs.
- [ ] Daemon service and launch docs.
- [ ] Performance tuning notes.
- [ ] Known limitations, especially around PDFs and soft filters.

## Acceptance Criteria

- [ ] A user can install, start, inspect, and troubleshoot the system without reading code.
- [ ] Upgrade behavior is documented and safe.
- [ ] Performance and failure signals are visible enough to diagnose real issues.

## Verification

- [ ] Install the binaries into a clean test environment and verify XDG paths.
- [ ] Start `lssd` as a user service and query it through `lssctl` and `lssi`.
- [ ] Exercise `doctor` and failure-reporting flows.
- [ ] Validate migration or rebuild behavior across version changes.

## Notes And Risks

- Operational visibility is part of product quality for a local daemon. If users cannot understand why indexing failed or why search is slow, the system is not finished.
