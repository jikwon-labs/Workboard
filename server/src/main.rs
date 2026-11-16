// workboard-server [--bind ADDRESS] [--port PORT] [--home FOLDER]
// workboard-server people | backups | invite MEMBER [--role admin|owner] | restore FILE | mail-test ADDRESS   [--home FOLDER]
//
// Listens on this Mac's Tailscale address and 127.0.0.1 by default (only
// 127.0.0.1 when Tailscale isn't running), port 8787. 127.0.0.1 is where
// Cloudflare Tunnel's connector (cloudflared) on the same Mac reaches it.
// It never listens on every network (0.0.0.0): pick specific addresses.

use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

const USAGE: &str = "usage:
  workboard-server [--bind ADDRESS] [--port PORT] [--home FOLDER]
      run the server
      --bind   addresses to listen on, separated by commas
               (default: this Mac's Tailscale address and 127.0.0.1)
      --port   port (default 8787)
      --home   folder that keeps the team's copy
               (default: $WORKBOARD_SERVER_HOME, else ~/Library/Application Support/WorkboardServer)

  workboard-server people                       who is in the workspace, their role and computers
  workboard-server backups                      list the backups
  workboard-server restore FILE                 go back to a backup (a file from `backups`)
  workboard-server invite MEMBER [--role R]     a one-time invite code for member id MEMBER
                                                (R: member, admin or owner; for when the owner lost access)
  workboard-server mail-test ADDRESS            send a test mail with the settings in mail.json";

fn main() {
    let mut bind: Option<String> = std::env::var("WORKBOARD_SERVER_BIND")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let mut port: u16 = wb_net::DEFAULT_PORT;
    let mut home: Option<PathBuf> = None;
    let mut role: Option<String> = None;
    let mut words: Vec<String> = vec![];
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned();
        match (args[i].as_str(), val) {
            ("--bind", Some(v)) => bind = Some(v),
            ("--port", Some(v)) => {
                port = v.parse().unwrap_or_else(|_| fail("--port needs a number"))
            }
            ("--home", Some(v)) => home = Some(PathBuf::from(v)),
            ("--role", Some(v)) => role = Some(v),
            ("-h" | "--help", _) => {
                println!("{}", USAGE);
                return;
            }
            (a, _) if !a.starts_with('-') => {
                words.push(a.to_string());
                i += 1;
                continue;
            }
            _ => fail(USAGE),
        }
        i += 2;
    }
    let home = home.unwrap_or_else(workboard_server::home_dir);
    match words
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => serve(bind, port, home),
        ["people"] => people(&home),
        ["backups"] => backups(&home),
        ["restore", file] => {
            let path = std::path::Path::new(file);
            let path = if path.exists() {
                path.to_path_buf()
            } else {
                home.join(workboard_server::BACKUP_DIR).join(file)
            };
            if !path.is_file() {
                fail(&format!(
                    "{} isn't a file (see `workboard-server backups`)",
                    file
                ));
            }
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            request(&home, json!({"restore": path.to_string_lossy()}));
        }
        ["invite", member] => {
            let member: u64 = member.parse().unwrap_or_else(|_| {
                fail("MEMBER is a member id number (see `workboard-server people`)")
            });
            let role = role.unwrap_or_else(|| "member".into());
            if !["member", "admin", "owner"].contains(&role.as_str()) {
                fail("--role is member, admin or owner");
            }
            request(&home, json!({"invite": member, "role": role}));
        }
        ["mail-test", to] => mail_test(&home, to),
        _ => fail(USAGE),
    }
}

fn mail_test(home: &std::path::Path, to: &str) {
    let Some(cfg) = workboard_server::load_mail(home) else {
        fail(&format!(
            "No usable {} in {}. See the top of server/src/mail.rs for what goes in it.",
            workboard_server::MAIL_FILE,
            home.display()
        ));
    };
    let mail = workboard_server::mail::Mail { to: to.to_string(), subject: "Workboard test mail".into(), body: "This is a test from workboard-server. Invite codes and password-reset codes will come like this.\n".into() };
    match workboard_server::mail::send(home, &cfg, &mail) {
        Ok(()) if cfg.transport == "folder" => println!(
            "Written to {}",
            home.join(workboard_server::OUTBOX_DIR).display()
        ),
        Ok(()) => println!("Sent to {}.", to),
        Err(e) => fail(&e),
    }
}

fn serve(bind: Option<String>, port: u16, home: PathBuf) {
    let bind = bind.unwrap_or_else(|| {
        let ts = workboard_server::default_bind();
        if ts == "127.0.0.1" {
            ts
        } else {
            format!("{},127.0.0.1", ts)
        }
    });
    let mut addrs: Vec<SocketAddr> = vec![];
    for b in bind.split(',').map(str::trim).filter(|b| !b.is_empty()) {
        let ip: IpAddr = b
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .unwrap_or_else(|_| {
                fail("--bind needs IP addresses, like 100.101.102.103 or 127.0.0.1")
            });
        if ip.is_unspecified() {
            fail("Listening on every network (0.0.0.0) isn't allowed. Use the Tailscale address (tailscale ip -4) or 127.0.0.1.");
        }
        let a = SocketAddr::new(ip, port);
        if !addrs.contains(&a) {
            addrs.push(a);
        }
    }
    if addrs.is_empty() {
        fail("--bind needs IP addresses, like 100.101.102.103 or 127.0.0.1");
    }
    let rt = tokio::runtime::Runtime::new().expect("start runtime");
    rt.block_on(async move {
        let mut listeners = vec![];
        for addr in &addrs {
            listeners.push(
                tokio::net::TcpListener::bind(addr)
                    .await
                    .unwrap_or_else(|e| fail(&format!("couldn't listen on {}: {}", addr, e))),
            );
        }
        let shown: Vec<String> = addrs
            .iter()
            .map(|a| format!("ws://{}{}", a, wb_net::PATH))
            .collect();
        println!(
            "workboard-server listening on {}  (data: {})",
            shown.join(" and "),
            home.display()
        );
        let res = workboard_server::serve_on(
            listeners,
            home,
            workboard_server::Options::default(),
            async {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("signal");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
                println!("stopping");
            },
        )
        .await;
        if let Err(e) = res {
            fail(&e.to_string());
        }
    });
}

/// Hand a request to the running server (or the next start).
fn request(home: &std::path::Path, req: serde_json::Value) {
    match workboard_server::admin_request(home, &req, Duration::from_secs(5)) {
        Ok(Some(res)) => {
            println!(
                "{}",
                res.get("message").and_then(|m| m.as_str()).unwrap_or("")
            );
            if res.get("ok").and_then(|o| o.as_bool()) != Some(true) {
                std::process::exit(1);
            }
        }
        Ok(None) => println!(
            "The server isn't running. It will do this when it starts; the answer will be in {}.",
            home.join(workboard_server::RESULT_FILE).display()
        ),
        Err(e) => fail(&e.to_string()),
    }
}

fn people(home: &std::path::Path) {
    let saved = workboard_server::load_saved(home).unwrap_or_else(|e| fail(&e.to_string()));
    let canon: serde_json::Value =
        std::fs::read_to_string(home.join(workboard_server::WORKSPACE_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let members = canon
        .get("members")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    if members.is_empty() {
        println!("No workspace yet. The first app that connects sets it up and becomes the owner.");
        return;
    }
    println!(
        "{:<22} {:<28} {:<7} {:>9}  {:<30} {}",
        "id", "name", "role", "computers", "account", "open invite"
    );
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    for m in members {
        let id = m.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
        let name = m.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let devices = saved.devices.values().filter(|d| d.member == id).count();
        let invite = saved
            .open_invite(id, now_ms)
            .map(|c| c.as_str())
            .unwrap_or("");
        let account = saved
            .accounts
            .get(&id)
            .map(|a| a.email.as_str())
            .unwrap_or("-");
        println!(
            "{:<22} {:<28} {:<7} {:>9}  {:<30} {}",
            id,
            name,
            saved.role_of(id).label(),
            devices,
            account,
            invite
        );
    }
}

fn backups(home: &std::path::Path) {
    let dir = home.join(workboard_server::BACKUP_DIR);
    let mut any = false;
    for kind in ["daily", "hourly", "before-restore"] {
        for p in workboard_server::backup_files(&dir, kind) {
            any = true;
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            println!(
                "{:<34} {:>8} KB",
                p.file_name().unwrap_or_default().to_string_lossy(),
                size.div_ceil(1024)
            );
        }
    }
    if !any {
        println!("No backups yet (the first one is made within a minute of the server having a workspace).");
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("{}", msg);
    std::process::exit(2);
}
