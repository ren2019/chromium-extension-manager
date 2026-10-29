"""Throwaway fixture. Only writes inside the project's disposable .scratch tree."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("version", choices=["1.0.0", "1.1.0"])
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
extension = root / ".scratch" / "browser-persistence" / "installed-extension"
extension.mkdir(parents=True, exist_ok=True)
manifest = {
    "manifest_version": 3,
    "name": "CEPM Persistence Prototype",
    "version": args.version,
    "description": "Disposable local fixture: version, extension identity, and storage survival. No host access.",
    "permissions": ["storage"],
    "background": {"service_worker": "worker.js"},
    "options_ui": {"page": "status.html", "open_in_tab": True},
    "action": {"default_title": "CEPM persistence fixture", "default_popup": "status.html"},
}
(extension / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
(extension / "worker.js").write_text("""chrome.runtime.onInstalled.addListener(async () => {
  const state = await chrome.storage.local.get('marker');
  if (state.marker === undefined) await chrome.storage.local.set({marker: 'initial'});
});
""")
(extension / "status.html").write_text("""<!doctype html><meta charset="utf-8">
<title>CEPM Persistence Prototype</title>
<style>body{font:20px system-ui;padding:32px;min-width:450px}button{font:inherit;padding:10px}pre{white-space:pre-wrap}</style>
<h1>CEPM Persistence Prototype</h1>
<p>Disposable fixture. Only extension-local storage; no website access.</p>
<pre id="state">Loading…</pre>
<button id="save">Save persistence marker</button>
<script src="status.js"></script>
""")
(extension / "status.js").write_text("""async function show() {
  const stored = await chrome.storage.local.get('marker');
  document.querySelector('#state').textContent = JSON.stringify({
    id: chrome.runtime.id,
    loadedVersion: chrome.runtime.getManifest().version,
    marker: stored.marker ?? '(unset)',
    pageBuild: 'BUILD_VERSION'
  }, null, 2);
}
document.querySelector('#save').addEventListener('click', async () => {
  await chrome.storage.local.set({marker: 'cepm-persistence-survived'});
  await show();
});
show();
""".replace("BUILD_VERSION", args.version))
print(extension)
