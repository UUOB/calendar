//! From stored events to what each day on screen shows (app/lib/instances.js).
//! Repeating events become their occurrences, events in another time zone
//! move to this device's time, and events longer than a day become one piece
//! per day: on the all-day row when they last a day or more, otherwise in the
//! hour grid (an evening event that ends after midnight shows on both days).

use std::collections::HashSet;

use serde_json::{Map, Value as Json, json};

use crate::recur::occurrences;
use crate::zones::{add_days, days_between, to_local};

fn s<'a>(e: &'a Json, k: &str) -> Option<&'a str> {
    e.get(k).and_then(Json::as_str).filter(|x| !x.is_empty())
}

pub fn truthy(v: Option<&Json>) -> bool {
    match v {
        None | Some(Json::Null) => false,
        Some(Json::Bool(b)) => *b,
        Some(Json::String(x)) => !x.is_empty(),
        Some(Json::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        _ => true,
    }
}

pub fn mins(t: Option<&str>) -> i64 {
    match t {
        Some(t) if !t.is_empty() => t.get(0..2).and_then(|x| x.parse().ok()).unwrap_or(0) * 60 + t.get(3..5).and_then(|x| x.parse().ok()).unwrap_or(0),
        _ => 0,
    }
}

/// The event's last day (its first, for a one-day event).
pub fn last_day(e: &Json) -> String {
    let date = s(e, "date").unwrap_or("");
    match s(e, "end_date") {
        Some(end) if end > date => end.into(),
        _ => date.into(),
    }
}

/// How long a timed event lasts, in minutes.
pub fn length(e: &Json) -> i64 {
    days_between(s(e, "date").unwrap_or(""), &last_day(e)) * 1440 + mins(s(e, "end")) - mins(s(e, "start"))
}

/// Every occurrence of `events` that touches `from`..`to`, on this device's
/// clock: the event's record with its own `date`, `end_date`, `start` and
/// `end`, plus `key` (unique on screen), `occurrence` (its date in the
/// series, on the event's own clock) and `series` (the repeating record).
pub fn expand(events: &[Json], from: &str, to: &str) -> Vec<Json> {
    let changed: HashSet<String> = events
        .iter()
        .filter(|e| truthy(e.get("recurrence_of")))
        .map(|e| format!("{}|{}|{}", js_str(e.get("recurrence_of")), js_str(e.get("recurrence_date")), s(e, "container").unwrap_or("")))
        .collect();
    let mut out = vec![];
    let add = |e: &Json, date: &str, extra: Vec<(&str, Json)>, out: &mut Vec<Json>| {
        let span = days_between(s(e, "date").unwrap_or(""), &last_day(e));
        let mut o = e.as_object().cloned().unwrap_or_default();
        if truthy(e.get("all_day")) {
            o.insert("date".into(), json!(date));
            o.insert("end_date".into(), json!(add_days(date, span)));
        } else {
            let tz = s(e, "tz");
            let a = to_local(date, e.get("start").and_then(Json::as_str), tz);
            let b = to_local(&add_days(date, span), e.get("end").and_then(Json::as_str), tz);
            o.insert("date".into(), json!(a.date));
            put_time(&mut o, "start", &a.time, e.get("start"), tz);
            o.insert("end_date".into(), json!(b.date));
            put_time(&mut o, "end", &b.time, e.get("end"), tz);
        }
        if o.get("end_date") == o.get("date") {
            o.remove("end_date");
        }
        let oj = Json::Object(o);
        if s(&oj, "date").unwrap_or("") > to || last_day(&oj).as_str() < from {
            return;
        }
        let mut o = oj.as_object().cloned().unwrap();
        for (k, v) in extra {
            o.insert(k.into(), v);
        }
        out.push(Json::Object(o));
    };
    for e in events {
        let Some(date) = s(e, "date") else { continue };
        let container = s(e, "container").unwrap_or("");
        let id = js_str(e.get("id"));
        if truthy(e.get("recurrence_of")) {
            add(e, date, vec![("key", json!(format!("{container}/{id}"))), ("occurrence", e.get("recurrence_date").cloned().unwrap_or(Json::Null))], &mut out);
            continue;
        }
        let span = days_between(date, &last_day(e));
        // Look a little wider: a long event, or a time zone, can move an occurrence onto the screen.
        let rrule = truthy(e.get("rrule"));
        for d in occurrences(e, &add_days(from, -span - 1), &add_days(to, 1)) {
            if rrule && changed.contains(&format!("{id}|{d}|{container}")) {
                continue;
            }
            let mut extra = vec![("key", json!(format!("{container}/{id}/{d}"))), ("occurrence", json!(d))];
            if rrule {
                extra.push(("series", e.clone()));
            }
            add(e, &d, extra, &mut out);
        }
    }
    out
}

/// JS's toLocal leaves `time` out when the event had none (and no zone).
fn put_time(o: &mut Map<String, Json>, k: &str, t: &str, had: Option<&Json>, tz: Option<&str>) {
    if tz.is_none() && had.and_then(Json::as_str).is_none() {
        match had {
            Some(v) => o.insert(k.into(), v.clone()),
            None => o.remove(k),
        };
    } else {
        o.insert(k.into(), json!(t));
    }
}

/// A value as JS writes it in a template string.
pub fn js_str(v: Option<&Json>) -> String {
    match v {
        None => "undefined".into(),
        Some(Json::Null) => "null".into(),
        Some(Json::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// What each day from `from` to `to` shows: date -> { allDay: [...], timed: [...] }.
/// Timed pieces have the `start` and `end` of that day ("24:00" when they go
/// on past midnight); pieces know whether they `continues` from the day
/// before or `goesOn` to the next.
pub fn by_day(instances: &[Json], from: &str, to: &str) -> Vec<(String, Vec<Json>, Vec<Json>)> {
    let mut days: Vec<(String, Vec<Json>, Vec<Json>)> = vec![];
    let mut d = from.to_string();
    while d.as_str() <= to {
        days.push((d.clone(), vec![], vec![]));
        d = add_days(&d, 1);
    }
    for o in instances {
        let last = last_day(o);
        let date = s(o, "date").unwrap_or("").to_string();
        let on_row = truthy(o.get("all_day")) || length(o) >= 1440;
        let mut d = if date.as_str() > from { date.clone() } else { from.to_string() };
        while d <= last && d.as_str() <= to {
            if let Some(day) = days.iter_mut().find(|x| x.0 == d) {
                let mut p = o.as_object().cloned().unwrap_or_default();
                if on_row {
                    p.insert("row".into(), json!(true));
                    p.insert("continues".into(), json!(d > date));
                    p.insert("goesOn".into(), json!(d < last));
                    day.1.push(Json::Object(p));
                } else {
                    let start = if d == date { o.get("start").cloned().unwrap_or(Json::Null) } else { json!("00:00") };
                    let end = if d == last { o.get("end").cloned().unwrap_or(Json::Null) } else { json!("24:00") };
                    if !(d != date && end == json!("00:00")) {
                        // (ends exactly at midnight: nothing on the next day)
                        p.insert("start".into(), start);
                        p.insert("end".into(), end);
                        p.insert("continues".into(), json!(d > date));
                        p.insert("goesOn".into(), json!(d < last));
                        day.2.push(Json::Object(p));
                    }
                }
            }
            d = add_days(&d, 1);
        }
    }
    for day in days.iter_mut() {
        day.1.sort_by(|a, b| {
            s(a, "date").unwrap_or("").cmp(s(b, "date").unwrap_or("")).then_with(|| last_day(b).cmp(&last_day(a))).then_with(|| {
                crate::details::locale_cmp(a.get("title").and_then(Json::as_str).unwrap_or(""), b.get("title").and_then(Json::as_str).unwrap_or(""))
            })
        });
    }
    days
}

/// [`by_day`] as JSON: { date: { allDay, timed } }.
pub fn by_day_json(days: &[(String, Vec<Json>, Vec<Json>)]) -> Json {
    let mut m = Map::new();
    for (d, a, t) in days {
        m.insert(d.clone(), json!({ "allDay": a, "timed": t }));
    }
    Json::Object(m)
}
