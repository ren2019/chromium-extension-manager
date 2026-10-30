# Embedded application artwork

- `chrome.png` / `edge.png`: browser `product_logo_32.png` resources used by the approved visual prototype at `959eefcc89fd5f72bead89138ab656a7eda8aaae`. Browser trademarks remain with Google and Microsoft; used only to identify targets.
- `logo.png` / `logo.icns`: CEPM blue package mark, matching the approved prototype's package motif. Used by the window, About page and macOS bundle. The PNG and ICNS have transparent outer corners.
- Operation icons come from the embedded `gpui-kit-assets` 0.8.1 Lucide catalog. No runtime CDN or network artwork fetch. Lucide license is included here.
- Extension icons are not bundled: bounded local manifest resources are decoded by the core, normalized to PNG, then cached by the GUI. Missing/invalid/escaping resources use the bundled Puzzle icon.
