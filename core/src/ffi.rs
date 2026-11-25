//! Calendar's core for Kotlin and Swift (UniFFI): JSON text in and out, as
//! UUOB's own API. The SDK object (`UuobClient`) is handed in for the calls
//! that write to UUOB.

use std::sync::Arc;

use serde_json::{Value as Json, json};
use uuob_client::ffi::UuobClient;

use crate::{app, ical, upcoming};

fn parse(s: &str) -> Json {
    serde_json::from_str(s).unwrap_or(Json::Null)
}
fn list(s: &str) -> Vec<Json> {
    parse(s).as_array().cloned().unwrap_or_default()
}
fn strings(s: &str) -> Vec<String> {
    list(s).iter().filter_map(|x| x.as_str().map(String::from)).collect()
}
fn failed(detail: String) -> uuob_client::Error {
    uuob_client::Error::new("failed", detail)
}

/// What the screen shows (see `app::screen` for `state`).
#[uniffi::export]
pub fn screen(state: String) -> String {
    app::screen(&parse(&state)).to_string()
}

/// The card of the occurrence `key`, as shown between `from` and `to`.
#[uniffi::export]
pub fn card(state: String, key: String, from: String, to: String) -> String {
    app::card(&parse(&state), &key, &from, &to).to_string()
}

#[uniffi::export]
pub fn new_event(state: String, date: String, start: Option<String>, end: Option<String>) -> String {
    app::new_event(&parse(&state), &date, start.as_deref(), end.as_deref()).to_string()
}

#[uniffi::export]
pub fn edit_event(state: String, key: String, from: String, to: String) -> String {
    app::edit_event(&parse(&state), &key, &from, &to).to_string()
}

#[uniffi::export]
pub fn move_start(editor: String, date: String) -> String {
    app::move_start(&parse(&editor), &date).to_string()
}

/// Whether saving needs "only this one or all?" first.
#[uniffi::export]
pub fn asks_scope(editor: String) -> bool {
    crate::series::asks_scope(&parse(&editor))
}

#[uniffi::export]
pub fn upcoming_list(state: String) -> String {
    app::upcoming_list(&parse(&state)).to_string()
}

#[uniffi::export]
pub fn prefs_of(prefs: String) -> String {
    app::prefs_of(&list(&prefs)).to_string()
}

#[uniffi::export]
pub fn move_cursor(view: String, cursor: String, dir: i64, sunday_first: bool) -> String {
    app::move_cursor(&view, &cursor, dir, sunday_first)
}

#[uniffi::export]
pub fn alert_options(current: String, default_before: Option<i64>, with_default: bool) -> String {
    app::alert_options(&current, default_before, with_default).to_string()
}

#[uniffi::export]
pub fn before_label(minutes: Option<i64>) -> String {
    upcoming::before_label(minutes)
}

/// What Custom… typed, as minutes before ("15:00", "90", "2h"); None when it isn't a time.
#[uniffi::export]
pub fn parse_before(text: String) -> Option<i64> {
    upcoming::parse_before(&text)
}

/// Minutes before as Custom… shows them: "15:00".
#[uniffi::export]
pub fn before_hhmm(minutes: i64) -> String {
    upcoming::before_hhmm(minutes)
}

#[uniffi::export]
pub fn ago(ms: i64) -> String {
    app::ago(ms, crate::zones::now_ms())
}

/// Calendar links due to be read again.
#[uniffi::export]
pub fn due_subscriptions(subs: String, containers: String) -> String {
    json!(app::due_subscriptions(&list(&subs), &list(&containers), crate::zones::now_ms())).to_string()
}

/// A file for the import sheet.
#[uniffi::export]
pub fn read_file(name: String, text: String) -> String {
    app::read_file(&name, &text).to_string()
}

/// `{ name, text, count }`: a calendar's events as an .ics file.
#[uniffi::export]
pub fn export_calendar(events: String, container: String, name: String) -> String {
    let (text, count) = app::export_calendar(&list(&events), &container, &name);
    json!({ "name": app::export_name(&name), "text": text, "count": count }).to_string()
}

#[uniffi::export]
pub fn parse_ics(text: String) -> String {
    ical::parse_ics(&text).to_string()
}

// ---------------------------------------------------------------- writes

#[uniffi::export]
pub async fn save_event(uuob: Arc<UuobClient>, editor: String, scope: Option<String>, events: String, main: String) -> Result<(), uuob_client::Error> {
    app::save_event(uuob.inner(), &parse(&editor), scope.as_deref(), &list(&events), &main).await
}

#[uniffi::export]
pub async fn delete_event(uuob: Arc<UuobClient>, editor: String, scope: Option<String>, events: String, main: String) -> Result<(), uuob_client::Error> {
    app::delete_event(uuob.inner(), &parse(&editor), scope.as_deref(), &list(&events), &main).await
}

#[uniffi::export]
pub async fn set_own_alert(uuob: Arc<UuobClient>, key: String, choice: String) -> Result<(), uuob_client::Error> {
    app::set_own_alert(uuob.inner(), &key, &choice).await
}

#[uniffi::export]
pub async fn set_answer(uuob: Arc<UuobClient>, key: String, answer: String) -> Result<(), uuob_client::Error> {
    app::set_answer(uuob.inner(), &key, &answer).await
}

/// Returns the calendar's id (null for the first calendar).
#[uniffi::export]
pub async fn save_calendar(uuob: Arc<UuobClient>, sheet: String, infos: String, calendars: String) -> Result<Option<String>, uuob_client::Error> {
    app::save_calendar(uuob.inner(), &parse(&sheet), &list(&infos), &list(&calendars)).await
}

#[uniffi::export]
pub async fn delete_calendar(uuob: Arc<UuobClient>, id: String, subscribed: bool) -> Result<(), uuob_client::Error> {
    app::delete_calendar(uuob.inner(), &id, subscribed).await
}

#[uniffi::export]
pub async fn save_view_prefs(uuob: Arc<UuobClient>, hidden: String, show_tasks: bool) -> Result<(), uuob_client::Error> {
    app::save_view_prefs(uuob.inner(), &strings(&hidden), show_tasks).await
}

#[uniffi::export]
pub async fn save_switch(uuob: Arc<UuobClient>, views: String) -> Result<(), uuob_client::Error> {
    app::save_switch(uuob.inner(), &parse(&views)).await
}

/// A switch (one of `views.narrow` / `views.wide`) with its views set to `list`,
/// in order, the fields older copies read kept in step.
#[uniffi::export]
pub fn switch_with_list(slot: String, list: String) -> String {
    app::with_list(&parse(&slot), &strings(&list)).to_string()
}

#[uniffi::export]
pub async fn save_default_alert(uuob: Arc<UuobClient>, before: Option<i64>) -> Result<(), uuob_client::Error> {
    app::save_default_alert(uuob.inner(), before).await
}

/// How dates are written in numbers: "dmy", "mdy" or "ymd".
#[uniffi::export]
pub async fn save_date_format(uuob: Arc<UuobClient>, format: String) -> Result<(), uuob_client::Error> {
    app::save_date_format(uuob.inner(), &format).await
}

#[uniffi::export]
pub async fn save_emails(uuob: Arc<UuobClient>, emails: String) -> Result<(), uuob_client::Error> {
    app::save_emails(uuob.inner(), &strings(&emails)).await
}

/// Reads a link again; returns how many events it has.
#[uniffi::export]
pub async fn refresh(uuob: Arc<UuobClient>, calendar: String, url: String, events: String, emails: String) -> Result<u32, uuob_client::Error> {
    let mut e = strings(&emails);
    app::refresh(uuob.inner(), &calendar, &url, &list(&events), &mut e).await.map(|n| n as u32).map_err(failed)
}

/// Subscribes to a link; returns the new calendar's id.
#[uniffi::export]
pub async fn subscribe(uuob: Arc<UuobClient>, url: String, name: String, calendars: u32, emails: String) -> Result<String, uuob_client::Error> {
    let mut e = strings(&emails);
    app::subscribe(uuob.inner(), &url, &name, calendars as usize, &mut e).await.map_err(failed)
}

/// Imports the files read with `read_file` (each with `target`: "new" or a calendar).
#[uniffi::export]
pub async fn import_files(uuob: Arc<UuobClient>, items: String, calendars: u32, emails: String) -> String {
    let mut e = strings(&emails);
    app::import(uuob.inner(), &list(&items), calendars as usize, &mut e, |_| {}).await.to_string()
}

#[uniffi::export]
pub async fn move_old_calendars(uuob: Arc<UuobClient>) -> Result<(), uuob_client::Error> {
    app::move_old_calendars(uuob.inner()).await
}
