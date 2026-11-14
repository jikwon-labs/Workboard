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

## Setting up and inviting

1. The owner (whoever sets the team up) opens Workboard on their own Mac:
   Work together > "Or connect to your team's server", types the Mac mini's
   Tailscale address (`tailscale ip -4` on the Mac mini) and leaves the
   invite code empty. Their workspace goes up and they become the owner.
2. Invite each teammate to your tailnet (Tailscale admin console > Users >
   Invite, or share just this machine: Machines > the Mac mini > Share).
3. In Workboard, add them with Invite, then Work together > People >
   Invite code. Give them the code (it works once, for 7 days).
4. They install Tailscale and Workboard, then Work together > server address
   + code > Connect. They are signed in as that person.

Owners and admins can make someone admin, or Remove access (signs out every
computer of theirs at once) in Work together > People.

## On the Mac mini

    ~/.local/bin/workboard-server people              who has access
    ~/.local/bin/workboard-server backups             list backups
    ~/.local/bin/workboard-server restore FILE        go back to a backup
    ~/.local/bin/workboard-server invite ID --role owner
                                                      new owner code (owner lost their computer)

Backups: `backups/` in the data folder, hourly (48 kept) and daily (30 kept).
Health: `curl http://<tailscale address>:8787/health`

## App updates

The server also hands out Workboard updates to the apps signed in to it
(`updates/` in the data folder, served at `/updates/<file>`). How to publish
a version: [docs/release/README.md](../docs/release/README.md).
