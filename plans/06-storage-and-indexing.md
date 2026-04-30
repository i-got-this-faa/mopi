# Storage And Indexing

Overall Status: IN PROGRESS  
Current Owner: OpenCode  
Blocked By: Workspace scaffold, daemon and IPC, config and policy, crawl and discovery, extraction and normalization  
Last Updated: 2026-04-27

## Objective

Implement the persistent storage and indexing layer that coordinates file metadata, crawl state, lexical search documents, chunk mappings, vector metadata, and crash-safe update recovery.

## Scope

- [x] SQLite schema and access layer
- [x] lexical index schema and update path
- [ ] vector metadata coordination
- [x] alias-path storage
- [ ] indexing journals and recovery
- [x] incremental updates and deletions

## Recommended Persistence Model

- [x] SQLite for authoritative metadata and job state
- [x] Tantivy for lexical search
- [ ] embedded ANN library for vector search
- [ ] a recovery journal so interrupted index updates can be resumed or rolled back cleanly

## Core Identity Model

- [ ] A stable file record keyed by canonical target identity.
- [ ] Alias path records keyed separately from the canonical file record.
- [ ] Chunk records linked to the canonical file record.
- [ ] Index state tracking for lexical and vector ingestion versioning.

## Suggested SQLite Tables

- [x] `roots`
- [x] `files`
- [x] `file_aliases`
- [x] `chunks`
- [ ] `jobs`
- [ ] `failures`
- [ ] `settings_snapshots`
- [ ] `ingest_journal`

## Suggested File Fields

- [x] internal file id
- [x] root id
- [x] canonical path
- [x] current filename
- [x] extension
- [x] mime
- [x] size
- [x] modified time
- [ ] content fingerprint when needed
- [x] extractor status
- [x] lexical index generation
- [ ] vector index generation
- [ ] last successful ingest time

## Suggested Alias Fields

- [x] alias path
- [ ] alias filename
- [x] canonical file id
- [ ] first seen time
- [ ] last seen time

## Tantivy Field Plan

- [ ] `file_id`
- [ ] `canonical_path`
- [ ] `alias_paths`
- [ ] `filename`
- [ ] `alias_filenames`
- [ ] `extension`
- [ ] `mime`
- [ ] `content`
- [ ] optional stored metadata fields for result display

## Detailed Checklist

- [x] Design and implement the SQLite schema.
- [x] Choose SQLite pragmas for embedded performance and durability.
- [x] Implement schema migration support.
- [x] Implement file upsert logic for new, changed, and deleted files.
- [x] Implement alias-path maintenance logic.
- [x] Implement chunk record storage and chunk deletion cascades.
- [x] Implement lexical document construction and update logic.
- [ ] Implement vector metadata persistence and ANN record mapping.
- [ ] Implement a journal or staged commit flow for cross-store consistency.
- [ ] Implement daemon startup recovery for interrupted indexing jobs.
- [x] Implement tombstone or delete handling for removed files and aliases.

## Incremental Update Requirements

- [x] Skip unchanged files without re-extracting or re-embedding them.
- [x] Re-index changed files atomically across metadata, lexical, and vector stores.
- [x] Remove deleted files and alias paths from all stores.
- [ ] Recover safely if the daemon crashes mid-update.

## Acceptance Criteria

- [ ] A file and its alias paths are represented consistently across storage and search.
- [ ] Interrupted indexing work can be resumed or safely cleaned up.
- [ ] Full reindex and changed-only refresh both converge to correct search results.
- [ ] SQLite, Tantivy, and vector metadata remain logically consistent after updates and deletes.

## Verification

- [ ] Run database migration tests.
- [ ] Run add, modify, delete, and alias-change integration tests.
- [ ] Simulate a daemon crash during indexing and confirm recovery behavior.
- [ ] Validate lexical index contents against known fixtures.

## Notes And Risks

- The hardest part here is cross-store consistency. Do not wave this away with best-effort writes.
- Alias paths are not cosmetic. They materially affect discoverability and must be stored deliberately.
