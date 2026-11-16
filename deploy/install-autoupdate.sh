#!/bin/bash
# Make workboard-server on this Mac follow main by itself: within about
# 5 minutes of a push it is built, tested and installed (deploy/auto-update.sh
# says how, and what happens when something fails).
#
#   deploy/install-autoupdate.sh            set up (or set up again), run once now
#   deploy/install-autoupdate.sh uninstall  stop updating (the checkout stays)
#
# It keeps its own checkout of the repository in ~/.local/src/Workboard
# (WB_AUTOUPDATE_SRC=<folder> for another place outside ~/Documents, ~/Desktop
# and ~/Downloads, which macOS keeps background jobs out of) and installs the
# LaunchAgent com.jikwon-labs.workboard-autoupdate, which runs
# deploy/auto-update.sh from that checkout every 5 minutes (INTERVAL=<seconds>
# for another pace). Nobody edits that checkout; updates stop if someone does.
#
# Needs: workboard-server installed (deploy/install-launchd.sh) and this Mac
# able to fetch the repository without typing anything.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

LABEL="com.jikwon-labs.workboard-autoupdate"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
SRC="${WB_AUTOUPDATE_SRC:-$HOME/.local/src/Workboard}"
LOG="$HOME/Library/Logs/workboard-autoupdate.log"
INTERVAL="${INTERVAL:-300}"
DOMAIN="gui/$(id -u)"

fail() {
  echo "install-autoupdate: $*" >&2
  exit 1
}

if [ "${1:-}" = "uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  echo "removed $LABEL: the server stays as it is and no longer updates by itself (the checkout $SRC stays)"
  exit 0
fi

[ -f "$HOME/Library/LaunchAgents/com.jikwon-labs.workboard-server.plist" ] ||
  fail "workboard-server isn't installed here: run deploy/install-launchd.sh first"
command -v cargo >/dev/null || fail "cargo not found (install Rust: https://rustup.rs)"
case "$SRC" in
"$HOME/Documents"* | "$HOME/Desktop"* | "$HOME/Downloads"* | "$HOME/Library/Mobile Documents"*)
  fail "$SRC is in a folder macOS keeps background jobs out of; choose another WB_AUTOUPDATE_SRC" ;;
esac

URL="$(git remote get-url origin)"
if [ -d "$SRC/.git" ]; then
  echo "checkout: $SRC (already there)"
  git -C "$SRC" fetch -q origin main || fail "can't fetch in $SRC: fix that first (git -C \"$SRC\" fetch)"
else
  echo "checkout: cloning $URL into $SRC"
  mkdir -p "$(dirname "$SRC")"
  git clone -q --branch main "$URL" "$SRC" || fail "can't clone $URL: this Mac needs to fetch the repository without a prompt"
  # start from this checkout's build cache (an APFS copy: no disk space, and
  # the first build isn't a cold one); not on the same disk, it builds from scratch
  if [ -d target ]; then
    cp -cR target "$SRC/target" 2>/dev/null || rm -rf "$SRC/target"
  fi
fi
[ -f "$SRC/deploy/auto-update.sh" ] || fail "$SRC has no deploy/auto-update.sh: is it on main with this change pushed?"
mkdir -p "$HOME/Library/LaunchAgents" "$(dirname "$LOG")"

# Background: macOS may slow the build down so the running server and the
# rest of the Mac stay quick.
cat >"$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>$SRC/deploy/auto-update.sh</string>
  </array>
  <key>StartInterval</key><integer>$INTERVAL</integer>
  <key>RunAtLoad</key><false/>
  <key>ProcessType</key><string>Background</string>
  <key>StandardOutPath</key><string>$LOG</string>
  <key>StandardErrorPath</key><string>$LOG</string>
</dict>
</plist>
PL

echo "first run now (the first build in a new checkout can take several minutes)..."
lines_before="$(wc -l <"$LOG" 2>/dev/null || echo 0)"
bash "$SRC/deploy/auto-update.sh" --now
tail -n +"$((lines_before + 1))" "$LOG" 2>/dev/null | sed 's/^/  /'

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
for _ in $(seq 1 50); do
  launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1 || break
  sleep 0.2
done
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "installed $LABEL: checks main every ${INTERVAL}s  (log: $LOG)"
