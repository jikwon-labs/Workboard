# Running workboard-server on the Mac mini

Next steps on the Mac mini (accounts and invite mail): `docs/macmini/README.md`.
Design and current state: `docs/server-design.md`. Testing (also from another
Mac): `docs/testing.md`. App releases: `docs/release/README.md`.

## Start automatically

    deploy/install-launchd.sh

Builds the server, copies it to `~/.local/bin/workboard-server` and installs
the LaunchAgent `com.jikwon-labs.workboard-server`: it starts when this user
logs in and restarts within 10 seconds if it stops. It listens on
127.0.0.1 port 8787 only, where Cloudflare Tunnel (below) reaches it
(`PORT=9000 deploy/install-launchd.sh` for another port). Run the script
again after pulling new code.

- Status: `launchctl print gui/$(id -u)/com.jikwon-labs.workboard-server | grep state`
- Log: `~/Library/Logs/workboard-server.log`
- Data: `~/Library/Application Support/WorkboardServer/` (`workspace.json`, `server.json`)
- Stop and remove: `deploy/install-launchd.sh uninstall`

## Cloudflare Tunnel (how the team reaches the server)

    brew install cloudflared
    TUNNEL_TOKEN='<token>' deploy/install-cloudflared.sh

The team reaches the server at an https address (https://wb.wlrnjs.xyz)
through Cloudflare Tunnel; cloudflared runs as the LaunchAgent
`com.jikwon-labs.cloudflared` and passes requests to 127.0.0.1:8787. The
token comes from the Cloudflare dashboard and is kept in the data folder
(`cloudflared-token`, mode 600). Log: `~/Library/Logs/cloudflared.log`.
Stop and remove: `deploy/install-cloudflared.sh uninstall`.
The whole setup, in order (Cloudflare, the domain, LiveKit Cloud, the apps):
[docs/cloudflare-tunnel.md](../docs/cloudflare-tunnel.md).

## Voice huddles (LiveKit Cloud)

Huddles can't go through the tunnel (UDP), so they use LiveKit Cloud:

    LIVEKIT_URL='wss://<project>.livekit.cloud' LIVEKIT_KEY='<key>' LIVEKIT_SECRET='<secret>' deploy/use-livekit-cloud.sh

It writes `livekit.json` (mode 600), which workboard-server reads for each
huddle to hand out call tokens, and removes a local livekit-server left from
before.

## SSH (admin access through Cloudflare)

SSH to the Mac mini, and with it `scripts/publish-update.sh` (rsync), goes
through the same tunnel, behind Cloudflare Access so only you can reach it.
Remote Login stays on on the Mac mini (System Settings > General > Sharing).

1. Cloudflare Zero Trust (<https://one.dash.cloudflare.com>): pick a team
   name and the Free plan (it may ask for a payment method).
2. Tunnel `workboard` > Routes > Add route > Published application:
   subdomain `ssh`, domain `wlrnjs.xyz`, Service URL `ssh://localhost:22`.
3. **Before using it**, Access > Applications > Add > Self-hosted: domain
   `ssh.wlrnjs.xyz`, a policy that allows only your email. Without this the
   Mac mini's SSH is open to the whole internet.
4. On the MacBook: `brew install cloudflared`, then in `~/.ssh/config`:

        Host macmini
          HostName ssh.wlrnjs.xyz
          User seojigwon
          ProxyCommand /opt/homebrew/bin/cloudflared access ssh --hostname %h

5. `ssh macmini hostname` opens a browser once for the Access login, then
   answers with the Mac mini's name. `ssh seojigwon@macmini` and
   `UPDATES_DEST='seojigwon@macmini:...'` keep working unchanged.

## iPhone notifications (APNs)

The server pushes new DMs, @mentions and thread replies to iPhones that are
not connected, once Apple's push service is set up: put the APNs key (`.p8`)
and an `apns.json` (`key`, `key_id`, `team_id`, `topic` = the iPhone app's
bundle id) in the data folder, both mode 600, and restart the server
(`launchctl kickstart -k gui/$(id -u)/com.jikwon-labs.workboard-server`).
The log says `iPhone notifications on` or `off: <why>` once at start.
Details: the top of `server/src/push.rs`; steps: `docs/macmini/README.md`.

## Keep the Mac mini reachable

Run once in Terminal on the Mac mini (asks for your password):

    deploy/setup-power.sh

It turns off sleep and disk sleep, keeps wake-for-network on, and makes the
Mac mini power on by itself when electricity comes back (`autorestart`).

This Mac mini keeps FileVault on (decided 2026-11-15), so macOS won't log in
automatically: after a power cut it powers on and waits at the FileVault
screen until someone types the password; then workboard-server and
cloudflared start by themselves. For a planned restart without being at
the keyboard, use `sudo fdesetup authrestart`. (With FileVault off the same
script also turns on automatic login.)

## After a restart

    deploy/check-after-boot.sh            check that everything is back
    deploy/check-after-boot.sh --install  check once, 3 minutes after the next login

Results go to `~/Library/Logs/workboard-boot-check.log` (each LaunchAgent,
workboard /health on 127.0.0.1 and through the tunnel, Docker containers,
the ngrok tunnel, the camera). The `--install` form runs a copy from `~/.local/bin`
(background jobs can't read `~/Documents`) and removes itself after one run.

## Setting up and inviting

1. The owner (whoever sets the team up) opens Workboard on their own Mac:
   the first-start window's "Set up a server for my team" (or Work together >
   "Or connect to your team's server" > New server), types the server's
   https address (https://wb.wlrnjs.xyz), their email and a password. Their
   workspace goes up and they become the owner.
2. In Workboard, add teammates with Invite (their email). With email set up
   (below) the server mails each of them their invite code and a link;
   otherwise copy the code from Work together > People and send it yourself
   with the address https://wb.wlrnjs.xyz (it works once, for 7 days).
3. They install Workboard, open the link (or "Join my team" > server address >
   Invite code), and choose a password. On their other computers and phones
   they sign in with their email and that password.

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
      "public_address": "https://wb.wlrnjs.xyz"
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
Health: `curl http://127.0.0.1:8787/health` on the Mac mini (everything), or
`curl https://wb.wlrnjs.xyz/health` from anywhere (only `{"ok":true}`)

## App updates

The server also hands out Workboard updates to the apps signed in to it
(`updates/` in the data folder, served at `/updates/<file>`). How to publish
a version: [docs/release/README.md](../docs/release/README.md).
