#!/bin/bash
# Hold voice huddles on LiveKit Cloud instead of a livekit-server on this Mac.
# Behind Cloudflare Tunnel there is no way in for a local LiveKit's audio and
# video (UDP), so huddles go to LiveKit Cloud (its free plan is enough for a
# small team). workboard-server keeps handing out the join tokens.
#
#   LIVEKIT_URL=wss://<project>.livekit.cloud LIVEKIT_KEY=<key> LIVEKIT_SECRET=<secret> deploy/use-livekit-cloud.sh
#
# The three values come from cloud.livekit.io > the project > Settings > Keys
# (create a key: it shows the URL, API key and secret once). They go into
# livekit.json in the server's data folder (mode 600), which the server reads
# for each huddle, so it needs no restart. A local livekit-server left on
# this Mac from before (LaunchAgent com.jikwon-labs.livekit-server) is
# stopped and removed.
set -euo pipefail

HOME_DIR="${WORKBOARD_SERVER_HOME:-$HOME/Library/Application Support/WorkboardServer}"
JSON="$HOME_DIR/livekit.json"

URL="${LIVEKIT_URL:-}"
KEY="${LIVEKIT_KEY:-}"
SECRET="${LIVEKIT_SECRET:-}"
[ -n "$URL" ] && [ -n "$KEY" ] && [ -n "$SECRET" ] || {
  echo "Set LIVEKIT_URL, LIVEKIT_KEY and LIVEKIT_SECRET (see the top of $0)." >&2
  exit 1
}
case "$URL" in
  wss://*) ;;
  https://*) URL="wss://${URL#https://}" ;;
  *) echo "LIVEKIT_URL should start with wss:// (like wss://team-abc123.livekit.cloud)" >&2; exit 1 ;;
esac
URL="${URL%/}"

mkdir -p "$HOME_DIR"
( umask 077
  python3 -c 'import json,sys; json.dump({"url": sys.argv[1], "key": sys.argv[2], "secret": sys.argv[3]}, open(sys.argv[4], "w")); print()' \
    "$URL" "$KEY" "$SECRET" "$JSON" )
chmod 600 "$JSON"
echo "huddles now use $URL (written to $JSON)"

LK_PLIST="$HOME/Library/LaunchAgents/com.jikwon-labs.livekit-server.plist"
if [ -f "$LK_PLIST" ]; then
  launchctl bootout "gui/$(id -u)/com.jikwon-labs.livekit-server" 2>/dev/null || true
  rm -f "$LK_PLIST"
  echo "removed the local livekit-server"
fi
