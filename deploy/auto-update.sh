#!/bin/bash
# Keep workboard-server on the Mac mini in step with main, by itself.
#
# deploy/install-autoupdate.sh sets this up: a checkout of its own outside
# ~/Documents (macOS keeps background jobs out of Documents, Desktop and
# Downloads) and a LaunchAgent that runs this script from that checkout every
# 5 minutes. Each run:
#
#   1. fetches origin/main; nothing new means nothing to do (and no log line)
#   2. fast-forwards the checkout; local edits or local commits stop it
#   3. builds the server; the same binary as the installed one means done
#   4. runs the server, sync and network tests; a failure leaves the running
#      server alone
#   5. installs with deploy/install-launchd.sh on the address and port the
#      server has now, and waits for /health; no answer means the previous
#      binary goes back
#
# A build that failed its tests or its health check is remembered (by its
# sha256 and the tests' sources) and not tried again; the next build that
# differs is, and so is the same build once the tests change (a test that
# failed only now and then, fixed).
#
#   deploy/auto-update.sh         what the LaunchAgent runs
#   deploy/auto-update.sh --now   also when main hasn't moved (first run, or
#                                 after fixing what made a run fail)
#
# Log: ~/Library/Logs/workboard-autoupdate.log
#
# WB_AU_* variables replace the real commands and paths; only
# deploy/test-auto-update.sh sets them.
set -uo pipefail

# Everything is inside main so that bash has read the whole script before the
# git pull below can change this very file.
main() {
  local repo
  repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
  cd "$repo" || exit 1
  # launchd starts us with a bare PATH
  export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

  local label="com.jikwon-labs.workboard-server"
  PLIST="${WB_AU_SERVER_PLIST:-$HOME/Library/LaunchAgents/$label.plist}"
  BIN="${WB_AU_BIN:-$HOME/.local/bin/workboard-server}"
  NEW_BIN="${WB_AU_NEW_BIN:-target/release/workboard-server}"
  LOG="${WB_AU_LOG:-$HOME/Library/Logs/workboard-autoupdate.log}"
  STATE="${WB_AU_STATE:-$HOME/.local/state/workboard-autoupdate}"
  local build_cmd="${WB_AU_BUILD:-cargo build --release -p workboard-server}"
  local test_cmd="${WB_AU_TEST:-cargo test -p workboard-server -p wb-sync -p wb-net}"
  # what test_cmd tests; a change here is a new try even with the same binary
  local test_paths="${WB_AU_TEST_PATHS:-server crates/wb-sync crates/wb-net Cargo.lock}"
  local install_cmd="${WB_AU_INSTALL:-deploy/install-launchd.sh}"
  RESTART_CMD="${WB_AU_RESTART:-launchctl kickstart -k gui/$(id -u)/$label}"
  HEALTH_SECS="${WB_AU_HEALTH_SECS:-60}"
  mkdir -p "$STATE" "$(dirname "$LOG")"

  # one run at a time (launchd never overlaps its runs, but a hand run might)
  LOCK="$STATE/run.pid"
  if [ -f "$LOCK" ] && kill -0 "$(cat "$LOCK" 2>/dev/null)" 2>/dev/null; then
    exit 0
  fi
  echo $$ >"$LOCK"
  trap 'rm -f "$LOCK"' EXIT

  if ! git fetch -q origin main 2>"$STATE/fetch.err"; then
    say_once "can't fetch origin/main: $(tail -1 "$STATE/fetch.err")"
    return
  fi
  if [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] && [ "${1:-}" != "--now" ]; then
    return
  fi
  if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    say_once "stopped: the checkout $repo has local edits (this checkout is only for updates)"
    return
  fi
  if ! git merge -q --ff-only origin/main 2>/dev/null; then
    say_once "stopped: $repo has commits that aren't on main"
    return
  fi
  rm -f "$STATE/last-note"

  local rev out sum
  rev="$(git rev-parse --short HEAD) $(git log -1 --format=%s | cut -c1-60)"
  if ! out="$(bash -c "$build_cmd" 2>&1)"; then
    say "$rev: build failed, the running server stays"
    printf '%s\n' "$out" | tail -15 | sed 's/^/    /' >>"$LOG"
    return
  fi
  if [ -f "$BIN" ] && cmp -s "$NEW_BIN" "$BIN"; then
    say "$rev: server unchanged"
    return
  fi
  local p
  sum="$({
    shasum -a 256 "$NEW_BIN" | cut -d' ' -f1
    for p in $test_paths; do git rev-parse "HEAD:$p" 2>/dev/null; done
  } | shasum -a 256 | cut -d' ' -f1)"
  if [ -f "$STATE/failed-build" ] && [ "$(cat "$STATE/failed-build")" = "$sum" ]; then
    say "$rev: same server build that failed before, skipped"
    return
  fi

  # the address and port the server is installed with now: [bin, --bind, B, --port, P]
  local bind port host
  if [ ! -f "$PLIST" ]; then
    say "$rev: workboard-server isn't installed (deploy/install-launchd.sh), nothing to update"
    return
  fi
  bind="$(/usr/libexec/PlistBuddy -c "Print :ProgramArguments:2" "$PLIST" 2>/dev/null)"
  port="$(/usr/libexec/PlistBuddy -c "Print :ProgramArguments:4" "$PLIST" 2>/dev/null)"
  if [ -z "$bind" ] || ! [[ "$port" =~ ^[0-9]+$ ]]; then
    say "$rev: can't read the address and port from $PLIST, left alone"
    return
  fi
  case ",$bind," in
  *,127.0.0.1,*) host=127.0.0.1 ;;
  *) host="${bind%%,*}" ;;
  esac
  HEALTH_URL="${WB_AU_HEALTH_URL:-http://$host:$port/health}"

  say "$rev: new server build, testing"
  if ! out="$(bash -c "$test_cmd" 2>&1)"; then
    echo "$sum" >"$STATE/failed-build"
    say "$rev: tests failed, the running server stays"
    printf '%s\n' "$out" | grep -E "FAILED|panicked|error(\[|:)" | tail -15 | sed 's/^/    /' >>"$LOG"
    return
  fi

  cp -p "$BIN" "$BIN.prev" 2>/dev/null
  if ! out="$(BIND="$bind" PORT="$port" bash -c "$install_cmd" 2>&1)"; then
    echo "$sum" >"$STATE/failed-build"
    say "$rev: install failed, putting the previous server back"
    printf '%s\n' "$out" | tail -10 | sed 's/^/    /' >>"$LOG"
    put_back
    return
  fi
  if answers; then
    rm -f "$STATE/failed-build"
    say "$rev: installed, /health answers"
  else
    echo "$sum" >"$STATE/failed-build"
    say "$rev: installed but /health didn't answer in ${HEALTH_SECS}s, putting the previous server back"
    put_back
  fi
}

say() {
  echo "$(date '+%Y-%m-%d %H:%M:%S')  $*" >>"$LOG"
}

# For problems that would otherwise be logged every 5 minutes.
say_once() {
  if [ "$(cat "$STATE/last-note" 2>/dev/null)" != "$*" ]; then
    say "$*"
    echo "$*" >"$STATE/last-note"
  fi
}

answers() {
  local waited=0
  while [ "$waited" -lt "$HEALTH_SECS" ]; do
    if curl -fsS -m 3 "$HEALTH_URL" 2>/dev/null | grep -q '"ok": *true'; then
      return 0
    fi
    sleep 2
    waited=$((waited + 2))
  done
  return 1
}

put_back() {
  if [ ! -f "$BIN.prev" ]; then
    say "    no previous server to put back"
    return
  fi
  cp -p "$BIN.prev" "$BIN" && bash -c "$RESTART_CMD" >/dev/null 2>&1
  if answers; then
    say "    the previous server is back and answering"
  else
    say "    the previous server doesn't answer either: look at ~/Library/Logs/workboard-server.log"
  fi
}

main "$@"
exit
