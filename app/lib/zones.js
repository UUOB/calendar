// Time zones without a library: the platform's Intl knows every IANA zone.
//
// An imported event keeps the zone it was made in (`tz`), so a weekly 9:00
// meeting in New York stays at 9:00 New York time on both sides of a daylight
// saving change, and each occurrence is moved to this device's time on its own.
// Events made here have no `tz`: they are in whatever time this device is in.

import { iso, pad } from '../../uuob/ui/dates.js';

const formats = new Map();
function format(tz) {
  if (!formats.has(tz)) {
    formats.set(
      tz,
      new Intl.DateTimeFormat('en-US', {
        timeZone: tz, hourCycle: 'h23',
        year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric', second: 'numeric',
      }),
    );
  }
  return formats.get(tz);
}

/** Whether the platform knows this zone ("Europe/Prague", "UTC" …). */
export function knownZone(tz) {
  if (!tz) return false;
  try {
    format(tz);
    return true;
  } catch {
    formats.delete(tz);
    return false;
  }
}

/** The wall clock in `tz` at instant `ms`, as UTC milliseconds of that wall clock. */
function wallMs(ms, tz) {
  const p = Object.fromEntries(format(tz).formatToParts(new Date(ms)).map((x) => [x.type, x.value]));
  return Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour % 24, +p.minute, +p.second);
}

/** The instant (ms) at which the wall clock in `tz` shows `date` `time`. A
 *  time skipped by a daylight saving change lands just after it. */
export function zonedInstant(date, time, tz) {
  const [y, m, d] = date.split('-').map(Number);
  const [h, mi] = (time ?? '00:00').split(':').map(Number);
  const want = Date.UTC(y, m - 1, d, h, mi);
  if (tz === 'UTC') return want;
  let t = want - (wallMs(want, tz) - want);
  t = want - (wallMs(t, tz) - t);
  return t;
}

/** The instant (ms) at which this device's clock shows `date` `time`. */
export function localInstant(date, time) {
  const [y, m, d] = date.split('-').map(Number);
  const [h, mi] = (time ?? '00:00').split(':').map(Number);
  return new Date(y, m - 1, d, h, mi).getTime();
}

/** `{ date, time }` on this device's clock at instant `ms`. */
export function localWall(ms) {
  const d = new Date(ms);
  return { date: iso(d), time: `${pad(d.getHours())}:${pad(d.getMinutes())}` };
}

/** `{ date, time }` on the wall clock of `tz` at instant `ms`. */
export function zonedWall(ms, tz) {
  const w = new Date(wallMs(ms, tz));
  return { date: w.toISOString().slice(0, 10), time: w.toISOString().slice(11, 16) };
}

/** A date and time in `tz` (or this device's time without one), on this device's clock. */
export function toLocal(date, time, tz) {
  if (!tz) return { date, time };
  return localWall(zonedInstant(date, time, tz));
}

/** A date and time on this device's clock, in `tz` (or unchanged without one). */
export function fromLocal(date, time, tz) {
  if (!tz) return { date, time };
  return zonedWall(localInstant(date, time), tz);
}

/** Whole days from `a` to `b` (both "YYYY-MM-DD"). */
export function daysBetween(a, b) {
  return Math.round((Date.UTC(...ymd(b)) - Date.UTC(...ymd(a))) / 86400000);
}
const ymd = (s) => {
  const [y, m, d] = s.split('-').map(Number);
  return [y, m - 1, d];
};
