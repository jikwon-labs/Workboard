// Email from the server: invite codes and password-reset codes.
//
// Set up by a mail.json in the server folder:
//
//   {
//     "transport": "smtp",                  (or "folder": write each mail to
//                                            outbox/ instead, for trying it out)
//     "smtp_host": "smtp.gmail.com",
//     "smtp_port": 587,                     (587 STARTTLS, 465 TLS)
//     "username": "team@example.com",
//     "password": "app password",
//     "from": "Workboard <team@example.com>",
//     "public_address": "mac-mini.tail1.ts.net:8787"   (optional: the address
//                                            people use in links; else the
//                                            address the inviter connected to)
//   }
//
// Without mail.json nothing is emailed: invite codes are handed over by hand
// as before, and "Forgot password" says to ask for a new invite code.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAIL_FILE: &str = "mail.json";
pub const OUTBOX_DIR: &str = "outbox";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct MailCfg {
    pub transport: String,
    pub smtp_host: String,
    pub smtp_port: Option<u16>,
    pub username: String,
    pub password: String,
    pub from: String,
    pub public_address: Option<String>,
}

pub fn load_mail(home: &Path) -> Option<MailCfg> {
    let cfg: MailCfg =
        serde_json::from_str(&std::fs::read_to_string(home.join(MAIL_FILE)).ok()?).ok()?;
    match cfg.transport.as_str() {
        "folder" => Some(cfg),
        "smtp" | "" if !cfg.smtp_host.trim().is_empty() && !cfg.from.trim().is_empty() => Some(cfg),
        _ => None,
    }
}

pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// Send one mail. Blocks (up to about 30 seconds for SMTP).
pub fn send(home: &Path, cfg: &MailCfg, mail: &Mail) -> Result<(), String> {
    if cfg.transport == "folder" {
        let dir = home.join(OUTBOX_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let name = format!(
            "{}-{}.txt",
            chrono::Local::now().format("%Y%m%d-%H%M%S%.3f"),
            mail.to.replace(
                |c: char| !c.is_ascii_alphanumeric() && c != '@' && c != '.',
                "_"
            )
        );
        let text = format!(
            "To: {}\nSubject: {}\n\n{}",
            mail.to, mail.subject, mail.body
        );
        return std::fs::write(dir.join(name), text).map_err(|e| e.to_string());
    }
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{Message, SmtpTransport, Transport};
    let msg = Message::builder()
        .from(
            cfg.from
                .parse()
                .map_err(|e| format!("mail.json \"from\" isn't an address: {}", e))?,
        )
        .to(mail
            .to
            .parse()
            .map_err(|e| format!("{} isn't an email address: {}", mail.to, e))?)
        .subject(mail.subject.clone())
        .header(ContentType::TEXT_PLAIN)
        .body(mail.body.clone())
        .map_err(|e| e.to_string())?;
    let port = cfg.smtp_port.unwrap_or(587);
    let builder = if port == 465 {
        SmtpTransport::relay(&cfg.smtp_host)
    } else {
        SmtpTransport::starttls_relay(&cfg.smtp_host)
    }
    .map_err(|e| e.to_string())?;
    let mut builder = builder
        .port(port)
        .timeout(Some(std::time::Duration::from_secs(20)));
    if !cfg.username.is_empty() {
        builder = builder.credentials(Credentials::new(cfg.username.clone(), cfg.password.clone()));
    }
    builder
        .build()
        .send(&msg)
        .map(|_| ())
        .map_err(|e| format!("The mail server said: {}", e))
}

/// The address links in mails point at.
pub fn link_address(cfg: &MailCfg, connected_to: Option<&str>) -> Option<String> {
    cfg.public_address
        .clone()
        .filter(|a| !a.trim().is_empty())
        .or_else(|| connected_to.map(|s| s.to_string()))
}

pub fn invite_mail(
    to: &str,
    workspace: &str,
    inviter: &str,
    code: &str,
    server: Option<&str>,
    days: i64,
) -> Mail {
    let ws = if workspace.trim().is_empty() {
        "your team's workspace".to_string()
    } else {
        workspace.trim().to_string()
    };
    let mut body = format!(
        "{} invited you to {} on Workboard.\n\n",
        if inviter.trim().is_empty() {
            "Your team"
        } else {
            inviter.trim()
        },
        ws
    );
    if let Some(s) = server {
        body.push_str(&format!(
            "Open this link on a computer or phone that can reach your team's server (Tailscale on):\n\n  http://{}/join?code={}&email={}\n\nOr open Workboard, choose \"Join my team\" and type:\n\n  Server:       {}\n  Invite code:  {}\n",
            s,
            code,
            pct(to),
            s,
            code
        ));
    } else {
        body.push_str(&format!("Open Workboard, choose \"Join my team\", type your team's server address and this invite code:\n\n  {}\n", code));
    }
    body.push_str(&format!(
        "\nThe code works once, for {} days. When you join you choose a password; after that you sign in on your other computers and phones with {} and that password.\n",
        days, to
    ));
    Mail {
        to: to.to_string(),
        subject: format!("You're invited to {} on Workboard", ws),
        body,
    }
}

pub fn reset_mail(to: &str, code: &str, minutes: i64) -> Mail {
    Mail {
        to: to.to_string(),
        subject: "Your Workboard password reset code".into(),
        body: format!(
            "Someone asked to reset the Workboard password for {}.\n\nYour code:  {}\n\nIn Workboard, under \"Forgot password\", type this code and a new password. The code works once, for {} minutes.\n\nIf it wasn't you, ignore this mail: your password stays the same.\n",
            to, code, minutes
        ),
    }
}

fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
}
