# Edge task connection (#14)

Implementation: `crates/cepm-core/src/browser.rs`; desktop entry: `browser_connection.rs`.

Only one regular Profile in the selected user-data root is supported for automatic verification. Shared roots are rejected before connecting, irrespective of which Profile is selected or last active. The root `Extensions` domain has no Profile parameter and operates on the last-used Profile; attaching a page session does not change this. Browser/Profile lists and the manual workflow remain available for all targets.

Each task checks the selected canonical Profile, root `Local State` directory index and extra Profile directories; reads only that root's `DevToolsActivePort`; connects only to loopback; checks Edge product; creates its own `edge://version` tab and reads only its Profile Path; then matches a single extension by exact fixed directory, ID, specified deployment version and enabled state. It rechecks the binding after metadata read. Shared-root/new-Profile, stale deployment/target, cancelled and expired results cannot be saved. Observations are not public writable structs. Automatic confirmation is stored separately from manual confirmation, fills a previously missing ID and does not change deployment timestamps.

Transport uses one bounded 30-second task, cancellable nonblocking authorization handshake and 100ms cancellation polling during replies. The version page is allowed to finish loading; unexpected navigation is rejected. Success, error, cancel, target navigation and application close release the socket and request closure of only the task-created version tab. There is no background reconnect or saved active session. Closing the socket does not close Edge's debugging preference; the UI links to browser authorization and explains the distinction.

Read capability is established by an actual successful metadata request. Persistent load, reload and automatic uninstall are not enabled by this ticket. No temporary CDP load is used. #16 must separately validate and implement removal of UI-persisted unpacked extensions.

## Checks

Public use-case tests use real temporary CEPM/browser directories and a local fake WebSocket server at the external protocol boundary (not mocked internal helpers). Covered: persisted automatic identity recovery from manual no-ID confirmation, ambiguous Profile root, live Profile path mismatch, missing/disabled/wrong-version/wrong-path/invalid-ID/ambiguous extensions, selection and deployment changes, cancellation after a result and during an unresponsive request, topology changes, delayed authorization handshake and delayed native version-page readiness. Red/green recorded for initial gate, observation use case, post-read Profile creation rejection and delayed handshake. Cargo format/check/test run on the ticket branch; real Edge/native GUI acceptance is performed by the integration owner, not claimed by these tests.

One-shot production-path runner (close the CEPM app for file locks first):

```
cargo run -p cepm-core --example verify_edge -- <CEPM-data-root> <installation-id>
```

It resolves the existing ProfileBook target, requires actual browser authorization, runs the same adapter and saves Automatic only after validation. It never loads, reloads or removes an extension.

## Limits

CDP offers no atomic Profile-targeted uninstall. The single-Profile gate plus fresh checks is a constrained supported mode, not proof against arbitrary concurrent creation/activation of new browser Profiles. Shared roots stay disabled; this implementation does not claim general multi-Profile CDP isolation. Chromium's default-context semantics are source evidence and must still be checked in the supported Edge build. Windows runtime validation remains outside this change.

Sources: [Extensions protocol](https://chromedevtools.github.io/devtools-protocol/tot/Extensions/), [Chromium handler](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/devtools/protocol/extensions_handler.cc), [session wiring](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/devtools/chrome_devtools_session.cc), [default context](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/devtools/devtools_browser_context_manager.cc), [Profile manager](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/profiles/profile_manager.cc).

## Fresh Default runtime compatibility

The controlled Edge validation exposed a fresh root whose `info_cache` contains only `Default` but omits `profile.last_used`. Chromium registers this preference with an empty default ([primary source](https://chromium.googlesource.com/chromium/src/+/lkgr/chrome/browser/profiles/profiles_state.cc)); its presence on disk is not itself reliable proof of the running Profile. The adapter now permits an **omitted** value only when the selected and sole indexed Profile is `Default`, and still checks extra directories and the live native `edge://version` Profile Path before reading extension metadata. Explicit null, empty, mismatched values, other sole Profiles and shared roots remain rejected. This change does not enable automatic writes in default builds.

Regression evidence: the public production connection/confirmation test reproduced the rejection before the fix and then passed with omitted `last_used`; guard cases and a wrong native Profile Path remain refused. The test fixture changes no actual browser preferences.
