#!/bin/bash
# Sparkle(macOS 자동 업데이트 프레임워크)를 vendor/Sparkle 에 받아 둔다.
# 버전과 체크섬은 여기 한 곳에서 고정한다. 이미 받아 두었으면 아무것도 하지 않는다.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=2.10.1
SHA256=1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27
DEST=vendor/Sparkle

if [ -f "$DEST/.version" ] && [ "$(cat "$DEST/.version")" = "$VERSION" ]; then
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fsSL -o "$TMP/sparkle.tar.xz" "https://github.com/sparkle-project/Sparkle/releases/download/$VERSION/Sparkle-$VERSION.tar.xz"
echo "$SHA256  $TMP/sparkle.tar.xz" | shasum -a 256 -c - >/dev/null
rm -rf "$DEST"
mkdir -p "$DEST"
tar -xJf "$TMP/sparkle.tar.xz" -C "$DEST"
echo "$VERSION" > "$DEST/.version"
echo "Sparkle $VERSION -> $DEST"
