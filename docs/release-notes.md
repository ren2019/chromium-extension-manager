## Downloads

- **macOS Apple Silicon:** extract the ZIP and move `CEPM.app` to Applications.
- **Windows x64:** extract the ZIP and run `CEPM/CEPM.exe`; see its included README.
- Each ZIP has a `.sha256` checksum file.

These packages are unsigned; macOS packages are not Developer ID signed or
notarized. Windows GUI/browser workflows have not yet been validated. Building
and packaging successfully does not establish full runtime compatibility.

GitHub sources require an installed, authenticated GitHub CLI (`gh`). Browser
extensions are first loaded persistently through the browser's native extension
manager using the path copied by CEPM. Automatic verification/removal is limited
to supported, precisely bound targets; other cases use the manual flow.

Keep CEPM's application data directory in place when upgrading the app: browser
extensions depend on their fixed installation directories there.
