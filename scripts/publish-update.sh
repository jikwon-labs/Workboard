#!/bin/bash
# 배포용 zip 을 자동 업데이트(Sparkle)로 올린다. 전체 절차: docs/release/README.md
#
#   NOTARIZE=1 scripts/bundle.sh          # 먼저 공증된 zip 을 만든다
#   UPDATES_DEST=<대상> scripts/publish-update.sh [zip]
#
# 1. zip 을 build/updates/ 에 모으고 generate_appcast 로 appcast.xml 을 다시 만든다.
#    zip 마다 EdDSA 서명이 붙는다(키체인의 Sparkle 키, 계정 workboard).
#    다운로드 주소는 상대 경로라서 앱이 어느 서버 주소로 붙든 그대로 맞는다.
#    같은 폴더에 Workboard-<버전>-arm64.html 이 있으면 업데이트 창에 릴리즈 노트로 보인다.
# 2. UPDATES_DEST 가 있으면 build/updates/ 를 서버 폴더의 updates/ 로 복사한다(rsync).
#    Mac mini 로:  UPDATES_DEST='seojigwon@macmini:Library/Application Support/WorkboardServer/updates'
#                  (SSH 호스트 macmini 는 ~/.ssh/config 에서 Cloudflare Tunnel 로 간다: deploy/README.md "SSH".
#                   계정 seojigwon, 경로는 그 계정의 홈 기준)
#    이 맥이 서버:  UPDATES_DEST="$HOME/Library/Application Support/WorkboardServer/updates"
#    서버는 그 폴더를 GET /updates/<파일> 로 내준다. 서버를 다시 띄울 필요는 없다.
#
# 공증 안 된 zip 은 다른 맥에서 Gatekeeper 에 막히므로 거절한다(시험할 때만 ALLOW_UNNOTARIZED=1).
set -euo pipefail
cd "$(dirname "$0")/.."

fail() { echo "$1" >&2; exit 1; }

scripts/fetch-sparkle.sh
VERSION="${WB_VERSION:-$(sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)".*/\1/p' Cargo.toml | head -1)}"
ZIPFILE="${1:-build/Workboard-$VERSION-$(uname -m).zip}"
[ -f "$ZIPFILE" ] || fail "$ZIPFILE 이 없다. 먼저 NOTARIZE=1 scripts/bundle.sh 를 돌린다."

CHECK="$(mktemp -d)"
trap 'rm -rf "$CHECK"' EXIT
ditto -x -k "$ZIPFILE" "$CHECK"
[ -d "$CHECK/Workboard.app/Contents/Frameworks/Sparkle.framework" ] || fail "$ZIPFILE 안의 앱에 Sparkle 이 없다(이전 bundle.sh 로 만든 zip)."
if ! xcrun stapler validate "$CHECK/Workboard.app" >/dev/null 2>&1; then
  [ "${ALLOW_UNNOTARIZED:-0}" = 1 ] || fail "$ZIPFILE 은 공증되지 않았다. NOTARIZE=1 scripts/bundle.sh 로 다시 만든다."
  echo "주의: 공증 안 된 zip 을 올린다(ALLOW_UNNOTARIZED=1)."
fi

OUT="${UPDATES_DIR:-build/updates}"
mkdir -p "$OUT"
cp "$ZIPFILE" "$OUT/"
vendor/Sparkle/bin/generate_appcast --account workboard "$OUT"
echo "appcast: $OUT/appcast.xml"

if [ -n "${UPDATES_DEST:-}" ]; then
  DEST="$UPDATES_DEST"
  case "$DEST" in
    /*|.*|~*) mkdir -p "$DEST" ;;
    # 원격(호스트:경로): 경로는 원격 셸을 거치므로 공백을 이스케이프한다(Application Support)
    *:*) DEST="${DEST%%:*}:$(printf '%s' "${DEST#*:}" | sed 's/ /\\ /g')" ;;
    *) mkdir -p "$DEST" ;;
  esac
  rsync -a --exclude '.*' "$OUT/" "$DEST/"
  echo "copied to $UPDATES_DEST"
else
  echo "UPDATES_DEST 가 없어 복사는 하지 않았다. $OUT/ 의 파일을 서버 폴더의 updates/ 에 두면 된다."
fi
