# Config And Policy

Overall Status: IN PROGRESS  
Current Owner: OpenCode  
Blocked By: Workspace scaffold  
Last Updated: 2026-04-27

## Objective

Define and implement the single global XDG configuration model for `lss`, including root selection, whitelist and blacklist policies, hidden-file handling, extractor limits, daemon settings, and ranking knobs.

## Scope

- global config path resolution
- config schema
- defaults and validation
- whitelist and blacklist modes
- hidden-file policy
- symlink policy
- concurrency and limit settings

## Required Paths

- Config: `XDG_CONFIG_HOME/lss/config.toml`
- Data: `XDG_DATA_HOME/lss/`
- Cache: `XDG_CACHE_HOME/lss/`
- Runtime: `XDG_RUNTIME_DIR/lss/`

## Minimum Config Domains

- [x] indexed roots
- [x] include and exclude patterns
- [x] policy mode: whitelist or blacklist
- [x] hidden file and hidden directory behavior
- [x] symlink following behavior
- [x] file size and extraction limits
- [x] extractor enablement flags
- [x] embedding model location and runtime settings
- [x] lexical and semantic ranking weights
- [x] daemon socket location override if needed
- [x] concurrency and queue sizing
- [x] logging verbosity

## Suggested Config Shape

- [x] `[roots]` or repeated `[[roots]]` entries with path-local include and exclude policies
- [x] `[indexing]` for concurrency, fingerprinting, and refresh rules
- [x] `[extraction]` for bytes, pages, and timeout limits
- [x] `[embedding]` for model path, runtime backend, and batching controls
- [x] `[ranking]` for content, filename, path, and metadata boost weights
- [x] `[daemon]` for socket path and runtime behavior
- [x] `[logging]` for log level and persistence

## Policy Semantics

- [x] Hidden files and directories are ignored by default.
- [x] Whitelist mode indexes only paths and types that explicitly match include rules.
- [x] Blacklist mode indexes broadly but drops paths and types that match exclude rules.
- [x] Symlinks are followed, but cycle and duplication protection remains mandatory.
- [ ] Content type checks should not rely on filename extension alone.

## Validation Requirements

- [x] Reject config entries pointing to missing or unreadable roots with actionable errors.
- [x] Reject contradictory include and exclude configuration that cannot be resolved deterministically.
- [x] Validate numeric limits for sane ranges.
- [x] Validate ranking weights and fallback behavior.
- [x] Validate model path existence when strict embedding startup is requested.

## Detailed Checklist

- [x] Implement XDG path resolution with safe defaults.
- [x] Define Rust config structs with serde support.
- [x] Implement default config generation.
- [x] Implement config load, merge, and validation pipeline.
- [x] Implement path-pattern matching strategy, likely via glob sets.
- [x] Implement hidden path detection helper shared with the crawler.
- [x] Implement config reload behavior for the daemon.
- [x] Document the schema with a commented sample config.
- [x] Expose config validation through `lssctl`.

## Acceptance Criteria

- [x] A first-time user can generate a working config file.
- [x] Invalid config fails fast with precise messages.
- [x] Config reload updates daemon behavior safely.
- [x] Both whitelist and blacklist modes can be demonstrated with tests.

## Verification

- [x] Validate a clean default config.
- [x] Validate malformed config examples and confirm helpful error messages.
- [x] Validate whitelist and blacklist examples against a test corpus.
- [x] Reload config in a running daemon and confirm changed settings take effect.

## Notes And Risks

- Keep the schema global and coherent. Do not introduce overlapping config sources in v1.
- Ranking weights belong in config, but the default values should be conservative and content-first.
