#!/bin/bash
# release 빌드 후 build/Workboard.app 번들을 만든다.
# NOTARIZE=1 이면 Apple 공증까지 하고 배포용 build/Workboard-<버전>-arm64.zip 을 만든다.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

# 버전은 Cargo.toml 한 곳에만 둔다. 빌드 번호는 커밋 수(늘기만 한다).
VERSION="$(sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)".*/\1/p' Cargo.toml | head -1)"
if [ -z "$VERSION" ]; then echo "Cargo.toml 에서 버전을 읽지 못했다" >&2; exit 1; fi
BUILD="$(git rev-list --count HEAD 2>/dev/null || echo 1)"

cargo build --release

APP=build/Workboard.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/workboard "$APP/Contents/MacOS/Workboard"
if [ -f Resources/AppIcon.icns ]; then cp Resources/AppIcon.icns "$APP/Contents/Resources/"; fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Workboard</string>
  <key>CFBundleDisplayName</key><string>Workboard</string>
  <key>CFBundleIdentifier</key><string>local.workboard.app</string>
  <key>CFBundleExecutable</key><string>Workboard</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$BUILD</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <key>NSMicrophoneUsageDescription</key><string>Workboard uses the microphone for huddles and Talk to Text.</string>
  <key>NSCameraUsageDescription</key><string>Workboard uses the camera for huddles and clips.</string>
</dict>
</plist>
PLIST

# 서명: Developer ID 인증서가 키체인에 있으면 그걸로, 없으면 임시(ad-hoc) 서명.
ID="${SIGN_IDENTITY:-Developer ID Application: jikwon seo (K6599UJ4KM)}"
if security find-identity -v -p codesigning | grep -qF "$ID"; then
  # 마이크·카메라 권한(entitlements)을 붙여 서명한다
  codesign --force --options runtime --timestamp --entitlements Resources/Workboard.entitlements --sign "$ID" "$APP"
  codesign --verify --strict --deep "$APP"
  SIGNED_WITH_ID=1
else
  codesign --force --sign - "$APP" >/dev/null 2>&1 || true
  SIGNED_WITH_ID=0
fi
echo "built $APP ($VERSION, build $BUILD)"

# 공증: NOTARIZE=1 일 때만 (몇 분 걸림). 다른 맥에서 경고 없이 열리게 하려면 필요하다.
# 계정 정보는 키체인 프로필로 저장해 둔 것을 쓴다(`xcrun notarytool store-credentials`).
if [ "${NOTARIZE:-0}" = 1 ]; then
  if [ "$SIGNED_WITH_ID" != 1 ]; then echo "Developer ID 서명이 없어 공증할 수 없다" >&2; exit 1; fi
  ZIP="build/Workboard-$VERSION-$(uname -m).zip"
  rm -f "$ZIP"
  ditto -c -k --keepParent "$APP" "$ZIP"
  xcrun notarytool submit "$ZIP" --keychain-profile "${NOTARY_PROFILE:-symboard-notary}" --wait
  xcrun stapler staple "$APP"
  # 배포용 zip 은 공증 결과를 붙인 뒤 다시 만든다.
  rm -f "$ZIP"
  ditto -c -k --keepParent "$APP" "$ZIP"
  spctl -a -vv "$APP"
  echo "notarized $ZIP"
fi
