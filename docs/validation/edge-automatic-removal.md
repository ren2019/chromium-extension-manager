# Edge automatic removal (#16)

## Delivery state

The implementation is available for controlled acceptance; **real UI-persisted installation/removal and final native acceptance have not been completed by the implementing agent**. Normal/default builds keep automatic browser writes disabled. No existing personal browser extension was modified while implementing this ticket.

The small `edge-uninstall` Cargo feature is opt-in, is forwarded by `cepm-app`, and additionally permits only macOS Edge **156.0.4314.50**. This build number is a test candidate from earlier metadata/temporary-load evidence, not a claim that persistent removal has passed. Other versions or shared multi-Profile roots use the manual flow. Parent integration must record real evidence before enabling this feature by default or changing the verified version restriction.

## Production behavior

The uninstall dialog identifies the extension and exact browser/Profile, warns that browser extension data may be deleted, and offers a single “连接 Edge 并卸载” confirmation when the controlled capability is available and the extension already has an ID. It connects only for that task, verifies the root and Profile, and matches both the recorded ID and canonical fixed directory. Missing identity goes through identification first or the supported manual removal flow. Disabled or failed-version extensions may still be removed after exact identity checks; successful metadata confirmation is a separate concern.

`Manager::uninstall_edge` owns the session. It requires explicit confirmation, the current target/generation and a current managed record. The session performs a fresh complete extension-list read, checks unique ID/path, checks Profile binding again, requests removal, and reads back the complete same-target list. Request success, malformed metadata, read error, still-present ID/path, wrong/changed Profile, missing ID, cancellation or expired session cannot authorize cleanup. It never calls temporary loading or guesses a browser ID.

Only successful absence readback enters #15's persisted removal state with the **Automatic** method. Local cleanup failure retains a phase/error record; reopening and “重试清理” resumes without repeating browser removal. History distinguishes automatic removal/readback from user-confirmed manual removal. Source directories and other managed installations are outside the cleanup boundary.

The UI supports cancellation and target navigation; stale results cannot act on a new target. The one-shot session closes on every result; it does not silently toggle browser debugging, restart Edge or reconnect in the background. If a user cancels after the browser has accepted removal but before absence is verified, files/record remain and the user can inspect the browser then use manual confirmation. This is an unknown result, not a reported successful uninstall.

## Automated verification

Tests exercise the public Manager use case with real temporary managed directories and a local WebSocket server at the external protocol boundary:

- selected installation removed, source and other Profile installation retained, automatic history preserved;
- default build refuses the write even with a successful metadata connection;
- unsupported Edge version, denied write, failed or malformed readback and still-present extension retain files/record;
- missing ID, wrong target, stale generation, cancelled task, no confirmation, newly shared root and wrong browser ID never write;
- cancellation after the request does not claim verified absence or clean files;
- browser removal followed by local filesystem cleanup failure persists the phase, survives reopen and retries without a browser.

The malformed-readback test first failed because an incomplete entry could look absent; the implementation now requires complete metadata before cleanup authorization. These are transport/core checks, not substitutes for real-browser persistence or native screenshots.

## Controlled acceptance commands

Build the native test application explicitly:

```
cargo build -p cepm-app --features edge-uninstall
```

The existing packaging script rebuilds without feature arguments; **do not assume it preserves an earlier opt-in binary**. Use the directly built native executable during controlled validation. After recorded runtime success, an explicit final change can enable the feature in the application's default features and use normal packaging, retaining the exact tested-build restriction.

A one-shot runner uses the identical production use case (close the CEPM application first for file locks):

```
cargo run -p cepm-core --features edge-uninstall --example uninstall_edge -- <CEPM-data-root> <installation-id> --confirm-removal
```

This is destructive to the specified extension and its owned files; use only the explicitly authorized disposable fixture or user-authorized target. Browser approval and precise binding still apply. Read-only identity preparation remains `--example verify_edge`.

## Remaining acceptance / activation gate

Integration owner must perform and record: real Edge UI load → restart persistence → production automatic removal → absence and restart absence; exact target/other-root isolation; two regular Profiles in one root rejected without writing; native automatic/manual flows and cleanup retry; then the user's expressly requested OpenCLI uninstall/reinstall and persistence/functional check. The new final user request authorizes that specific OpenCLI cycle, but the app still needs browser authorization and any separately required restart permission.

Shared-root arbitrary concurrent Profile targeting remains unsupported because root CDP Extensions operations have no Profile selector. The narrow single-Profile gate and fresh checks are not an atomic lease against concurrent Profile creation. Do not mark general multi-Profile automatic writes as passed. Existing Windows runtime, other Edge versions and automatic persistent loading/reload remain unverified.
