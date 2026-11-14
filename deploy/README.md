# Running workboard-server on the Mac mini

## Start automatically

    deploy/install-launchd.sh

Builds the server, copies it to `~/.local/bin/workboard-server` and installs
the LaunchAgent `com.jikwon-labs.workboard-server`: it starts when this user
logs in and restarts within 10 seconds if it stops. It listens only on this
Mac's Tailscale address, port 8787 (`PORT=9000 deploy/install-launchd.sh` for
another port). Run the script again after pulling new code.

- Status: `launchctl print gui/$(id -u)/com.jikwon-labs.workboard-server | grep state`
- Log: `~/Library/Logs/workboard-server.log`
- Data: `~/Library/Application Support/WorkboardServer/` (`workspace.json`, `server.json`)
- Stop and remove: `deploy/install-launchd.sh uninstall`

## Keep the Mac mini reachable

A LaunchAgent runs while the user is logged in. For a Mac mini that should
come back by itself after a power cut (run these yourself; they need sudo):

    sudo pmset -a sleep 0          # never sleep (already 0 on this Mac mini)
    sudo pmset -a autorestart 1    # power back on after a power failure

and turn on automatic login in System Settings > Users & Groups (FileVault
must be off for automatic login).

## Teammates

1. Invite them to your tailnet (Tailscale admin console > Users > Invite, or
   share just this machine: Machines > the Mac mini > Share).
2. They install Tailscale and Workboard, then Work together > "Or connect to
   your team's server" and type the Mac mini's Tailscale name or address
   (`tailscale ip -4` on the Mac mini).
