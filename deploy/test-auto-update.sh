#!/bin/bash
# Tests for deploy/auto-update.sh, without cargo, launchd or a real server:
# a throwaway "origin" repository whose one file stands for the server's
# source, and small commands in place of build, test, install and restart.
#
#   deploy/test-auto-update.sh
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT

pass=0
failures=0
ok() { pass=$((pass + 1)); echo "ok    $1"; }
bad() { failures=$((failures + 1)); echo "FAIL  $1"; }
check() { if eval "$2"; then ok "$1"; else bad "$1"; fi; }

g() { git -c user.name=t -c user.email=t@t -c commit.gpgsign=false "$@"; }

# origin with the script under test and a "server source" file
g init -q -b main "$T/origin"
mkdir -p "$T/origin/deploy"
cp "$HERE/auto-update.sh" "$T/origin/deploy/"
echo "server v1" >"$T/origin/server.txt"
echo "readme" >"$T/origin/README.md"
g -C "$T/origin" add -A && g -C "$T/origin" commit -qm "first"
g clone -q "$T/origin" "$T/src"

commit() { # file text message
  echo "$2" >"$T/origin/$1"
  g -C "$T/origin" commit -qam "$3"
}

# the installed server: v1, listening on 127.0.0.1 port 9999
mkdir -p "$T/bin"
echo "server v1" >"$T/bin/workboard-server"
cat >"$T/server.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>ProgramArguments</key><array>
<string>$T/bin/workboard-server</string><string>--bind</string><string>127.0.0.1</string><string>--port</string><string>9999</string>
</array></dict></plist>
PL
echo '{"ok":true}' >"$T/health"

export WB_AU_SERVER_PLIST="$T/server.plist"
export WB_AU_BIN="$T/bin/workboard-server"
export WB_AU_LOG="$T/log"
export WB_AU_STATE="$T/state"
export WB_AU_HEALTH_URL="file://$T/health"
export WB_AU_HEALTH_SECS=2
# build: the "binary" is the source file; tests fail on BROKEN; a server
# with DEAD in it never answers /health; restart brings the old one back up
export WB_AU_BUILD='mkdir -p target/release && cp server.txt target/release/workboard-server'
export WB_AU_TEST='! grep -q BROKEN server.txt'
export WB_AU_INSTALL="cp target/release/workboard-server \"\$WB_AU_BIN\" && echo \"\$BIND \$PORT\" >\"$T/install-args\" && if grep -q DEAD \"\$WB_AU_BIN\"; then rm -f \"$T/health\"; fi"
export WB_AU_RESTART="echo '{\"ok\":true}' >\"$T/health\""

run() { bash "$T/src/deploy/auto-update.sh" "$@"; }
installed() { cat "$T/bin/workboard-server"; }
lines() { wc -l <"$T/log" 2>/dev/null | tr -d ' ' || echo 0; }
last() { tail -1 "$T/log"; }

# 1. nothing new on main: no work, no log
run
check "nothing new: no log line" '[ ! -s "$T/log" ]'

# 2. --now with the same source: binary equals the installed one
run --now
check "--now, same build: says unchanged" 'last | grep -q "server unchanged"'

# 3. a server change is built, tested, installed with the same bind and port
commit server.txt "server v2" "server two"
run
check "server change: installed" '[ "$(installed)" = "server v2" ] && last | grep -q "installed, /health answers"'
check "server change: same bind and port" '[ "$(cat "$T/install-args")" = "127.0.0.1 9999" ]'
check "server change: checkout fast-forwarded" '[ "$(git -C "$T/src" rev-parse HEAD)" = "$(git -C "$T/origin" rev-parse HEAD)" ]'

# 4. a change that doesn't touch the server
commit README.md "readme 2" "docs only"
run
check "docs only: server unchanged" '[ "$(installed)" = "server v2" ] && last | grep -q "docs only: server unchanged"'

# 5. failing tests keep the running server, and the same build isn't retried
commit server.txt "server v3 BROKEN" "broken server"
run
check "tests fail: running server stays" '[ "$(installed)" = "server v2" ] && last | grep -q "tests failed"'
commit README.md "readme 3" "docs after broken"
run
check "same failed build: skipped" '[ "$(installed)" = "server v2" ] && last | grep -q "same server build that failed before"'

# 6. a fix after that is installed
commit server.txt "server v4" "fixed server"
run
check "fix after failure: installed" '[ "$(installed)" = "server v4" ]'

# 7. a server that doesn't answer /health: the previous one goes back
commit server.txt "server v5 DEAD" "dead server"
run
check "no health: previous binary back" '[ "$(installed)" = "server v4" ]'
check "no health: logged" 'grep -q "didn.t answer" "$T/log" && last | grep -q "previous server is back"'
check "no health: previous server answering" '[ -f "$T/health" ]'

# 8. local edits in the checkout stop updates, logged once
commit server.txt "server v6" "after dead"
echo "edited" >>"$T/src/README.md"
before="$(lines)"
run
run
check "local edits: not installed" '[ "$(installed)" = "server v4" ]'
check "local edits: logged once" '[ "$(( $(lines) - before ))" = 1 ] && last | grep -q "local edits"'
g -C "$T/src" checkout -q README.md
run
check "edits gone: installed again" '[ "$(installed)" = "server v6" ]'

# 9. a build that doesn't compile
export WB_AU_BUILD_SAVED="$WB_AU_BUILD"
export WB_AU_BUILD='echo "error: nope" >&2; false'
commit server.txt "server v7" "does not build"
run
check "build fails: running server stays" '[ "$(installed)" = "server v6" ] && grep -q "build failed" "$T/log"'
export WB_AU_BUILD="$WB_AU_BUILD_SAVED"

# 10. no installed server: nothing installed
mv "$T/server.plist" "$T/server.plist.off"
commit server.txt "server v8" "no plist"
run
check "not installed: left alone" '[ "$(installed)" = "server v6" ] && last | grep -q "isn.t installed"'
mv "$T/server.plist.off" "$T/server.plist"

# 11. one run at a time, and the lock goes away after a run
check "lock removed after a run" '[ ! -e "$T/state/run.pid" ]'
sleep 30 &
echo $! >"$T/state/run.pid"
commit server.txt "server v9" "while another run"
run
check "another run going: does nothing" '[ "$(installed)" = "server v6" ]'
kill $! 2>/dev/null
wait 2>/dev/null
run
check "stale lock: runs" '[ "$(installed)" = "server v9" ]'

echo
echo "$pass passed, $failures failed"
[ "$failures" = 0 ]
