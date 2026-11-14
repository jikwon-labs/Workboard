#!/bin/bash
# Install workboard-server as a LaunchAgent: it starts when this user logs in
# and launchd restarts it if it stops. Run again after pulling new code.
#
#   deploy/install-launchd.sh            build, install, (re)start
#   deploy/install-launchd.sh uninstall  stop and remove
#
# It listens on this Mac's Tailscale address only (looked up now and written
# into the plist). If Tailscale isn't up yet at login, the server exits and
# launchd tries again every 10 seconds.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

LABEL="com.jikwon-labs.workboard-server"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/workboard-server"
LOG="$HOME/Library/Logs/workboard-server.log"
PORT="${PORT:-8787}"
DOMAIN="gui/$(id -u)"

if [ "${1:-}" = "uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  echo "removed $LABEL (the binary $BIN and the data folder stay)"
  exit 0
fi

TS=""
for t in tailscale /usr/local/bin/tailscale /opt/homebrew/bin/tailscale /Applications/Tailscale.app/Contents/MacOS/Tailscale; do
  if command -v "$t" >/dev/null 2>&1 || [ -x "$t" ]; then TS="$t"; break; fi
done
[ -n "$TS" ] || { echo "Tailscale isn't installed" >&2; exit 1; }
IP="$("$TS" ip -4 | head -1)"
[ -n "$IP" ] || { echo "Tailscale has no address. Log in to Tailscale first." >&2; exit 1; }

cargo build --release -p workboard-server
mkdir -p "$BIN_DIR" "$HOME/Library/LaunchAgents" "$(dirname "$LOG")"
# copy then rename, so a running server keeps its old file until restarted
cp target/release/workboard-server "$BIN.new" && mv "$BIN.new" "$BIN"

cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string>
    <string>--bind</string><string>$IP</string>
    <string>--port</string><string>$PORT</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>ProcessType</key><string>Background</string>
  <key>StandardOutPath</key><string>$LOG</string>
  <key>StandardErrorPath</key><string>$LOG</string>
</dict>
</plist>
PL

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "installed $LABEL: ws://$IP:$PORT/ws  (log: $LOG)"
