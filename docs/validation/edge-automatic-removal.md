# Edge automatic removal (#16)

## Delivery state

The integration owner completed the isolated real-browser acceptance on macOS Edge **156.0.4314.50**. The application now enables `edge-uninstall` by default, so normal packaging includes the validated adapter. `cepm-core` keeps the feature off by default; the runtime still refuses automatic removal outside that exact macOS Edge build or in shared multi-Profile roots. Other cases retain the manual flow.

**The user's daily OpenCLI uninstall/reinstall and persistence/functional validation is still pending.** This activation change is prepared for integration after that acceptance; it does not claim the daily cycle has passed.

## Isolated real-browser evidence

The integration owner reported these completed production-path checks:

- Native Edge UI loaded the fixture from its fixed directory; it remained installed after Edge restarted.
- Native CEPM performed automatic removal with the opt-in build, verified absence, and cleaned its owned installation. Edge restart still showed the fixture absent.
- A separate browser root B retained its exact extension ID/path, version `1.0.0` and enabled state. Its CEPM record and file hashes remained unchanged.
- A real launch created `Profile 1` alongside the existing Profile in root B. The production removal path rejected the shared root before writing.

These results establish the narrow tested-build capability. They do not establish arbitrary shared-Profile routing, other Edge releases or Windows behavior. Earlier automated checks remain separate from this real-browser evidence.

## Production behavior

The uninstall dialog identifies the extension and exact browser/Profile, warns that browser extension data may be deleted, and offers a single “连接 Edge 并卸载” confirmation when the controlled capability is available and the extension already has an ID. It connects only for that task, verifies the root and Profile, and matches both the recorded ID and canonical fixed directory. Missing identity goes through identification first or the supported manual removal flow. Disabled or failed-version extensions may still be removed after exact identity checks; successful metadata confirmation is a separate concern.

`Manager::uninstall_edge` owns the session. It requires explicit confirmation, the current target/generation and a current managed record. The session performs a fresh complete extension-list read, checks unique ID/path, checks Profile binding again, requests removal, and reads back the complete same-target list. Request success, malformed metadata, read error, still-present ID/path, wrong/changed Profile, missing ID, cancellation or expired session cannot authorize cleanup. It never calls temporary loading or guesses a browser ID.

Only successful absence readback enters #15's persisted removal state with the **Automatic** method. Local cleanup failure retains a phase/error record; reopening and “重试清理” resumes without repeating browser removal. History distinguishes automatic removal/readback from user-confirmed manual removal. Source directories and other managed installations are outside the cleanup boundary.

The UI supports cancellation and target navigation; stale results cannot act on a new target. The one-shot session closes on every result; it does not silently toggle browser debugging, restart Edge or reconnect in the background. If a user cancels after the browser has accepted removal but before absence is verified, files/record remain and the user can inspect the browser then use manual confirmation. This is an unknown result, not a reported successful uninstall.

## Automated verification

Tests exercise the public Manager use case with real temporary managed directories and a local WebSocket server at the external protocol boundary:

- selected installation removed, source and other Profile installation retained, automatic history preserved;
- standalone core builds without the feature refuse the write even with a successful metadata connection;
- unsupported Edge version, denied write, failed or malformed readback and still-present extension retain files/record;
- missing ID, wrong target, stale generation, cancelled task, no confirmation, newly shared root and wrong browser ID never write;
- cancellation after the request does not claim verified absence or clean files;
- browser removal followed by local filesystem cleanup failure persists the phase, survives reopen and retries without a browser.

The malformed-readback test first failed because an incomplete entry could look absent; the implementation now requires complete metadata before cleanup authorization. These are transport/core checks, not substitutes for real-browser persistence or native screenshots.

## Build and diagnostic commands

Normal application builds and packaging include the tested adapter:

```
cargo build -p cepm-app
```

For a manual-only application build, use `cargo build -p cepm-app --no-default-features`. The standalone core diagnostic runner still requires explicit feature activation; the exact tested-build restriction applies in either case.

A one-shot runner uses the identical production use case (close the CEPM application first for file locks):

```
cargo run -p cepm-core --features edge-uninstall --example uninstall_edge -- <CEPM-data-root> <installation-id> --confirm-removal
```

This is destructive to the specified extension and its owned files; use only the explicitly authorized disposable fixture or user-authorized target. Browser approval and precise binding still apply. Read-only identity preparation remains `--example verify_edge`.

## Remaining daily acceptance

The integration owner must complete and record the user's expressly requested daily OpenCLI uninstall/reinstall, restart persistence and functional check before merging this activation. The request authorizes that specific cycle; browser authorization and separately required restart permission still apply. Isolated native acceptance above does not substitute for that daily result.

Shared-root arbitrary concurrent Profile targeting remains unsupported because root CDP Extensions operations have no Profile selector. The narrow single-Profile gate and fresh checks are not an atomic lease against concurrent Profile creation. Do not mark general multi-Profile automatic writes as passed. Existing Windows runtime, other Edge versions and automatic persistent loading/reload remain unverified.
