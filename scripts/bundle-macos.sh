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
cp -R licenses "$bundle/Contents/Resources/"
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
# Artifact services do not preserve executable modes when uploading a directory.
# Upload an already verified archive, including the application directory itself.
archive=target/release/bundle/Peer-native-macos.zip
rm -f "$archive"
ditto -c -k --sequesterRsrc --keepParent "$bundle" "$archive"
python3 scripts/verify-macos-archive.py "$archive"
verify_dir=$(mktemp -d "${TMPDIR:-/tmp}/peer-bundle-verify.XXXXXX")
trap 'rm -rf "$verify_dir"' EXIT
ditto -x -k "$archive" "$verify_dir"
test -x "$verify_dir/Peer.app/Contents/MacOS/peer"
cmp "$bundle/Contents/MacOS/peer" "$verify_dir/Peer.app/Contents/MacOS/peer"
codesign --verify --deep --strict "$verify_dir/Peer.app"
echo "Verified extracted Peer.app executable, identical bytes and strict signature"
shasum -a 256 "$archive" "$verify_dir/Peer.app/Contents/MacOS/peer"
echo "$archive"
