//! The rules Calendar.svelte held beside the library, now in the core.

use calendar_core::app::*;
use calendar_core::upcoming::Own;
use calendar_core::zones::with_device_zone;
use serde_json::{Value as Json, json};

fn state() -> Json {
    json!({
        "containers": [
            { "id": "calendar", "name": "Calendar", "main": true, "canWrite": true },
            { "id": "calendar.work", "name": "Work", "main": false, "canWrite": true },
            { "id": "calendar.hol", "name": "Holidays", "main": false, "canWrite": true },
            { "id": "calendar.club", "name": "Club", "main": false, "canWrite": false, "from": { "name": "Ben" } },
        ],
        "infos": [
            { "id": "info", "container": "calendar.work", "color": "green", "created": 5 },
            { "id": "info", "container": "calendar.hol", "color": "red", "created": 2, "subscribed": true },
        ],
        "first": { "name": "Personal", "color": "purple" },
        "prefs": [ { "id": "me", "emails": ["ana@x.test"] }, { "id": "alerts", "before": 30 } ],
        "answers": [ { "id": "inv", "answer": "no" } ],
        "alerts": [ { "id": "w1", "before": false }, { "id": "w1.20261123", "before": 5 }, { "id": "e2", "before": 60 } ],
        "events": [
            { "id": "e1", "container": "calendar", "title": "A", "all_day": false, "date": "2026-11-19", "start": "09:00", "end": "10:00" },
            { "id": "e2", "container": "calendar.work", "title": "B", "all_day": false, "date": "2026-11-19", "start": "09:30", "end": "11:00" },
            { "id": "e3", "container": "calendar.work", "title": "C", "all_day": false, "date": "2026-11-19", "start": "10:00", "end": "10:30" },
            { "id": "e4", "container": "calendar.work", "title": "D", "all_day": false, "date": "2026-11-19", "start": "13:00", "end": "14:00" },
            { "id": "w1", "container": "calendar", "title": "Weekly", "all_day": false, "date": "2026-11-02", "start": "08:00", "end": "08:15", "rrule": "FREQ=WEEKLY;BYDAY=MO" },
            { "id": "w1.20261123", "container": "calendar", "title": "Weekly moved", "all_day": false, "date": "2026-11-23", "start": "09:00", "end": "09:15", "recurrence_of": "w1", "recurrence_date": "2026-11-23" },
            { "id": "inv", "container": "calendar.work", "title": "Invite", "all_day": false, "date": "2026-11-20", "start": "15:00", "end": "16:00",
              "attendees": [ { "email": "ana@x.test", "answer": null }, { "email": "ben@x.test", "answer": "yes" } ], "organizer": { "email": "ben@x.test" } },
            { "id": "trip", "container": "calendar.hol", "title": "Trip", "all_day": true, "date": "2026-11-18", "end_date": "2026-11-20" },
        ],
        "view": "1w", "cursor": "2026-11-19", "narrow": false, "sunday_first": false, "hour12": false,
        "now": 1795089600000i64,
    })
}

#[test]
fn calendars_first_one_first_then_oldest() {
    let s = state();
    let cals = calendars(s["containers"].as_array().unwrap(), s["infos"].as_array().unwrap(), &s["first"]);
    let names: Vec<&str> = cals.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Personal", "Club", "Holidays", "Work"]); // created 0, 1 (unknown), 2, 5
    assert_eq!(cals[0]["color"], json!("purple"));
    assert_eq!(cals[1]["note"], json!("From Ben · view only"));
    assert_eq!(cals[2]["canEdit"], json!(false)); // subscribed: changes only at the link
}

#[test]
fn the_week_lays_out_overlaps_side_by_side() {
    with_device_zone("Europe/Prague", || {
        let sc = screen(&state());
        assert_eq!(sc["days"][0], json!("2026-11-16"));
        assert_eq!(sc["title"], json!({ "month": "November", "year": 2026 }));
        assert_eq!(sc["views"], json!([{ "id": "1w", "name": "Week" }, { "id": "month", "name": "Month" }]));
        let thu = &sc["columns"][3];
        assert_eq!(thu["today"], json!(true));
        let lanes: Vec<(String, i64, i64)> = thu["timed"].as_array().unwrap().iter().map(|b| (b["title"].as_str().unwrap().into(), b["lane"].as_i64().unwrap(), b["lanes"].as_i64().unwrap())).collect();
        assert_eq!(lanes, [("A".into(), 0, 2), ("B".into(), 1, 2), ("C".into(), 0, 2), ("D".into(), 0, 1)]);
        assert_eq!(thu["timed"][1]["time"], json!("9:30–11:00"));
        assert_eq!(thu["allDay"][0]["title"], json!("")); // Trip goes on from Wednesday
        assert_eq!(sc["columns"][2]["allDay"][0]["title"], json!("Trip"));
        assert_eq!(sc["columns"][4]["timed"][0]["answer"], json!("no")); // declined here
    });
}

#[test]
fn the_month_shows_three_and_a_count() {
    with_device_zone("UTC", || {
        let mut st = state();
        st["view"] = json!("month");
        for i in 0..4 {
            st["events"].as_array_mut().unwrap().push(json!({ "id": format!("m{i}"), "container": "calendar", "title": format!("M{i}"), "all_day": false, "date": "2026-11-25", "start": format!("1{i}:00"), "end": format!("1{i}:30") }));
        }
        let sc = screen(&st);
        assert_eq!(sc["cells"].as_array().unwrap().len(), 42);
        assert_eq!(sc["cells"][0]["date"], json!("2026-10-26"));
        let wed = sc["cells"].as_array().unwrap().iter().find(|c| c["date"] == json!("2026-11-25")).unwrap();
        assert_eq!(wed["items"].as_array().unwrap().len(), 3);
        assert_eq!(wed["more"], json!(1));
        assert_eq!(wed["items"][0]["time"], json!("10:00"));
        assert_eq!(sc["cells"][0]["other"], json!(true));
    });
}

#[test]
fn answers_and_own_alerts_are_keyed_as_before() {
    let st = state();
    let alerts = st["alerts"].as_array().unwrap().clone();
    let occ = json!({ "id": "w1", "series": { "id": "w1" } });
    assert_eq!(answer_key(&occ), "w1");
    assert_eq!(own_alert(&occ, &alerts), Own::None); // `before: false` is none
    let changed = json!({ "id": "w1.20261123", "recurrence_of": "w1" });
    assert_eq!(answer_key(&changed), "w1.20261123");
    assert_eq!(own_alert(&changed, &alerts), Own::Minutes(5)); // its own first
    let other = json!({ "id": "w1.20261130", "recurrence_of": "w1" });
    assert_eq!(own_alert(&other, &alerts), Own::None); // then the series'
    assert_eq!(alert_choice(&json!({ "id": "x" }), &alerts), "default");
}

#[test]
fn upcoming_leaves_out_declined_and_uses_your_alerts() {
    with_device_zone("Europe/Prague", || {
        let mut st = state();
        st["now"] = json!(1795075200000i64); // Thu 2026-11-19 08:00 UTC = 09:00 Prague
        let list = upcoming_list(&st);
        let ids: Vec<&str> = list.as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
        assert!(!ids.iter().any(|i| i.ends_with("~inv")), "declined");
        let b = list.as_array().unwrap().iter().find(|x| x["id"] == json!("calendar.work~e2~2026-11-19")).unwrap();
        assert_eq!(b["alert"], json!(60));
        assert_eq!(b["color"], json!("green"));
        let c = list.as_array().unwrap().iter().find(|x| x["id"] == json!("calendar.work~e3~2026-11-19")).unwrap();
        assert_eq!(c["alert"], json!(30)); // the default from prefs
        let moved = list.as_array().unwrap().iter().find(|x| x["id"] == json!("calendar/w1.20261123".replace('/', "~"))).unwrap();
        assert_eq!(moved["alert"], json!(5));
    });
}

#[test]
fn views_from_settings_and_moving_on() {
    let p = prefs_of(&[json!({ "id": "views", "wide": { "three": true, "mid": "2d" } })]);
    assert_eq!(segments(&p["views"]["wide"], false), ["2d", "1w", "month"]);
    assert_eq!(segments(&p["views"]["narrow"], false), ["1d", "month"]);
    assert_eq!(prefs_of(&[])["before"], json!(10));
    assert_eq!(prefs_of(&[json!({ "id": "alerts", "before": null })])["before"], Json::Null);
    assert_eq!(move_cursor("month", "2026-01-31", 1, false), "2026-02-01");
    assert_eq!(move_cursor("4d", "2026-11-19", -1, false), "2026-11-15");
    assert_eq!(days_of("1w", "2026-11-19", true)[0], "2026-11-15");
    assert_eq!(name_from_url("https://calendar.google.com/calendar/ical/x%40gmail.com/public/basic.ics"), "calendar.google.com");
    assert_eq!(name_from_url("https://www.example.test/cal/Czech%20Holidays.ics?x=1"), "Czech Holidays");
    assert_eq!(export_name("Work: a/b"), "Work a b.ics");
}

#[test]
fn views_as_a_list() {
    use serde_json::json;
    let old = json!({ "three": false, "small": "1d", "mid": "2d", "big": "month" });
    let sw = with_list(&old, &["month".into(), "3d".into(), "1d".into(), "year".into(), "3d".into()]);
    assert_eq!(sw["list"], json!(["1d", "3d", "month", "year"]));
    // Older copies see the views they know.
    assert_eq!((sw["small"].as_str(), sw["big"].as_str(), sw["three"].as_bool()), (Some("1d"), Some("month"), Some(false)));
    assert_eq!(segments(&sw, true), ["1d", "3d", "month", "year"]);
    assert_eq!(segments(&sw, false), ["1d", "3d", "month"]);
    // An older copy changed the old fields since: those win.
    let mut edited = sw.clone();
    edited["small"] = json!("1w");
    assert_eq!(segments(&edited, true), ["1w", "month"]);
}

#[test]
fn planned_time_of_a_day() {
    use serde_json::json;
    let e = |a: &str, b: &str| json!({ "start": a, "end": b });
    assert_eq!(planned_minutes(&[e("09:00", "10:00"), e("09:30", "11:00"), e("13:00", "13:30")]), 150);
    assert_eq!(planned_minutes(&[e("22:00", "24:00")]), 120);
    assert_eq!(planned_minutes(&[json!({ "all_day": true, "start": "00:00", "end": "24:00" })]), 0);
    assert_eq!([0, 30, 60, 200, 400].map(plan_level), [0, 1, 2, 3, 4]);
    let days = year_days("2026-06-15", false);
    assert_eq!(days.len() % 7, 0);
    assert!(days.first().unwrap().as_str() <= "2026-01-01" && days.last().unwrap().as_str() >= "2026-12-31");
}
