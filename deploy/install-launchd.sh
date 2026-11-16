#!/bin/bash
# Install workboard-server as a LaunchAgent: it starts when this user logs in
# and launchd restarts it if it stops. Run again after pulling new code.
#
#   deploy/install-launchd.sh            build, install, (re)start
#   deploy/install-launchd.sh uninstall  stop and remove
#
# It listens on 127.0.0.1 (where cloudflared, Cloudflare Tunnel's connector
# on this Mac, reaches it) and, when Tailscale is up now, on this Mac's
# Tailscale address too (looked up now and written into the plist). If
# Tailscale isn't up yet at login, the server exits and launchd tries again
# every 10 seconds. BIND=127.0.0.1 deploy/install-launchd.sh leaves Tailscale
# out (run it that way after taking Tailscale off this Mac).
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

if [ -z "${BIND:-}" ]; then
  TS=""
  for t in tailscale /usr/local/bin/tailscale /opt/homebrew/bin/tailscale /Applications/Tailscale.app/Contents/MacOS/Tailscale; do
    if command -v "$t" >/dev/null 2>&1 || [ -x "$t" ]; then TS="$t"; break; fi
  done
  IP="$([ -n "$TS" ] && "$TS" ip -4 2>/dev/null | head -1 || true)"
  if [ -n "$IP" ]; then
    BIND="$IP,127.0.0.1"
  else
    echo "Tailscale isn't up: listening on 127.0.0.1 only (for Cloudflare Tunnel)"
    BIND="127.0.0.1"
  fi
fi

cargo build --release -p workboard-server
mkdir -p "$BIN_DIR" "$HOME/Library/LaunchAgents" "$(dirname "$LOG")"
# copy then rename, so a running server keeps its old file until restarted
cp target/release/workboard-server "$BIN.new" && mv "$BIN.new" "$BIN"

# ProcessType Standard: Background lets macOS throttle CPU and I/O (and
# timers) when it decides to, which delays chat messages and pushes for
# everyone. Standard runs it like a normal program without asking for the
# extra priority of Interactive, which only LiveKit's real-time audio needs.
cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string>
    <string>--bind</string><string>$BIND</string>
    <string>--port</string><string>$PORT</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>ProcessType</key><string>Standard</string>
  <key>StandardOutPath</key><string>$LOG</string>
  <key>StandardErrorPath</key><string>$LOG</string>
</dict>
</plist>
PL

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
# bootout returns before the old one has stopped; bootstrap fails until it has
for _ in $(seq 1 50); do
  launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1 || break
  sleep 0.2
done
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "installed $LABEL: listening on $BIND, port $PORT  (log: $LOG)"
