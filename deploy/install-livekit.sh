#!/bin/bash
# Install livekit-server (voice huddles) as a LaunchAgent next to
# workboard-server. Run once on the Mac mini; run again to restart it.
#
#   deploy/install-livekit.sh            set up keys (first time), install, (re)start
#   deploy/install-livekit.sh uninstall  stop and remove (keys stay)
#
# It listens only on this Mac's Tailscale address: 7880 (signalling) and
# 7882/udp (audio and video). The API key and secret are made here once
# and kept in the workboard-server data folder (never in the repo):
#   livekit.yaml  livekit-server's config
#   livekit.json  what workboard-server needs to hand out call tokens
set -euo pipefail

LABEL="com.jikwon-labs.livekit-server"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
HOME_DIR="${WORKBOARD_SERVER_HOME:-$HOME/Library/Application Support/WorkboardServer}"
YAML="$HOME_DIR/livekit.yaml"
JSON="$HOME_DIR/livekit.json"
LOG="$HOME/Library/Logs/livekit-server.log"
DOMAIN="gui/$(id -u)"

if [ "${1:-}" = "uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  echo "removed $LABEL (keys stay in $HOME_DIR)"
  exit 0
fi

LK="$(command -v livekit-server || true)"
[ -n "$LK" ] || { echo "livekit-server isn't installed: brew install livekit" >&2; exit 1; }
TS=""
for t in tailscale /usr/local/bin/tailscale /opt/homebrew/bin/tailscale /Applications/Tailscale.app/Contents/MacOS/Tailscale; do
  if command -v "$t" >/dev/null 2>&1 || [ -x "$t" ]; then TS="$t"; break; fi
done
[ -n "$TS" ] || { echo "Tailscale isn't installed" >&2; exit 1; }
IP="$("$TS" ip -4 | head -1)"
[ -n "$IP" ] || { echo "Tailscale has no address. Log in to Tailscale first." >&2; exit 1; }

mkdir -p "$HOME_DIR" "$HOME/Library/LaunchAgents" "$(dirname "$LOG")"
umask 077
if [ -f "$JSON" ]; then
  KEY="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["key"])' "$JSON")"
  SECRET="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["secret"])' "$JSON")"
else
  KEY="wb$(openssl rand -hex 6)"
  SECRET="$(openssl rand -hex 32)"
fi

cat > "$YAML" <<YML
port: 7880
bind_addresses: ["$IP"]
rtc:
  # no TCP fallback: it would listen on every network, and UDP always
  # works inside Tailscale
  tcp_port: 0
  udp_port: 7882
  use_external_ip: false
  node_ip: $IP
  # only the Tailscale network (not the home LAN) for audio and video
  ips:
    includes:
      - 100.64.0.0/10
      - fd7a:115c:a1e0::/48
keys:
  $KEY: $SECRET
logging:
  level: info
YML
printf '{"url": "ws://%s:7880", "key": "%s", "secret": "%s"}\n' "$IP" "$KEY" "$SECRET" > "$JSON"
chmod 600 "$YAML" "$JSON"

cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$LK</string>
    <string>--config</string><string>$YAML</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>ProcessType</key><string>Interactive</string>
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
echo "installed $LABEL: ws://$IP:7880  (log: $LOG)"
