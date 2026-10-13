#!/usr/bin/env bash
set -euo pipefail

# Run on an ephemeral macOS GitHub-hosted runner after cargo bundle.
for name in MACOS_CERTIFICATE_BASE64 MACOS_CERTIFICATE_PASSWORD \
  MACOS_SIGNING_IDENTITY APPLE_ID APPLE_TEAM_ID APPLE_APP_PASSWORD \
  RUNNER_TEMP BUILD_TARGET RELEASE_ARCH; do
  if [[ -z "${!name:-}" ]]; then
    echo "Missing required secret or environment variable: $name" >&2
    exit 1
  fi
done

app="target/$BUILD_TARGET/release/bundle/osx/RepoStatus.app"
test -d "$app"
work_dir=$(mktemp -d "$RUNNER_TEMP/repostatus-signing.XXXXXX")
keychain="$work_dir/signing.keychain-db"
keychain_password=$(openssl rand -hex 32)

cleanup() {
  security delete-keychain "$keychain" >/dev/null 2>&1 || true
  rm -rf "$work_dir"
}
trap cleanup EXIT

printf '%s' "$MACOS_CERTIFICATE_BASE64" | base64 --decode > "$work_dir/certificate.p12"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$work_dir/certificate.p12" -P "$MACOS_CERTIFICATE_PASSWORD" \
  -A -t cert -f pkcs12 -k "$keychain"
security set-key-partition-list -S apple-tool:,apple: -k "$keychain_password" "$keychain"
security list-keychains -d user -s "$keychain" "$HOME/Library/Keychains/login.keychain-db"

# This app contains one Rust executable and no bundled helpers/frameworks.
codesign --force --options runtime --timestamp --keychain "$keychain" \
  --sign "$MACOS_SIGNING_IDENTITY" "$app"
codesign --verify --deep --strict --verbose=2 "$app"

xcrun notarytool store-credentials repostatus-notary --keychain "$keychain" \
  --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD"
ditto -c -k --keepParent "$app" "$work_dir/submission.zip"
submit_exit=0
xcrun notarytool submit "$work_dir/submission.zip" \
  --keychain-profile repostatus-notary --keychain "$keychain" \
  --wait --timeout 45m --output-format json > "$work_dir/result.json" || submit_exit=$?

if ! jq -e '.id and .status' "$work_dir/result.json" >/dev/null; then
  echo "Notarization did not return a submission result (exit $submit_exit)." >&2
  cat "$work_dir/result.json"
  exit 1
fi

status=$(jq -r '.status' "$work_dir/result.json")
submission_id=$(jq -r '.id' "$work_dir/result.json")
echo "Notarization submission $submission_id: $status"
if [[ "$status" != Accepted || "$submit_exit" != 0 ]]; then
  xcrun notarytool log "$submission_id" \
    --keychain-profile repostatus-notary --keychain "$keychain" || true
  exit 1
fi

xcrun stapler staple "$app"
xcrun stapler validate "$app"
spctl --assess --type execute --verbose=2 "$app"

mkdir -p dist
ditto -c -k --keepParent "$app" "dist/RepoStatus-macos-$RELEASE_ARCH.zip"
