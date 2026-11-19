// Link previews: the title, description and site name of the web page a
// chat message's first web address points to, from the page's <meta>
// tags (Open Graph, Twitter cards, description) or its <title>. Fetched
// once by the writer's app with the system's curl (on macOS, and on
// Windows 10 and later), at most a few seconds and the first 512 KB of the
// page, and kept on the message (`Message::preview`).

use crate::model::LinkPreview;

/// How much of a page is read for its <head>.
const MAX_BYTES: usize = 512 * 1024;
/// The longest title and description kept (characters).
const MAX_TITLE: usize = 200;
const MAX_DESC: usize = 300;

/// The preview of `url`, or None when the page can't be read or says
/// nothing worth showing. Blocks: run it on its own thread.
#[cfg(not(test))]
pub fn fetch(url: &str) -> Option<LinkPreview> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let out = crate::os::quiet_command("curl")
        .args([
            "-sL",
            "--compressed",
            "--max-time",
            "6",
            "--max-redirs",
            "5",
            "--max-filesize",
            "5000000",
            "-A",
            "Mozilla/5.0 (compatible; Workboard link preview)",
            "-H",
            "Accept: text/html,application/xhtml+xml",
            "--proto",
            "=http,https",
            "--",
            url,
        ])
        .output()
        .ok()?;
    let body = &out.stdout[..out.stdout.len().min(MAX_BYTES)];
    from_html(url, &String::from_utf8_lossy(body))
}

/// In tests no page is visited: example.com addresses get a made-up
/// preview, anything else none.
#[cfg(test)]
pub fn fetch(url: &str) -> Option<LinkPreview> {
    url.contains("example.com").then(|| LinkPreview {
        url: url.to_string(),
        title: "Example page".into(),
        description: "A page for tests".into(),
        site: "example.com".into(),
    })
}

/// The preview a page's HTML gives (see the top of this file).
pub fn from_html(url: &str, html: &str) -> Option<LinkPreview> {
    let head = match find_ci(html, "</head") {
        Some(i) => &html[..i],
        None => html,
    };
    let metas = meta_tags(head);
    let meta = |keys: &[&str]| {
        keys.iter().find_map(|k| {
            metas
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(k))
                .map(|(_, c)| c.clone())
                .filter(|c| !c.trim().is_empty())
        })
    };
    let title = meta(&["og:title", "twitter:title"]).or_else(|| title_tag(head));
    let description = meta(&["og:description", "twitter:description", "description"]);
    let site = meta(&["og:site_name"]).unwrap_or_else(|| host(url));
    let title = clean(&title.unwrap_or_default(), MAX_TITLE);
    let description = clean(&description.unwrap_or_default(), MAX_DESC);
    if title.is_empty() && description.is_empty() {
        return None;
    }
    Some(LinkPreview {
        url: url.to_string(),
        title,
        description,
        site: clean(&site, 80),
    })
}

/// The address's host without "www." ("github.com").
pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |x| x.1);
    let h = rest.split(['/', '?', '#']).next().unwrap_or("");
    let h = h.rsplit_once('@').map_or(h, |x| x.1);
    h.trim_start_matches("www.").to_string()
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

/// Each <meta> tag's name (its property= or name=) and content=.
fn meta_tags(head: &str) -> Vec<(String, String)> {
    let low = head.to_ascii_lowercase();
    let mut out = vec![];
    let mut from = 0;
    while let Some(i) = low[from..].find("<meta") {
        let start = from + i;
        let end = low[start..].find('>').map_or(low.len(), |e| start + e);
        let tag = &head[start..end];
        let name = attr(tag, "property").or_else(|| attr(tag, "name"));
        if let (Some(n), Some(c)) = (name, attr(tag, "content")) {
            out.push((n, c));
        }
        from = end.max(start + 5);
    }
    out
}

/// An attribute's value in a tag, quoted with " or ' or not at all.
fn attr(tag: &str, name: &str) -> Option<String> {
    let low = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = low[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        // a whole attribute name, followed by =
        let before_ok = at == 0 || low.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = low[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let vstart = tag.len() - rest.len() + 1;
        let v = tag[vstart..].trim_start();
        let value = match v.chars().next() {
            Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or(""),
            _ => v
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or(""),
        };
        return Some(decode(value));
    }
    None
}

fn title_tag(head: &str) -> Option<String> {
    let low = head.to_ascii_lowercase();
    let i = low.find("<title")?;
    let open_end = low[i..].find('>')? + i + 1;
    let close = low[open_end..].find("</title")? + open_end;
    Some(decode(&head[open_end..close]))
}

/// The HTML character references pages use most.
fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let ch = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ if name.starts_with("#x") || name.starts_with("#X") => {
                u32::from_str_radix(&name[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if name.starts_with('#') => name[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// One line, spaces squeezed, at most `max` characters.
fn clean(s: &str, max: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > max {
        format!(
            "{}...",
            one.chars().take(max).collect::<String>().trim_end()
        )
    } else {
        one
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_graph_tags_win_over_the_title() {
        let html = r#"<html><head><title>Plain title</title>
            <meta property="og:title" content="Launch &amp; ship">
            <meta name='description' content='The plan
               for Friday'>
            <META PROPERTY="og:site_name" CONTENT="Example Co"/>
            </head><body><meta property="og:title" content="not this"></body>"#;
        let p = from_html("https://www.example.com/a", html).unwrap();
        assert_eq!(p.title, "Launch & ship");
        assert_eq!(p.description, "The plan for Friday");
        assert_eq!(p.site, "Example Co");
    }

    #[test]
    fn the_title_tag_and_host_when_there_are_no_meta_tags() {
        let p = from_html(
            "https://www.rust-lang.org/learn",
            "<title> Learn &#8212; Rust </title>",
        )
        .unwrap();
        assert_eq!(p.title, "Learn \u{2014} Rust");
        assert_eq!(p.site, "rust-lang.org");
        assert!(from_html("https://x.example", "<html><body>nothing</body>").is_none());
        assert_eq!(host("http://user@host.example:80/x?y"), "host.example:80");
        let long = format!("<title>{}</title>", "a".repeat(400));
        assert_eq!(
            from_html("https://x", &long).unwrap().title.chars().count(),
            203
        );
    }
}
