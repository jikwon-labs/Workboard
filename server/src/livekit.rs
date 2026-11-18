// Voice and video: LiveKit settings and the tokens that let a member
// join a call.

use super::*;

// ------------------------------------------------------------------ voice

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LiveKitCfg {
    /// What apps connect to, e.g. wss://<project>.livekit.cloud
    pub url: String,
    pub key: String,
    pub secret: String,
}

pub fn load_livekit(home: &Path) -> Option<LiveKitCfg> {
    serde_json::from_str(&std::fs::read_to_string(home.join(LIVEKIT_FILE)).ok()?).ok()
}

pub(crate) fn b64url(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..(c.len() + 1) {
            out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// A LiveKit access token (a JWT signed with HS256) that lets `identity`
/// join `room` to talk and listen, for CALL_TOKEN_SECS.
pub fn call_token(
    cfg: &LiveKitCfg,
    room: &str,
    identity: &str,
    name: &str,
    now_secs: i64,
) -> String {
    use hmac::{KeyInit, Mac};
    let header = b64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = json!({
        "iss": cfg.key, "sub": identity, "name": name,
        "nbf": now_secs - 10, "exp": now_secs + CALL_TOKEN_SECS,
        "video": {"room": room, "roomJoin": true, "canPublish": true, "canSubscribe": true, "canPublishData": true},
    });
    let body = b64url(claims.to_string().as_bytes());
    let mut mac =
        hmac::Hmac::<sha2::Sha256>::new_from_slice(cfg.secret.as_bytes()).expect("any key length");
    mac.update(format!("{}.{}", header, body).as_bytes());
    format!(
        "{}.{}.{}",
        header,
        body,
        b64url(&mac.finalize().into_bytes())
    )
}

/// The huddle room of a channel or DM.
pub fn room_name(channel: u64) -> String {
    format!("wb-{}", channel)
}

impl Hub {
    /// A token for the huddle of `channel`, for someone allowed in it.
    pub(crate) fn call(&self, member: u64, device: &str, channel: u64) -> Result<Msg, String> {
        let cfg = load_livekit(self.setup_home(LIVEKIT_FILE))
            .ok_or("Voice huddles aren't set up on this server yet.")?;
        let g = self.state();
        let canon = g
            .canon
            .as_ref()
            .ok_or("There's no workspace on the server yet.")?;
        let ch = canon
            .get("channels")
            .and_then(|c| c.as_array())
            .and_then(|a| {
                a.iter()
                    .find(|c| c.get("id").and_then(|i| i.as_u64()) == Some(channel))
            })
            .ok_or("That conversation isn't on the server yet. Try again in a moment.")?;
        // the same rule as seeing it: a public channel is everyone's, a DM
        // or private channel only its members'
        if wb_sync::closed_to(ch, member) {
            return Err("You're not in that conversation.".into());
        }
        // as written (member_field trims it)
        let name = member_record(Some(canon), member)
            .and_then(|m| m.get("name")?.as_str())
            .unwrap_or("")
            .to_string();
        let token = call_token(
            &cfg,
            &self.room(channel),
            &wb_net::call_identity(member, device),
            &name,
            now_ms() / 1000,
        );
        Ok(Msg::CallToken {
            channel,
            url: cfg.url,
            token,
        })
    }
}
