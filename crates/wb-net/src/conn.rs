// The byte stream under the sync connection and the /files requests: plain
// TCP for ws:// and http:// addresses (a server on the LAN or a tailnet),
// TLS on top of it for wss:// and https:// (a server behind Cloudflare
// Tunnel or another HTTPS front). Certificates are checked against the
// roots macOS trusts (the system keychain).

use rustls::pki_types::{CertificateDer, ServerName};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub enum Conn {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Conn {
    /// The TCP connection underneath, for timeouts.
    pub fn tcp(&self) -> &TcpStream {
        match self {
            Conn::Plain(s) => s,
            Conn::Tls(t) => t.get_ref(),
        }
    }
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.read(buf),
            // a front that closes without TLS's goodbye: the end, as with
            // plain TCP (what was read is checked by length and sha256)
            Conn::Tls(t) => match t.read(buf) {
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(0),
                r => r,
            },
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.write(buf),
            Conn::Tls(t) => t.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Plain(s) => s.flush(),
            Conn::Tls(t) => t.flush(),
        }
    }
}

/// Certificates trusted on top of the system's (tests trust their own).
static EXTRA_ROOTS: Mutex<Vec<CertificateDer<'static>>> = Mutex::new(Vec::new());

/// Trust `der` (a DER certificate) for secure connections from now on.
/// For tests that put a TLS front with their own certificate before a
/// server; the app itself only trusts what macOS trusts.
pub fn trust_certificate(der: Vec<u8>) {
    EXTRA_ROOTS.lock().unwrap().push(CertificateDer::from(der));
}

fn system_roots() -> &'static [CertificateDer<'static>] {
    static ROOTS: OnceLock<Vec<CertificateDer<'static>>> = OnceLock::new();
    ROOTS.get_or_init(|| rustls_native_certs::load_native_certs().certs)
}

fn tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    let mut roots = rustls::RootCertStore::empty();
    // a certificate the system has that this TLS library can't read is skipped
    let _ = roots.add_parsable_certificates(system_roots().iter().cloned());
    for c in EXTRA_ROOTS.lock().unwrap().iter() {
        roots.add(c.clone()).map_err(|e| e.to_string())?;
    }
    if roots.is_empty() {
        return Err("This computer has no trusted certificates to check the server with.".into());
    }
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| e.to_string())?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Arc::new(cfg))
}

/// Connect to `host`:`port`, trying each address it has, and start TLS
/// when `tls`. The handshake itself happens on the first read or write.
pub fn dial(host: &str, port: u16, tls: bool, timeout: Duration) -> Result<Conn, String> {
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("Can't find {}: {}", host, e))?
        .collect();
    let mut last = format!("Can't find {}", host);
    for sa in addrs {
        match TcpStream::connect_timeout(&sa, timeout) {
            Ok(s) => {
                let _ = s.set_nodelay(true);
                if !tls {
                    return Ok(Conn::Plain(s));
                }
                let name = ServerName::try_from(host.to_string())
                    .map_err(|_| format!("{} isn't a name a certificate can be for.", host))?;
                let c = rustls::ClientConnection::new(tls_config()?, name)
                    .map_err(|e| e.to_string())?;
                return Ok(Conn::Tls(Box::new(rustls::StreamOwned::new(c, s))));
            }
            Err(e) => last = format!("Can't reach {}:{}: {}", host, port, e),
        }
    }
    Err(last)
}
