#!/bin/bash
# Produce a native bundle for this Mac; no Developer ID signing or notarization.
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
  echo 'macOS packaging must run on macOS.' >&2
  exit 1
fi

profile="${1:-debug}"
case "$profile" in
  debug|release) ;;
  *) echo 'Usage: scripts/package-macos.sh [debug|release]' >&2; exit 1 ;;
esac
if [[ $# -gt 1 || -n "${CARGO_BUILD_TARGET:-}" ]]; then
  echo 'Package a native host build; unset CARGO_BUILD_TARGET and supply at most one profile.' >&2
  exit 1
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"
if [[ "$profile" == release ]]; then
  cargo build -p cepm-app --locked --release
else
  cargo build -p cepm-app --locked
fi
target_dir="$(cargo metadata --no-deps --format-version 1 | /usr/bin/plutil -extract target_directory raw -o - -)"
binary="$target_dir/$profile/cepm-app"
version="$(sed -n 's/^version = "\([^"]*\)"$/\1/p' crates/cepm-app/Cargo.toml)"
arch="$(/usr/bin/lipo -archs "$binary" | tr ' ' '-')"
output_dir="$target_dir/bundle/$profile"
mkdir -p "$output_dir"
staging="$(mktemp -d "$output_dir/.cepm.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
app="$staging/CEPM.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp crates/cepm-app/assets/logo.icns "$app/Contents/Resources/CEPM.icns"
cp "$binary" "$app/Contents/MacOS/cepm-app"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Chromium Extension Package Manager</string>
  <key>CFBundleDisplayName</key><string>Chromium Extension Package Manager</string>
  <key>CFBundleIconFile</key><string>CEPM.icns</string>
  <key>CFBundleIdentifier</key><string>dev.cepm.app</string>
  <key>CFBundleExecutable</key><string>cepm-app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
/usr/bin/plutil -lint "$app/Contents/Info.plist"
archive="$output_dir/CEPM-macos-$arch-$profile.zip"
if [[ "$profile" == release ]]; then
  archive="$output_dir/CEPM-$version-macos-$arch.zip"
fi
/usr/bin/ditto -c -k --keepParent "$app" "$staging/CEPM.zip"
mv -f "$staging/CEPM.zip" "$archive"
# Only replace this script's generated bundle, never application data.
rm -rf "$output_dir/CEPM.app"
mv "$app" "$output_dir/CEPM.app"
(cd "$output_dir" && /usr/bin/shasum -a 256 "$(basename "$archive")" > "$(basename "$archive").sha256")
printf 'App: %s\nZIP: %s\nNo Developer ID signature or notarization; architecture: %s\n' "$output_dir/CEPM.app" "$archive" "$arch"
