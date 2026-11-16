#!/bin/bash
# Connect this Mac to Cloudflare Tunnel, so the team reaches workboard-server
# at an https address (like https://wb.example.com) without Tailscale.
#
#   TUNNEL_TOKEN=<token> deploy/install-cloudflared.sh   install, (re)start
#   deploy/install-cloudflared.sh                        restart with the saved token
#   deploy/install-cloudflared.sh uninstall              stop and remove (token stays)
#
# The token comes from the Cloudflare dashboard (Zero Trust > Networks >
# Tunnels > the tunnel > the install command's long "eyJ..." part). The
# tunnel's public hostname is set there too, pointing at http://127.0.0.1:8787.
# Steps: docs/cloudflare-tunnel.md.
#
# cloudflared runs as the LaunchAgent com.jikwon-labs.cloudflared next to
# workboard-server: it starts at login and restarts if it stops. The token is
# kept in the server's data folder (cloudflared-token, mode 600), never in
# the repo; the plist that carries it is mode 600 too.
set -euo pipefail
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

LABEL="com.jikwon-labs.cloudflared"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
HOME_DIR="${WORKBOARD_SERVER_HOME:-$HOME/Library/Application Support/WorkboardServer}"
TOKEN_FILE="$HOME_DIR/cloudflared-token"
LOG="$HOME/Library/Logs/cloudflared.log"
DOMAIN="gui/$(id -u)"

if [ "${1:-}" = "uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  echo "removed $LABEL (the token stays in $TOKEN_FILE)"
  exit 0
fi

CF="$(command -v cloudflared || true)"
[ -n "$CF" ] || { echo "cloudflared isn't installed: brew install cloudflared" >&2; exit 1; }

mkdir -p "$HOME_DIR" "$HOME/Library/LaunchAgents" "$(dirname "$LOG")"
if [ -n "${TUNNEL_TOKEN:-}" ]; then
  # a pasted "cloudflared service install eyJ..." line works too
  TOKEN="$(printf '%s' "$TUNNEL_TOKEN" | tr ' ' '\n' | grep -E '^eyJ' | tail -1 || true)"
  [ -n "$TOKEN" ] || { echo "That doesn't look like a tunnel token (it starts with eyJ)." >&2; exit 1; }
  ( umask 077; printf '%s\n' "$TOKEN" > "$TOKEN_FILE" )
fi
[ -s "$TOKEN_FILE" ] || { echo "No token yet. Run: TUNNEL_TOKEN=<token> $0" >&2; exit 1; }
TOKEN="$(head -1 "$TOKEN_FILE")"

( umask 077; cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$CF</string>
    <string>tunnel</string>
    <string>--no-autoupdate</string>
    <string>run</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>TUNNEL_TOKEN</key><string>$TOKEN</string>
  </dict>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>ProcessType</key><string>Standard</string>
  <key>StandardOutPath</key><string>$LOG</string>
  <key>StandardErrorPath</key><string>$LOG</string>
</dict>
</plist>
PL
)

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
# bootout returns before the old one has stopped; bootstrap fails until it has
for _ in $(seq 1 50); do
  launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1 || break
  sleep 0.2
done
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "installed $LABEL  (log: $LOG)"

# the server must answer where the tunnel sends requests
if curl -s -m 5 http://127.0.0.1:8787/health | grep -q '"ok":true'; then
  echo "workboard-server answers on 127.0.0.1:8787"
else
  echo "workboard-server doesn't answer on 127.0.0.1:8787 yet: run deploy/install-launchd.sh" >&2
fi
echo "check from anywhere: curl https://<your hostname>/health   (should say {\"ok\":true})"
