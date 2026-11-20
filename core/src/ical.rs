//! iCalendar (.ics) files (app/lib/ical.js): reading what Google, Apple and
//! Outlook export or publish, and writing calendars they can import.
//!
//! An event record looks like
//!   { title, notes, location, all_day, date, end_date?, start?, end?, tz?,
//!     rrule?, exdates?, recurrence_of?, recurrence_date?, uid? }
//! `date`..`end_date` are the first and last day (all-day: inclusive);
//! `start`/`end` are "HH:MM" on `date` and `end_date` (or `date`). Times are
//! in `tz` when it is set, otherwise in this device's time.

use std::sync::LazyLock;

use regex_lite::Regex;
use serde_json::{Map, Value as Json, json};

use crate::recur::supported;
use crate::zones::{self, Wall, add_days, date_utc, known_zone, local_instant, utc_date, utc_time, zoned_instant, zoned_wall};

// ---------------------------------------------------------------- reading

struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, k: &str) -> Option<&str> {
        self.params.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
    }
}

/// Lines of the file, with folded lines joined back.
fn lines(text: &str) -> Vec<String> {
    let t = text.replace("\r\n", "\n").replace('\r', "\n");
    let t = t.replace("\n ", "").replace("\n\t", "");
    t.split('\n').filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

/// "DTSTART;TZID=Europe/Prague:20261116T090000" -> name, params, value.
fn property(line: &str) -> Prop {
    let mut quoted = false;
    let mut at = line.len();
    for (i, c) in line.char_indices() {
        if c == '"' {
            quoted = !quoted;
        } else if c == ':' && !quoted {
            at = i;
            break;
        }
    }
    let head = &line[..at];
    let value = if at < line.len() { line[at + 1..].to_string() } else { String::new() };
    let mut parts = head.split(';');
    let name = parts.next().unwrap_or("").to_uppercase();
    let mut params = vec![];
    for p in parts {
        if let Some(eq) = p.find('=').filter(|e| *e > 0) {
            let v = &p[eq + 1..];
            let v = v.strip_prefix('"').unwrap_or(v);
            let v = v.strip_suffix('"').unwrap_or(v);
            let k = p[..eq].to_uppercase();
            match params.iter_mut().find(|(x, _): &&mut (String, String)| *x == k) {
                Some(slot) => slot.1 = v.to_string(),
                None => params.push((k, v.to_string())),
            }
        }
    }
    Prop { name, params, value }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.peek() {
                Some('n') | Some('N') => {
                    it.next();
                    out.push('\n');
                    continue;
                }
                Some(&x @ (',' | ';' | '\\')) => {
                    it.next();
                    out.push(x);
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

/// A short stable id for a record, from an iCalendar UID: two 32-bit FNV-1a
/// hashes of its UTF-16 code units (as the JS version counts them).
pub fn record_id(parts: &[&str]) -> String {
    let s = parts.join("|");
    let mut a: u32 = 0x811c9dc5;
    let mut b: u32 = 0x01000193 ^ 0x5bd1e995;
    for c in s.encode_utf16() {
        a = (a ^ c as u32).wrapping_mul(0x01000193);
        b = (b ^ c as u32).wrapping_mul(0x5bd1e995);
    }
    format!("ics-{a:08x}{b:08x}")
}

#[derive(Clone, Debug)]
struct When {
    date: String,
    time: Option<String>,
    tz: Option<String>,
}

fn slice(s: &str, a: usize, b: usize) -> &str {
    s.get(a.min(s.len())..b.min(s.len())).unwrap_or("")
}

/// A date or date-time value. A "Z" value is in UTC; a TZID that isn't
/// known (Outlook's "W. Europe Standard Time") falls back to `fallback`,
/// then to this device's time.
fn when(p: &Prop, value: &str, fallback: Option<&str>, zones_seen: &mut Vec<String>) -> When {
    let v = value.trim();
    let date = format!("{}-{}-{}", slice(v, 0, 4), slice(v, 4, 6), slice(v, 6, 8));
    if p.param("VALUE") == Some("DATE") || v.chars().count() == 8 {
        return When { date, time: None, tz: None };
    }
    let time = format!("{}:{}", slice(v, 9, 11), slice(v, 11, 13));
    if v.ends_with('Z') {
        return When { date, time: Some(time), tz: Some("UTC".into()) };
    }
    if let Some(tzid) = p.param("TZID").map(|t| t.strip_prefix('/').unwrap_or(t)).filter(|t| !t.is_empty()) {
        if known_zone(tzid) {
            return When { date, time: Some(time), tz: Some(tzid.into()) };
        }
        if !zones_seen.iter().any(|z| z == tzid) {
            zones_seen.push(tzid.into());
        }
        if let Some(f) = fallback {
            return When { date, time: Some(time), tz: Some(f.into()) };
        }
    }
    When { date, time: Some(time), tz: None }
}

fn instant(w: &When) -> i64 {
    match &w.tz {
        Some(tz) => zoned_instant(&w.date, w.time.as_deref(), tz),
        None => local_instant(&w.date, w.time.as_deref()),
    }
}

/// Moves a value to another zone (or keeps it when the zone is the same).
fn into(w: &When, tz: Option<&str>) -> When {
    if w.time.is_none() || w.tz.as_deref() == tz {
        return w.clone();
    }
    let ms = instant(w);
    match tz {
        Some(tz) => {
            let x = zoned_wall(ms, tz);
            When { date: x.date, time: Some(x.time), tz: Some(tz.into()) }
        }
        None => {
            let x = zones::local_wall(ms);
            When { date: x.date, time: Some(x.time), tz: None }
        }
    }
}

/// "P1DT2H30M" / "PT45M" / "P1W" -> minutes.
fn duration(s: &str) -> Option<i64> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([+-])?P(?:(\d+)W)?(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?)?$").unwrap());
    let m = RE.captures(s)?;
    let n = |i: usize| m.get(i).and_then(|x| x.as_str().parse::<i64>().ok()).unwrap_or(0);
    let mins = n(2) * 10080 + n(3) * 1440 + n(4) * 60 + n(5);
    Some(if m.get(1).map(|x| x.as_str()) == Some("-") { -mins } else { mins })
}

fn wall_ms(date: &str, time: Option<&str>) -> i64 {
    let (y, m, d) = zones::ymd(date);
    let (h, mi) = zones::hm(time);
    date_utc(y, m - 1, d, h, mi)
}

/// A wall date and time plus minutes.
fn plus(date: &str, time: Option<&str>, minutes: i64) -> Wall {
    let t = wall_ms(date, time) + minutes * 60_000;
    Wall { date: utc_date(t), time: utc_time(t) }
}

fn minutes_between(a: &When, b: &When) -> i64 {
    let d = (wall_ms(&b.date, b.time.as_deref()) - wall_ms(&a.date, a.time.as_deref())) as f64 / 60000.0;
    js_round(d)
}

/// JS's Math.round (halves go up).
pub fn js_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

const ANSWERS: [(&str, &str); 3] = [("ACCEPTED", "yes"), ("TENTATIVE", "maybe"), ("DECLINED", "no")];

fn person(p: &Prop) -> Map<String, Json> {
    let v = p.value.as_str();
    let email = if v.len() >= 7 && v[..7].eq_ignore_ascii_case("mailto:") { &v[7..] } else { v }.trim().to_string();
    let name = p.param("CN").unwrap_or("").trim().to_string();
    let mut o = Map::new();
    if !name.is_empty() && name.to_lowercase() != email.to_lowercase() {
        o.insert("name".into(), json!(name));
    }
    o.insert("email".into(), json!(email));
    o
}

fn is_http(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.starts_with("http:") || l.starts_with("https:")
}

/// Who organizes the event, who is invited and how they answered, and its links.
fn invitation(props: &[Prop], e: &mut Map<String, Json>) {
    if let Some(org) = props.iter().find(|p| p.name == "ORGANIZER") {
        if org.value.contains('@') {
            e.insert("organizer".into(), Json::Object(person(org)));
        }
    }
    let attendees: Vec<Json> = props
        .iter()
        .filter(|p| p.name == "ATTENDEE" && p.value.contains('@') && !matches!(p.param("CUTYPE").unwrap_or("").to_uppercase().as_str(), "ROOM" | "RESOURCE"))
        .take(300)
        .map(|p| {
            let mut o = person(p);
            let st = p.param("PARTSTAT").unwrap_or("").to_uppercase();
            o.insert("answer".into(), ANSWERS.iter().find(|(k, _)| *k == st).map_or(Json::Null, |(_, v)| json!(v)));
            Json::Object(o)
        })
        .collect();
    if !attendees.is_empty() {
        e.insert("attendees".into(), Json::Array(attendees));
    }
    if let Some(c) = props.iter().find(|p| p.name == "X-GOOGLE-CONFERENCE" || (p.name == "CONFERENCE" && is_http(&p.value))) {
        e.insert("conference".into(), json!(unescape(&c.value).trim()));
    }
    if let Some(u) = props.iter().find(|p| p.name == "URL" && is_http(p.value.trim())) {
        e.insert("url".into(), json!(unescape(&u.value).trim()));
    }
}

fn is_email(s: &str) -> bool {
    crate::details::is_email(s)
}

/// Reads an .ics file: `{ name, owner, events, skipped, unreadableRules, zones }`.
/// `events` are records ready to save (each with its `id`); repeating events
/// come with their changed occurrences as records of their own.
pub fn parse_ics(text: &str) -> Json {
    let mut zones_seen: Vec<String> = vec![];
    let mut name: Option<String> = None;
    let mut cal_tz: Option<String> = None;
    let mut raw: Vec<Vec<Prop>> = vec![];
    let mut depth: Vec<String> = vec![];
    let mut cur: Option<Vec<Prop>> = None;
    for line in lines(text) {
        let p = property(&line);
        if p.name == "BEGIN" {
            depth.push(p.value.to_uppercase());
            if p.value.to_uppercase() == "VEVENT" && depth.len() == 2 {
                cur = Some(vec![]);
            }
            continue;
        }
        if p.name == "END" {
            if p.value.to_uppercase() == "VEVENT" && depth.len() == 2 {
                if let Some(c) = cur.take() {
                    raw.push(c);
                }
            }
            depth.pop();
            continue;
        }
        if depth.len() == 1 && p.name == "X-WR-CALNAME" {
            let n = unescape(&p.value).trim().to_string();
            name = (!n.is_empty()).then_some(n);
        }
        if depth.len() == 1 && p.name == "X-WR-TIMEZONE" && known_zone(p.value.trim()) {
            cal_tz = Some(p.value.trim().to_string());
        }
        if depth.len() == 2 {
            if let Some(c) = cur.as_mut() {
                c.push(p);
            }
        }
    }

    let mut masters: Vec<(String, Map<String, Json>)> = vec![];
    let mut changed: Vec<(String, When, Map<String, Json>)> = vec![];
    let mut cancelled: Vec<(String, When)> = vec![];
    let mut skipped = 0;
    let mut unreadable = 0;
    let fb = cal_tz.as_deref();

    for props in &raw {
        let one = |n: &str| props.iter().find(|p| p.name == n);
        let text = |n: &str| one(n).map(|p| unescape(&p.value).trim().to_string()).unwrap_or_default();
        let Some(dtstart) = one("DTSTART") else {
            skipped += 1;
            continue;
        };
        let uid = Some(text("UID")).filter(|u| !u.is_empty()).unwrap_or_else(|| format!("{}|{}", text("SUMMARY"), dtstart.value));
        let start = when(dtstart, &dtstart.value, fb, &mut zones_seen);
        let rid = one("RECURRENCE-ID").map(|p| when(p, &p.value, fb, &mut zones_seen));
        if text("STATUS").to_uppercase() == "CANCELLED" {
            match rid {
                Some(r) => cancelled.push((uid, r)),
                None => skipped += 1,
            }
            continue;
        }
        let mut e = Map::new();
        e.insert("title".into(), json!(Some(text("SUMMARY")).filter(|t| !t.is_empty()).unwrap_or_else(|| "Untitled".into())));
        e.insert("notes".into(), json!(text("DESCRIPTION")));
        e.insert("location".into(), json!(text("LOCATION")));
        e.insert("uid".into(), json!(uid));
        invitation(props, &mut e);
        match &start.time {
            None => {
                // All-day: DTEND is the day after the last day.
                let mut after = one("DTEND").map(|p| when(p, &p.value, fb, &mut zones_seen).date);
                if after.is_none() {
                    if let Some(d) = one("DURATION") {
                        let mins = duration(d.value.trim()).unwrap_or(1440);
                        after = Some(add_days(&start.date, js_round(mins as f64 / 1440.0).max(1)));
                    }
                }
                let after = after.unwrap_or_else(|| add_days(&start.date, 1));
                let last = if after > start.date { add_days(&after, -1) } else { start.date.clone() };
                e.insert("all_day".into(), json!(true));
                e.insert("date".into(), json!(start.date));
                if last > start.date {
                    e.insert("end_date".into(), json!(last));
                }
            }
            Some(st) => {
                let mins = if let Some(p) = one("DTEND") {
                    let end = into(&when(p, &p.value, fb, &mut zones_seen), start.tz.as_deref());
                    minutes_between(&start, &end)
                } else if let Some(d) = one("DURATION") {
                    duration(d.value.trim()).unwrap_or(60)
                } else {
                    0
                };
                let end = plus(&start.date, Some(st), mins.max(0));
                e.insert("all_day".into(), json!(false));
                e.insert("date".into(), json!(start.date));
                e.insert("start".into(), json!(st));
                e.insert("end".into(), json!(end.time));
                if end.date > start.date {
                    e.insert("end_date".into(), json!(end.date));
                }
                if let Some(tz) = &start.tz {
                    e.insert("tz".into(), json!(tz));
                }
            }
        }
        let rule = one("RRULE").map(|p| p.value.trim().to_string()).filter(|r| !r.is_empty());
        if let (Some(rule), None) = (&rule, &rid) {
            let rr = if rule.len() >= 6 && rule[..6].eq_ignore_ascii_case("RRULE:") { rule[6..].to_string() } else { rule.clone() };
            if !supported(&rr) {
                unreadable += 1;
            }
            e.insert("rrule".into(), json!(rr));
            let mut ex: Vec<String> = vec![];
            for p in props.iter().filter(|p| p.name == "EXDATE") {
                for v in p.value.split(',') {
                    let d = into(&when(p, v, fb, &mut zones_seen), start.tz.as_deref()).date;
                    if !ex.contains(&d) {
                        ex.push(d);
                    }
                }
            }
            if !ex.is_empty() {
                ex.sort();
                e.insert("exdates".into(), json!(ex));
            }
        }
        match rid {
            Some(r) => changed.push((uid, r, e)),
            None => match masters.iter_mut().find(|(u, _)| *u == uid) {
                Some(slot) => slot.1 = e,
                None => masters.push((uid, e)),
            },
        }
    }

    // Changed and cancelled occurrences belong to their series.
    for (uid, rid) in &cancelled {
        let Some((_, m)) = masters.iter_mut().find(|(u, _)| u == uid) else { continue };
        let tz = m.get("tz").and_then(Json::as_str).map(String::from);
        let d = into(rid, tz.as_deref()).date;
        let mut ex: Vec<String> = m.get("exdates").and_then(Json::as_array).into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        if !ex.contains(&d) {
            ex.push(d);
        }
        ex.sort();
        m.insert("exdates".into(), json!(ex));
    }
    let mut events: Vec<Json> = vec![];
    for (uid, e) in &masters {
        let mut e = e.clone();
        e.insert("id".into(), json!(record_id(&[uid])));
        events.push(Json::Object(e));
    }
    for (uid, rid, e) in &changed {
        let mut e = e.clone();
        match masters.iter().find(|(u, _)| u == uid) {
            None => {
                e.insert("id".into(), json!(record_id(&[uid, &rid.date])));
            }
            Some((_, m)) => {
                let d = into(rid, m.get("tz").and_then(Json::as_str)).date;
                e.insert("id".into(), json!(record_id(&[uid, &d])));
                e.insert("recurrence_of".into(), json!(record_id(&[uid])));
                e.insert("recurrence_date".into(), json!(d));
            }
        }
        events.push(Json::Object(e));
    }
    // Google names your own calendar by your address: that's how the card knows which guest is you.
    let owner = name.as_ref().filter(|n| is_email(n) && !n.trim().contains(char::is_whitespace)).map(|n| n.to_lowercase());
    json!({ "name": name, "owner": owner, "events": events, "skipped": skipped, "unreadableRules": unreadable, "zones": zones_seen })
}

// ---------------------------------------------------------------- writing

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace(',', "\\,").replace(';', "\\;")
}

/// Lines longer than 75 bytes are folded, as the format asks.
fn fold(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut out: Vec<String> = vec![];
    let mut chunk = String::new();
    let mut size = 0;
    for ch in line.chars() {
        let n = ch.len_utf8();
        if size + n > if out.is_empty() { 75 } else { 74 } {
            out.push(std::mem::take(&mut chunk));
            size = 0;
        }
        chunk.push(ch);
        size += n;
    }
    out.push(chunk);
    out.join("\r\n ")
}

fn d8(date: &str) -> String {
    date.replace('-', "")
}

fn dt(date: &str, time: &str) -> String {
    format!("{}T{}00", d8(date), time.replacen(':', "", 1))
}

fn s<'a>(e: &'a Json, k: &str) -> Option<&'a str> {
    e.get(k).and_then(Json::as_str).filter(|x| !x.is_empty())
}

fn truthy(v: Option<&Json>) -> bool {
    match v {
        None | Some(Json::Null) => false,
        Some(Json::Bool(b)) => *b,
        Some(Json::String(x)) => !x.is_empty(),
        Some(Json::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        _ => true,
    }
}

fn stamp(name: &str, e: &Json, date: &str, time: Option<&str>) -> String {
    if truthy(e.get("all_day")) {
        return format!("{name};VALUE=DATE:{}", d8(date));
    }
    let t = time.unwrap_or("00:00");
    match s(e, "tz") {
        Some("UTC") => format!("{name}:{}Z", dt(date, t)),
        Some(tz) => format!("{name};TZID={tz}:{}", dt(date, t)),
        None => format!("{name}:{}", dt(date, t)),
    }
}

/// An .ics file of `events`, which Google, Apple and Outlook can import.
/// `now`: the moment written as each event's DTSTAMP.
pub fn write_ics(events: &[Json], name: Option<&str>, now: i64) -> String {
    let (y, m, d, h, mi) = zones::utc_parts(now);
    let sec = now.rem_euclid(60_000) / 1000;
    let stampnow = format!("{y:04}{m:02}{d:02}T{h:02}{mi:02}{sec:02}Z");
    let uid_of = |id: &str| -> Option<String> {
        events.iter().find(|e| s(e, "id") == Some(id)).map(|e| s(e, "uid").map(String::from).unwrap_or_else(|| format!("{id}@uuob")))
    };
    let mut out: Vec<String> = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//UUOB//Calendar//EN", "CALSCALE:GREGORIAN", "METHOD:PUBLISH"].iter().map(|x| x.to_string()).collect();
    if let Some(n) = name.filter(|n| !n.is_empty()) {
        out.push(format!("X-WR-CALNAME:{}", escape(n)));
    }
    for e in events {
        let id = s(e, "id").unwrap_or("");
        let uid = match s(e, "recurrence_of") {
            Some(of) => uid_of(of).unwrap_or_else(|| format!("{of}@uuob")),
            None => uid_of(id).unwrap_or_else(|| format!("{id}@uuob")),
        };
        out.push("BEGIN:VEVENT".into());
        out.push(format!("UID:{}", escape(&uid)));
        out.push(format!("DTSTAMP:{stampnow}"));
        let date = s(e, "date").unwrap_or("");
        if truthy(e.get("all_day")) {
            out.push(stamp("DTSTART", e, date, None));
            out.push(stamp("DTEND", e, &add_days(s(e, "end_date").unwrap_or(date), 1), None));
        } else {
            out.push(stamp("DTSTART", e, date, s(e, "start")));
            out.push(stamp("DTEND", e, s(e, "end_date").unwrap_or(date), s(e, "end")));
        }
        if let Some(r) = s(e, "rrule") {
            out.push(format!("RRULE:{r}"));
        }
        for x in e.get("exdates").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str) {
            out.push(stamp("EXDATE", e, x, s(e, "start")));
        }
        if let (Some(of), Some(rd)) = (s(e, "recurrence_of"), s(e, "recurrence_date")) {
            let master = events.iter().find(|m| s(m, "id") == Some(of)).unwrap_or(e);
            out.push(stamp("RECURRENCE-ID", master, rd, s(master, "start")));
        }
        out.push(format!("SUMMARY:{}", escape(e.get("title").and_then(Json::as_str).unwrap_or(""))));
        if let Some(l) = s(e, "location") {
            out.push(format!("LOCATION:{}", escape(l)));
        }
        if let Some(n) = s(e, "notes") {
            out.push(format!("DESCRIPTION:{}", escape(n)));
        }
        if let Some(u) = s(e, "url") {
            out.push(format!("URL:{u}"));
        }
        if let Some(c) = s(e, "conference") {
            out.push(format!("X-GOOGLE-CONFERENCE:{c}"));
        }
        let cn = |p: &Json| s(p, "name").map(|n| format!(";CN=\"{}\"", n.replace('"', "'"))).unwrap_or_default();
        if let Some(org) = e.get("organizer").filter(|o| s(o, "email").is_some()) {
            out.push(format!("ORGANIZER{}:mailto:{}", cn(org), s(org, "email").unwrap_or("")));
        }
        for a in e.get("attendees").and_then(Json::as_array).into_iter().flatten() {
            let st = match a.get("answer").and_then(Json::as_str) {
                Some("yes") => "ACCEPTED",
                Some("maybe") => "TENTATIVE",
                Some("no") => "DECLINED",
                _ => "NEEDS-ACTION",
            };
            out.push(format!("ATTENDEE{};PARTSTAT={st}:mailto:{}", cn(a), a.get("email").and_then(Json::as_str).unwrap_or("")));
        }
        out.push("END:VEVENT".into());
    }
    out.push("END:VCALENDAR".into());
    out.iter().map(|l| fold(l)).collect::<Vec<_>>().join("\r\n") + "\r\n"
}
