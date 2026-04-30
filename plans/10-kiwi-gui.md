# Kiwi GUI

Overall Status: IN PROGRESS
Current Owner: OpenCode  
Blocked By: Daemon and IPC, query and ranking  
Last Updated: 2026-04-27

## Objective

Build `kiwi` as a Wayland-friendly launcher-style GUI that streams results from `mopid`, emphasizes content relevance, stays keyboard-first, and feels instant during interactive typing.

## Scope

- launcher window and search box
- streaming search requests
- result list and snippet rendering
- keyboard navigation and actions
- progressive update UX

## UX Priorities

- [x] Fast startup.
- [x] Fast first-result display.
- [x] Content-rich result presentation.
- [x] Keyboard-first interaction.
- [x] Minimal friction between typing and opening a file.

## Result Presentation Requirements

- [x] Prominent filename.
- [x] Visible path context.
- [x] Snippet preview with the strongest matching text.
- [ ] Clear filetype cue.
- [x] Optional subtle reason tags such as `name`, `content`, or `semantic`.

## Interaction Checklist

- [x] Debounce or cancel stale searches aggressively.
- [x] Request progressive results from the daemon.
- [ ] Display lexical candidates immediately when available.
- [ ] Merge or refresh results when semantic reranking completes.
- [x] Support keyboard navigation.
- [x] Support open file action.
- [ ] Support reveal-in-folder action.
- [ ] Support copy-path action.

## Engineering Checklist

- [x] Choose the GUI toolkit and document why it fits the Wayland launcher use case.
- [x] Implement daemon connection handling and reconnect behavior.
- [x] Implement a responsive search input loop.
- [x] Implement result virtualization or efficient list rendering if needed.
- [ ] Handle empty state, no-result state, and daemon-unavailable state.
- [ ] Preserve result ordering stability enough to avoid jarring UI jumps.

## Performance Targets

- [ ] Window becomes interactive quickly on startup.
- [ ] First results appear within the launcher-like latency target on warm indexes.
- [ ] Rapid typing does not leave stale results visible for long.

## Acceptance Criteria

- [ ] `kiwi` is usable as a daily keyboard-first search launcher.
- [ ] Content relevance is obvious from the displayed snippets.
- [ ] The UI remains responsive during rapid interactive searching.
- [ ] Result actions work reliably on local files.

## Verification

- [ ] Test rapid typing and cancellation behavior against a live daemon.
- [ ] Test keyboard-only workflows end to end.
- [ ] Test daemon-down and reconnect states.
- [ ] Test long snippets, narrow windows, and large result sets.

## Notes And Risks

- Do not let the GUI own search logic. It should remain a thin, responsive client.
- Do not overbuild window chrome or application-launcher extras before core search UX is excellent.
