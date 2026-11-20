//! Time zones (app/lib/zones.js). An imported event keeps the zone it was
//! made in (`tz`), so a weekly 9:00 meeting in New York stays at 9:00 New
//! York time on both sides of a daylight saving change, and each occurrence
//! is moved to this device's time on its own. Events made here have no `tz`:
//! they are in whatever time this device is in.
//!
//! Instants are milliseconds since 1970 (as JS's `Date`), dates "YYYY-MM-DD"
//! and times "HH:MM". The arithmetic follows the JS version step by step, so
//! both land on the same minute, also in a skipped or repeated hour.

use std::cell::RefCell;

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;

thread_local! {
    static DEVICE: RefCell<Option<TimeZone>> = const { RefCell::new(None) };
}

/// This device's time zone (or the one a test set with [`with_device_zone`]).
pub fn device() -> TimeZone {
    DEVICE.with(|d| d.borrow().clone()).unwrap_or_else(TimeZone::system)
}

/// Runs `f` as if this device were in `tz` (tests: the JS answers were
/// written in four zones).
pub fn with_device_zone<T>(tz: &str, f: impl FnOnce() -> T) -> T {
    let zone = TimeZone::get(tz).expect("zone");
    DEVICE.with(|d| *d.borrow_mut() = Some(zone));
    let out = f();
    DEVICE.with(|d| *d.borrow_mut() = None);
    out
}

/// A wall date and time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wall {
    pub date: String,
    pub time: String,
}

const DAY_MS: i64 = 86_400_000;

/// Days from 1970-01-01 to y-m-d (proleptic Gregorian; m 1..=12).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// y, m (1..=12), d of a day number.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// JS's `Date.UTC(y, m0, d, h, mi)`: a month, day, hour or minute past its end runs on.
pub fn date_utc(y: i64, m0: i64, d: i64, h: i64, mi: i64) -> i64 {
    let y = y + m0.div_euclid(12);
    let m = m0.rem_euclid(12) + 1;
    (days_from_civil(y, m, 1) + d - 1) * DAY_MS + h * 3_600_000 + mi * 60_000
}

/// The UTC wall clock of `ms`: (y, m 1..=12, d, h, mi).
pub fn utc_parts(ms: i64) -> (i64, i64, i64, i64, i64) {
    let days = ms.div_euclid(DAY_MS);
    let rest = ms.rem_euclid(DAY_MS);
    let (y, m, d) = civil_from_days(days);
    (y, m, d, rest / 3_600_000, (rest / 60_000) % 60)
}

pub fn iso_parts(y: i64, m: i64, d: i64) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// "YYYY-MM-DD" of a UTC instant (JS's `toISOString().slice(0, 10)`).
pub fn utc_date(ms: i64) -> String {
    let (y, m, d, _, _) = utc_parts(ms);
    iso_parts(y, m, d)
}

/// "HH:MM" of a UTC instant.
pub fn utc_time(ms: i64) -> String {
    let (_, _, _, h, mi) = utc_parts(ms);
    format!("{h:02}:{mi:02}")
}

pub fn ymd(s: &str) -> (i64, i64, i64) {
    let mut it = s.split('-').map(|p| p.trim().parse::<i64>().unwrap_or(0));
    (it.next().unwrap_or(1970), it.next().unwrap_or(1), it.next().unwrap_or(1))
}

pub fn hm(s: Option<&str>) -> (i64, i64) {
    let s = s.unwrap_or("00:00");
    let mut it = s.split(':').map(|p| p.trim().parse::<i64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn zone(tz: &str) -> Option<TimeZone> {
    if tz.is_empty() {
        return None;
    }
    TimeZone::get(tz).ok()
}

/// Whether this zone is known ("Europe/Prague", "UTC" …).
pub fn known_zone(tz: &str) -> bool {
    zone(tz).is_some()
}

/// The offset of `tz` at instant `ms`, in milliseconds (what JS's
/// `wallMs(ms, tz) - ms` gives).
fn offset_ms(ms: i64, tz: &TimeZone) -> i64 {
    let Ok(ts) = Timestamp::from_millisecond(ms) else { return 0 };
    tz.to_offset(ts).seconds() as i64 * 1000
}

/// The instant at which the wall clock in `tz` shows `date` `time`. A time
/// skipped by a daylight saving change lands just after it (two passes of
/// the offset, as the JS version).
pub fn zoned_instant(date: &str, time: Option<&str>, tz: &str) -> i64 {
    let (y, m, d) = ymd(date);
    let (h, mi) = hm(time);
    let want = date_utc(y, m - 1, d, h, mi);
    if tz == "UTC" {
        return want;
    }
    let Some(z) = zone(tz) else { return want };
    let t = want - offset_ms(want, &z);
    want - offset_ms(t, &z)
}

/// The instant at which this device's clock shows `date` `time` (JS's
/// `new Date(y, m - 1, d, h, mi)`).
pub fn local_instant(date: &str, time: Option<&str>) -> i64 {
    let (y, m, d) = ymd(date);
    let (h, mi) = hm(time);
    // Normalise as JS does (a minute or hour past the end runs on), then ask the zone.
    let (y, m, d, h, mi) = utc_parts(date_utc(y, m - 1, d, h, mi));
    let Ok(dt) = DateTime::new(y as i16, m as i8, d as i8, h as i8, mi as i8, 0, 0) else { return 0 };
    // "compatible": a skipped time moves on by the gap, a repeated one takes the first.
    dt.to_zoned(device()).map(|z| z.timestamp().as_millisecond()).unwrap_or(0)
}

fn wall_in(ms: i64, tz: &TimeZone) -> Wall {
    let w = ms + offset_ms(ms, tz);
    // JS goes through whole seconds of the wall clock; minutes are what's kept.
    Wall { date: utc_date(w), time: utc_time(w) }
}

/// The date and time on this device's clock at instant `ms`.
pub fn local_wall(ms: i64) -> Wall {
    wall_in(ms, &device())
}

/// The date and time on the wall clock of `tz` at instant `ms`.
pub fn zoned_wall(ms: i64, tz: &str) -> Wall {
    match zone(tz) {
        Some(z) => wall_in(ms, &z),
        None => Wall { date: utc_date(ms), time: utc_time(ms) },
    }
}

/// A date and time in `tz` (or this device's time without one), on this device's clock.
pub fn to_local(date: &str, time: Option<&str>, tz: Option<&str>) -> Wall {
    match tz.filter(|t| !t.is_empty()) {
        None => Wall { date: date.into(), time: time.unwrap_or("").into() },
        Some(tz) => local_wall(zoned_instant(date, time, tz)),
    }
}

/// A date and time on this device's clock, in `tz` (or unchanged without one).
pub fn from_local(date: &str, time: Option<&str>, tz: Option<&str>) -> Wall {
    match tz.filter(|t| !t.is_empty()) {
        None => Wall { date: date.into(), time: time.unwrap_or("").into() },
        Some(tz) => zoned_wall(local_instant(date, time), tz),
    }
}

/// Whole days from `a` to `b`.
pub fn days_between(a: &str, b: &str) -> i64 {
    let (ya, ma, da) = ymd(a);
    let (yb, mb, db) = ymd(b);
    (date_utc(yb, mb - 1, db, 0, 0) - date_utc(ya, ma - 1, da, 0, 0)) / DAY_MS
}

/// `date` plus `n` days.
pub fn add_days(date: &str, n: i64) -> String {
    let (y, m, d) = ymd(date);
    utc_date(date_utc(y, m - 1, d + n, 0, 0))
}

/// 0 for Sunday (JS's `getUTCDay()` of the date).
pub fn weekday(date: &str) -> i64 {
    let (y, m, d) = ymd(date);
    (days_from_civil(y, m, d) + 4).rem_euclid(7)
}

/// The date today on this device.
pub fn today() -> String {
    local_wall(Timestamp::now().as_millisecond()).date
}

pub fn now_ms() -> i64 {
    Timestamp::now().as_millisecond()
}
