// The words for the connection: which server, whether it can be reached,
// and what went wrong.

use super::*;

/// What the team server (or its client) said, in the interface language:
/// the server sends English, and some of it carries a number inside.
pub(crate) fn server_text(s: &str) -> String {
    if s.starts_with("Couldn't reach the server: ") {
        // the system's own words follow, untranslated
        return tr("Couldn't reach the server. Check the address and your connection.").into();
    }
    const WITH_VALUES: [&str; 5] = [
        "Choose a password of at least {} characters.",
        "This server already keeps {} workspaces, the most it can.",
        "Too many wrong tries. Wait {} minutes and try again, or use Forgot password.",
        "That reset code doesn't work. It may be used already or expired (codes last {} minutes). Ask for a new one.",
        "The server answered {}.",
    ];
    for pattern in WITH_VALUES {
        if let Some(values) = super::files::fill_back(pattern, s) {
            return crate::i18n::fill(tr(pattern), &values);
        }
    }
    tr(s).to_string()
}

/// The colour of a connection that isn't working.
pub(crate) const WARN: Color32 = Color32::from_rgb(0xe1, 0x6b, 0x16);

/// Failed tries in a row after which "Reconnecting" becomes "Can't reach
/// the server" (the app keeps trying either way).
pub(crate) const FAILS_BEFORE_CANT_REACH: u32 = 3;

/// Why the server can't be reached, in plain words: the connection code
/// says it in English, with the system's own error inside.
pub(crate) fn reach_text(why: &str) -> String {
    use super::files::fill_back;
    if let Some(v) = fill_back("Can't reach {}: {}", why) {
        return if v[1].to_lowercase().contains("refused") {
            trf!(
                "Nothing answers at {}. Check that workboard-server is running there.",
                v[0]
            )
        } else {
            trf!(
                "Can't reach {}. Check the address and that this computer is on the team's network.",
                v[0]
            )
        };
    }
    if let Some(v) = fill_back("Can't find {}: {}", why).or_else(|| fill_back("Can't find {}", why))
    {
        return trf!("Can't find {}. Check the server address.", v[0]);
    }
    if let Some(v) = fill_back("The secure connection to {} failed: {}", why) {
        return trf!(
            "Couldn't make a secure connection to {}. Check the address: the certificate there isn't one this computer trusts for it.",
            v[0]
        );
    }
    if why.starts_with("The server didn't answer like Workboard") {
        return tr("Something answered at this address, but not a Workboard server. Check the address and port.").into();
    }
    let said = server_text(why);
    // what the connection library or the system said, untranslated
    let raw = why.contains("os error")
        || why.starts_with("IO error")
        || (crate::i18n::is_ko() && said == why);
    if raw {
        tr("The connection to the server broke.").into()
    } else {
        said
    }
}

/// How the server connection is doing, in a word or two. `fails`: failed
/// tries in a row.
pub(crate) fn net_status_text(net: bool, s: Option<&NetStatus>, fails: u32) -> String {
    match (net, s) {
        (false, _) => tr("Disconnected").into(),
        (true, None) | (true, Some(NetStatus::Connecting)) => tr("Connecting\u{2026}").into(),
        (true, Some(NetStatus::Connected)) => tr("Connected").into(),
        (true, Some(NetStatus::Reconnecting(_))) if fails >= FAILS_BEFORE_CANT_REACH => {
            tr("Can't reach the server").into()
        }
        (true, Some(NetStatus::Reconnecting(_))) => tr("Reconnecting\u{2026}").into(),
        (true, Some(NetStatus::Refused(_))) => tr("Not signed in").into(),
        (true, Some(NetStatus::Gone(_))) => tr("Deleted from the server").into(),
    }
}

/// Why the connection isn't working, when it isn't.
pub(crate) fn net_problem(s: Option<&NetStatus>) -> Option<String> {
    match s {
        Some(NetStatus::Reconnecting(why)) => {
            Some(trf!("{} Trying again every few seconds.", reach_text(why)))
        }
        Some(NetStatus::Refused(why) | NetStatus::Gone(why)) => Some(server_text(why)),
        _ => None,
    }
}
