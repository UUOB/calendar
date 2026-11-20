//! What the event card shows besides the times (app/lib/details.js): the
//! meeting link to join, the notes without the joining instructions Google
//! and Teams paste in, links in the notes, the people invited and your own
//! answer. Only what is shown changes; the record keeps its notes as they are.

use std::sync::LazyLock;

use regex_lite::Regex;
use serde_json::{Map, Value as Json, json};

static URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\b(?:https?://|mailto:|tel:)[^\s<>"'()]+[^\s<>"'().,;:!?]"#).unwrap());

/// Video meeting services, by the host of their links.
fn service_of_host(host: &str) -> Option<&'static str> {
    let ends = |h: &str, d: &str| h == d || h.ends_with(&format!(".{d}"));
    if ends(host, "meet.google.com") {
        Some("Google Meet")
    } else if ends(host, "zoom.us") {
        Some("Zoom")
    } else if host == "teams.microsoft.com" || host == "teams.live.com" {
        Some("Microsoft Teams")
    } else if ends(host, "webex.com") {
        Some("Webex")
    } else if host == "meet.jit.si" {
        Some("Jitsi Meet")
    } else if ends(host, "whereby.com") {
        Some("Whereby")
    } else {
        None
    }
}

/// The host of a URL, lowercased, as JS's `new URL(url).hostname` (None if it isn't one).
pub fn host_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once(':')?;
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    if !["http", "https", "ws", "wss", "ftp", "file"].contains(&scheme.as_str()) {
        // mailto:, tel: … have no host.
        return Some(String::new());
    }
    let rest = rest.trim_start_matches(['/', '\\']);
    let end = rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len());
    let auth = &rest[..end];
    let hostport = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
    let host = if hostport.starts_with('[') {
        let close = hostport.find(']')?;
        &hostport[..=close]
    } else {
        hostport.split(':').next().unwrap_or("")
    };
    if host.is_empty() || host.contains(['<', '>', ' ', '%', '^', '|']) {
        return None;
    }
    Some(host.to_lowercase())
}

fn service(url: &str) -> Option<&'static str> {
    service_of_host(&host_of(url)?)
}

fn s<'a>(e: &'a Json, k: &str) -> Option<&'a str> {
    e.get(k).and_then(Json::as_str).filter(|x| !x.is_empty())
}

/// The video meeting to join, if the event has one: { url, name, host }.
pub fn meeting(e: &Json) -> Json {
    let notes = plain(s(e, "notes").unwrap_or(""));
    let sources = [s(e, "conference").map(String::from), s(e, "location").map(String::from), s(e, "url").map(String::from), Some(notes).filter(|n| !n.is_empty())];
    for src in sources.into_iter().flatten() {
        for m in URL_RE.find_iter(&src) {
            let u = m.as_str();
            if let Some(name) = service(u) {
                let host = host_of(u).unwrap_or_default();
                return json!({ "url": u, "name": name, "host": host.strip_prefix("www.").unwrap_or(&host) });
            }
        }
    }
    Json::Null
}

/// Notes as plain text: Google sometimes stores them as HTML.
pub fn plain(notes: &str) -> String {
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<(br|p|div|a|b|i|u|ul|ol|li|span)\b[^>]*>").unwrap());
    static A: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<a\b[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).unwrap());
    static ANY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
    static BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
    static CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</(p|div|li)>").unwrap());
    static LI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<li\b[^>]*>").unwrap());
    static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
    if !TAG.is_match(notes) {
        return notes.to_string();
    }
    let t = A.replace_all(notes, |c: &regex_lite::Captures| {
        let href = &c[1];
        let text = &c[2];
        let inner = ANY.replace_all(text, "");
        if inner.trim() == href || text.trim().is_empty() { href.to_string() } else { format!("{text} ({href})") }
    });
    let t = BR.replace_all(&t, "\n");
    let t = CLOSE.replace_all(&t, "\n");
    let t = LI.replace_all(&t, "• ");
    let t = ANY.replace_all(&t, "");
    let t = t.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&amp;", "&");
    BLANKS.replace_all(&t, "\n\n").trim().to_string()
}

fn tidy(lines: &[String]) -> String {
    static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
    BLANKS.replace_all(&lines.join("\n"), "\n\n").trim().to_string()
}

/// The notes split into what someone wrote and the joining instructions a
/// meeting service added: { text, joining }.
pub fn notes_of(notes: &str) -> Json {
    static GOOGLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-::~[:~]+-?$").unwrap());
    static TEAMS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^_{20,}$").unwrap());
    static EDIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)please do not edit this section").unwrap());
    let p = plain(notes);
    let mut keep: Vec<String> = vec![];
    let mut joining: Vec<String> = vec![];
    let mut block: Option<(&str, Vec<String>)> = None;
    for line in p.split('\n') {
        let t = line.trim();
        let kind = if GOOGLE.is_match(t) {
            Some("google")
        } else if TEAMS.is_match(t) {
            Some("teams")
        } else {
            None
        };
        match block.take() {
            Some((k, mut lines)) => {
                if kind == Some(k) {
                    joining.extend(lines.drain(1..));
                } else {
                    lines.push(line.to_string());
                    block = Some((k, lines));
                }
            }
            None => match kind {
                Some(k) => block = Some((k, vec![line.to_string()])),
                None => keep.push(line.to_string()),
            },
        }
    }
    if let Some((_, lines)) = block {
        keep.extend(lines); // a divider that never closes is just text
    }
    let joining: Vec<String> = joining.into_iter().filter(|l| !EDIT.is_match(l)).collect();
    json!({ "text": tidy(&keep), "joining": tidy(&joining) })
}

/// Text as pieces to show, with links: [{ text } | { text, href }].
pub fn linkify(text: &str) -> Json {
    static SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(mailto|tel):").unwrap());
    static WWW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://(www\.)?").unwrap());
    let mut out = vec![];
    let mut at = 0;
    for m in URL_RE.find_iter(text) {
        if m.start() > at {
            out.push(json!({ "text": &text[at..m.start()] }));
        }
        let href = m.as_str();
        let shown = WWW.replace(&SCHEME.replace(href, ""), "").to_string();
        out.push(json!({ "text": shown, "href": href }));
        at = m.end();
    }
    if at < text.len() {
        out.push(json!({ "text": &text[at..] }));
    }
    Json::Array(out)
}

fn lower(s: Option<&str>) -> String {
    s.unwrap_or("").trim().to_lowercase()
}

/// Whether `text` is an email address (Google names a person's calendar by theirs).
pub fn is_email(text: &str) -> bool {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap());
    RE.is_match(text.trim())
}

/// The people of an event, organizer first, then by answer: each
/// { name, email, answer, organizer, you }. `mine`: your addresses;
/// `answer`: your own answer, which replaces the one in the file.
pub fn people_of(e: &Json, mine: &[String], answer: Option<&str>) -> Json {
    let is_mine = |email: Option<&str>| mine.contains(&lower(email));
    let mut list: Vec<Map<String, Json>> = e
        .get("attendees")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| a.as_object().cloned())
        .map(|mut a| {
            let you = is_mine(a.get("email").and_then(Json::as_str));
            a.insert("organizer".into(), json!(false));
            a.insert("you".into(), json!(you));
            a
        })
        .collect();
    if let Some(org) = e.get("organizer").filter(|o| s(o, "email").is_some()) {
        let oe = lower(s(org, "email"));
        if let Some(at) = list.iter_mut().find(|a| lower(a.get("email").and_then(Json::as_str)) == oe) {
            at.insert("organizer".into(), json!(true));
        } else if !list.is_empty() {
            let mut o = Map::new();
            o.insert("name".into(), org.get("name").cloned().unwrap_or(Json::Null));
            o.insert("email".into(), json!(s(org, "email")));
            o.insert("answer".into(), json!("yes"));
            o.insert("organizer".into(), json!(true));
            o.insert("you".into(), json!(is_mine(s(org, "email"))));
            if o["name"].is_null() {
                o.remove("name");
            }
            list.push(o);
        }
    }
    if let Some(a) = answer.filter(|a| !a.is_empty()) {
        for p in list.iter_mut().filter(|p| p.get("you") == Some(&json!(true))) {
            p.insert("answer".into(), json!(a));
        }
    }
    let rank = |p: &Map<String, Json>| {
        if p.get("organizer") == Some(&json!(true)) {
            0
        } else {
            match p.get("answer").and_then(Json::as_str) {
                Some("yes") => 1,
                Some("maybe") => 2,
                Some("no") => 3,
                _ => 4,
            }
        }
    };
    let label = |p: &Map<String, Json>| {
        p.get("name").and_then(Json::as_str).filter(|x| !x.is_empty()).or_else(|| p.get("email").and_then(Json::as_str).filter(|x| !x.is_empty())).unwrap_or("").to_string()
    };
    list.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| locale_cmp(&label(a), &label(b))));
    Json::Array(list.into_iter().map(Json::Object).collect())
}

/// Close to JS's `localeCompare` for names: letters before their accented
/// forms, case second.
pub fn locale_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn base(c: char) -> char {
        let l = c.to_lowercase().next().unwrap_or(c);
        match l {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'č' | 'ç' | 'ć' => 'c',
            'ď' => 'd',
            'é' | 'è' | 'ê' | 'ë' | 'ě' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ň' | 'ñ' => 'n',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ř' => 'r',
            'š' | 'ś' => 's',
            'ť' => 't',
            'ú' | 'ù' | 'û' | 'ü' | 'ů' => 'u',
            'ý' | 'ÿ' => 'y',
            'ž' | 'ź' | 'ż' => 'z',
            other => other,
        }
    }
    let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).map(base).collect::<String>();
    key(a).cmp(&key(b)).then_with(|| {
        let punct = |s: &str| s.chars().map(base).collect::<String>();
        punct(a).cmp(&punct(b))
    }).then_with(|| b.cmp(a))
}

/// "30 people · 4 yes · 2 maybe · 2 no"
pub fn people_summary(people: &Json) -> String {
    let list = people.as_array().cloned().unwrap_or_default();
    let n = |x: &str| list.iter().filter(|p| p.get("answer").and_then(Json::as_str) == Some(x)).count();
    let mut parts = vec![format!("{} {}", list.len(), if list.len() == 1 { "person" } else { "people" })];
    for w in ["yes", "maybe", "no"] {
        let c = n(w);
        if c > 0 {
            parts.push(format!("{c} {w}"));
        }
    }
    parts.join(" · ")
}

fn has_attendees(e: &Json) -> bool {
    e.get("attendees").and_then(Json::as_array).is_some_and(|a| !a.is_empty())
}

fn organizes(e: &Json, mine: &[String]) -> bool {
    e.get("organizer").and_then(|o| s(o, "email")).is_some_and(|m| mine.contains(&lower(Some(m))))
}

/// Your answer to an invitation: what you chose here, else what the file
/// says for your address. 'yes' | 'maybe' | 'no' | 'invited', or null.
pub fn answer_of(e: &Json, mine: &[String], chosen: Option<&str>) -> Option<String> {
    if !has_attendees(e) || organizes(e, mine) {
        return None;
    }
    if let Some(c) = chosen.filter(|c| !c.is_empty()) {
        return Some(c.into());
    }
    let me = e["attendees"].as_array()?.iter().find(|a| mine.contains(&lower(a.get("email").and_then(Json::as_str))))?;
    Some(me.get("answer").and_then(Json::as_str).unwrap_or("invited").to_string())
}

/// Whether to offer Going / Maybe / No: an invitation you don't organize.
pub fn asks_answer(e: &Json, mine: &[String]) -> bool {
    has_attendees(e) && !organizes(e, mine)
}
