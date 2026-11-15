# Running workboard-server on the Mac mini

Next steps on the Mac mini (accounts and invite mail): `docs/macmini/README.md`.
Design and current state: `docs/server-design.md`. Testing (also from another
Mac): `docs/testing.md`. App releases: `docs/release/README.md`.

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

## Voice huddles (LiveKit)

    brew install livekit
    deploy/install-livekit.sh

Installs livekit-server as the LaunchAgent `com.jikwon-labs.livekit-server`
on the Tailscale address only: 7880 (signalling) and 7882/udp (audio and
video). No TCP fallback (it would listen on every network; UDP always works
inside Tailscale), and the home LAN address isn't used.
The first run makes an API key and secret and keeps them in the data folder
(`livekit.yaml`, `livekit.json`, mode 600); workboard-server reads
`livekit.json` to hand out call tokens. Log: `~/Library/Logs/livekit-server.log`.
Stop and remove: `deploy/install-livekit.sh uninstall`.

## Keep the Mac mini reachable

Run once in Terminal on the Mac mini (asks for your password):

    deploy/setup-power.sh

It turns off sleep and disk sleep, keeps wake-for-network on, and makes the
Mac mini power on by itself when electricity comes back (`autorestart`).

This Mac mini keeps FileVault on (decided 2026-11-15), so macOS won't log in
automatically: after a power cut it powers on and waits at the FileVault
screen until someone types the password; then workboard-server and
livekit-server start by themselves. For a planned restart without being at
the keyboard, use `sudo fdesetup authrestart`. (With FileVault off the same
script also turns on automatic login.)

## After a restart

    deploy/check-after-boot.sh            check that everything is back
    deploy/check-after-boot.sh --install  check once, 3 minutes after the next login

Results go to `~/Library/Logs/workboard-boot-check.log` (Tailscale address,
each LaunchAgent, workboard /health, LiveKit, Docker containers, the ngrok
tunnel, the camera). The `--install` form runs a copy from `~/.local/bin`
(background jobs can't read `~/Documents`) and removes itself after one run.

## Setting up and inviting

1. The owner (whoever sets the team up) opens Workboard on their own Mac:
   the first-start window's "Set up a server for my team" (or Work together >
   "Or connect to your team's server" > New server), types the Mac mini's
   Tailscale address (`tailscale ip -4` on the Mac mini), their email and a
   password. Their workspace goes up and they become the owner.
2. Invite each teammate to your tailnet (Tailscale admin console > Users >
   Invite, or share just this machine: Machines > the Mac mini > Share).
3. In Workboard, add them with Invite (their email). With email set up (below)
   the server mails each of them their invite code and a link; otherwise copy
   the code from Work together > People and send it yourself (it works once,
   for 7 days).
4. They install Tailscale and Workboard, open the link (or "Join my team" >
   server address > Invite code), and choose a password. On their other
   computers and phones they sign in with their email and that password.

Forgot password: the sign-in screen's "Forgot password?" mails a reset code
(needs email set up). Without email, give them a new invite code instead.

## Email (invite and password-reset mails)

Put a `mail.json` in the data folder
(`~/Library/Application Support/WorkboardServer/`), readable only by you:

    {
      "transport": "smtp",
      "smtp_host": "smtp.gmail.com",
      "smtp_port": 587,
      "username": "team@example.com",
      "password": "an app password",
      "from": "Workboard <team@example.com>",
      "public_address": "mac-mini.your-tailnet.ts.net:8787"
    }

    chmod 600 ~/Library/Application\ Support/WorkboardServer/mail.json
    ~/.local/bin/workboard-server mail-test you@example.com

For Gmail / Google Workspace use an app password (Google Account > Security >
App passwords). Port 465 uses TLS, anything else STARTTLS. `public_address`
is what links in mails point at; without it the address the inviter's app
used. The server reads the file each time it sends, so no restart is needed.
`"transport": "folder"` writes mails to `outbox/` instead, for trying it out.

Owners and admins can make someone admin, or Remove access (signs out every
computer of theirs at once) in Work together > People.

## On the Mac mini

    ~/.local/bin/workboard-server people              who has access (and their account email)
    ~/.local/bin/workboard-server mail-test ADDRESS   check mail.json by sending a test mail
    ~/.local/bin/workboard-server backups             list backups
    ~/.local/bin/workboard-server restore FILE        go back to a backup
                                                      (the owner can also do this in the app:
                                                       Work together > Backups)
    ~/.local/bin/workboard-server invite ID --role owner
                                                      new owner code (owner lost their computer)

Backups: `backups/` in the data folder, hourly (48 kept) and daily (30 kept).
Health: `curl http://<tailscale address>:8787/health`

## App updates

The server also hands out Workboard updates to the apps signed in to it
(`updates/` in the data folder, served at `/updates/<file>`). How to publish
a version: [docs/release/README.md](../docs/release/README.md).
