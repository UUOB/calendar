// From stored events to what each day on screen shows.
//
// Repeating events become their occurrences, events in another time zone move
// to this device's time, and events longer than a day become one piece per
// day: on the all-day row when they last a day or more, otherwise in the hour
// grid (an evening event that ends after midnight shows on both days).

import { addDays } from '../../uuob/ui/dates.js';
import { occurrences } from './recur.js';
import { toLocal, daysBetween } from './zones.js';

const mins = (t) => (t ? Number(t.slice(0, 2)) * 60 + Number(t.slice(3, 5)) : 0);

/** The event's last day (its first, for a one-day event). */
export const lastDay = (e) => (e.end_date && e.end_date > e.date ? e.end_date : e.date);

/** How long a timed event lasts, in minutes. */
export function length(e) {
  return daysBetween(e.date, lastDay(e)) * 1440 + mins(e.end) - mins(e.start);
}

/**
 * Every occurrence of `events` that touches `from`..`to`, on this device's
 * clock. Each one is the event's record with its own `date`, `end_date`,
 * `start` and `end`, plus `key` (unique on screen), `occurrence` (its date in
 * the series, on the event's own clock) and `series` (the repeating record it
 * comes from, if any).
 */
export function expand(events, from, to) {
  const changed = new Set(events.filter((e) => e.recurrence_of).map((e) => `${e.recurrence_of}|${e.recurrence_date}|${e.container ?? ''}`));
  const out = [];
  const add = (e, date, extra) => {
    const span = daysBetween(e.date, lastDay(e));
    let o;
    if (e.all_day) o = { ...e, date, end_date: addDays(date, span) };
    else {
      const a = toLocal(date, e.start, e.tz);
      const b = toLocal(addDays(date, span), e.end, e.tz);
      o = { ...e, date: a.date, start: a.time, end_date: b.date, end: b.time };
    }
    if (o.end_date === o.date) delete o.end_date;
    if (o.date > to || lastDay(o) < from) return;
    out.push({ ...o, ...extra });
  };
  for (const e of events) {
    if (!e.date) continue;
    if (e.recurrence_of) {
      add(e, e.date, { key: `${e.container ?? ''}/${e.id}`, occurrence: e.recurrence_date });
      continue;
    }
    const span = daysBetween(e.date, lastDay(e));
    // Look a little wider: a long event, or a time zone, can move an occurrence onto the screen.
    const dates = occurrences(e, addDays(from, -span - 1), addDays(to, 1));
    for (const d of dates) {
      if (e.rrule && changed.has(`${e.id}|${d}|${e.container ?? ''}`)) continue;
      add(e, d, { key: `${e.container ?? ''}/${e.id}/${d}`, occurrence: d, ...(e.rrule ? { series: e } : {}) });
    }
  }
  return out;
}

/**
 * What each day from `from` to `to` shows: a Map from date to
 * { allDay: [...], timed: [...] }. Timed pieces have the `start` and `end` of
 * that day ("24:00" when they go on past midnight); all-day pieces know
 * whether they `continues` from the day before or `goesOn` to the next.
 */
export function byDay(instances, from, to) {
  const days = new Map();
  for (let d = from; d <= to; d = addDays(d, 1)) days.set(d, { allDay: [], timed: [] });
  for (const o of instances) {
    const last = lastDay(o);
    const onRow = o.all_day || length(o) >= 1440;
    for (let d = o.date > from ? o.date : from; d <= last && d <= to; d = addDays(d, 1)) {
      const day = days.get(d);
      if (!day) continue;
      if (onRow) day.allDay.push({ ...o, continues: d > o.date, goesOn: d < last });
      else {
        const start = d === o.date ? o.start : '00:00';
        const end = d === last ? o.end : '24:00';
        if (d !== o.date && end === '00:00') continue; // ends exactly at midnight
        day.timed.push({ ...o, start, end, continues: d > o.date, goesOn: d < last });
      }
    }
  }
  for (const day of days.values()) {
    day.allDay.sort((a, b) => a.date.localeCompare(b.date) || lastDay(b).localeCompare(lastDay(a)) || a.title.localeCompare(b.title));
  }
  return days;
}
