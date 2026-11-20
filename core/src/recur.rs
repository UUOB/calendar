//! Repeating events (app/lib/recur.js): the part of iCalendar's RRULE that
//! calendars use. A repeating event is stored once, with its rule (`rrule`,
//! e.g. "FREQ=WEEKLY;BYDAY=MO,WE;UNTIL=20270101"), the dates taken out of the
//! series (`exdates`) and, as records of their own, the occurrences changed
//! on their own. Occurrences are worked out only for the days on screen.
//!
//! Understood: FREQ=DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL, COUNT, UNTIL,
//! BYDAY (also "2TU", "-1FR"), BYMONTHDAY (also negative), BYMONTH, BYSETPOS
//! and WKST. Anything else is reported by `supported`; such an event shows
//! only its first occurrence.

use serde_json::{Map, Value as Json, json};

use crate::zones::{self, date_utc, utc_date, utc_parts, zoned_wall};

const DAYS: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];
const KNOWN: [&str; 9] = ["FREQ", "INTERVAL", "COUNT", "UNTIL", "BYDAY", "BYMONTHDAY", "BYMONTH", "BYSETPOS", "WKST"];
const FREQS: [&str; 4] = ["DAILY", "WEEKLY", "MONTHLY", "YEARLY"];
const LISTS: [&str; 4] = ["BYDAY", "BYMONTHDAY", "BYMONTH", "BYSETPOS"];
/// Occurrences looked at, at most, per event and screen.
const MAX: usize = 5000;
const DAY_MS: i64 = 86_400_000;

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    One(String),
    List(Vec<String>),
}

/// A rule: its parts in the order they were written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rule(pub Vec<(String, Part)>);

impl Rule {
    pub fn get(&self, k: &str) -> Option<&Part> {
        self.0.iter().find(|(x, _)| x == k).map(|(_, v)| v)
    }
    pub fn one(&self, k: &str) -> Option<&str> {
        match self.get(k)? {
            Part::One(s) => Some(s),
            Part::List(_) => None,
        }
    }
    pub fn list(&self, k: &str) -> Option<&[String]> {
        match self.get(k)? {
            Part::List(v) => Some(v),
            Part::One(_) => None,
        }
    }
    pub fn has(&self, k: &str) -> bool {
        self.get(k).is_some()
    }
    pub fn set(&mut self, k: &str, v: Part) {
        match self.0.iter_mut().find(|(x, _)| x == k) {
            Some(slot) => slot.1 = v,
            None => self.0.push((k.into(), v)),
        }
    }

    pub fn to_json(&self) -> Json {
        let mut m = Map::new();
        for (k, v) in &self.0 {
            m.insert(k.clone(), match v {
                Part::One(s) => json!(s),
                Part::List(l) => json!(l),
            });
        }
        Json::Object(m)
    }

    /// From an object like `{ FREQ: 'WEEKLY', BYDAY: ['MO'] }` (keys in their order).
    pub fn from_json(v: &Json) -> Rule {
        let mut r = Rule::default();
        for (k, x) in v.as_object().into_iter().flatten() {
            let part = match x {
                Json::Array(a) => Part::List(a.iter().map(|s| s.as_str().map(String::from).unwrap_or_else(|| s.to_string())).collect()),
                Json::String(s) => Part::One(s.clone()),
                other => Part::One(other.to_string()),
            };
            r.0.push((k.clone(), part));
        }
        r
    }
}

/// JS's `Number(s)` for the parts of a rule (None for NaN).
fn num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    t.parse::<f64>().ok()
}

/// The rule as its parts: { FREQ: 'WEEKLY', BYDAY: ['MO','WE'], … }.
pub fn parse_rule(text: &str) -> Rule {
    let mut r = Rule::default();
    let t = text.strip_prefix("RRULE:").or_else(|| (text.len() >= 6 && text[..6].eq_ignore_ascii_case("RRULE:")).then(|| &text[6..])).unwrap_or(text);
    for part in t.split(';') {
        let mut it = part.split('=');
        let (Some(k), Some(v)) = (it.next(), it.next()) else { continue };
        if k.is_empty() {
            continue;
        }
        let key = k.trim().to_uppercase();
        let val = v.trim().to_uppercase();
        let p = if LISTS.contains(&key.as_str()) { Part::List(val.split(',').filter(|s| !s.is_empty()).map(String::from).collect()) } else { Part::One(val) };
        r.set(&key, p);
    }
    r
}

/// The rule back as RRULE text, in a stable order.
pub fn rule_text(r: &Rule) -> String {
    let order = ["FREQ", "INTERVAL", "COUNT", "UNTIL", "BYMONTH", "BYMONTHDAY", "BYDAY", "BYSETPOS", "WKST"];
    let mut keys: Vec<&str> = order.iter().copied().filter(|k| r.has(k)).collect();
    keys.extend(r.0.iter().map(|(k, _)| k.as_str()).filter(|k| !order.contains(k)));
    keys.iter()
        .map(|k| {
            let v = match r.get(k).unwrap() {
                Part::One(s) => s.clone(),
                Part::List(l) => l.join(","),
            };
            format!("{k}={v}")
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Whether every occurrence of the rule can be shown.
pub fn supported(text: &str) -> bool {
    let r = parse_rule(text);
    r.one("FREQ").is_some_and(|f| FREQS.contains(&f)) && r.0.iter().all(|(k, _)| KNOWN.contains(&k.as_str()))
}

/// A date as a UTC midnight (ms).
fn to_utc(s: &str) -> i64 {
    let (y, m, d) = zones::ymd(s);
    date_utc(y, m - 1, d, 0, 0)
}

fn days_in(y: i64, m0: i64) -> i64 {
    let (_, _, d, _, _) = utc_parts(date_utc(y, m0 + 1, 0, 0, 0));
    d
}

fn utc_day(ms: i64) -> i64 {
    (ms.div_euclid(DAY_MS) + 4).rem_euclid(7)
}

/// The end of the series as a wall date and time in the event's zone.
fn until(r: &Rule, tz: Option<&str>) -> Option<zones::Wall> {
    let u = r.one("UNTIL")?;
    let s = |a: usize, b: usize| u.get(a..b.min(u.len())).unwrap_or("").to_string();
    let date = format!("{}-{}-{}", s(0, 4), s(4, 6), s(6, 8));
    if u.chars().count() <= 8 {
        return Some(zones::Wall { date, time: "23:59".into() });
    }
    let time = format!("{}:{}", s(9, 11), s(11, 13));
    if u.ends_with('Z') {
        let n = |a, b| s(a, b).parse::<i64>().unwrap_or(0);
        let ms = date_utc(n(0, 4), n(4, 6) - 1, n(6, 8), n(9, 11), n(11, 13));
        return Some(match tz.filter(|t| !t.is_empty()) {
            Some(tz) => zoned_wall(ms, tz),
            None => zones::local_wall(ms), // a floating event: compare on this device's clock
        });
    }
    Some(zones::Wall { date, time })
}

/// "2TU" -> (2, 2); "FR" -> (0, 5). Day -1 for unknown letters.
fn byday(s: &str) -> Option<(i64, i64)> {
    let b = s.as_bytes();
    if b.len() < 2 {
        return None;
    }
    let (num_part, day_part) = s.split_at(s.len() - 2);
    if !day_part.bytes().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let n = if num_part.is_empty() {
        0
    } else {
        let digits = num_part.trim_start_matches(['+', '-']);
        if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|c| c.is_ascii_digit()) || num_part.len() - digits.len() > 1 {
            return None;
        }
        num_part.parse::<i64>().ok()?
    };
    Some((n, DAYS.iter().position(|d| *d == day_part).map_or(-1, |i| i as i64)))
}

/// Days of one month (y, m0) matching BYMONTHDAY / BYDAY, or the start's day without either.
fn month_days(y: i64, m0: i64, r: &Rule, start_day: i64) -> Vec<i64> {
    let n = days_in(y, m0);
    let mut days: Option<Vec<i64>> = None;
    if let Some(md) = r.list("BYMONTHDAY") {
        days = Some(
            md.iter()
                .filter_map(|s| num(s))
                .filter(|d| d.fract() == 0.0)
                .map(|d| d as i64)
                .map(|d| if d < 0 { n + d + 1 } else { d })
                .filter(|d| *d >= 1 && *d <= n)
                .collect(),
        );
    }
    if let Some(bd) = r.list("BYDAY") {
        let mut set: Vec<i64> = vec![];
        for (k, day) in bd.iter().filter_map(|s| byday(s)) {
            let all: Vec<i64> = (1..=n).filter(|d| utc_day(date_utc(y, m0, *d, 0, 0)) == day).collect();
            let mut push = |d: i64| {
                if !set.contains(&d) {
                    set.push(d)
                }
            };
            if k == 0 {
                all.iter().for_each(|d| push(*d));
            } else {
                let idx = if k > 0 { k - 1 } else { all.len() as i64 + k };
                if idx >= 0 {
                    if let Some(d) = all.get(idx as usize) {
                        push(*d);
                    }
                }
            }
        }
        days = Some(match days {
            Some(d) => d.into_iter().filter(|x| set.contains(x)).collect(),
            None => set,
        });
    }
    let mut days = days.unwrap_or_else(|| if start_day <= n { vec![start_day] } else { vec![] });
    days.sort();
    days.dedup();
    days
}

/// Keep the BYSETPOS-th of one period's candidates (1 = first, -1 = last).
fn setpos(list: Vec<i64>, r: &Rule) -> Vec<i64> {
    let Some(sp) = r.list("BYSETPOS") else { return list };
    let mut out: Vec<i64> = sp
        .iter()
        .filter_map(|s| num(s))
        .filter(|p| p.fract() == 0.0)
        .map(|p| p as i64)
        .filter_map(|p| {
            let idx = if p > 0 { p - 1 } else { list.len() as i64 + p };
            if idx < 0 { None } else { list.get(idx as usize).copied() }
        })
        .filter(|v| *v != 0)
        .collect();
    out.sort();
    out
}

/// Candidate dates (UTC midnights, ms) of each period, in order, from the start onward.
struct Periods<'a> {
    r: &'a Rule,
    freq: String,
    interval: i64,
    months: Option<Vec<i64>>,
    plain_days: Option<Vec<i64>>,
    s: i64,
    sy: i64,
    sm: i64,
    sd: i64,
    i: i64,
}

impl<'a> Periods<'a> {
    fn new(start: &str, r: &'a Rule) -> Periods<'a> {
        let interval = r.one("INTERVAL").and_then(num).filter(|n| *n >= 1.0).map_or(1, |n| n as i64).max(1);
        let s = to_utc(start);
        let (sy, sm, sd, _, _) = utc_parts(s);
        Periods {
            r,
            freq: r.one("FREQ").unwrap_or("").to_string(),
            interval,
            months: r.list("BYMONTH").map(|l| l.iter().filter_map(|x| num(x)).map(|x| x as i64).collect()),
            plain_days: r.list("BYDAY").map(|l| l.iter().filter_map(|x| byday(x)).map(|b| b.1).collect()),
            s,
            sy,
            sm: sm - 1,
            sd,
            i: 0,
        }
    }
}

impl Iterator for Periods<'_> {
    type Item = Vec<i64>;
    fn next(&mut self) -> Option<Vec<i64>> {
        let r = self.r;
        match self.freq.as_str() {
            "DAILY" => {
                // (A day no filter lets through would make the web version look forever; stop instead.)
                for _ in 0..200_000 {
                    let d = self.s + self.i * self.interval * DAY_MS;
                    self.i += 1;
                    let (y, m, day, _, _) = utc_parts(d);
                    if self.months.as_ref().is_some_and(|ms| !ms.contains(&m)) {
                        continue;
                    }
                    if self.plain_days.as_ref().is_some_and(|ds| !ds.contains(&utc_day(d))) {
                        continue;
                    }
                    if let Some(md) = r.list("BYMONTHDAY") {
                        let only = Rule(vec![("BYMONTHDAY".into(), Part::List(md.to_vec()))]);
                        if !month_days(y, m - 1, &only, 0).contains(&day) {
                            continue;
                        }
                    }
                    return Some(vec![d]);
                }
                None
            }
            "WEEKLY" => {
                let wkst = match r.one("WKST") {
                    Some(w) if !w.is_empty() => DAYS.iter().position(|d| *d == w).map_or(-1, |i| i as i64),
                    _ => 1,
                };
                let days: Vec<i64> = match &self.plain_days {
                    Some(d) if !d.is_empty() => d.clone(),
                    _ => vec![utc_day(self.s)],
                };
                let back = (utc_day(self.s) - wkst + 7).rem_euclid(7);
                let w = self.s - back * DAY_MS + self.i * self.interval * 7 * DAY_MS;
                self.i += 1;
                let mut list = vec![];
                for k in 0..7 {
                    let d = w + k * DAY_MS;
                    let (_, m, _, _, _) = utc_parts(d);
                    if days.contains(&utc_day(d)) && self.months.as_ref().is_none_or(|ms| ms.contains(&m)) {
                        list.push(d);
                    }
                }
                Some(setpos(list, r))
            }
            "MONTHLY" => {
                let i = self.i;
                self.i += self.interval;
                let y = self.sy + (self.sm + i).div_euclid(12);
                let m = (self.sm + i) % 12;
                if self.months.as_ref().is_some_and(|ms| !ms.contains(&(m + 1))) {
                    return Some(vec![]);
                }
                let days = setpos(month_days(y, m, r, self.sd), r);
                Some(days.into_iter().map(|d| date_utc(y, m, d, 0, 0)).collect())
            }
            "YEARLY" => {
                let y = self.sy + self.i;
                self.i += self.interval;
                let ms: Vec<i64> = match &self.months {
                    Some(ms) => ms.iter().map(|x| x - 1).collect(),
                    None => vec![self.sm],
                };
                let mut list = vec![];
                // Without BYMONTH, "BYDAY=1MO" counts in the start's month, as calendars mean it.
                for m in ms {
                    for d in month_days(y, m, r, self.sd) {
                        list.push(date_utc(y, m, d, 0, 0));
                    }
                }
                list.sort();
                Some(setpos(list, r))
            }
            _ => None,
        }
    }
}

fn str_of<'a>(e: &'a Json, k: &str) -> Option<&'a str> {
    e.get(k).and_then(Json::as_str).filter(|s| !s.is_empty())
}

fn truthy(v: Option<&Json>) -> bool {
    match v {
        None | Some(Json::Null) => false,
        Some(Json::Bool(b)) => *b,
        Some(Json::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Json::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// Start dates (on the event's own clock) of the occurrences of `event` that
/// begin between `from` and `to` (inclusive). The first occurrence is always
/// the event's own date; `exdates` are left out.
pub fn occurrences(event: &Json, from: &str, to: &str) -> Vec<String> {
    let start = event.get("date").and_then(Json::as_str).unwrap_or("");
    let rrule = str_of(event, "rrule");
    let in_range = |d: &str| d >= from && d <= to;
    let Some(rrule) = rrule.filter(|r| supported(r)) else {
        return if in_range(start) { vec![start.into()] } else { vec![] };
    };
    let r = parse_rule(rrule);
    let count = r.one("COUNT").filter(|c| !c.is_empty()).map_or(f64::INFINITY, |c| num(c).unwrap_or(f64::NAN));
    let end = until(&r, str_of(event, "tz"));
    let time = if truthy(event.get("all_day")) { "00:00".to_string() } else { str_of(event, "start").unwrap_or("00:00").to_string() };
    let ex: Vec<&str> = event.get("exdates").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).collect();
    let mut out: Vec<String> = vec![];
    let mut seen = 0.0;
    let take = |d: &str, out: &mut Vec<String>, seen: &mut f64| {
        *seen += 1.0;
        if in_range(d) && !ex.contains(&d) {
            out.push(d.to_string());
        }
    };
    take(start, &mut out, &mut seen);
    let mut looked = 0;
    'outer: for list in Periods::new(start, &r) {
        looked += 1;
        if looked > MAX * 4 {
            break;
        }
        for day in list {
            let d = utc_date(day);
            if d.as_str() <= start {
                continue;
            }
            if seen >= count || d.as_str() > to {
                break 'outer;
            }
            if let Some(e) = &end {
                if d > e.date || (d == e.date && time > e.time) {
                    break 'outer;
                }
            }
            take(&d, &mut out, &mut seen);
            if out.len() >= MAX {
                break 'outer;
            }
        }
    }
    out
}

/// What the rule says, in a few words: "Weekly on Mon, Wed", "Monthly" …
pub fn describe(text: &str) -> String {
    if text.is_empty() {
        return "Never".into();
    }
    if !supported(text) {
        return "Custom".into();
    }
    let r = parse_rule(text);
    let n = r.one("INTERVAL").and_then(num).filter(|x| *x != 0.0).unwrap_or(1.0);
    let freq = r.one("FREQ").unwrap_or("");
    let unit = match freq {
        "DAILY" => "day",
        "WEEKLY" => "week",
        "MONTHLY" => "month",
        _ => "year",
    };
    let mut s = if n == 1.0 {
        match freq {
            "DAILY" => "Daily",
            "WEEKLY" => "Weekly",
            "MONTHLY" => "Monthly",
            _ => "Yearly",
        }
        .to_string()
    } else {
        format!("Every {} {unit}s", js_num(n))
    };
    if let Some(bd) = r.list("BYDAY") {
        let names = |d: i64| match d {
            0 => "Sun",
            1 => "Mon",
            2 => "Tue",
            3 => "Wed",
            4 => "Thu",
            5 => "Fri",
            6 => "Sat",
            _ => "undefined",
        };
        let ord = |k: i64| match k {
            1 => "1st".to_string(),
            2 => "2nd".into(),
            3 => "3rd".into(),
            4 => "4th".into(),
            5 => "5th".into(),
            -1 => "last".into(),
            -2 => "2nd last".into(),
            other => other.to_string(),
        };
        let parts: Vec<String> = bd
            .iter()
            .map(|b| match byday(b) {
                Some((k, d)) => format!("{}{}", if k != 0 { format!("{} ", ord(k)) } else { String::new() }, names(d)),
                None => b.clone(),
            })
            .collect();
        s += &format!(" on {}", parts.join(", "));
    }
    if let Some(c) = r.one("COUNT").filter(|c| !c.is_empty()) {
        s += &format!(", {c} times");
    } else if let Some(u) = r.one("UNTIL").filter(|u| !u.is_empty()) {
        let g = |a: usize, b: usize| u.get(a..b.min(u.len())).unwrap_or("");
        s += &format!(", until {}.{}.{}", g(6, 8), g(4, 6), g(0, 4));
    }
    s
}

/// A number as JS writes it.
pub fn js_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}
