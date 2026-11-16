#!/bin/bash
# release 빌드 후 build/Workboard.app 번들을 만든다.
# NOTARIZE=1 이면 Apple 공증까지 하고 배포용 build/Workboard-<버전>-arm64.zip 을 만든다.
# ZIP=1 이면 공증 없이 zip 만 만든다(업데이트 시험용).
# 자동 업데이트(Sparkle) 프레임워크를 Contents/Frameworks 에 넣는다. 업데이트를 올리는 건 scripts/publish-update.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

# 버전은 Cargo.toml 한 곳에만 둔다. 빌드 번호는 커밋 수(늘기만 한다).
# Sparkle 은 빌드 번호(CFBundleVersion)로 새 버전인지 판단한다.
# 시험할 때만 WB_VERSION / WB_BUILD 로 덮어쓴다.
VERSION="${WB_VERSION:-$(sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)".*/\1/p' Cargo.toml | head -1)}"
if [ -z "$VERSION" ]; then echo "Cargo.toml 에서 버전을 읽지 못했다" >&2; exit 1; fi
BUILD="${WB_BUILD:-$(git rev-list --count HEAD 2>/dev/null || echo 1)}"

scripts/fetch-sparkle.sh
cargo build --release

APP=build/Workboard.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/workboard "$APP/Contents/MacOS/Workboard"
if [ -f Resources/AppIcon.icns ]; then cp Resources/AppIcon.icns "$APP/Contents/Resources/"; fi

# Sparkle: 앱이 실행 중에 불러 쓴다(src/updater.rs). 샌드박스 앱이 아니라 XPC 서비스는 필요 없어 뺀다.
FW="$APP/Contents/Frameworks/Sparkle.framework"
mkdir -p "$APP/Contents/Frameworks"
ditto vendor/Sparkle/Sparkle.framework "$FW"
rm -rf "$FW/Versions/B/XPCServices" "$FW/XPCServices"

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
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <!-- 앱 안의 언어는 설정 > 언어에서 고른다. 여기 목록은 macOS 권한 창과 Sparkle 업데이트 창의 언어를 정한다. -->
  <key>CFBundleLocalizations</key><array><string>en</string><string>ko</string></array>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <!-- 초대 링크(workboard://join?server=...&code=...)를 이 앱이 연다. src/links.rs 가 받는다. -->
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key><string>local.workboard.app.join</string>
      <key>CFBundleURLSchemes</key><array><string>workboard</string></array>
    </dict>
  </array>
  <key>NSMicrophoneUsageDescription</key><string>Workboard uses the microphone for huddles and Talk to Text.</string>
  <key>NSCameraUsageDescription</key><string>Workboard uses the camera for huddles and clips.</string>
  <!-- Sparkle: 업데이트는 팀 서버(GET /updates/appcast.xml)에서 받는다. 하루 한 번 스스로 확인한다. -->
  <key>SUPublicEDKey</key><string>DL5Ve/iIbEju0K4LeI9lffHyXPGWwExhA7/X0GZ8Y28=</string>
  <key>SUEnableAutomaticChecks</key><true/>
  <key>SUVerifyUpdateBeforeExtraction</key><true/>
  <!-- 팀 서버는 Cloudflare Tunnel 의 https 라 예외가 필요 없다. 다만 https 없이 붙는 서버(로컬 시험, IP 주소)에서도
       업데이트 확인이 되도록 http 를 전부 허용해 둔다. ATS 예외 도메인은 IP 주소를 받지 않는다(-1022). 업데이트 파일은 EdDSA 서명으로 확인한다.
       NSAllowsLocalNetworking 을 같이 두면 NSAllowsArbitraryLoads 가 무시되므로 넣지 않는다. -->
  <key>NSAppTransportSecurity</key>
  <dict>
    <key>NSAllowsArbitraryLoads</key><true/>
  </dict>
</dict>
</plist>
PLIST

# 권한 창(마이크·카메라)의 한국어 문구. 영어는 위 Info.plist 의 기본값을 쓴다.
mkdir -p "$APP/Contents/Resources/en.lproj" "$APP/Contents/Resources/ko.lproj"
cat > "$APP/Contents/Resources/ko.lproj/InfoPlist.strings" <<'STRINGS'
"NSMicrophoneUsageDescription" = "Workboard는 허들과 음성 입력에 마이크를 사용합니다.";
"NSCameraUsageDescription" = "Workboard는 허들과 클립 녹화에 카메라를 사용합니다.";
STRINGS

# 서명: Developer ID 인증서가 키체인에 있으면 그걸로, 없으면 임시(ad-hoc) 서명.
ID="${SIGN_IDENTITY:-Developer ID Application: jikwon seo (K6599UJ4KM)}"
# 안쪽부터 서명한다: Sparkle 의 실행 파일들 -> 프레임워크 -> 앱.
SPARKLE_PARTS=("$FW/Versions/B/Autoupdate" "$FW/Versions/B/Updater.app" "$FW")
if security find-identity -v -p codesigning | grep -qF "$ID"; then
  for part in "${SPARKLE_PARTS[@]}"; do
    codesign --force --options runtime --timestamp --sign "$ID" "$part"
  done
  # 마이크·카메라 권한(entitlements)을 붙여 서명한다
  codesign --force --options runtime --timestamp --entitlements Resources/Workboard.entitlements --sign "$ID" "$APP"
  codesign --verify --strict --deep "$APP"
  SIGNED_WITH_ID=1
else
  for part in "${SPARKLE_PARTS[@]}"; do
    codesign --force --sign - "$part" >/dev/null 2>&1 || true
  done
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
if [ "${ZIP:-0}" = 1 ] && [ "${NOTARIZE:-0}" != 1 ]; then
  ZIPFILE="build/Workboard-$VERSION-$(uname -m).zip"
  rm -f "$ZIPFILE"
  ditto -c -k --keepParent "$APP" "$ZIPFILE"
  echo "zipped $ZIPFILE (not notarized)"
fi
