#!/bin/bash
# Keep the Mac mini serving after power cuts and restarts. Run once in
# Terminal on the Mac mini (it asks for your password):
#
#   deploy/setup-power.sh
#
# What it does:
#   - never sleep, never spin disks down, wake for network access
#   - power on by itself when electricity comes back (autorestart)
#   - log in automatically at startup, so the LaunchAgents (workboard-server,
#     livekit-server) start without anyone at the keyboard
#
# Automatic login is impossible while FileVault is on (macOS refuses it).
# With FileVault on, the script still sets the power options and explains
# the choice; see deploy/README.md ("Keep the Mac mini reachable").
set -euo pipefail

USER_NAME="$(id -un)"

echo "== power settings (sudo)"
sudo pmset -a sleep 0 disksleep 0 womp 1 autorestart 1
pmset -g | grep -E "^\s*(sleep|disksleep|womp|autorestart)\b" || true

echo
echo "== automatic login"
if fdesetup status | grep -q "FileVault is On"; then
  cat <<'TXT'
FileVault is on, so macOS won't log in automatically. After a power cut
the Mac mini powers on and waits at the FileVault screen until someone
types the password; once they do, the servers start by themselves.

For a planned restart (updates) without being at the keyboard, use:
    sudo fdesetup authrestart
(it unlocks the disk once for the next start).

To make it fully unattended instead, turn FileVault off (System Settings >
Privacy & Security > FileVault, or `sudo fdesetup disable`; decrypting
takes a while), then run this script again.
TXT
  exit 0
fi

current="$(defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser 2>/dev/null || true)"
if [ "$current" = "$USER_NAME" ]; then
  echo "already logging in automatically as $USER_NAME"
else
  echo "Type the login password for $USER_NAME to turn on automatic login:"
  sudo sysadminctl -autologin set -userName "$USER_NAME" -password -
  echo "automatic login: $(defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser 2>/dev/null || echo off)"
fi
echo
echo "Done. Test it once: restart the Mac mini and check"
echo "  curl http://\$(tailscale ip -4):8787/health"
