#!/bin/bash
# Windows 판 zip 을 팀 서버의 updates/ 에 올린다. Windows 앱은 거기서 새 버전을 찾아
# 스스로 바꾼다(src/updater.rs 의 own). 전체 절차: docs/release/README.md "Windows 판"
#
#   UPDATES_DEST=<대상> scripts/publish-windows.sh [Actions 실행 번호]
#
# 1. GitHub Actions "Windows" 실행의 artifact Workboard-windows-zip 을 받는다(gh run download).
#    실행 번호가 없으면 지금 커밋(HEAD)에서 성공한 실행을 쓴다. main push 로는 실행이 돌지
#    않으니 push 뒤 gh workflow run windows.yml --ref main 으로 시작해 끝나 있어야 한다. 이 맥은 Windows 판을 빌드하지 못한다.
#    이미 받아 둔 zip 으로 시험할 때는 ZIPFILE=<zip>.
# 2. zip 이름이 Cargo.toml 의 버전과 맞는지(Workboard-<버전>-windows-x64.zip), 안에
#    workboard.exe 가 있는지 보고 build/updates-windows/ 에 둔다.
# 3. 같은 폴더에 appcast-windows.xml(버전, 파일 이름, 크기, sha256)을 만든다. 앱이 이걸 읽는다.
#    Mac 의 build/updates/ 와 appcast.xml 은 건드리지 않는다. 폴더를 나눈 건 Sparkle 의
#    generate_appcast 가 Windows zip 을 읽지 않게 하려는 것이다.
# 4. UPDATES_DEST 가 있으면 zip 을 먼저, appcast-windows.xml 을 나중에 복사한다(rsync).
#    앱이 아직 없는 zip 을 가리키는 순간이 없다. 서버의 다른 파일은 지우지 않는다.
#    Mac mini 로:  UPDATES_DEST='seojigwon@macmini:Library/Application Support/WorkboardServer/updates'
#    이 맥이 서버:  UPDATES_DEST="$HOME/Library/Application Support/WorkboardServer/updates"
#    없으면 복사하지 않고 build/updates-windows/ 에만 둔다.
#
# 코드 서명은 아직 없다. 처음 받는 사람에게는 SmartScreen 경고가 뜬다(앱 안 업데이트는 해당 없음).
set -euo pipefail
cd "$(dirname "$0")/.."

fail() { echo "$1" >&2; exit 1; }

VERSION="${WB_VERSION:-$(sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)".*/\1/p' Cargo.toml | head -1)}"
[ -n "$VERSION" ] || fail "Cargo.toml 에서 버전을 찾지 못했다."
NAME="Workboard-$VERSION-windows-x64.zip"
OUT="${UPDATES_DIR:-build/updates-windows}"
mkdir -p "$OUT"

if [ -n "${ZIPFILE:-}" ]; then
  [ -f "$ZIPFILE" ] || fail "$ZIPFILE 이 없다."
  cp "$ZIPFILE" "$OUT/$NAME"
else
  command -v gh >/dev/null || fail "gh 가 없다. brew install gh 후 gh auth login."
  RUN="${1:-}"
  if [ -z "$RUN" ]; then
    SHA="$(git rev-parse HEAD)"
    RUN="$(gh run list --workflow windows.yml --commit "$SHA" --status success --limit 1 \
      --json databaseId --jq '.[0].databaseId // empty')"
    [ -n "$RUN" ] || fail "커밋 ${SHA:0:7} 에서 성공한 Windows 실행이 없다. gh workflow run windows.yml --ref main 으로 시작해 끝나길 기다리거나(gh run list --workflow windows.yml) 실행 번호를 준다."
  fi
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  gh run download "$RUN" --name Workboard-windows-zip --dir "$TMP"
  [ -f "$TMP/$NAME" ] || fail "실행 $RUN 의 artifact 에 $NAME 이 없다(있는 것: $(ls "$TMP" | tr '\n' ' ')). Cargo.toml 버전과 그 실행의 커밋을 확인한다."
  cp "$TMP/$NAME" "$OUT/$NAME"
fi

ZIP="$OUT/$NAME"
unzip -Z1 "$ZIP" | grep -qx 'workboard.exe' || fail "$NAME 안에 workboard.exe 가 없다."
LENGTH="$(wc -c < "$ZIP" | tr -d ' ')"
SHA256="$(shasum -a 256 "$ZIP" | cut -d' ' -f1)"
cat > "$OUT/appcast-windows.xml" <<XML
<?xml version="1.0" encoding="utf-8"?>
<!-- The newest Workboard for Windows, read by the app (src/updater.rs). Made by scripts/publish-windows.sh. -->
<update>
  <version>$VERSION</version>
  <file>$NAME</file>
  <length>$LENGTH</length>
  <sha256>$SHA256</sha256>
</update>
XML
echo "appcast: $OUT/appcast-windows.xml ($NAME, $LENGTH bytes)"

if [ -n "${UPDATES_DEST:-}" ]; then
  DEST="$UPDATES_DEST"
  case "$DEST" in
    /*|.*|~*) mkdir -p "$DEST" ;;
    # 원격(호스트:경로): 경로는 원격 셸을 거치므로 공백을 이스케이프한다(Application Support)
    *:*) DEST="${DEST%%:*}:$(printf '%s' "${DEST#*:}" | sed 's/ /\\ /g')" ;;
    *) mkdir -p "$DEST" ;;
  esac
  rsync -a "$ZIP" "$DEST/"
  rsync -a "$OUT/appcast-windows.xml" "$DEST/"
  echo "copied to $UPDATES_DEST"
else
  echo "UPDATES_DEST 가 없어 복사는 하지 않았다. $ZIP 과 $OUT/appcast-windows.xml 을 서버 폴더의 updates/ 에 두면 된다."
fi
