#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $(uname -s) != Darwin ]]; then echo 'macOS is required to build and verify this app bundle' >&2; exit 1; fi
export MACOSX_DEPLOYMENT_TARGET=15.0
cargo build --locked --release -p peer
bundle=target/release/bundle/Peer.app
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp target/release/peer "$bundle/Contents/MacOS/peer"
cp assets/icons/icon.icns "$bundle/Contents/Resources/icon.icns"
cp -R licenses "$bundle/Contents/Resources/licenses"
cp THIRD_PARTY_NOTICES.md "$bundle/Contents/Resources/"
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.repoinspector.app</string>
<key>CFBundleName</key><string>Peer</string>
<key>CFBundleExecutable</key><string>peer</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>CFBundleIconFile</key><string>icon.icns</string>
<key>LSMinimumSystemVersion</key><string>15.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
echo "$bundle"
