//! The web version's answers (tests/oracle/*.json, written by generate.mjs in
//! four time zones) against this core's, call for call.

use calendar_core::*;
use serde_json::{Value as Json, json};

fn s(v: &Json) -> &str {
    v.as_str().unwrap_or("")
}
fn os(v: &Json) -> Option<&str> {
    v.as_str()
}
fn wall(w: zones::Wall) -> Json {
    json!({ "date": w.date, "time": w.time })
}

/// The same call in Rust; None for what isn't ported yet.
fn call(f: &str, a: &[Json]) -> Option<Json> {
    Some(match f {
        "zonedInstant" => json!(zones::zoned_instant(s(&a[0]), os(&a[1]), s(&a[2]))),
        "localInstant" => json!(zones::local_instant(s(&a[0]), os(&a[1]))),
        "localWall" => wall(zones::local_wall(a[0].as_i64()?)),
        "zonedWall" => wall(zones::zoned_wall(a[0].as_i64()?, s(&a[1]))),
        "toLocal" => wall(zones::to_local(s(&a[0]), os(&a[1]), os(&a[2]))),
        "fromLocal" => wall(zones::from_local(s(&a[0]), os(&a[1]), os(&a[2]))),
        "daysBetween" => json!(zones::days_between(s(&a[0]), s(&a[1]))),
        "knownZone" => json!(zones::known_zone(s(&a[0]))),
        "parseRule" => recur::parse_rule(s(&a[0])).to_json(),
        "supported" => json!(recur::supported(s(&a[0]))),
        "describe" => json!(recur::describe(s(&a[0]))),
        "ruleText" => json!(recur::rule_text(&recur::Rule::from_json(&a[0]))),
        "occurrences" => json!(recur::occurrences(&a[0], s(&a[1]), s(&a[2]))),
        _ => extra::call(f, a)?,
    })
}

mod extra {
    use calendar_core::*;
    use serde_json::{Value as Json, json};

    fn s(v: &Json) -> &str {
        v.as_str().unwrap_or("")
    }
    fn arr(v: &Json) -> Vec<Json> {
        v.as_array().cloned().unwrap_or_default()
    }
    fn strs(v: &Json) -> Vec<String> {
        arr(v).iter().filter_map(|x| x.as_str().map(String::from)).collect()
    }

    pub fn call(f: &str, a: &[Json]) -> Option<Json> {
        Some(match f {
            "parseIcs" => ical::parse_ics(s(&a[0])),
            "recordId" => {
                let parts: Vec<&str> = a.iter().map(s).collect();
                json!(ical::record_id(&parts))
            }
            "writeIcs" => json!(ical::write_ics(&arr(&a[0]), a[1].as_str(), super::NOW)),
            "record" => series::record(&a[0]),
            "repeatFrom" => series::repeat_from(a[0].as_str(), s(&a[1])),
            "ruleFrom" => series::rule_from(&a[0], &a[1]).map_or(Json::Null, |r| json!(r)),
            "editorFor" => series::editor_for(&a[0], &arr(&a[1])),
            "asksScope" => json!(series::asks_scope(&a[0])),
            "planSave" => series::plan_save(&a[0], a[1].as_str(), &arr(&a[2])),
            "planDelete" => series::plan_delete(&a[0], a[1].as_str(), &arr(&a[2])),
            "expand" => json!(instances::expand(&arr(&a[0]), s(&a[1]), s(&a[2]))),
            "byDay" => instances::by_day_json(&instances::by_day(&arr(&a[0]), s(&a[1]), s(&a[2]))),
            "lastDay" => json!(instances::last_day(&a[0])),
            "length" => json!(instances::length(&a[0])),
            "meeting" => details::meeting(&a[0]),
            "plain" => json!(details::plain(s(&a[0]))),
            "notesOf" => details::notes_of(s(&a[0])),
            "linkify" => details::linkify(s(&a[0])),
            "peopleOf" => details::people_of(&a[0], &strs(&a[1]), a[2].as_str()),
            "answerOf" => details::answer_of(&a[0], &strs(&a[1]), a[2].as_str()).map_or(Json::Null, |x| json!(x)),
            "peopleSummary" => json!(details::people_summary(&a[0])),
            "asksAnswer" => json!(details::asks_answer(&a[0], &strs(&a[1]))),
            "isEmail" => json!(details::is_email(s(&a[0]))),
            "beforeLabel" => json!(upcoming::before_label(a[0].as_i64())),
            "calendarUrl" => upcoming::calendar_url(s(&a[0])).map_or(Json::Null, |x| json!(x)),
            "upcomingFor" => {
                let o = &a[2];
                let skip_ids = strs(&o["skipIds"]);
                let own: Vec<(String, Json)> = arr(&o["ownKeys"]).iter().map(|p| (s(&p[0]).to_string(), p[1].clone())).collect();
                let has_opts = o.get("ownKeys").is_some();
                let list = upcoming::upcoming_for(
                    &arr(&a[0]),
                    a[1].as_i64(),
                    super::NOW,
                    |x| skip_ids.iter().any(|i| Some(i.as_str()) == x["id"].as_str()),
                    |x| {
                        if !has_opts {
                            return upcoming::Own::Default;
                        }
                        let rec = x.get("recurrence_of").and_then(Json::as_str).filter(|v| !v.is_empty());
                        let keys = [rec.and(x["id"].as_str()), rec.or(x["series"]["id"].as_str()).or(x["id"].as_str())];
                        for k in keys.into_iter().flatten() {
                            if let Some((_, v)) = own.iter().find(|(kk, _)| kk == k) {
                                return match v.as_i64() {
                                    Some(m) => upcoming::Own::Minutes(m),
                                    None => upcoming::Own::None,
                                };
                            }
                        }
                        upcoming::Own::Default
                    },
                    |x| has_opts.then(|| if x["container"] == json!("calendar") { "blue".to_string() } else { "green".to_string() }),
                );
                json!(list)
            }
            _ => return None,
        })
    }
}

/// The moment the fixtures were written at (generate.mjs).
const NOW: i64 = 1795089600000;

#[test]
fn the_web_versions_answers() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/oracle");
    let mut failed = 0;
    let mut by_fn: std::collections::BTreeMap<String, (usize, usize, usize)> = Default::default();
    for file in ["UTC", "Europe_Prague", "America_New_York", "Pacific_Auckland"] {
        let text = std::fs::read_to_string(format!("{dir}/{file}.json")).unwrap();
        let d: Json = serde_json::from_str(&text).unwrap();
        let tz = d["tz"].as_str().unwrap().to_string();
        let mut shown = 0;
        zones::with_device_zone(&tz, || {
            for c in d["cases"].as_array().unwrap() {
                let f = c["fn"].as_str().unwrap();
                let args = c["args"].as_array().unwrap();
                let e = by_fn.entry(f.into()).or_default();
                match call(f, args) {
                    None => e.2 += 1,
                    Some(got) if got == c["out"] => e.0 += 1,
                    Some(got) => {
                        e.1 += 1;
                        failed += 1;
                        if shown < 25 {
                            shown += 1;
                            eprintln!("[{tz}] {f}({})\n   want {}\n   got  {}", serde_json::to_string(args).unwrap().chars().take(300).collect::<String>(), c["out"], got);
                        }
                    }
                }
            }
        });
    }
    for (f, (ok, bad, todo)) in &by_fn {
        eprintln!("{f:16} ok {ok:4}  wrong {bad:4}  not ported {todo:4}");
    }
    assert_eq!(failed, 0, "{failed} answers differ from the web version");
}
