# Daemon And IPC

Overall Status: IN PROGRESS  
Current Owner: OpenCode  
Blocked By: Workspace scaffold  
Last Updated: 2026-04-27

## Objective

Implement `mopid` as the single owner of indexes, model state, background jobs, and query execution, and expose it through a fast local IPC layer that `kiwi` and `mopictl` can share.

## Scope

- daemon lifecycle
- Unix socket server
- request and response protocol
- streaming search responses
- cancellation and backpressure
- graceful shutdown and restart behavior

## Protocol Decisions

- Transport: Unix domain socket only
- Preferred runtime location: `$XDG_RUNTIME_DIR/mopi/mopid.sock`
- Fallback runtime location when needed: XDG cache/runtime-managed private directory with strict permissions
- Framing: length-delimited binary frames
- Serialization: versioned binary format optimized for local same-machine use
- Server access: current user only by file permissions

## Core Responsibilities

- [x] Start and own SQLite, lexical index, vector index, model runtime, and worker pools.
- [x] Accept client connections from `mopictl` and `kiwi`.
- [x] Validate protocol version and message framing.
- [x] Handle query, admin, indexing, status, and diagnostics requests.
- [ ] Stream progressive query results where applicable.
- [ ] Support cancellation for stale interactive queries.
- [x] Shut down cleanly without corrupting index state.

## Suggested Request Surface

- [x] `Ping`
- [x] `GetStatus`
- [x] `GetStats`
- [x] `Search`
- [ ] `CancelSearch`
- [ ] `AddRoot`
- [ ] `RemoveRoot`
- [x] `ListRoots`
- [ ] `StartReindex`
- [x] `RefreshChanged`
- [ ] `IndexPath`
- [x] `ReloadConfig`
- [ ] `GetFailures`
- [x] `Doctor`

## Suggested Search Response Shape

- [ ] request id
- [ ] result item id
- [ ] canonical path
- [ ] display path or alias path
- [ ] filename
- [ ] matched snippet
- [ ] matched file type and mime
- [ ] reason flags such as `name`, `content`, `semantic`, `metadata`
- [ ] score
- [ ] optional debug explanation fields guarded behind a debug flag

## Detailed Checklist

- [x] Implement daemon startup with config load and path initialization.
- [x] Create the socket directory with correct permissions.
- [x] Refuse to start if another healthy daemon instance already owns the socket.
- [x] Detect and clean up stale socket files safely.
- [x] Implement a request dispatcher that isolates search from admin operations.
- [x] Add a bounded search request queue to avoid unbounded memory growth.
- [ ] Add query cancellation for launcher-style rapid keystroke updates.
- [x] Add worker pools for crawl, extract, embed, and commit stages.
- [x] Add a structured daemon status snapshot for CLI and GUI use.
- [x] Add graceful shutdown handling for SIGINT and SIGTERM.
- [ ] Persist enough job state to recover after interruption.
- [x] Ensure malformed client frames do not crash the daemon.

## Integration Contracts

- [x] `mopictl` can connect without embedding search logic.
- [ ] `kiwi` can request progressive results for each keystroke.
- [ ] Search requests can return partial lexical results before semantic reranking completes.
- [ ] Admin commands and status commands do not block on long-running search work.

## Acceptance Criteria

- [ ] A single daemon process can serve concurrent CLI and GUI clients.
- [ ] Interactive queries can be cancelled without leaking tasks.
- [ ] Indexing work continues independently of client process lifetime.
- [ ] Socket permissions prevent other users from connecting.
- [x] Protocol version mismatches fail cleanly with actionable errors.

## Verification

- [x] Start `mopid` and confirm the socket appears in the expected runtime directory.
- [x] Use `mopictl` to issue `Ping`, `GetStatus`, and `Search` requests.
- [x] Use `mopictl` to issue `RefreshChanged` requests.
- [x] Reload config in a running daemon and confirm watcher-backed indexing follows the new config.
- [ ] Open multiple concurrent clients and confirm results remain correct.
- [ ] Force-cancel repeated GUI-like search requests and confirm task cleanup.
- [ ] Kill and restart the daemon during indexing and confirm recovery behavior.

## Notes And Risks

- Do not let `kiwi` or `mopictl` bypass the daemon for direct index access.
- Progressive results are mandatory for the launcher UX. A one-shot only search response is not sufficient.
