//! What Calendar.svelte worked out beside the library: the calendars as the
//! sidebar lists them, what a view shows (days, the month grid, overlapping
//! events side by side), the card's lines, the alert and answer keys, and
//! the writes to UUOB (events, calendars, preferences, subscriptions,
//! import). The screens of each platform only draw `screen` and call these.
//!
//! Records, all as the web version writes them:
//!   events            `events` in each calendar's container
//!   first calendar    `calendars/personal {name, color}` in the first container
//!   added calendar    `calendar/info {color, created, subscribed?, …}` in its own
//!   preferences       `prefs/alerts {before}`, `prefs/me {emails}`, `prefs/views {narrow, wide}`,
//!                     `prefs/calendar {show_tasks, hidden}` (first container)
//!   private per event `answers/<key> {answer}`, `alerts/<key> {before: minutes | false}`
//!   links             `subscriptions/<calendar> {url, refreshed}`

use serde_json::{Map, Value as Json, json};
use uuob_client::Client;

use crate::details::{answer_of, people_of};
use crate::ical::{parse_ics, write_ics};
use crate::instances::{by_day, expand, last_day, mins, truthy};
use crate::recur::describe;
use crate::series::{editor_for, plan_delete, plan_save, record};
use crate::upcoming::{DEFAULT_BEFORE, Own, before_label, calendar_url, upcoming_for};
use crate::zones::{self, add_days, days_between, weekday};
use uuob_client::dates::friendly_time;

pub const COLORS: [&str; 8] = ["blue", "red", "orange", "yellow", "green", "teal", "purple", "pink"];
/// The views, shortest first: id, name, days side by side (the month: 31, the year: 366).
pub const VIEWS: [(&str, &str, i64); 9] = [
    ("1d", "Day", 1), ("2d", "2 days", 2), ("3d", "3 days", 3), ("4d", "4 days", 4), ("5d", "5 days", 5),
    ("6d", "6 days", 6), ("1w", "Week", 7), ("month", "Month", 31), ("year", "Year", 366),
];
/// The views older copies know: the old `small` / `mid` / `big` fields hold only these.
const OLD_VIEWS: [&str; 5] = ["1d", "2d", "4d", "1w", "month"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

fn s<'a>(v: &'a Json, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Json::as_str).filter(|x| !x.is_empty())
}
fn arr(v: &Json) -> Vec<Json> {
    v.as_array().cloned().unwrap_or_default()
}
fn find<'a>(list: &'a [Json], id: &str) -> Option<&'a Json> {
    list.iter().find(|r| s(r, "id") == Some(id))
}

/// The switch's views when nothing was chosen: phones Day and Month,
/// computers Week and Month (a third one off).
pub fn default_switch() -> Json {
    json!({ "narrow": { "three": false, "small": "1d", "mid": "2d", "big": "month" }, "wide": { "three": false, "small": "1w", "mid": "4d", "big": "month" } })
}

fn view_order(v: &str) -> usize {
    VIEWS.iter().position(|x| x.0 == v).unwrap_or(99)
}

pub fn view_label(v: &str) -> String {
    VIEWS.iter().find(|x| x.0 == v).map_or(v.to_string(), |x| x.1.to_string())
}

/// The preferences, read: { hidden, show_tasks, views, before, emails }.
pub fn prefs_of(prefs: &[Json]) -> Json {
    let cal = find(prefs, "calendar");
    let alerts = find(prefs, "alerts");
    let before = match alerts {
        Some(a) => a.get("before").cloned().filter(|b| b.is_number()).unwrap_or(Json::Null),
        None => json!(DEFAULT_BEFORE),
    };
    let mut views = default_switch();
    if let Some(v) = find(prefs, "views") {
        for k in ["narrow", "wide"] {
            if let (Some(base), Some(over)) = (views[k].as_object_mut(), v.get(k).and_then(Json::as_object)) {
                for (kk, vv) in over {
                    base.insert(kk.clone(), vv.clone());
                }
            }
        }
    }
    json!({
        "hidden": cal.and_then(|c| c.get("hidden")).cloned().unwrap_or(json!([])),
        "show_tasks": cal.is_some_and(|c| truthy(c.get("show_tasks"))),
        "views": views,
        "before": before,
        "emails": find(prefs, "me").and_then(|m| m.get("emails")).cloned().unwrap_or(json!([])),
    })
}

/// The old fields for a list of views, so older copies still show something
/// close: the first and last they know, and the second when there are three.
/// None when the list has none they know (the old fields then stay as they were).
fn old_fields(list: &[String]) -> Option<Json> {
    let k: Vec<&String> = list.iter().filter(|v| OLD_VIEWS.contains(&v.as_str())).collect();
    let (first, last) = (k.first()?, k.last()?);
    let three = k.len() >= 3;
    let mid = if three { k[1].as_str() } else { "2d" };
    Some(json!({ "three": three, "small": first, "mid": mid, "big": last }))
}

/// The list of views a switch names (`list`), when an older copy hasn't
/// changed the old fields since: if it has, those win.
fn view_list(switch: &Json) -> Option<Vec<String>> {
    let list: Vec<String> = switch.get("list")?.as_array()?.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    if list.is_empty() {
        return None;
    }
    if let Some(old) = old_fields(&list) {
        let three = truthy(switch.get("three"));
        let same = three == truthy(old.get("three"))
            && s(switch, "small") == s(&old, "small")
            && s(switch, "big") == s(&old, "big")
            && (!three || s(switch, "mid") == s(&old, "mid"));
        if !same {
            return None;
        }
    }
    Some(list)
}

/// A switch with its list of views (any of [`VIEWS`]) set, in order, and the
/// old fields filled from it.
pub fn with_list(switch: &Json, list: &[String]) -> Json {
    let mut list: Vec<String> = list.iter().filter(|v| VIEWS.iter().any(|x| x.0 == v.as_str())).cloned().collect();
    list.sort_by_key(|v| view_order(v));
    list.dedup();
    let mut out = switch.clone();
    if !out.is_object() {
        out = json!({});
    }
    if let Some(old) = old_fields(&list) {
        for (k, v) in old.as_object().into_iter().flatten() {
            out[k] = v.clone();
        }
    }
    out["list"] = json!(list);
    out
}

/// The switch's views, shortest first (Settings may name them in any order, or one twice).
/// The year is left out where the screen can't draw it (`year_ok`).
pub fn segments(switch: &Json, year_ok: bool) -> Vec<String> {
    if let Some(mut list) = view_list(switch) {
        list.retain(|v| VIEWS.iter().any(|x| x.0 == v.as_str()) && (year_ok || v != "year"));
        list.sort_by_key(|v| view_order(v));
        list.dedup();
        // At most four, so the switch stays small (Settings lets no more be ticked).
        list.truncate(4);
        if !list.is_empty() {
            return list;
        }
    }
    let three = truthy(switch.get("three"));
    let keys: &[&str] = if three { &["small", "mid", "big"] } else { &["small", "big"] };
    let mut out: Vec<String> = vec![];
    for k in keys {
        if let Some(v) = s(switch, k) {
            if !out.iter().any(|x| x == v) {
                out.push(v.into());
            }
        }
    }
    out.sort_by_key(|v| view_order(v));
    out
}

/// The calendars, first one first, then oldest first: each container with
/// its name and colour.
pub fn calendars(containers: &[Json], infos: &[Json], first: &Json) -> Vec<Json> {
    let mut cals: Vec<(bool, i64, Json)> = containers
        .iter()
        .map(|c| {
            let main = truthy(c.get("main"));
            let mut m = Map::new();
            if main {
                m.insert("created".into(), json!(0));
                for (k, v) in c.as_object().into_iter().flatten() {
                    m.insert(k.clone(), v.clone());
                }
                m.insert("name".into(), json!(s(first, "name").unwrap_or("Personal")));
                m.insert("color".into(), json!(s(first, "color").unwrap_or("blue")));
                m.insert("id".into(), c["id"].clone());
            } else {
                m.insert("created".into(), json!(1));
                m.insert("color".into(), json!("blue"));
                if let Some(Json::Object(i)) = infos.iter().find(|i| i.get("container") == c.get("id")) {
                    for (k, v) in i {
                        m.insert(k.clone(), v.clone());
                    }
                }
                for (k, v) in c.as_object().into_iter().flatten() {
                    m.insert(k.clone(), v.clone());
                }
            }
            let created = m.get("created").and_then(Json::as_f64).unwrap_or(0.0) as i64;
            (main, created, Json::Object(m))
        })
        .collect();
    cals.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    cals.into_iter()
        .map(|(_, _, mut c)| {
            let note = shared_note(&c);
            let shared = c.get("from").is_some_and(Json::is_object) || c.get("sharedWith").and_then(Json::as_array).is_some_and(|a| !a.is_empty());
            let can_edit = c.get("canWrite") != Some(&json!(false)) && !truthy(c.get("subscribed"));
            c["note"] = json!(note);
            c["shared"] = json!(shared);
            c["canEdit"] = json!(can_edit);
            c
        })
        .collect()
}

/// "From Ana · view only", "Shared with Ana, Ben", or "".
pub fn shared_note(c: &Json) -> String {
    if let Some(from) = c.get("from").filter(|f| f.is_object()) {
        return format!("From {}{}", s(from, "name").unwrap_or(""), if c.get("canWrite") == Some(&json!(true)) { "" } else { " · view only" });
    }
    let with: Vec<&str> = c.get("sharedWith").and_then(Json::as_array).into_iter().flatten().filter_map(|p| s(p, "name")).collect();
    if with.is_empty() { String::new() } else { format!("Shared with {}", with.join(", ")) }
}

/// Monday (or Sunday) of the week `d` is in.
pub fn week_start(d: &str, sunday_first: bool) -> String {
    let wd = weekday(d);
    add_days(d, -((wd + if sunday_first { 0 } else { 6 }) % 7))
}

fn first_of_month(d: &str) -> String {
    let (y, m, _) = zones::ymd(d);
    zones::iso_parts(y, m, 1)
}

/// The days a view shows: a week starts on its first day; other views start on the day you are on.
pub fn days_of(view: &str, cursor: &str, sunday_first: bool) -> Vec<String> {
    if view == "1w" {
        let ws = week_start(cursor, sunday_first);
        return (0..7).map(|i| add_days(&ws, i)).collect();
    }
    let n = VIEWS.iter().find(|x| x.0 == view).map_or(7, |x| x.2);
    (0..n).map(|i| add_days(cursor, i)).collect()
}

/// The month grid: six weeks from the week of the 1st.
pub fn month_days(cursor: &str, sunday_first: bool) -> Vec<String> {
    let start = week_start(&first_of_month(cursor), sunday_first);
    (0..42).map(|i| add_days(&start, i)).collect()
}

/// Where Previous / Next go: a month, or as many days as are shown.
pub fn move_cursor(view: &str, cursor: &str, dir: i64, sunday_first: bool) -> String {
    if view == "month" || view == "year" {
        let (y, m, _) = zones::ymd(cursor);
        let ms = zones::date_utc(y, m - 1 + dir * if view == "year" { 12 } else { 1 }, 1, 0, 0);
        return zones::utc_date(ms);
    }
    let n = days_of(view, cursor, sunday_first).len() as i64;
    add_days(cursor, dir * n)
}

fn long_day(d: &str) -> String {
    let (_, m, day) = zones::ymd(d);
    format!("{} {} {}", WEEKDAYS[weekday(d) as usize], day, MONTHS[(m - 1) as usize])
}

/// "2:30–4:00 PM", "14:30–16:00".
pub fn span(a: &str, b: &str, hour12: bool) -> String {
    let x = friendly_time(a, hour12);
    let y = friendly_time(if b == "24:00" { "00:00" } else { b }, hour12);
    for suffix in [" AM", " PM"] {
        if x.ends_with(suffix) && y.ends_with(suffix) {
            return format!("{}–{y}", &x[..x.len() - suffix.len()]);
        }
    }
    format!("{x}–{y}")
}

/// What a block in the grid says for its time ("until 10:00" for the rest of a long event).
pub fn block_time(e: &Json, hour12: bool) -> String {
    let end = s(e, "end").unwrap_or("00:00");
    if truthy(e.get("continues")) {
        return format!("until {}", friendly_time(if end == "24:00" { "00:00" } else { end }, hour12));
    }
    span(s(e, "start").unwrap_or("00:00"), end, hour12)
}

/// The card's first line: "Monday 23 November · 9:00–10:00".
pub fn when_text(e: &Json, hour12: bool) -> String {
    let date = s(e, "date").unwrap_or("");
    let last = last_day(e);
    if truthy(e.get("all_day")) {
        return if last.as_str() > date { format!("{} – {}", long_day(date), long_day(&last)) } else { format!("{} · all day", long_day(date)) };
    }
    let (st, en) = (s(e, "start").unwrap_or("00:00"), s(e, "end").unwrap_or("00:00"));
    if last.as_str() > date {
        return format!("{}, {} – {}, {}", long_day(date), friendly_time(st, hour12), long_day(&last), friendly_time(en, hour12));
    }
    format!("{} · {}", long_day(date), span(st, en, hour12))
}

/// A place to show: not a link.
pub fn place(e: &Json) -> String {
    let l = s(e, "location").unwrap_or("").trim();
    let low = l.to_ascii_lowercase();
    if low.starts_with("http://") || low.starts_with("https://") { String::new() } else { l.to_string() }
}

/// The key your answer and your own alert are kept under: a changed
/// occurrence's own id, else the series (or the event).
pub fn answer_key(e: &Json) -> String {
    if truthy(e.get("recurrence_of")) {
        return s(e, "id").unwrap_or("").into();
    }
    e.get("series").and_then(|x| s(x, "id")).or(s(e, "id")).unwrap_or("").into()
}

fn answers_map(answers: &[Json]) -> Vec<(String, String)> {
    answers.iter().filter_map(|a| Some((s(a, "id")?.to_string(), s(a, "answer")?.to_string()))).collect()
}

/// Your own alert for an event: a changed occurrence's own, else the series'.
pub fn own_alert(e: &Json, alerts: &[Json]) -> Own {
    let rec = truthy(e.get("recurrence_of"));
    let keys = [
        if rec { s(e, "id") } else { None },
        if rec { s(e, "recurrence_of") } else { e.get("series").and_then(|x| s(x, "id")).or(s(e, "id")) },
    ];
    for k in keys.into_iter().flatten() {
        if let Some(a) = find(alerts, k) {
            return match a.get("before").and_then(Json::as_i64) {
                Some(m) if a.get("before").is_some_and(Json::is_number) => Own::Minutes(m),
                _ => Own::None,
            };
        }
    }
    Own::Default
}

/// The choice as the pickers show it: "default", "none" or minutes.
pub fn alert_choice(e: &Json, alerts: &[Json]) -> String {
    match own_alert(e, alerts) {
        Own::Default => "default".into(),
        Own::None => "none".into(),
        Own::Minutes(m) => m.to_string(),
    }
}

/// The alert choices a picker offers: [{ value, label }].
pub fn alert_options(current: &str, default_before: Option<i64>, with_default: bool) -> Json {
    let mut out = vec![];
    if with_default {
        out.push(json!({ "value": "default", "label": format!("Default ({})", before_label(default_before).to_lowercase()) }));
    }
    for m in crate::upcoming::BEFORE {
        out.push(json!({ "value": m.map_or("none".to_string(), |x| x.to_string()), "label": m.map_or("No alert".to_string(), |x| before_label(Some(x))) }));
    }
    let preset = current == "default" || current == "none" || crate::upcoming::BEFORE.iter().flatten().any(|m| m.to_string() == current);
    if !preset {
        out.push(json!({ "value": current, "label": before_label(current.parse().ok()) }));
    }
    json!(out)
}

/// What the screen shows. `st`: { events, containers, infos, first, prefs,
/// answers, alerts, tasks, view, cursor, narrow, sunday_first, hour12, now,
/// hidden?, show_tasks?, views? } (the last three override what `prefs` say,
/// while a change is on its way).
pub fn screen(st: &Json) -> Json {
    let p = prefs_of(&arr(&st["prefs"]));
    let hidden: Vec<String> = arr(st.get("hidden").filter(|h| h.is_array()).unwrap_or(&p["hidden"])).iter().filter_map(|x| x.as_str().map(String::from)).collect();
    let show_tasks = st.get("show_tasks").and_then(Json::as_bool).unwrap_or(p["show_tasks"] == json!(true));
    let views = st.get("views").filter(|v| v.is_object()).cloned().unwrap_or(p["views"].clone());
    let narrow = truthy(st.get("narrow"));
    let segs = segments(&views[if narrow { "narrow" } else { "wide" }], truthy(st.get("year_ok")));
    let mut view = s(st, "view").unwrap_or("").to_string();
    if !segs.contains(&view) {
        view = segs.first().cloned().unwrap_or_else(|| "1w".into());
    }
    let day_view = segs.iter().find(|v| *v != "month" && *v != "year").cloned().unwrap_or_else(|| "1d".into());
    let sunday = truthy(st.get("sunday_first"));
    let hour12 = truthy(st.get("hour12"));
    let now = st.get("now").and_then(Json::as_i64).unwrap_or_else(zones::now_ms);
    let today_wall = zones::local_wall(now);
    let t = today_wall.date.clone();
    let cursor = s(st, "cursor").unwrap_or(&t).to_string();
    let cals = calendars(&arr(&st["containers"]), &arr(&st["infos"]), &st["first"]);
    let color_of = |container: Option<&str>| -> String {
        cals.iter().find(|c| s(c, "id") == container).or(cals.first()).and_then(|c| s(c, "color")).unwrap_or("blue").to_string()
    };
    let events: Vec<Json> = arr(&st["events"])
        .into_iter()
        .map(|mut e| {
            e["calendar"] = e.get("container").cloned().unwrap_or(Json::Null);
            e
        })
        .collect();
    let shown: Vec<Json> = events.iter().filter(|e| !hidden.iter().any(|h| Some(h.as_str()) == s(e, "calendar"))).cloned().collect();
    let month = view == "month";
    let is_year = view == "year";
    let mut days = if is_year { year_days(&cursor, sunday) } else if month { month_days(&cursor, sunday) } else { days_of(&view, &cursor, sunday) };
    // A narrow window shows fewer days than the view has (`fit`: as many as fit),
    // from the day you are on, rather than columns too thin to read.
    let mut fitted = Json::Null;
    if !month && !is_year {
        if let Some(f) = st.get("fit").and_then(Json::as_i64).map(|f| f.max(1)) {
            if days.len() as i64 > f {
                days = days_of(&format!("{f}d"), &cursor, sunday);
                fitted = json!(f);
            }
        }
    }
    let (from, to) = (days[0].clone(), days[days.len() - 1].clone());
    let inst = expand(&shown, &from, &to);
    let map = by_day(&inst, &from, &to);
    let emails: Vec<String> = arr(&p["emails"]).iter().filter_map(|x| x.as_str().map(|e| e.to_lowercase())).collect();
    let answers = answers_map(&arr(&st["answers"]));
    let answer_for = |e: &Json| -> Json {
        let chosen = answers.iter().find(|(k, _)| *k == answer_key(e)).map(|(_, v)| v.as_str());
        answer_of(e, &emails, chosen).map_or(Json::Null, |a| json!(a))
    };
    let tasks: Vec<Json> = if show_tasks { arr(&st["tasks"]).into_iter().filter(|x| !truthy(x.get("done"))).collect() } else { vec![] };
    let tasks_on = |d: &str| -> Vec<Json> { tasks.iter().filter(|x| s(x, "due") == Some(d)).map(|x| json!({ "id": x["id"], "title": x["title"], "time": x["time"] })).collect() };

    // What kind of event it is, for a quiet shade of its calendar's colour on screen.
    let now_min = { let (h, m) = zones::hm(Some(&today_wall.time)); h * 60 + m };
    let joinable = |e: &Json| !crate::details::meeting(e).is_null();
    let repeats = |e: &Json| e.get("series").is_some_and(|x| !x.is_null());
    let past = |d: &str, end: Option<i64>| d < t.as_str() || (d == t.as_str() && end.is_some_and(|m| m <= now_min));
    let pill = |e: &Json, d: &str, first_shown: &str| {
        let cont = truthy(e.get("continues"));
        json!({
            "key": e["key"], "title": if cont && d != first_shown { "" } else { e.get("title").and_then(Json::as_str).unwrap_or("") },
            "color": color_of(s(e, "calendar")), "answer": answer_for(e), "continues": cont, "goesOn": truthy(e.get("goesOn")),
            "repeats": repeats(e), "join": joinable(e), "past": past(d, None),
        })
    };
    let mut columns = vec![];
    let mut cells = vec![];
    let mut year = Json::Null;
    if is_year {
        year = year_grid(&days, &map, &cursor, &t);
    } else if month {
        let (_, cm, _) = zones::ymd(&cursor);
        for d in &days {
            let (_, m, dn) = zones::ymd(d);
            let (all_day, timed) = map.iter().find(|x| &x.0 == d).map(|x| (x.1.clone(), x.2.clone())).unwrap_or_default();
            let mut timed = timed;
            timed.sort_by_key(|e| mins(s(e, "start")));
            let list: Vec<Json> = all_day.iter().chain(timed.iter()).cloned().collect();
            let items: Vec<Json> = list
                .iter()
                .take(3)
                .map(|e| {
                    let row = truthy(e.get("row"));
                    let time = if !row && !truthy(e.get("continues")) { friendly_time(s(e, "start").unwrap_or("00:00"), hour12) } else { String::new() };
                    json!({ "key": e["key"], "title": e.get("title").and_then(Json::as_str).unwrap_or(""), "time": time, "solid": row, "color": color_of(s(e, "calendar")), "answer": answer_for(e),
                        "repeats": repeats(e), "join": joinable(e), "past": past(d, if row { None } else { Some(mins(s(e, "end"))) }) })
                })
                .collect();
            let mut tk = tasks_on(d);
            tk.truncate(2);
            cells.push(json!({ "date": d, "day": dn, "other": m != cm, "today": *d == t, "items": items, "more": list.len().saturating_sub(3), "tasks": tk }));
        }
    } else {
        for d in &days {
            let (all_day, timed) = map.iter().find(|x| &x.0 == d).map(|x| (x.1.clone(), x.2.clone())).unwrap_or_default();
            let pills: Vec<Json> = all_day.iter().map(|e| pill(e, d, &days[0])).collect();
            let blocks: Vec<Json> = layout(&timed)
                .into_iter()
                .map(|(e, lane, lanes)| {
                    json!({
                        "key": e["key"], "title": e.get("title").and_then(Json::as_str).unwrap_or(""),
                        "start": mins(s(&e, "start")), "end": mins(s(&e, "end")), "lane": lane, "lanes": lanes,
                        "time": block_time(&e, hour12), "startTime": friendly_time(s(&e, "start").unwrap_or("00:00"), hour12),
                        "place": place(&e), "color": color_of(s(&e, "calendar")), "answer": answer_for(&e),
                        "repeats": repeats(&e), "join": joinable(&e), "past": past(d, Some(mins(s(&e, "end")))),
                    })
                })
                .collect();
            let (_, _, dn) = zones::ymd(d);
            let tk = tasks_on(d);
            columns.push(json!({
                "date": d, "weekday": &WEEKDAYS[weekday(d) as usize][..3], "day": dn, "today": *d == t,
                "allDay": pills, "timed": blocks,
                "tasksAllDay": tk.iter().filter(|x| x["time"].as_str().is_none_or(str::is_empty)).cloned().collect::<Vec<_>>(),
                "tasksTimed": tk.iter().filter(|x| x["time"].as_str().is_some_and(|v| !v.is_empty())).map(|x| { let mut x = x.clone(); x["minute"] = json!(mins(x["time"].as_str())); x }).collect::<Vec<_>>(),
            }));
        }
    }
    let title_day = if month || is_year { cursor.clone() } else { days[days.len() / 2].clone() };
    let (ty, tm, _) = zones::ymd(&title_day);
    let heads: Vec<String> = month_days(&cursor, sunday).iter().take(7).map(|d| WEEKDAYS[weekday(d) as usize][..3].to_string()).collect();
    let now_minute = zones::hm(Some(&today_wall.time));
    json!({
        "view": view, "views": segs.iter().map(|v| json!({ "id": v, "name": view_label(v) })).collect::<Vec<_>>(),
        "dayView": day_view, "fitted": fitted, "cursor": cursor, "today": t, "nowMinute": now_minute.0 * 60 + now_minute.1,
        "title": { "month": MONTHS[(tm - 1) as usize], "year": ty },
        "calendars": cals.iter().map(|c| { let off = hidden.iter().any(|h| Some(h.as_str()) == s(c, "id")); let mut c = c.clone(); c["hidden"] = json!(off); c }).collect::<Vec<_>>(),
        "days": if is_year { json!([]) } else { json!(days) }, "columns": columns, "cells": cells, "monthHeads": heads, "year": year,
        "showTasks": show_tasks, "hidden": hidden, "switch": views,
        "emails": p["emails"], "before": p["before"],
    })
}

/// The year grid's days: whole weeks from the week of 1 January to the week of 31 December.
pub fn year_days(cursor: &str, sunday_first: bool) -> Vec<String> {
    let (y, _, _) = zones::ymd(cursor);
    let start = week_start(&format!("{y:04}-01-01"), sunday_first);
    let end = format!("{y:04}-12-31");
    let mut out = vec![];
    let mut d = start;
    while d <= end || out.len() % 7 != 0 {
        out.push(d.clone());
        d = add_days(&d, 1);
    }
    out
}

/// Minutes of a day covered by timed events, overlaps counted once.
pub fn planned_minutes(timed: &[Json]) -> i64 {
    let mut spans: Vec<(i64, i64)> = timed
        .iter()
        .filter(|e| !truthy(e.get("all_day")))
        .map(|e| (mins(s(e, "start")), mins(s(e, "end"))))
        .filter(|(a, b)| b > a)
        .collect();
    spans.sort();
    let (mut total, mut upto) = (0, 0);
    for (a, b) in spans {
        let a = a.max(upto);
        if b > a {
            total += b - a;
            upto = b;
        }
    }
    total
}

/// How strongly a day is shaded: none, under 1 h, 1–3 h, 3–5 h, 5 h or more.
pub fn plan_level(minutes: i64) -> i64 {
    match minutes {
        0 => 0,
        m if m < 60 => 1,
        m if m < 180 => 2,
        m if m < 300 => 3,
        _ => 4,
    }
}

/// The year view: weeks of 7 days, each with how much of it was planned, and
/// where each month starts.
fn year_grid(days: &[String], map: &[(String, Vec<Json>, Vec<Json>)], cursor: &str, today: &str) -> Json {
    let (y, _, _) = zones::ymd(cursor);
    let mut weeks = vec![];
    let mut months = vec![];
    let mut total = 0;
    for (w, chunk) in days.chunks(7).enumerate() {
        let mut week = vec![];
        for d in chunk {
            let (dy, dm, dn) = zones::ymd(d);
            let minutes = map.iter().find(|x| &x.0 == d).map_or(0, |x| planned_minutes(&x.2));
            let other = dy != y;
            if !other {
                total += minutes;
                if dn == 1 {
                    months.push(json!({ "name": &MONTHS[(dm - 1) as usize][..3], "week": w }));
                }
            }
            week.push(json!({ "date": d, "minutes": minutes, "level": if other { 0 } else { plan_level(minutes) }, "other": other, "today": d == today }));
        }
        weeks.push(json!(week));
    }
    json!({ "weeks": weeks, "months": months, "hours": total / 60 })
}

/// Lays out overlapping timed events side by side: (event, lane, lanes).
pub fn layout(list: &[Json]) -> Vec<(Json, i64, i64)> {
    let mut timed: Vec<&Json> = list.iter().filter(|e| !truthy(e.get("all_day"))).collect();
    timed.sort_by(|a, b| mins(s(a, "start")).cmp(&mins(s(b, "start"))).then(mins(s(b, "end")).cmp(&mins(s(a, "end")))));
    let mut out: Vec<(Json, i64, i64)> = vec![];
    let mut cluster: Vec<&Json> = vec![];
    let mut cluster_end = -1;
    let flush = |cluster: &mut Vec<&Json>, out: &mut Vec<(Json, i64, i64)>| {
        let mut lanes: Vec<i64> = vec![];
        let first = out.len();
        for e in cluster.iter() {
            let st = mins(s(e, "start"));
            let lane = lanes.iter().position(|end| *end <= st).unwrap_or(lanes.len());
            let end = mins(s(e, "end")).max(st + 15);
            if lane == lanes.len() {
                lanes.push(end);
            } else {
                lanes[lane] = end;
            }
            out.push(((*e).clone(), lane as i64, 0));
        }
        for o in out[first..].iter_mut() {
            o.2 = lanes.len() as i64;
        }
        cluster.clear();
    };
    for e in timed {
        if mins(s(e, "start")) >= cluster_end && !cluster.is_empty() {
            flush(&mut cluster, &mut out);
        }
        cluster.push(e);
        cluster_end = cluster_end.max(mins(s(e, "end")));
    }
    if !cluster.is_empty() {
        flush(&mut cluster, &mut out);
    }
    out
}

/// The card for the occurrence with `key` (from what `screen` showed): its
/// lines, people, notes and answer.
pub fn card(st: &Json, key: &str, from: &str, to: &str) -> Json {
    let p = prefs_of(&arr(&st["prefs"]));
    let hour12 = truthy(st.get("hour12"));
    let events: Vec<Json> = arr(&st["events"]).into_iter().map(|mut e| { e["calendar"] = e.get("container").cloned().unwrap_or(Json::Null); e }).collect();
    let Some(e) = expand(&events, from, to).into_iter().find(|o| s(o, "key") == Some(key)) else { return Json::Null };
    let cals = calendars(&arr(&st["containers"]), &arr(&st["infos"]), &st["first"]);
    let c = cals.iter().find(|c| s(c, "id") == s(&e, "calendar")).or(cals.first()).cloned().unwrap_or(Json::Null);
    let emails: Vec<String> = arr(&p["emails"]).iter().filter_map(|x| x.as_str().map(|e| e.to_lowercase())).collect();
    let answers = answers_map(&arr(&st["answers"]));
    let chosen = answers.iter().find(|(k, _)| *k == answer_key(&e)).map(|(_, v)| v.clone());
    let n = crate::details::notes_of(s(&e, "notes").unwrap_or(""));
    let call = crate::details::meeting(&e);
    let people = people_of(&e, &emails, chosen.as_deref());
    let cal_line = format!(
        "{}{}",
        s(&c, "name").unwrap_or(""),
        if truthy(c.get("subscribed")) { " · from a link".to_string() } else if c.get("from").is_some_and(Json::is_object) { format!(" · {}", shared_note(&c)) } else { String::new() }
    );
    let url = s(&e, "url").filter(|u| call.get("url").and_then(Json::as_str) != Some(*u)).map(String::from);
    json!({
        "event": e, "title": e["title"], "when": when_text(&e, hour12),
        "repeat": e.get("series").and_then(|x| s(x, "rrule")).map(describe),
        "calendar": cal_line, "color": s(&c, "color").unwrap_or("blue"), "canEdit": c.get("canEdit") == Some(&json!(true)),
        "meeting": call, "place": place(&e), "url": url,
        "url_shown": url.as_deref().map(|u| { let t = u.trim_start_matches("https://").trim_start_matches("http://"); t.strip_prefix("www.").unwrap_or(t).to_string() }),
        "timed": !truthy(e.get("all_day")), "alert": alert_choice(&e, &arr(&st["alerts"])),
        "alertOptions": alert_options(&alert_choice(&e, &arr(&st["alerts"])), p["before"].as_i64(), true),
        "people": people, "peopleSummary": crate::details::people_summary(&people),
        "notes": crate::details::linkify(n["text"].as_str().unwrap_or("")), "joining": crate::details::linkify(n["joining"].as_str().unwrap_or("")),
        "hasNotes": n["text"].as_str().is_some_and(|t| !t.is_empty()), "hasJoining": n["joining"].as_str().is_some_and(|t| !t.is_empty()),
        "asksAnswer": crate::details::asks_answer(&e, &emails), "answer": answer_of(&e, &emails, chosen.as_deref()),
        "answerKey": answer_key(&e),
    })
}

/// The sheet for a new event on `date` (from `start` to `end`, or an hour),
/// in the first calendar you can change that is shown.
pub fn new_event(st: &Json, date: &str, start: Option<&str>, end: Option<&str>) -> Json {
    let p = prefs_of(&arr(&st["prefs"]));
    let hidden: Vec<String> = arr(st.get("hidden").filter(|h| h.is_array()).unwrap_or(&p["hidden"])).iter().filter_map(|x| x.as_str().map(String::from)).collect();
    let cals = calendars(&arr(&st["containers"]), &arr(&st["infos"]), &st["first"]);
    let editable = |c: &&Json| c.get("canEdit") == Some(&json!(true));
    let cal = cals.iter().filter(editable).find(|c| !hidden.iter().any(|h| Some(h.as_str()) == s(c, "id"))).or_else(|| cals.iter().find(editable)).and_then(|c| s(c, "id").map(String::from)).unwrap_or_else(|| "calendar".into());
    let start = start.unwrap_or("09:00");
    let end = end.map(String::from).unwrap_or_else(|| {
        let m = mins(Some(start)) + 60;
        format!("{:02}:{:02}", (m / 60) % 24, m % 60)
    });
    let mut ed = editor_for(&json!({ "date": date, "start": start, "end": end, "calendar": cal }), &[]);
    ed["alert"] = json!("default");
    ed["alertWas"] = json!("default");
    ed
}

/// The sheet for an occurrence on screen.
pub fn edit_event(st: &Json, key: &str, from: &str, to: &str) -> Json {
    let events: Vec<Json> = arr(&st["events"]).into_iter().map(|mut e| { e["calendar"] = e.get("container").cloned().unwrap_or(Json::Null); e }).collect();
    let Some(e) = expand(&events, from, to).into_iter().find(|o| s(o, "key") == Some(key)) else { return Json::Null };
    let alert = alert_choice(&e, &arr(&st["alerts"]));
    let mut ed = editor_for(&e, &events);
    ed["alert"] = json!(alert);
    ed["alertWas"] = json!(alert);
    ed
}

/// A new start date keeps the event's length in days.
pub fn move_start(ed: &Json, date: &str) -> Json {
    let mut ed = ed.clone();
    let old = s(&ed, "date").unwrap_or(date).to_string();
    let span = days_between(&old, s(&ed, "end_date").unwrap_or(&old)).max(0);
    ed["date"] = json!(date);
    ed["end_date"] = json!(add_days(date, span));
    ed
}

/// What is coming up, for the widget and alerts (14 days; declined ones left out).
pub fn upcoming_list(st: &Json) -> Json {
    let p = prefs_of(&arr(&st["prefs"]));
    let events: Vec<Json> = arr(&st["events"]).into_iter().map(|mut e| { e["calendar"] = e.get("container").cloned().unwrap_or(Json::Null); e }).collect();
    let cals = calendars(&arr(&st["containers"]), &arr(&st["infos"]), &st["first"]);
    let emails: Vec<String> = arr(&p["emails"]).iter().filter_map(|x| x.as_str().map(|e| e.to_lowercase())).collect();
    let answers = answers_map(&arr(&st["answers"]));
    let alerts = arr(&st["alerts"]);
    let now = st.get("now").and_then(Json::as_i64).unwrap_or_else(zones::now_ms);
    let list = upcoming_for(
        &events,
        p["before"].as_i64(),
        now,
        |o| {
            let chosen = answers.iter().find(|(k, _)| *k == answer_key(o)).map(|(_, v)| v.as_str());
            answer_of(o, &emails, chosen).as_deref() == Some("no")
        },
        |o| own_alert(o, &alerts),
        |o| Some(cals.iter().find(|c| s(c, "id") == s(o, "calendar")).or(cals.first()).and_then(|c| s(c, "color")).unwrap_or("blue").to_string()),
    );
    json!(list)
}

/// ".../CzechHolidays.ics" -> "CzechHolidays"; Google's ".../basic.ics" says nothing, so the site's name.
pub fn name_from_url(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |x| x.1);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = path.split(['?', '#']).next().unwrap_or("");
    let file = path.split('/').filter(|x| !x.is_empty()).last().unwrap_or("");
    let file = percent_decode(file);
    let file = if file.to_lowercase().ends_with(".ics") { file[..file.len() - 4].to_string() } else { file };
    let generic = ["basic", "calendar", "feed", "ical", "public", "full"];
    if !file.is_empty() && !generic.contains(&file.to_lowercase().as_str()) { file } else { host.strip_prefix("www.").unwrap_or(host).to_string() }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into()
}

/// "just now", "5 min ago", "3 h ago", "2 days ago", "not yet".
pub fn ago(ms: i64, now: i64) -> String {
    if ms == 0 {
        return "not yet".into();
    }
    let m = crate::ical::js_round((now - ms) as f64 / 60000.0);
    if m < 1 {
        "just now".into()
    } else if m < 60 {
        format!("{m} min ago")
    } else if m < 1440 {
        format!("{} h ago", crate::ical::js_round(m as f64 / 60.0))
    } else {
        format!("{} days ago", crate::ical::js_round(m as f64 / 1440.0))
    }
}

/// A file name for a calendar's .ics.
pub fn export_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| if "\\/:*?\"<>|".contains(c) { ' ' } else { c }).collect();
    let clean = clean.split(' ').filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ");
    format!("{}.ics", if clean.is_empty() { "Calendar" } else { &clean })
}

/// The .ics text of a calendar's events.
pub fn export_calendar(events: &[Json], container: &str, name: &str) -> (String, usize) {
    let list: Vec<Json> = events
        .iter()
        .filter(|e| s(e, "container") == Some(container))
        .map(|e| {
            let mut r = record(e);
            r["id"] = e["id"].clone();
            r
        })
        .collect();
    (write_ics(&list, Some(name), zones::now_ms()), list.len())
}

// ---------------------------------------------------------------- writing to UUOB

fn arg(c: &str, main: &str) -> Option<String> {
    (c != main && !c.is_empty()).then(|| c.to_string())
}

/// Writes what `plan_save` / `plan_delete` worked out; returns the id of the
/// first event written. Moving an event to another calendar moves it to that
/// calendar's container.
pub async fn apply(c: &Client, ops: &Json, main: &str) -> Result<Option<String>, uuob_client::Error> {
    let mut first: Option<String> = None;
    for o in arr(ops) {
        if let Some(to) = s(&o, "put") {
            let r = c.put("events", arg(to, main).as_deref(), s(&o, "id"), o["value"].clone()).await?;
            if first.is_none() {
                first = r.get("id").and_then(Json::as_str).map(String::from).or_else(|| s(&o, "id").map(String::from));
            }
        } else if let Some(from) = s(&o, "delete") {
            c.delete("events", arg(from, main).as_deref(), s(&o, "id").unwrap_or("")).await?;
        }
    }
    Ok(first)
}

/// Saves the sheet (`scope`: "this" / "all" for a repeating event), and its alert when it changed.
pub async fn save_event(c: &Client, ed: &Json, scope: Option<&str>, events: &[Json], main: &str) -> Result<(), uuob_client::Error> {
    let ops = plan_save(ed, scope, events);
    let id = apply(c, &ops, main).await?;
    if let (Some(id), Some(alert)) = (id, s(ed, "alert")) {
        if Some(alert) != s(ed, "alertWas") {
            set_own_alert(c, &id, alert).await?;
        }
    }
    Ok(())
}

pub async fn delete_event(c: &Client, ed: &Json, scope: Option<&str>, events: &[Json], main: &str) -> Result<(), uuob_client::Error> {
    apply(c, &plan_delete(ed, scope, events), main).await.map(drop)
}

/// Your own alert for an event: "default" (none of your own), "none", or minutes.
pub async fn set_own_alert(c: &Client, key: &str, choice: &str) -> Result<(), uuob_client::Error> {
    if choice == "default" {
        c.delete("alerts", None, key).await.map(drop)
    } else {
        let before = if choice == "none" { json!(false) } else { json!(choice.parse::<i64>().unwrap_or(0)) };
        c.put("alerts", None, Some(key), json!({ "before": before })).await.map(drop)
    }
}

/// Going / Maybe / No: kept on your devices only.
pub async fn set_answer(c: &Client, key: &str, answer: &str) -> Result<(), uuob_client::Error> {
    c.put("answers", None, Some(key), json!({ "answer": answer })).await.map(drop)
}

/// The calendar sheet saved: the first calendar's name and colour, or an
/// added one's (created when new; the rest of its info kept).
pub async fn save_calendar(c: &Client, sheet: &Json, infos: &[Json], cals: &[Json]) -> Result<Option<String>, uuob_client::Error> {
    let name = s(sheet, "name").unwrap_or("").trim().to_string();
    if name.is_empty() {
        return Ok(None);
    }
    let color = s(sheet, "color").unwrap_or("blue");
    if truthy(sheet.get("main")) {
        c.put("calendars", None, Some("personal"), json!({ "name": name, "color": color })).await?;
        return Ok(None);
    }
    let id = match s(sheet, "id") {
        Some(id) => {
            if Some(name.as_str()) != cals.iter().find(|x| s(x, "id") == Some(id)).and_then(|x| s(x, "name")) {
                c.rename_container(id, &name).await?;
            }
            id.to_string()
        }
        None => {
            let made = c.create_container(&name, None).await?;
            made.as_str().map(String::from).unwrap_or_else(|| made.to_string())
        }
    };
    let mut info = infos.iter().find(|i| s(i, "container") == Some(id.as_str())).and_then(Json::as_object).cloned().unwrap_or_default();
    info.remove("container");
    info.remove("id");
    info.insert("color".into(), json!(color));
    info.insert("created".into(), sheet.get("created").filter(|x| !x.is_null()).cloned().unwrap_or(json!(zones::now_ms())));
    c.put("calendar", Some(&id), Some("info"), Json::Object(info)).await?;
    Ok(Some(id))
}

/// Deletes the calendar everywhere; one someone shared with you is only left.
pub async fn delete_calendar(c: &Client, id: &str, subscribed: bool) -> Result<(), uuob_client::Error> {
    c.delete_container(id).await?;
    if subscribed {
        c.delete("subscriptions", None, id).await?;
    }
    Ok(())
}

/// Which calendars are hidden and whether tasks show (the whole record each time).
pub async fn save_view_prefs(c: &Client, hidden: &[String], show_tasks: bool) -> Result<(), uuob_client::Error> {
    c.put("prefs", None, Some("calendar"), json!({ "show_tasks": show_tasks, "hidden": hidden })).await.map(drop)
}

pub async fn save_switch(c: &Client, views: &Json) -> Result<(), uuob_client::Error> {
    c.put("prefs", None, Some("views"), views.clone()).await.map(drop)
}

pub async fn save_default_alert(c: &Client, before: Option<i64>) -> Result<(), uuob_client::Error> {
    c.put("prefs", None, Some("alerts"), json!({ "before": before })).await.map(drop)
}

pub async fn save_emails(c: &Client, emails: &[String]) -> Result<(), uuob_client::Error> {
    c.put("prefs", None, Some("me"), json!({ "emails": emails })).await.map(drop)
}

/// A new added calendar: its container and info. Its colour follows the
/// ones already there.
pub async fn new_calendar(c: &Client, name: &str, cals: usize, extra: Json) -> Result<String, uuob_client::Error> {
    let short: String = name.chars().take(64).collect();
    let made = c.create_container(if short.is_empty() { "Calendar" } else { &short }, None).await?;
    let id = made.as_str().map(String::from).unwrap_or_else(|| made.to_string());
    let mut info = Map::new();
    info.insert("color".into(), json!(COLORS[(cals + 1) % COLORS.len()]));
    info.insert("created".into(), json!(zones::now_ms()));
    for (k, v) in extra.as_object().into_iter().flatten() {
        info.insert(k.clone(), v.clone());
    }
    c.put("calendar", Some(&id), Some("info"), Json::Object(info)).await?;
    Ok(id)
}

/// Remembers your address when a file or link is your own calendar.
pub async fn note_address(c: &Client, owner: Option<&str>, emails: &mut Vec<String>) -> Result<(), uuob_client::Error> {
    let Some(o) = owner.filter(|o| !o.is_empty()) else { return Ok(()) };
    if emails.iter().any(|e| e == o) {
        return Ok(());
    }
    emails.push(o.to_string());
    save_emails(c, emails).await
}

/// The text at a calendar link (https only).
pub fn fetch_text(url: &str) -> Result<String, String> {
    let resp = ureq::get(url).header("Accept", "text/calendar, */*").call();
    let mut resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(404)) => return Err("Nothing was found at that link.".into()),
        Err(ureq::Error::StatusCode(code)) => return Err(format!("The link answered {code}.")),
        Err(_) => return Err("The link could not be read.".into()),
    };
    let text = resp.body_mut().with_config().limit(20 * 1024 * 1024).read_to_string().map_err(|_| "That file is too big to be a calendar.".to_string())?;
    let head: String = text.chars().take(2000).collect();
    if !head.to_uppercase().contains("BEGIN:VCALENDAR") {
        return Err("That link is not a calendar (.ics).".into());
    }
    Ok(text)
}

/// Puts the link's events into its calendar: new and changed ones are
/// written, ones no longer at the link are removed. Returns how many it has.
pub async fn refresh(c: &Client, calendar: &str, url: &str, events: &[Json], emails: &mut Vec<String>) -> Result<usize, String> {
    let u = url.to_string();
    let text = uuob_client::runtime().spawn_blocking(move || fetch_text(&u)).await.map_err(|e| e.to_string())??;
    let r = parse_ics(&text);
    let list = arr(&r["events"]);
    let keep: Vec<&str> = list.iter().filter_map(|e| s(e, "id")).collect();
    let err = |e: uuob_client::Error| e.to_string();
    for e in &list {
        c.put("events", Some(calendar), s(e, "id"), record(e)).await.map_err(err)?;
    }
    for e in events.iter().filter(|e| s(e, "container") == Some(calendar) && !keep.contains(&s(e, "id").unwrap_or(""))) {
        c.delete("events", Some(calendar), s(e, "id").unwrap_or("")).await.map_err(err)?;
    }
    c.put("subscriptions", None, Some(calendar), json!({ "url": url, "refreshed": zones::now_ms() })).await.map_err(err)?;
    note_address(c, s(&r, "owner"), emails).await.map_err(err)?;
    Ok(list.len())
}

/// Subscribes to a calendar link: a new calendar named from the link (or
/// `name`), filled from it. Returns the new calendar's id.
pub async fn subscribe(c: &Client, text: &str, name: &str, cals: usize, emails: &mut Vec<String>) -> Result<String, String> {
    let url = calendar_url(text).ok_or("That doesn't look like a calendar link.")?;
    let u = url.clone();
    let body = uuob_client::runtime().spawn_blocking(move || fetch_text(&u)).await.map_err(|e| e.to_string())??;
    let r = parse_ics(&body);
    let name = Some(name.trim().to_string()).filter(|n| !n.is_empty()).or_else(|| s(&r, "name").map(String::from)).unwrap_or_else(|| name_from_url(&url));
    let err = |e: uuob_client::Error| e.to_string();
    let id = new_calendar(c, &name, cals, json!({ "subscribed": true })).await.map_err(err)?;
    c.put("subscriptions", None, Some(&id), json!({ "url": url, "refreshed": 0 })).await.map_err(err)?;
    refresh(c, &id, &url, &[], emails).await?;
    Ok(id)
}

/// The links due to be read again: subscribed to, their calendar still
/// there, and last read more than three hours ago.
pub fn due_subscriptions(subs: &[Json], containers: &[Json], now: i64) -> Vec<Json> {
    subs.iter()
        .filter(|x| now - x.get("refreshed").and_then(Json::as_i64).unwrap_or(0) > 3 * 3600 * 1000 && containers.iter().any(|c| c.get("id") == x.get("id")))
        .cloned()
        .collect()
}

/// What a file holds, for the import sheet: { name, events, repeating, r } or { name, error }.
pub fn read_file(file_name: &str, text: &str) -> Json {
    let r = parse_ics(text);
    let looks = text.to_uppercase().contains("BEGIN:VCALENDAR");
    if !looks {
        return json!({ "name": file_name, "error": "This file could not be read as a calendar." });
    }
    let list = arr(&r["events"]);
    let base = file_name.strip_suffix(".ics").or_else(|| file_name.strip_suffix(".ICS")).unwrap_or(file_name);
    json!({
        "name": s(&r, "name").unwrap_or(base), "r": r,
        "events": list.iter().filter(|e| !truthy(e.get("recurrence_of"))).count(),
        "repeating": list.iter().filter(|e| truthy(e.get("rrule"))).count(),
    })
}

/// Imports the files read (each into a new calendar, or `target`): the result for the sheet.
pub async fn import(c: &Client, items: &[Json], cals: usize, emails: &mut Vec<String>, progress: impl Fn(usize)) -> Json {
    let (mut events, mut unreadable, mut zones_seen, mut done) = (0usize, 0i64, Vec::<String>::new(), 0usize);
    let mut count = cals;
    for item in items.iter().filter(|i| i["r"]["events"].as_array().is_some_and(|a| !a.is_empty())) {
        let r = &item["r"];
        let cal = match s(item, "target").filter(|t| *t != "new") {
            Some(t) => t.to_string(),
            None => match new_calendar(c, s(item, "name").unwrap_or("Calendar"), count, json!({})).await {
                Ok(id) => {
                    count += 1;
                    id
                }
                Err(e) => return json!({ "events": events, "unreadable": unreadable, "zones": zones_seen, "error": e.to_string() }),
            },
        };
        let _ = note_address(c, s(r, "owner"), emails).await;
        for e in arr(&r["events"]) {
            if let Err(err) = c.put("events", Some(&cal), s(&e, "id"), record(&e)).await {
                return json!({ "events": events, "unreadable": unreadable, "zones": zones_seen, "error": err.to_string() });
            }
            done += 1;
            events += 1;
            progress(done);
        }
        unreadable += r["unreadableRules"].as_i64().unwrap_or(0);
        for z in arr(&r["zones"]).iter().filter_map(Json::as_str) {
            if !zones_seen.iter().any(|x| x == z) {
                zones_seen.push(z.into());
            }
        }
    }
    json!({ "events": events, "unreadable": unreadable, "zones": zones_seen })
}

/// Added calendars used to be records in the app's first container. Moves
/// each into a container of its own (keyed by its old id, so every device
/// ends up with the same container), with its events.
pub async fn move_old_calendars(c: &Client) -> Result<(), uuob_client::Error> {
    let old: Vec<Json> = c.list("calendars", None).await?.into_iter().filter(|x| s(x, "id") != Some("personal")).collect();
    if old.is_empty() {
        return Ok(());
    }
    let loose = c.list("events", None).await?;
    for cal in old {
        let name = s(&cal, "name").unwrap_or("Calendar").to_string();
        let key = s(&cal, "id").unwrap_or("");
        let made = match c.create_container(&name, Some(key)).await {
            Ok(v) => v,
            Err(_) => c.create_container(&name, None).await?,
        };
        let id = made.as_str().map(String::from).unwrap_or_else(|| made.to_string());
        let info = json!({ "color": cal.get("color").filter(|x| !x.is_null()).cloned().unwrap_or(json!("blue")), "created": cal.get("created").filter(|x| !x.is_null()).cloned().unwrap_or(json!(1)) });
        c.put("calendar", Some(&id), Some("info"), info).await?;
        for e in loose.iter().filter(|e| e.get("calendar") == cal.get("id")) {
            let mut e = e.clone();
            e.as_object_mut().map(|o| o.remove("calendar"));
            let eid = s(&e, "id").unwrap_or("").to_string();
            c.put("events", Some(&id), None, e).await?;
            c.delete("events", None, &eid).await?;
        }
        c.delete("calendars", None, key).await?;
    }
    Ok(())
}
