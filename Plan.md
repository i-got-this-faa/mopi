# lss Development Plan And Integrated Tracker

Last updated: 2026-06-01

## Goal

Improve `lss` in three ordered phases tailored for **Agents and LLM tooling**:

1. Expand local corpora and benchmark tooling with git repositories.
2. Implement Git-exclusive crawling and native `.gitignore` parsing.
3. Improve the `lssctl` CLI with structured JSON toolcalls and MCP interfaces for agents like OpenCode.

## Current Baseline

Workspace shape:

- `bin/lssd`: daemon, crawling/indexing orchestration, IPC search handling.
- `bin/lssctl`: CLI control/query surface.
- `crates/extract`: text and config extraction.
- `crates/index-meta`: SQLite metadata, file records, chunks, failures, ingest journal.
- `crates/index-lexical`: Tantivy lexical index.
- `crates/index-vector`: vector index.
- `crates/query`: query parsing.
- `crates/rank`: hybrid ranking.

Known gaps this plan addresses:

- No repeatable large-corpus benchmark harness for git repos.
- Crawl mechanism currently walks generic directories rather than native `.git` parsing.
- Missing standardized JSON-schema toolcall outputs for agent usage.

## Progress Tracker

### Phase 1: Local Corpora And Benchmarks (Git-focused)

- [x] Define deterministic corpus profiles: `small`, `medium`, `large`, `stress`.
- [x] Add corpus manifest format.
- [x] Add generated text/code/config corpus builder.
- [ ] Add symlink, duplicate alias, hidden-file, whitelist, and blacklist corpus cases.
- [x] Add benchmark report types.

### Phase 2: Git-Exclusive Crawling

- [ ] Modify `lssd` to require and recognize `.git` boundaries.
- [ ] Incorporate `ignore` or `git2` crates to naturally follow `.gitignore` and `.git/info/exclude`.
- [ ] Drop generic file-walker overhead in favor of querying git for tracked files.

### Phase 3: Agent Toolcall Interfaces

- [ ] Add `lssctl skill search` returning strict JSON arrays of snippets.
- [ ] Add semantic chunking based on AST or indentation instead of naive text chunks.
- [ ] Create an MCP (Model Context Protocol) server adapter.
