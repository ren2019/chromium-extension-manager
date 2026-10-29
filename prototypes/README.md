# Throwaway feasibility probes

These are experiments, not production CEPM. Keep them on `prototype/gpui-browser-feasibility`.

## GPUI

From the repository root:

```sh
cargo run --manifest-path prototypes/gpui-smoke/Cargo.toml --locked
```

The probe uses published upstream gpui-kit 0.8.1 without OpenLogi patches. It displays an input/echo, native folder picker, 100 fixture rows and simulated 0–100% progress. It does not install or update extensions. Progress is deliberately a button-driven simulation, not a download benchmark.

Verify real Chinese IME composition separately from pasting Chinese text. Check native folder selection with a Unicode path, list scrolling, responsiveness, resizing and closing. A successful `--version` invocation in CI proves only executable startup, not a rendered window.

CI builds on Windows and macOS and produces an unsigned app ZIP / portable EXE ZIP. These are feasibility artifacts, not signed installers or distribution acceptance.

## Browser persistence

```sh
python3 prototypes/browser-persistence/prepare.py 1.0.0
```

Use a **new disposable browser profile**, load `.scratch/browser-persistence/installed-extension` via the browser's normal Load unpacked UI, then open the fixture's Options page. Record its ID/version and click Save persistence marker. Quit only the disposable browser instance and start the same profile again. Confirm ID, version and marker in the extension's own page.

Then run:

```sh
python3 prototypes/browser-persistence/prepare.py 1.1.0
```

Before Reload, distinguish the old loaded version from new disk files. Reload through the extension management UI and reopen Options. Confirm version 1.1.0, unchanged ID and saved marker, then repeat a full browser restart. Finally prepare 1.0.0 and Reload to exercise an explicit downgrade.

The fixture requests only its own local storage and has no host permissions or network behavior. It is not the OpenCLI end-to-end acceptance case. No Preferences or Secure Preferences files are edited, and no `--load-extension` or CDP load command is used for the persistent-install test.

Report Windows/macOS and Chrome/Edge results independently. Unexecuted cells remain unverified.
