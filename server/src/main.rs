// workboard-server [--bind ADDRESS] [--port PORT] [--home FOLDER]
//
// Listens on this Mac's Tailscale address by default (127.0.0.1 when
// Tailscale isn't running), port 8787. It never listens on every network
// (0.0.0.0): pick a specific address.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

const USAGE: &str = "usage: workboard-server [--bind ADDRESS] [--port PORT] [--home FOLDER]

  --bind   address to listen on (default: this Mac's Tailscale address, else 127.0.0.1)
  --port   port (default 8787)
  --home   folder that keeps the team's copy
           (default: $WORKBOARD_SERVER_HOME, else ~/Library/Application Support/WorkboardServer)";

fn main() {
    let mut bind: Option<String> = std::env::var("WORKBOARD_SERVER_BIND").ok().filter(|s| !s.trim().is_empty());
    let mut port: u16 = wb_net::DEFAULT_PORT;
    let mut home: Option<PathBuf> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned();
        match (args[i].as_str(), val) {
            ("--bind", Some(v)) => bind = Some(v),
            ("--port", Some(v)) => port = v.parse().unwrap_or_else(|_| fail("--port needs a number")),
            ("--home", Some(v)) => home = Some(PathBuf::from(v)),
            ("-h" | "--help", _) => {
                println!("{}", USAGE);
                return;
            }
            _ => fail(USAGE),
        }
        i += 2;
    }
    let bind = bind.unwrap_or_else(workboard_server::default_bind);
    let ip: IpAddr = bind.trim().trim_start_matches('[').trim_end_matches(']').parse().unwrap_or_else(|_| fail("--bind needs an IP address, like 100.101.102.103 or 127.0.0.1"));
    if ip.is_unspecified() {
        fail("Listening on every network (0.0.0.0) isn't allowed. Use the Tailscale address (tailscale ip -4) or 127.0.0.1.");
    }
    let addr = SocketAddr::new(ip, port);
    let home = home.unwrap_or_else(workboard_server::home_dir);
    let rt = tokio::runtime::Runtime::new().expect("start runtime");
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| fail(&format!("couldn't listen on {}: {}", addr, e)));
        println!("workboard-server listening on ws://{}{}  (data: {})", addr, wb_net::PATH, home.display());
        let res = workboard_server::serve(listener, home, async {
            let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            println!("stopping");
        })
        .await;
        if let Err(e) = res {
            fail(&e.to_string());
        }
    });
}

fn fail(msg: &str) -> ! {
    eprintln!("{}", msg);
    std::process::exit(2);
}
