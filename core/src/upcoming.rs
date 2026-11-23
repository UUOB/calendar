//! What Calendar hands UUOB as coming up (app/lib/upcoming.js): every event
//! of the next two weeks, for the "Up next" widget and the alerts on the
//! user's phones and computers. Times are written on the event's own clock
//! (its time zone, or none for an event that follows the device's), so every
//! device writes the same list.

use serde_json::{Map, Value as Json, json};

use crate::instances::{expand, last_day, truthy};
use crate::zones::{add_days, local_instant, local_wall, zoned_wall};

/// How far ahead the list goes.
pub const DAYS: i64 = 14;
/// The alert choices offered first, in minutes before the start (None: no alert).
pub const BEFORE: [Option<i64>; 7] = [None, Some(0), Some(5), Some(10), Some(30), Some(60), Some(1440)];
pub const DEFAULT_BEFORE: i64 = 10;
/// The earliest an alert can ring: a week before.
pub const MAX_BEFORE: i64 = 7 * 1440;

/// "None", "At the start", "1 hour 30 minutes before", "2 days before".
pub fn before_label(m: Option<i64>) -> String {
    let Some(m) = m else { return "None".into() };
    if m <= 0 {
        return "At the start".into();
    }
    let n = |x: i64, one: &str| format!("{x} {one}{}", if x == 1 { "" } else { "s" });
    let (d, h, min) = (m / 1440, (m % 1440) / 60, m % 60);
    let parts: Vec<String> = [(d, "day"), (h, "hour"), (min, "minute")].iter().filter(|(x, _)| *x != 0).map(|(x, u)| n(*x, u)).collect();
    format!("{} before", parts.join(" "))
}

/// An alert's time before, as typed in Custom…: "15:00" (15 hours), "0:30",
/// "1.30", "130" (1:30), "90" (two digits or fewer: minutes), "2h", "1h30",
/// "45 min", "1d". Minutes, at most a week; None when it isn't such a time.
pub fn parse_before(text: &str) -> Option<i64> {
    let mut t: String = text.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    for (long, short) in [("minutes", "m"), ("minute", "m"), ("mins", "m"), ("min", "m"), ("hours", "h"), ("hour", "h"), ("hrs", "h"), ("hr", "h"), ("days", "d"), ("day", "d")] {
        t = t.replace(long, short);
    }
    if t.is_empty() {
        return None;
    }
    let m = if let Some((h, min)) = t.split_once([':', '.']) {
        let h: i64 = if h.is_empty() { 0 } else { h.parse().ok()? };
        let min: i64 = min.parse().ok()?;
        if !(0..60).contains(&min) || h < 0 {
            return None;
        }
        h * 60 + min
    } else if t.chars().all(|c| c.is_ascii_digit()) {
        if t.len() > 4 {
            return None;
        }
        let n: i64 = t.parse().ok()?;
        if t.len() <= 2 {
            n
        } else if n % 100 < 60 {
            n / 100 * 60 + n % 100
        } else {
            return None;
        }
    } else {
        let (mut total, mut num, mut unit) = (0i64, String::new(), false);
        for c in t.chars() {
            if c.is_ascii_digit() {
                num.push(c);
                continue;
            }
            let n: i64 = num.parse().ok()?;
            num.clear();
            total += n * match c { 'd' => 1440, 'h' => 60, 'm' => 1, _ => return None };
            unit = true;
        }
        if !num.is_empty() {
            total += num.parse::<i64>().ok()?; // "1h30": the rest is minutes
        }
        if !unit {
            return None;
        }
        total
    };
    (m <= MAX_BEFORE).then_some(m)
}

/// Minutes before as Custom… shows them: "15:00", "0:30", "168:00".
pub fn before_hhmm(m: i64) -> String {
    let m = m.max(0);
    format!("{}:{:02}", m / 60, m % 60)
}

#[cfg(test)]
mod custom_tests {
    use super::*;

    #[test]
    fn typed_times_before() {
        for (t, m) in [("15:00", 900), ("0:30", 30), ("1:30", 90), ("1.30", 90), (":45", 45), ("130", 90), ("90", 90), ("5", 5),
            ("2h", 120), ("1h30", 90), ("1h 30m", 90), ("45 min", 45), ("1d", 1440), ("1 day 2 hours", 1560), ("168:00", 10080), ("0:00", 0)] {
            assert_eq!(parse_before(t), Some(m), "{t}");
        }
        for t in ["", "abc", "1:75", "190", "12345", "168:01", "200h", "2x"] {
            assert_eq!(parse_before(t), None, "{t}");
        }
        assert_eq!((before_hhmm(900), before_hhmm(30), before_hhmm(10080)), ("15:00".into(), "0:30".into(), "168:00".into()));
    }
}

/// A place to show: not a link (a meeting's address isn't one).
fn place(e: &Json) -> Option<String> {
    let l = e.get("location").and_then(Json::as_str)?.trim();
    let low = l.to_ascii_lowercase();
    (!l.is_empty() && !low.starts_with("http://") && !low.starts_with("https://")).then(|| l.to_string())
}

/// What an event's own alert says: not set (the default), none, or minutes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Own {
    Default,
    None,
    Minutes(i64),
}

/// The list for `events` (all calendars): each event from now to DAYS days
/// ahead that isn't over, leaving out those `skip` says (declined ones).
/// Timed ones ring `own` minutes before, or `before` by default. Each has
/// its calendar's colour.
pub fn upcoming_for(events: &[Json], before: Option<i64>, now: i64, skip: impl Fn(&Json) -> bool, own: impl Fn(&Json) -> Own, color: impl Fn(&Json) -> Option<String>) -> Vec<Json> {
    let from = local_wall(now).date;
    let mut out = vec![];
    for o in expand(events, &add_days(&from, -1), &add_days(&from, DAYS)) {
        if o.get("date").and_then(Json::as_str).is_none_or(str::is_empty) || skip(&o) {
            continue;
        }
        let mut item = Map::new();
        item.insert("id".into(), json!(o["key"].as_str().unwrap_or("").replace('/', "~")));
        let title = o.get("title").and_then(Json::as_str).map(str::trim).filter(|t| !t.is_empty()).unwrap_or("Event");
        item.insert("title".into(), json!(title));
        if let Some(p) = place(&o) {
            item.insert("body".into(), json!(p));
        }
        if let Some(c) = color(&o) {
            item.insert("color".into(), json!(c));
        }
        let date = o["date"].as_str().unwrap_or("").to_string();
        let start = o.get("start").and_then(Json::as_str).filter(|x| !x.is_empty());
        if truthy(o.get("all_day")) || start.is_none() {
            if last_day(&o) < from {
                continue;
            }
            item.insert("date".into(), json!(date));
            item.insert("end_date".into(), json!(last_day(&o)));
            out.push(Json::Object(item));
            continue;
        }
        let s_ms = local_instant(&date, start);
        let end = o.get("end").and_then(Json::as_str).filter(|x| !x.is_empty());
        let e_ms = end.map_or(s_ms, |e| local_instant(&last_day(&o), Some(e)));
        if e_ms <= now {
            continue;
        }
        let tz = o.get("tz").and_then(Json::as_str).filter(|x| !x.is_empty());
        let wall = |ms: i64| match tz {
            Some(tz) => zoned_wall(ms, tz),
            None => local_wall(ms),
        };
        let (a, b) = (wall(s_ms), wall(e_ms));
        item.insert("date".into(), json!(a.date));
        item.insert("time".into(), json!(a.time));
        if end.is_some() {
            item.insert("end_date".into(), json!(b.date));
            item.insert("end".into(), json!(b.time));
        }
        if let Some(tz) = tz {
            item.insert("tz".into(), json!(tz));
        }
        let alert = match own(&o) {
            Own::Default => before,
            Own::None => None,
            Own::Minutes(m) => Some(m),
        };
        if let Some(m) = alert {
            item.insert("alert".into(), json!(m));
        }
        out.push(Json::Object(item));
    }
    out
}

/// "webcal://…" and "http://…" -> "https://…"; anything else isn't a calendar link.
pub fn calendar_url(text: &str) -> Option<String> {
    let mut u = text.trim().to_string();
    let low = u.to_ascii_lowercase();
    if low.starts_with("webcal://") {
        u = format!("https:{}", &u[7..]);
    } else if low.starts_with("webcals://") {
        u = format!("https:{}", &u[8..]);
    }
    if u.to_ascii_lowercase().starts_with("http://") {
        u = format!("https:{}", &u[5..]);
    }
    if !u.to_ascii_lowercase().starts_with("https://") {
        u = format!("https://{}", u.trim_start_matches('/'));
    }
    // As the URL parser writes it: scheme and host lowercase, "/" for an empty path.
    let rest = &u[8..];
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].to_lowercase();
    if host.is_empty() || host.contains(char::is_whitespace) || !host.contains('.') {
        return None;
    }
    let mut path = rest[end..].to_string();
    if !path.starts_with('/') {
        path = format!("/{path}");
    }
    Some(format!("https://{host}{path}"))
}
