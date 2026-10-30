# Manual browser removal and resumable cleanup (#15)

The native detail action offers uninstall for installed, pending, failed and recovery states, including installations without an extension ID. Its compact confirmation identifies the extension and browser/Profile, warns about browser-owned extension data, and opens that target's native management page. A selectable manual-management address fallback remains available when launch cannot reach the page.

Only the explicit “我已在此配置中移除这个扩展” acknowledgement permits cleanup. Closing the dialog, following the link or switching Profile does not submit it. The core entry point independently checks installation identity, target, deployment generation and explicit manual acknowledgement. The ordinary update/confirmation path rejects a removal in progress.

Cleanup persists browser confirmation before removing the installation directory, transaction/backups, and finally the installation record. It checkpoints each phase. Retrying a partial failure requires no new browser acknowledgement. Operation history survives record deletion and uses a persistent operation identifier to avoid duplicates if appending history succeeds before its record checkpoint fails. The history describes user-confirmed browser removal and file cleanup; it does not claim automatic browser uninstall.

## Automated verification

`cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` passed on macOS on 2026-10-30. The upstream `block` dependency continues to emit a future incompatibility notice.

Five tests in `crates/cepm-core/tests/removal.rs` exercise public Manager use cases with real temporary files:

- Explicit acknowledgement, target and generation checks; absent extension ID; untouched source and another Profile's installation; retained history after reopen.
- A replaced installations ancestor symlink is rejected before any external file deletion (the test first reproduced the unsafe behavior before the guard was added).
- Interruption at each persisted phase during an unconfirmed repeated-update transaction; reopen never restores removed files; retry clears the original and repair backup tree without another acknowledgement.
- Actual filesystem permission failure after installation files are removed; persisted error and phase; successful retry after permission repair.
- History append succeeds but the next record checkpoint fails; reopen and retry retain exactly one history event.

Core guards also reject non-directory/symlink installation and transaction roots and canonical ancestor relocation. No browser internal data or source directory is removed by this use case.

## Native and browser verification

The integration owner performs the 1120×780 native dialog, cancellation, cleanup retry and navigation/count checks after merging this ticket. This ticket's tests did not manipulate a user's real browser or claim a real browser uninstall. Automatic readback/removal evidence belongs to the dependent browser integration ticket; the internal removal seam is crate-private and cannot be selected through a public UI boolean.
