#!/bin/bash
# Check that everything the Mac mini serves came back by itself after a
# restart, and write what it found to ~/Library/Logs/workboard-boot-check.log.
#
#   deploy/check-after-boot.sh            check now
#   deploy/check-after-boot.sh --install  check once, 3 minutes after the next login
#
# The --install form adds a one-time LaunchAgent that removes itself after it
# has run, so the check happens without anyone at the keyboard.
set -uo pipefail
# launchd starts us with a bare PATH; docker lives in ~/.docker/bin
export PATH="$HOME/.docker/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"

LABEL="com.jikwon-labs.boot-check"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
LOG="$HOME/Library/Logs/workboard-boot-check.log"
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"

if [ "${1:-}" = "--install" ]; then
  # run a copy outside ~/Documents, which macOS keeps background jobs out of
  mkdir -p "$HOME/.local/bin"
  cp "$SELF" "$HOME/.local/bin/workboard-boot-check.sh"
  SELF="$HOME/.local/bin/workboard-boot-check.sh"
  cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array><string>/bin/bash</string><string>$SELF</string><string>--after-boot</string></array>
  <key>RunAtLoad</key><true/>
  <key>StandardOutPath</key><string>/dev/null</string>
  <key>StandardErrorPath</key><string>/dev/null</string>
</dict>
</plist>
PL
  echo "will check 3 minutes after the next login; results: $LOG"
  exit 0
fi

if [ "${1:-}" = "--after-boot" ]; then
  # give everything time to start, then run once and remove ourselves
  sleep 180
  rm -f "$PLIST"
fi

# run a command with a time limit (camera queries can hang)
limit() {
  local secs=$1; shift
  "$@" & local pid=$!
  ( sleep "$secs"; kill "$pid" 2>/dev/null ) & local guard=$!
  wait "$pid" 2>/dev/null; local rc=$?
  kill "$guard" 2>/dev/null
  return $rc
}

{
  echo "=== $(date '+%Y-%m-%d %H:%M:%S')  ($([ "${1:-}" = "--after-boot" ] && echo "after login" || echo "by hand"))"
  echo "booted: $(sysctl -n kern.boottime | sed 's/.*} //')   uptime: $(uptime | sed 's/.*up //; s/,.*//')"
  TS=""
  for t in tailscale /usr/local/bin/tailscale /Applications/Tailscale.app/Contents/MacOS/Tailscale; do
    if command -v "$t" >/dev/null 2>&1 || [ -x "$t" ]; then TS="$t"; break; fi
  done
  IP="$([ -n "$TS" ] && "$TS" ip -4 2>/dev/null | head -1)"
  echo "tailscale: ${IP:-NOT UP}"
  for l in com.jikwon-labs.workboard-server com.jikwon-labs.livekit-server com.ngrok.metabase io.github.wlrnjs.branchport ai.hermes.gateway; do
    st="$(launchctl print "gui/$(id -u)/$l" 2>/dev/null | grep -m1 -E '^\s*state =' | awk '{print $3}')"
    echo "agent $l: ${st:-NOT LOADED}"
  done
  if [ -n "$IP" ]; then
    echo "workboard /health: $(curl -s -m 5 "http://$IP:8787/health" || echo FAILED)"
    echo "livekit http: $(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://$IP:7880/" || echo FAILED)"
  fi
  echo "livekit listening: $(lsof -nP -iUDP -iTCP -sTCP:LISTEN 2>/dev/null | awk '$1 ~ /livekit/ {print $9}' | sort -u | tr '\n' ' ')"
  if command -v docker >/dev/null 2>&1; then
    echo "docker: $(limit 20 docker ps --format '{{.Names}}={{.Status}}' 2>/dev/null | tr '\n' ' ' || echo 'not running')"
  fi
  # the tunnel can take a little while to connect after a restart
  tunnel=""
  for _ in $(seq 1 12); do
    tunnel="$(curl -s -m 5 http://127.0.0.1:4040/api/tunnels 2>/dev/null | grep -o '"public_url":"[^"]*"' | head -1)"
    [ -n "$tunnel" ] && break
    sleep 5
  done
  echo "ngrok tunnel: ${tunnel:-none after 60s}"
  # cameras the way the app sees them (system_profiler can hang for minutes)
  cams="$(limit 20 swift -e 'import AVFoundation; print(AVCaptureDevice.DiscoverySession(deviceTypes: [.builtInWideAngleCamera, .external, .continuityCamera], mediaType: .video, position: .unspecified).devices.map { $0.localizedName }.joined(separator: ", "))' 2>/dev/null)"
  echo "camera: ${cams:-none connected}"
  echo
} >> "$LOG" 2>&1

[ "${1:-}" = "--after-boot" ] || tail -20 "$LOG"
