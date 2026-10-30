# Spec #10 final review fixes

Review baseline: `828e32a...2301b01`; fix branch started at `2301b01`. The standards review found no documented-standard violations and two small maintenance issues. The spec review found four P2 issues; all four are addressed below. This review does not close the real-browser acceptance gate.

## Correctness fixes

1. **Unfinished removal appeared completed in history.** Removal now persists a `PendingCleanup` event before retiring its record. `operation_history` reports completion only when the record, fixed installation directory and transaction directory are all demonstrably absent and the owned ancestor directories are valid. Permission/read errors, symlinks and replacement-path conflicts never count as absence. The final record deletion remains the commit point; no post-deletion audit write can lose the retry state. Reopen preserves the event and derives its current status. Existing `Succeeded` removal events also receive this absence check, so old partial cleanup is no longer displayed as completed.
2. **Removed extensions lost their names in history.** New events retain a serde-default optional installation-name snapshot. Removal fills missing snapshots on prior events while the installation identity still exists. The native history uses the snapshot after removal; legacy events for already-deleted installations retain their installation ID as a recognizable fallback instead of becoming generic source operations.

Two public Manager use-case regression tests first reproduced these failures, then passed with the fixes. They use real temporary files: failure deleting the last record, reopen/retry, old prematurely-successful event migration, replacement-path conflict, and legacy events without name snapshots. They join the five existing removal tests.

## Native presentation fixes

- Update, version selection, restore, loading and uninstall task dialogs retain the extension detail behind them. Close/Escape returns there; a no-update check also keeps the detail context.
- Detail rendering uses cached related-installation membership filtered against the current record snapshot. It does not acquire the Manager mutex while browser work may hold it, and does not reset related counts while waiting.
- The active dialog owns its notice. Background notices are suppressed; a persisted removal error is shown once in the dialog body rather than repeated in a generic footer.
- The redundant generic close footer is removed. Version changes and restore keep a clear, specifically labelled confirmation action and compact cancel/action row; the extra checkbox is removed. The core still receives the exact explicit update kind, and restoration is still a separate user action.
- The remaining local-folder character icon is replaced with the existing Folder SVG. The primary color constant is now named `ACCENT`.
- The #16 connection controls already reveal authorization help only when needed. The default connection action uses a light style. A cleanup retry remains available even if the browser Profile is no longer launchable, because browser removal was already confirmed.

The small shared `commit_confirmation` helper now owns record persistence and backup retirement for manual, legacy-ID and browser-evidence confirmation. Each use case retains its original, separate validation; no new confirmation framework or browser capability was added.

## Validation and limits

On macOS, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build -p cepm-app`, formatting and `git diff --check` pass. The existing upstream `block` crate future-incompatibility notice remains. Seven real-file removal tests pass, alongside the existing update, verification and source tests.

Prior native smoke evidence is in [spec10-native.md](spec10-native.md), including original issue screenshots. The integration owner rebuilt and rechecked final dialog layout, Escape/cancellation, one-error rendering and removed-extension history at 1120×780, then installed and opened the generated package; see the final section of spec10-native.md. Build/test success alone is not native visual acceptance.

The real Edge persistent OpenCLI unload/reinstall verification remains pending user/browser access; installed-package presentation has passed. Automatic write capability stays disabled by default; protocol tests and the earlier temporary CDP experiment do not satisfy the persistent-browser acceptance requirement. This fix pass performed no real-browser or daily-data operations.
