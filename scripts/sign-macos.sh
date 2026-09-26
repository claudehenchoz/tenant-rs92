#!/usr/bin/env bash
# Signs and notarizes the plugin bundles and standalone binary in a folder
# (default: target/bundled) with a Developer ID certificate.
# Requires APPLE_CERT_P12 (base64), APPLE_CERT_PASSWORD, APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD.
set -euo pipefail
DIR="${1:-target/bundled}"
KEYCHAIN=build.keychain
echo "$APPLE_CERT_P12" | base64 --decode > cert.p12
security create-keychain -p ci "$KEYCHAIN"
security default-keychain -s "$KEYCHAIN"
security unlock-keychain -p ci "$KEYCHAIN"
security import cert.p12 -k "$KEYCHAIN" -P "$APPLE_CERT_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k ci "$KEYCHAIN"
rm cert.p12
IDENTITY=$(security find-identity -v -p codesigning "$KEYCHAIN" | head -1 | awk '{print $2}')

notarize() {
  ditto -c -k --keepParent "$1" "$1.zip"
  xcrun notarytool submit "$1.zip" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
  rm "$1.zip"
}

cd "$DIR"
for b in *.vst3 *.clap *.app; do
  [ -e "$b" ] || continue
  codesign --force --deep --options runtime --timestamp --sign "$IDENTITY" "$b"
  notarize "$b"
  xcrun stapler staple "$b"
done
# A bare executable can be notarized but not stapled; Gatekeeper checks it online.
if [ -e tenant-rs92 ]; then
  codesign --force --options runtime --timestamp --sign "$IDENTITY" tenant-rs92
  notarize tenant-rs92
fi
