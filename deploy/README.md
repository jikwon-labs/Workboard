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
- Data: `~/Library/Application Support/WorkboardServer/` (`workspace.json`, `server.json`, `mine.json`)
- Stop and remove: `deploy/install-launchd.sh uninstall`

## Follow main by itself

    deploy/install-autoupdate.sh

Once this is set up, nobody runs `git pull && deploy/install-launchd.sh` by
hand: within about 5 minutes of a push to main that changes the server, the
Mac mini builds it, runs the server, sync and network tests, installs it on
the same address and port and checks `/health`.

- It keeps its own checkout in `~/.local/src/Workboard` (background jobs
  can't read `~/Documents`). Nobody edits it; updates stop if someone does.
  The first run builds from that new checkout and installs the server from
  main once, even when the code is the same (a few seconds' restart).
- The LaunchAgent `com.jikwon-labs.workboard-autoupdate` runs
  `deploy/auto-update.sh` from that checkout every 5 minutes. Pushes that
  don't change the server binary (app, docs) only log `server unchanged`.
- Failing tests leave the running server alone. A new server that doesn't
  answer `/health` within 60 seconds is replaced by the previous binary.
  Either way that build isn't tried again; the next change is. A change to
  the server's or the sync crates' tests alone counts as a change, so a fix
  to a test that failed only now and then gets the waiting server installed.
- The Mac mini has to fetch the repository without a prompt (try
  `git -C ~/.local/src/Workboard fetch` after setting up).
- Log: `~/Library/Logs/workboard-autoupdate.log` (one line per change on
  main, plus failures)
- Run once now: `~/.local/src/Workboard/deploy/auto-update.sh --now`
- Stop: `deploy/install-autoupdate.sh uninstall`
- Tests for the script: `deploy/test-auto-update.sh`

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

1. Cloudflare Zero Trust (<https://one.dash.cloudflare.com>): the Free plan
   (up to 50 users, $0) still asks for a payment method; then a team name.
2. **First** Zero Trust > Access controls > Applications > Create new
   application > Self-hosted and private > Public DNS: subdomain `ssh`,
   domain `wlrnjs.xyz`, and a new policy: Allow, Include > Emails > your
   email. Do this before the route: a route without it opens the Mac mini's
   SSH to the whole internet.
3. Then Networking > Tunnels > `workboard` > Add route > Published
   application: subdomain `ssh`, domain `wlrnjs.xyz`, Service URL
   `ssh://localhost:22`. The DNS record is made for you. Check:
   `curl -sI https://ssh.wlrnjs.xyz` answers 302 to `...cloudflareaccess.com`.
4. On the MacBook: `brew install cloudflared`, then in `~/.ssh/config`:

        Host macmini
          HostName ssh.wlrnjs.xyz
          User seojigwon
          HostKeyAlias macmini
          ProxyCommand /opt/homebrew/bin/cloudflared access ssh --hostname %h

   `HostKeyAlias macmini` checks the Mac mini's key under the name it was
   saved with before (Tailscale days), so there is no "unknown host" prompt
   and a different machine behind the name would be refused.
5. `cloudflared access login https://ssh.wlrnjs.xyz` opens a browser for the
   Access login (a one-time code mailed to that email), then
   `ssh macmini hostname` answers with the Mac mini's name.
   `ssh seojigwon@macmini` and `UPDATES_DEST='seojigwon@macmini:...'` keep
   working unchanged. The login lasts for the Access session (24 hours by
   default); when it runs out ssh opens the browser again, so run
   `ssh macmini true` before `scripts/publish-update.sh`.

## iPhone notifications (APNs)

The server pushes new DMs, @mentions and thread replies to iPhones that are
not connected, once Apple's push service is set up: put the APNs key (`.p8`)
and an `apns.json` (`key`, `key_id`, `team_id`, `topic` = the iPhone app's
bundle id) in the data folder, both mode 600, and restart the server
(`launchctl kickstart -k gui/$(id -u)/com.jikwon-labs.workboard-server`).
The log says `iPhone notifications on` or `off: <why>` once at start.
Details: the top of `server/src/push.rs`; steps: `docs/macmini/README.md`.

## AI (summaries, writing help, questions about the workspace)

The apps' AI tools ask Anthropic's Claude through the server, so the API key
stays on the Mac mini. Put an `ai.json` in the data folder, readable only by
you, and restart the server:

    {
      "api_key": "sk-ant-...",
      "model": "claude-opus-5-5",
      "per_hour": 60
    }

    chmod 600 ~/Library/Application\ Support/WorkboardServer/ai.json
    launchctl kickstart -k gui/$(id -u)/com.jikwon-labs.workboard-server

Only `api_key` is needed (an API key from console.anthropic.com > API keys);
`model` defaults to `claude-opus-5-5`, `per_hour` (requests one person may
make in an hour, per workspace) to 60. `base_url` points at a proxy instead of
`https://api.anthropic.com` (plain `http://` only to this computer). The log
says `AI tools on (<model>)` or `AI tools off: <why>` once at start, then one
line per request with what it used (`AI: question for member 3: 12000 tokens
in (8000 from the cache), 400 out`); nothing of the questions or answers is
kept. Every request is billed to the key's Anthropic account. Without the file
the apps show no AI tools. Details: the top of `server/src/ai.rs`.

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
    ~/.local/bin/workboard-server invite-email EMAIL   invite a new teammate as Member only
    ~/.local/bin/workboard-server invite ID --role owner
                                                      Owner recovery only (existing member ID)

`invite-email EMAIL` creates a new plain Member, adds them to `general` when
present, and prints a one-time code (7 days). It does not send mail: give the
code and server address to that teammate privately. It uses the local admin
request, so connected apps receive the change and the server saves it; no
workspace-file edits or restart are needed. If the server is stopped, the
request waits until startup; read `admin-result.json` in the server home for
the answer. Run admin commands one at a time (the local request has one slot).
Use `--home FOLDER` for a separate server home.

Addresses are compared without case or surrounding spaces and new addresses
are stored lowercase. Use a plain ASCII mailbox, not a display name. A pending,
unregistered Member can be re-invited with the same ID and a fresh code; the
previous code stops working. Duplicate emails/IDs, Owner/Admin/Guest identities,
any existing account (including its login email), and members with signed-in
computers are refused without changing their data, roles or credentials.
`--role` is not allowed: this command cannot grant Owner or Admin.

**Do not use `invite ID --role owner` for teammates.** That existing command
recovers access to the named existing Owner identity; its code signs in as
that person. Existing registered members should sign in or use Forgot password,
not `invite-email` to reset their account.

Backups: `backups/` in the data folder, hourly (48 kept) and daily (30 kept).
Health: `curl http://127.0.0.1:8787/health` on the Mac mini (everything), or
`curl https://wb.wlrnjs.xyz/health` from anywhere (only `{"ok":true}`)

## App updates

The server also hands out Workboard updates to the apps signed in to it
(`updates/` in the data folder, served at `/updates/<file>`). How to publish
a version: [docs/release/README.md](../docs/release/README.md).
