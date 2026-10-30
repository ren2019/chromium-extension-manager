# Spec #10 — real Edge acceptance

Date: 2026-10-30. macOS, Edge **156.0.4314.50**, native CEPM at 1120×780. The user explicitly approved normal daily-Edge restart, temporary debugging authorization, and removal/reinstallation of the named CEPM-managed OpenCLI installation. No Windows claim is made.

## Isolated production-path proof

A minimal Manifest V3 fixture, `CEPM Profile Isolation Probe` 1.0.0, was installed through the public Manager use case into two distinct fixed directories and browser roots A/B. Both were loaded using Edge's native “Load unpacked” file chooser. No `Extensions.loadUnpacked` installation or extension-registration patching was used. The fixture setup created empty `Preferences` markers in brand-new test directories to satisfy ProfileBook binding before Edge first initialized those roots; no existing preferences or extension settings were patched, and native loading confirmation was still required.

1. Root A restarted normally: the same fixture ID, version, enabled state and fixed directory remained.
2. The native CEPM uninstall dialog was cancelled once without mutation, then explicitly confirmed. The production adapter verified the exact identity, requested removal, read back absence and cleaned only A's owned installation.
3. Edge A restarted normally again; its extension manager search returned no fixture. A's record and installation directory were absent.
4. Root B restarted: its distinct ID, fixed directory, version 1.0.0 and enabled state remained. B's complete installation-file hashes and record hash were unchanged; the original source directory remained.
5. A real Edge launch created an additional ordinary `Profile 1` in root B (not a fabricated metadata fixture). The production uninstall runner refused with “此浏览器目录包含多个配置或目标不明确，请使用手动确认”; it did not remove B's files or record.
6. Both isolated browser processes were closed. These isolated roots are not the user's daily Profile.

Fresh Edge's sole Default root omitted `profile.last_used`, revealing a conservative false rejection before connection. The runtime fix accepts only that absent-field/sole-Default case; negative variants and native live-path proof remain tested and enforced.

## Daily OpenCLI cycle

Target: existing daily Edge **Default**, bound in CEPM as “用户配置 1”. Only its CEPM-managed unpacked OpenCLI was authorized. The separately installed store OpenCLI and the Chrome-managed installation were outside the target.

- Existing managed version: **1.0.26**. A native CEPM automatic identity check was first denied at Edge's authorization dialog: the app retained the installation and offered retry. On retry with authorization, the exact fixed-directory/ID/version check succeeded as **Automatic**.
- Native CEPM “连接 Edge 并卸载” performed the production removal and absence readback. The old owned directory and record disappeared; the old ID no longer opened an extension detail in Edge. The unrelated store OpenCLI remained visible.
- Native CEPM's two-step GitHub Release flow selected the **original source**, `jackwener/OpenCLI`, tag **v2.0.2**, asset **opencli-extension-v1.0.26.zip**, ZIP root. It did not switch to a newer release. Downloaded source artifact SHA-256: `2b2eff46fe4a21a0b02516e743005066e6317b48f15455c1c7b07f69ad7af211`.
- The reinstalled extension-file hashes matched the original official 1.0.26 file set. The loading reminder copied the new full fixed installation directory. Edge's native file chooser loaded that directory. Edge displayed OpenCLI **1.0.26**, enabled, unpacked, with the exact new path. CEPM automatically recognized the new ID and recorded **Automatic / 1.0.26**; no ID was typed.
- CEPM was stopped and daily Edge exited normally via its Quit menu. Edge restarted without a debugging launch flag or extension injection. The same new ID, fixed directory, version **1.0.26** and enabled state remained.
- The exact installed extension's own popup reported **Connected to daemon**, extension **1.0.26**, daemon **1.8.6**. A read-only health query for that popup's specific context returned `state: ready`, `extensionConnected: true`, `extensionVersion: 1.0.26`. This is a functional connection check, separate from CEPM's metadata confirmation.
- The temporary remote-debugging preference was switched off in Edge's native UI, with the checkbox visibly off; local port 9222 no longer accepted connections. Test processes were closed; daily Edge was left running normally.
- The Chrome installation's entire managed-file hash set and record hash matched the pre-test snapshot. No Chrome extension operation was performed.

Uninstall/reinstall creates a new fixed directory and extension ID; this is distinct from in-place update or repair, which retains the existing directory. Browser extension settings may be removed by uninstall, as disclosed in the authorized flow.

## Evidence and boundaries

Native screenshots and exact local snapshots were retained in the local acceptance artifact directory. Browser screenshots include the user's browser chrome and are not published with personal bookmarks/profile information. The repository's native UI screenshots contain the application evidence; the sequence above records the minimal browser facts needed for review.

Protocol tests additionally cover wrong identity/target/generation, malformed readback, denied write, cancellation and cleanup failure/retry. Native controlled cleanup retry was independently exercised in [the UI record](spec10-native.md). Those tests are not described as real-browser failure injection.

Automatic removal remains limited to the tested macOS Edge build and a single ordinary Profile per root. Shared-root multiple Profile routing, other Edge builds, Windows runtime and automatic persistent loading/reload are not validated. Unsupported conditions retain the manual path. The adapter's session ends after each task; the browser debug toggle is separate and does not automatically reset on restart.
