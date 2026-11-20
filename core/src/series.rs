//! Editing events (app/lib/series.js): what the event sheet shows, and the
//! records to write when it is saved or an event is deleted, including "only
//! this one" or "all" for a repeating event. A change to one occurrence is a
//! record of its own (`recurrence_of` the series, `recurrence_date` the
//! occurrence it replaces); deleting one occurrence adds its date to the
//! series' `exdates`.

use serde_json::{Map, Value as Json, json};

use crate::instances::truthy;
use crate::recur::{Part, Rule, describe, parse_rule, rule_text, supported};
use crate::zones::{add_days, days_between, from_local, local_instant, utc_parts, weekday, zoned_instant};

/// What the app adds to an event on screen. Everything else belongs to the
/// record, including fields a newer version writes: an edit here keeps them.
const ADDED: [&str; 14] = ["id", "container", "calendar", "key", "occurrence", "series", "row", "continues", "goesOn", "readonly", "repeat", "repeatWas", "changed", "extra"];
/// What the event sheet sets; the rest of a record (people, links …) stays as it was.
const SHEET: [&str; 14] = ["title", "notes", "location", "all_day", "date", "end_date", "start", "end", "tz", "rrule", "exdates", "recurrence_of", "recurrence_date", "uid"];
const WEEKDAYS: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];

fn s<'a>(e: &'a Json, k: &str) -> Option<&'a str> {
    e.get(k).and_then(Json::as_str)
}

fn empty(v: &Json) -> bool {
    match v {
        Json::Null => true,
        Json::String(s) => s.is_empty(),
        Json::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// The record to store for an event: without what the app adds, empty values left out.
pub fn record(e: &Json) -> Json {
    let mut r = Map::new();
    for (k, v) in e.as_object().into_iter().flatten() {
        if !ADDED.contains(&k.as_str()) && !empty(v) {
            r.insert(k.clone(), v.clone());
        }
    }
    if truthy(r.get("all_day")) {
        r.remove("start");
        r.remove("end");
    }
    if let (Some(end), Some(date)) = (r.get("end_date").and_then(Json::as_str), r.get("date").and_then(Json::as_str)) {
        if end <= date {
            r.remove("end_date");
        }
    }
    r.remove("id");
    Json::Object(r)
}

fn mins(t: &str) -> i64 {
    t.get(0..2).and_then(|x| x.parse().ok()).unwrap_or(0) * 60 + t.get(3..5).and_then(|x| x.parse().ok()).unwrap_or(0)
}

fn hhmm(m: i64) -> String {
    format!("{:02}:{:02}", (m / 60) % 24, m % 60)
}

fn weekday_of(date: &str) -> &'static str {
    WEEKDAYS[weekday(date) as usize]
}

/// JS's `Number(x)` for a value from the sheet (None for NaN).
fn number(v: Option<&Json>) -> Option<f64> {
    match v? {
        Json::Number(n) => n.as_f64(),
        Json::String(s) if s.trim().is_empty() => Some(0.0),
        Json::String(s) => s.trim().parse().ok(),
        Json::Bool(b) => Some(*b as i64 as f64),
        Json::Null => Some(0.0),
        _ => None,
    }
}

fn js_number_json(n: f64) -> Json {
    if n.fract() == 0.0 { json!(n as i64) } else { json!(n) }
}

/// An object written with its keys in a given order (JS keeps insertion order;
/// `repeatWas` is compared as that text).
fn ordered_text(pairs: &[(&str, Json)]) -> String {
    let parts: Vec<String> = pairs.iter().map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), serde_json::to_string(v).unwrap())).collect();
    format!("{{{}}}", parts.join(","))
}

/// The sheet's repeat settings from a rule, as ordered pairs. Rules the
/// sheet can't show in full are kept as they are ("Custom").
fn repeat_pairs(rrule: Option<&str>, date: &str) -> Vec<(&'static str, Json)> {
    let none = vec![
        ("freq", json!("NONE")),
        ("interval", json!(1)),
        ("days", json!([weekday_of(date)])),
        ("ends", json!("never")),
        ("until", json!(add_days(date, 90))),
        ("count", json!(10)),
    ];
    let Some(rrule) = rrule.filter(|r| !r.is_empty()) else { return none };
    let r = parse_rule(rrule);
    let freq = r.one("FREQ").unwrap_or("");
    let mut allowed = vec!["FREQ", "INTERVAL", "COUNT", "UNTIL", "WKST"];
    if freq == "WEEKLY" {
        allowed.push("BYDAY");
    }
    let simple = supported(rrule) && r.0.iter().all(|(k, _)| allowed.contains(&k.as_str())) && r.list("BYDAY").is_none_or(|d| d.iter().all(|x| WEEKDAYS.contains(&x.as_str())));
    if !simple {
        let mut p = none;
        p[0].1 = json!("CUSTOM");
        p.push(("raw", json!(rrule)));
        p.push(("label", json!(describe(rrule))));
        return p;
    }
    let u = r.one("UNTIL").filter(|u| !u.is_empty());
    let count = r.one("COUNT").filter(|c| !c.is_empty());
    let g = |u: &str, a: usize, b: usize| u.get(a..b.min(u.len())).unwrap_or("").to_string();
    let mut p = vec![
        ("freq", json!(freq)),
        ("interval", js_number_json(r.one("INTERVAL").and_then(|x| x.trim().parse::<f64>().ok()).filter(|x| *x != 0.0).unwrap_or(1.0))),
        ("days", match r.list("BYDAY") {
            Some(d) => json!(d),
            None => json!([weekday_of(date)]),
        }),
        ("ends", json!(if count.is_some() { "count" } else if u.is_some() { "until" } else { "never" })),
        ("until", json!(u.map(|u| format!("{}-{}-{}", g(u, 0, 4), g(u, 4, 6), g(u, 6, 8))).unwrap_or_else(|| add_days(date, 90)))),
        ("count", js_number_json(count.and_then(|c| c.trim().parse::<f64>().ok()).filter(|x| *x != 0.0).unwrap_or(10.0))),
    ];
    if let Some(w) = r.one("WKST").filter(|w| !w.is_empty()) {
        p.push(("wkst", json!(w)));
    }
    p
}

fn pairs_json(p: &[(&str, Json)]) -> Json {
    Json::Object(p.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

/// The sheet's repeat settings from a rule.
pub fn repeat_from(rrule: Option<&str>, date: &str) -> Json {
    pairs_json(&repeat_pairs(rrule, date))
}

/// The rule text for the sheet's repeat settings (None for "Never").
pub fn rule_from(rep: &Json, ev: &Json) -> Option<String> {
    let freq = s(rep, "freq").unwrap_or("NONE");
    if rep.is_null() || freq == "NONE" {
        return None;
    }
    if freq == "CUSTOM" {
        return s(rep, "raw").map(String::from);
    }
    let mut r = Rule::default();
    r.set("FREQ", Part::One(freq.into()));
    let interval = number(rep.get("interval")).unwrap_or(f64::NAN);
    if interval > 1.0 {
        r.set("INTERVAL", Part::One(json_string(rep.get("interval"))));
    }
    let date = s(ev, "date").unwrap_or("");
    if freq == "WEEKLY" {
        let mut days: Vec<String> = rep.get("days").and_then(Json::as_array).into_iter().flatten().filter_map(|d| d.as_str().map(String::from)).collect();
        if days.is_empty() {
            days = vec![weekday_of(date).into()];
        }
        if days.len() > 1 || days[0] != weekday_of(date) {
            r.set("BYDAY", Part::List(WEEKDAYS.iter().filter(|d| days.iter().any(|x| x == *d)).map(|d| d.to_string()).collect()));
        }
    }
    let ends = s(rep, "ends").unwrap_or("never");
    if ends == "count" {
        let n = number(rep.get("count")).filter(|x| *x != 0.0 && !x.is_nan()).unwrap_or(1.0).max(1.0);
        r.set("COUNT", Part::One(crate::recur::js_num(n)));
    }
    if let Some(until) = s(rep, "until").filter(|u| ends == "until" && !u.is_empty()) {
        if truthy(ev.get("all_day")) {
            r.set("UNTIL", Part::One(until.replace('-', "")));
        } else {
            // With a start time the end has to be a moment too: the end of that day, in UTC.
            let ms = match s(ev, "tz").filter(|t| !t.is_empty()) {
                Some(tz) => zoned_instant(until, Some("23:59"), tz),
                None => local_instant(until, Some("23:59")),
            };
            let (y, m, d, h, mi) = utc_parts(ms);
            let sec = ms.rem_euclid(60_000) / 1000;
            r.set("UNTIL", Part::One(format!("{y:04}{m:02}{d:02}T{h:02}{mi:02}{sec:02}Z")));
        }
    }
    if let Some(w) = s(rep, "wkst").filter(|w| !w.is_empty()) {
        r.set("WKST", Part::One(w.into()));
    }
    Some(rule_text(&r))
}

fn json_string(v: Option<&Json>) -> String {
    match v {
        Some(Json::String(s)) => s.clone(),
        Some(Json::Number(n)) => n.as_f64().map(crate::recur::js_num).unwrap_or_default(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn or_null(v: Option<&Json>) -> Json {
    v.cloned().unwrap_or(Json::Null)
}

/// `a ?? b` for JSON values (null and missing both fall through).
fn coalesce(a: Option<&Json>, b: Json) -> Json {
    match a {
        Some(Json::Null) | None => b,
        Some(v) => v.clone(),
    }
}

/// The event sheet for an occurrence on screen (from `expand`), or for a new
/// event (no `id`). `events`: all stored events, to find a changed
/// occurrence's series.
pub fn editor_for(o: &Json, events: &[Json]) -> Json {
    let series = match o.get("series").filter(|x| !x.is_null()) {
        Some(sr) => sr.clone(),
        None if truthy(o.get("recurrence_of")) => events
            .iter()
            .find(|m| m.get("id") == o.get("recurrence_of") && m.get("container") == o.get("container"))
            .cloned()
            .unwrap_or(Json::Null),
        None => Json::Null,
    };
    let (rr, date) = if series.is_null() { (s(o, "rrule"), s(o, "date").unwrap_or("")) } else { (s(&series, "rrule").or(s(o, "rrule")), s(&series, "date").or(s(o, "date")).unwrap_or("")) };
    let rep = repeat_pairs(rr, date);
    let source = if truthy(o.get("recurrence_of")) || series.is_null() { o } else { &series };
    let extra: Map<String, Json> = record(source).as_object().cloned().unwrap_or_default().into_iter().filter(|(k, _)| !SHEET.contains(&k.as_str())).collect();
    let mut ed = Map::new();
    let put = |ed: &mut Map<String, Json>, k: &str, v: Json| {
        ed.insert(k.into(), v);
    };
    put(&mut ed, "id", or_null(o.get("id")));
    put(&mut ed, "container", or_null(o.get("container")));
    put(&mut ed, "calendar", coalesce(o.get("calendar"), or_null(o.get("container"))));
    put(&mut ed, "title", coalesce(o.get("title"), json!("")));
    put(&mut ed, "notes", coalesce(o.get("notes"), json!("")));
    put(&mut ed, "location", coalesce(o.get("location"), json!("")));
    put(&mut ed, "all_day", json!(truthy(o.get("all_day"))));
    put(&mut ed, "date", or_null(o.get("date")));
    put(&mut ed, "end_date", coalesce(o.get("end_date"), or_null(o.get("date"))));
    put(&mut ed, "start", coalesce(o.get("start"), json!("09:00")));
    put(&mut ed, "end", coalesce(o.get("end"), json!("10:00")));
    put(&mut ed, "repeat", pairs_json(&rep));
    put(&mut ed, "repeatWas", json!(ordered_text(&rep)));
    put(&mut ed, "series", series.clone());
    put(&mut ed, "occurrence", coalesce(o.get("occurrence"), or_null(o.get("recurrence_date"))));
    put(&mut ed, "changed", json!(truthy(o.get("recurrence_of"))));
    put(&mut ed, "uid", or_null(o.get("uid")));
    put(&mut ed, "readonly", json!(truthy(o.get("readonly"))));
    put(&mut ed, "extra", Json::Object(extra));
    // JS leaves out what is undefined (`id`, `container`, `uid` of a new event).
    for k in ["id", "container", "calendar", "date", "end_date", "uid", "series", "occurrence"] {
        if ed.get(k) == Some(&Json::Null) && !o.as_object().is_some_and(|m| m.get(k).is_some_and(|v| v.is_null())) {
            let keep_null = matches!(k, "series" | "occurrence");
            if !keep_null {
                ed.remove(k);
            }
        }
    }
    Json::Object(ed)
}

fn repeat_changed(ed: &Json) -> bool {
    let was: Json = s(ed, "repeatWas").and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Json::Null);
    ed.get("repeat").cloned().unwrap_or(Json::Null) != was
}

/// Whether saving needs "only this one or all?" first. A change of calendar
/// or of the repeat rule can only be made to the whole series.
pub fn asks_scope(ed: &Json) -> bool {
    let Some(sr) = ed.get("series").filter(|x| !x.is_null()) else { return false };
    ed.get("calendar") == sr.get("container") && !repeat_changed(ed)
}

/// The times from the sheet, made consistent (an end before the start moves).
fn times(ed: &Json) -> Map<String, Json> {
    let mut e = Map::new();
    let title = s(ed, "title").unwrap_or("").trim().to_string();
    e.insert("title".into(), json!(if title.is_empty() { "New event".to_string() } else { title }));
    e.insert("notes".into(), or_null(ed.get("notes")));
    e.insert("location".into(), or_null(ed.get("location")));
    e.insert("all_day".into(), or_null(ed.get("all_day")));
    let date = s(ed, "date").unwrap_or("").to_string();
    e.insert("date".into(), json!(date));
    let mut last = match s(ed, "end_date") {
        Some(x) if !x.is_empty() && x > date.as_str() => x.to_string(),
        _ => date.clone(),
    };
    if truthy(ed.get("all_day")) {
        e.insert("end_date".into(), json!(last));
        return e;
    }
    let start = s(ed, "start").unwrap_or("09:00").to_string();
    let mut end = s(ed, "end").unwrap_or("10:00").to_string();
    if last == date && mins(&end) <= mins(&start) {
        // "22:00 to 01:00" on one date means overnight; anything else gets an hour.
        if mins(&start) >= 18 * 60 && mins(&end) < 6 * 60 {
            last = add_days(&date, 1);
        } else {
            end = hhmm((mins(&start) + 60).min(23 * 60 + 59));
        }
    }
    e.insert("start".into(), json!(start));
    e.insert("end".into(), json!(end));
    e.insert("end_date".into(), json!(last));
    e
}

fn merge(base: &Json, over: &Map<String, Json>) -> Map<String, Json> {
    let mut m = base.as_object().cloned().unwrap_or_default();
    for (k, v) in over {
        m.insert(k.clone(), v.clone());
    }
    m
}

fn put_op(container: &Json, id: Option<&Json>, value: Json) -> Json {
    let mut o = Map::new();
    o.insert("put".into(), container.clone());
    if let Some(id) = id.filter(|i| !i.is_null()) {
        o.insert("id".into(), id.clone());
    }
    o.insert("value".into(), value);
    Json::Object(o)
}

fn delete_op(container: &Json, id: &Json) -> Json {
    json!({ "delete": container, "id": id })
}

/// The records to write when the sheet is saved: a list of
/// { put: container, id, value } and { delete: container, id }.
/// `scope` is "this" or "all" for an occurrence of a repeating event.
pub fn plan_save(ed: &Json, scope: Option<&str>, events: &[Json]) -> Json {
    let t = times(ed);
    let mut ops = vec![];
    let extra = ed.get("extra").cloned().unwrap_or(json!({}));
    let Some(sr) = ed.get("series").filter(|x| !x.is_null()) else {
        let mut v = merge(&extra, &t);
        v.insert("rrule".into(), rule_from(&ed["repeat"], &Json::Object(t.clone())).map_or(Json::Null, |r| json!(r)));
        v.insert("uid".into(), or_null(ed.get("uid")));
        ops.push(put_op(&or_null(ed.get("calendar")), ed.get("id"), record(&Json::Object(v))));
        if truthy(ed.get("id")) && truthy(ed.get("container")) && ed.get("container") != ed.get("calendar") {
            ops.push(delete_op(&ed["container"], &ed["id"]));
        }
        return Json::Array(ops);
    };
    let occurrence = s(ed, "occurrence").unwrap_or("");
    let s_tz = s(sr, "tz").filter(|x| !x.is_empty());
    let all_day = truthy(t.get("all_day"));
    let ts = |k: &str| t.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    if scope == Some("this") {
        let id = if truthy(ed.get("changed")) { or_null(ed.get("id")) } else { json!(format!("{}.{}", s(sr, "id").unwrap_or(""), occurrence.replace('-', ""))) };
        let mut when = t.clone();
        if let (Some(tz), false) = (s_tz, all_day) {
            let a = from_local(&ts("date"), Some(&ts("start")), Some(tz));
            let b = from_local(&ts("end_date"), Some(&ts("end")), Some(tz));
            when.insert("date".into(), json!(a.date));
            when.insert("start".into(), json!(a.time));
            when.insert("end_date".into(), json!(b.date));
            when.insert("end".into(), json!(b.time));
            when.insert("tz".into(), json!(tz));
        }
        let mut v = merge(&extra, &when);
        v.insert("recurrence_of".into(), or_null(sr.get("id")));
        v.insert("recurrence_date".into(), or_null(ed.get("occurrence")));
        v.insert("uid".into(), or_null(sr.get("uid")));
        ops.push(put_op(&or_null(sr.get("container")), Some(&id), record(&Json::Object(v))));
        return Json::Array(ops);
    }
    // All of them: the series moves by as much as this occurrence was moved.
    let (a_date, a_time) = if all_day {
        (ts("date"), None)
    } else {
        let a = from_local(&ts("date"), Some(&ts("start")), s_tz);
        (a.date, Some(a.time))
    };
    let shift = days_between(occurrence, &a_date);
    let date = add_days(s(sr, "date").unwrap_or(""), shift);
    let mut master = record(sr).as_object().cloned().unwrap_or_default();
    for k in ["title", "notes", "location", "all_day"] {
        master.insert(k.into(), or_null(t.get(k)));
    }
    master.insert("date".into(), json!(date));
    if all_day {
        master.insert("end_date".into(), json!(add_days(&date, days_between(&ts("date"), &ts("end_date")))));
        master.remove("tz");
    } else {
        let b = from_local(&ts("end_date"), Some(&ts("end")), s_tz);
        master.insert("start".into(), json!(a_time));
        master.insert("end".into(), json!(b.time));
        master.insert("end_date".into(), json!(add_days(&date, days_between(&a_date, &b.date))));
        if truthy(sr.get("all_day")) {
            master.remove("tz"); // was all-day: now in this device's time
        }
    }
    // An untouched rule is kept word for word (Google's own spelling of it).
    if repeat_changed(ed) {
        match rule_from(&ed["repeat"], &Json::Object(master.clone())) {
            Some(r) => master.insert("rrule".into(), json!(r)),
            None => master.remove("rrule"),
        };
    } else {
        master.insert("rrule".into(), or_null(sr.get("rrule")));
    }
    if shift != 0 {
        if let Some(ex) = master.get("exdates").and_then(Json::as_array).cloned() {
            master.insert("exdates".into(), json!(ex.iter().filter_map(Json::as_str).map(|x| add_days(x, shift)).collect::<Vec<_>>()));
        }
    }
    let value = record(&Json::Object(master));
    let others: Vec<&Json> = events.iter().filter(|e| e.get("recurrence_of") == sr.get("id") && e.get("container") == sr.get("container")).collect();
    let timing_changed = shift != 0 || value.get("rrule") != sr.get("rrule") || !truthy(value.get("rrule"));
    let to = coalesce(ed.get("calendar"), or_null(sr.get("container")));
    let from_c = or_null(sr.get("container"));
    ops.push(put_op(&to, sr.get("id"), value));
    for o in others {
        let drop = timing_changed || (truthy(ed.get("changed")) && o.get("id") == ed.get("id"));
        if drop {
            ops.push(delete_op(&or_null(o.get("container")), &or_null(o.get("id"))));
        } else if to != from_c {
            ops.push(put_op(&to, o.get("id"), record(o)));
            ops.push(delete_op(&or_null(o.get("container")), &or_null(o.get("id"))));
        }
    }
    if to != from_c {
        ops.push(delete_op(&from_c, &or_null(sr.get("id"))));
    }
    Json::Array(ops)
}

/// The records to write when an event is deleted (`scope` as for saving).
pub fn plan_delete(ed: &Json, scope: Option<&str>, events: &[Json]) -> Json {
    let Some(sr) = ed.get("series").filter(|x| !x.is_null()) else {
        return json!([delete_op(&or_null(ed.get("container")), &or_null(ed.get("id")))]);
    };
    if scope == Some("this") {
        let mut ex: Vec<String> = sr.get("exdates").and_then(Json::as_array).into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        if let Some(o) = s(ed, "occurrence") {
            if !ex.iter().any(|x| x == o) {
                ex.push(o.into());
            }
        }
        ex.sort();
        let mut v = sr.as_object().cloned().unwrap_or_default();
        v.insert("exdates".into(), json!(ex));
        let mut ops = vec![put_op(&or_null(sr.get("container")), sr.get("id"), record(&Json::Object(v)))];
        if truthy(ed.get("changed")) {
            ops.push(delete_op(&or_null(sr.get("container")), &or_null(ed.get("id"))));
        }
        return Json::Array(ops);
    }
    let mut ops = vec![delete_op(&or_null(sr.get("container")), &or_null(sr.get("id")))];
    for e in events.iter().filter(|e| e.get("recurrence_of") == sr.get("id") && e.get("container") == sr.get("container")) {
        ops.push(delete_op(&or_null(e.get("container")), &or_null(e.get("id"))));
    }
    Json::Array(ops)
}
