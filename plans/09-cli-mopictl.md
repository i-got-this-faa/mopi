# CLI: mopictl

Overall Status: NOT STARTED  
Current Owner: UNASSIGNED  
Blocked By: Daemon and IPC, config and policy, storage and indexing, query and ranking  
Last Updated: 2026-04-27

## Objective

Provide a fast administrative and query CLI that talks to `mopid`, exposes core indexing workflows, surfaces diagnostics, and supports machine-readable output for scripting.

## Scope

- query command
- root and config management
- reindex and refresh commands
- daemon status and diagnostics
- JSON output mode

## Minimum Command Surface

- [ ] `mopictl ping`
- [ ] `mopictl status`
- [ ] `mopictl stats`
- [ ] `mopictl query <terms>`
- [ ] `mopictl roots list`
- [ ] `mopictl roots add <path>`
- [ ] `mopictl roots remove <path>`
- [ ] `mopictl reindex`
- [ ] `mopictl refresh`
- [ ] `mopictl index-path <path>`
- [ ] `mopictl config show`
- [ ] `mopictl config validate`
- [ ] `mopictl failures`
- [ ] `mopictl doctor`

## Output Requirements

- [ ] Human-readable default output.
- [ ] `--json` for machine-readable output where appropriate.
- [ ] Stable field names for scripting.
- [ ] Search output includes path, filename, snippet, score, and reason tags.

## Detailed Checklist

- [ ] Choose a CLI framework and scaffold the command tree.
- [ ] Implement daemon connection bootstrap and error reporting.
- [ ] Implement query command with configurable result count.
- [ ] Implement config inspection and validation commands.
- [ ] Implement root management commands.
- [ ] Implement full reindex and changed-only refresh commands.
- [ ] Implement diagnostics and failure-inspection commands.
- [ ] Implement JSON output mode with stable shapes.
- [ ] Add shell-completion generation if the CLI framework supports it cleanly.

## Acceptance Criteria

- [ ] A user can configure roots, trigger indexing, run queries, and inspect failures entirely from the CLI.
- [ ] CLI errors are actionable and do not leak internal-only details.
- [ ] JSON output is stable enough for scripted use.

## Verification

- [ ] Exercise every command against a live daemon.
- [ ] Exercise `--json` output and validate it against sample scripts or fixtures.
- [ ] Confirm query results match daemon expectations.

## Notes And Risks

- `mopictl` is not a second search engine. Keep it as a client of the daemon, not a duplicate runtime.
