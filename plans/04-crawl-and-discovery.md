# Crawl And Discovery

Overall Status: IN PROGRESS  
Current Owner: OpenCode  
Blocked By: Workspace scaffold, config and policy  
Last Updated: 2026-04-27

## Objective

Implement the filesystem traversal engine that discovers candidate files for indexing under configured roots while respecting hidden-file defaults, whitelist and blacklist rules, symlink traversal, file identity deduplication, and incremental refresh behavior.

## Scope

- recursive root walking
- hidden path filtering
- whitelist and blacklist pattern application
- symlink following
- cycle detection
- duplicate-target suppression
- change detection and refresh planning

## File Identity Requirements

These rules are critical because symlinks must be followed.

- [x] Distinguish observed path from canonical target path.
- [x] Detect cycles when symlink chains point back into already-visited locations.
- [x] Detect duplicate canonical targets reached through multiple symlinked paths.
- [ ] Preserve useful alias paths for search relevance and result display.
- [x] Ensure a canonical target is not fully re-indexed for every alias path.

## Detailed Checklist

- [x] Choose and benchmark the directory walking strategy for speed and correctness.
- [x] Implement root registration and traversal entrypoints.
- [x] Implement hidden path detection that works on every path segment.
- [x] Apply whitelist and blacklist filters early enough to avoid wasted work.
- [x] Implement symlink resolution and loop detection.
- [x] Track canonical targets and alias paths separately.
- [x] Record file metadata needed for incremental refresh: size, timestamps, file identity, hash when needed.
- [ ] Skip clearly unsupported or binary file candidates before expensive extraction when possible.
- [x] Emit crawl events suitable for downstream indexing queues.
- [x] Support full scan and changed-only refresh.
- [x] Plan for file watcher integration, even if watcher support lands after the initial scan pipeline.
- [x] Implement real-time OS-level file watching using the `notify` and `notify-debouncer-full` crates.
- [x] Treat OS watcher events as refresh triggers while daemon-owned snapshots remain the source of truth.

## Candidate Metadata To Emit

- [x] observed path
- [x] canonical path
- [x] filename
- [x] extension hint
- [x] file size
- [x] modified time
- [x] hidden-path flag
- [x] root id
- [x] alias-path indicator

## Acceptance Criteria

- [x] Hidden files and directories are ignored by default.
- [x] Whitelist mode only emits explicitly allowed files.
- [x] Blacklist mode excludes forbidden files while preserving broad indexing coverage.
- [x] Symlink loops do not create infinite traversal.
- [x] Duplicate canonical targets do not create duplicate content indexing work.
- [ ] Alias paths remain available for name and path matching.
- [x] Changed-only refresh reliably skips unchanged content.

## Verification

- [x] Test against a corpus containing nested hidden directories.
- [x] Test against a corpus containing symlink loops.
- [x] Test against multiple alias paths to the same canonical file.
- [x] Test whitelist and blacklist behavior with mixed file types.
- [x] Test incremental refresh with unchanged, modified, added, and deleted files.
- [x] Smoke-test daemon-owned changed-only refresh through `mopictl refresh`.
- [x] Smoke-test config reload rebuilding watcher state and indexing the new roots.

## Notes And Risks

- Alias-path preservation is easy to lose if file identity is treated too aggressively. Make sure path-based queries can still find symlinked names that users actually type.
- Do not hash every file during every scan. Use a staged change-detection strategy to protect indexing throughput.
