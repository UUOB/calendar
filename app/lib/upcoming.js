// What Calendar hands UUOB as coming up (see uuob.upcoming): every event of
// the next two weeks, for the "Up next" widget and the alerts on the user's
// phones and computers.
//
// Times are written on the event's own clock (its time zone, or none for an
// event that follows the device's), so every device writes the same list and
// an event still shows and rings at the right moment after a trip.

import { addDays } from '../../uuob/ui/dates.js';
import { expand, lastDay } from './instances.js';
import { localInstant, localWall, zonedWall } from './zones.js';

/** How far ahead the list goes. Any copy of Calendar, on any device, moves
 *  the window on when it runs. */
export const DAYS = 14;

/** The alert choices, in minutes before the start (null: no alert). */
export const BEFORE = [
  [null, 'None'],
  [0, 'At the start'],
  [5, '5 minutes before'],
  [10, '10 minutes before'],
  [30, '30 minutes before'],
  [60, '1 hour before'],
  [1440, '1 day before'],
];
export const DEFAULT_BEFORE = 10;

/** A place to show: not a link (a meeting's address isn't one). */
const place = (e) => (e.location && !/^https?:\/\//i.test(e.location.trim()) ? e.location.trim() : '');

/**
 * `[{ id, date, time?, end_date?, end?, tz?, title, body?, alert? }]` for
 * `events` (all calendars): each event from now to DAYS days ahead that isn't
 * over, leaving out those `skip(occurrence)` says (declined ones). Timed ones
 * ring `own(occurrence)` minutes before (the alert you chose for that event:
 * null for none, undefined for the default), or `before` (null: none). Each
 * has its calendar's colour, `color(occurrence)`.
 */
export function upcomingFor(events, before, { now = Date.now(), skip = () => false, own = () => undefined, color = () => undefined } = {}) {
  const from = localWall(now).date;
  const out = [];
  for (const o of expand(events, addDays(from, -1), addDays(from, DAYS))) {
    if (!o.date || skip(o)) continue;
    const item = { id: o.key.replaceAll('/', '~'), title: o.title?.trim() || 'Event', body: place(o) || undefined, color: color(o) };
    if (o.all_day || !o.start) {
      if (lastDay(o) < from) continue;
      out.push({ ...item, date: o.date, end_date: lastDay(o) });
      continue;
    }
    const start = localInstant(o.date, o.start);
    const end = o.end ? localInstant(lastDay(o), o.end) : start;
    if (end <= now) continue;
    const wall = (ms) => (o.tz ? zonedWall(ms, o.tz) : localWall(ms));
    const [a, b] = [wall(start), wall(end)];
    const mine = own(o);
    const alert = mine === undefined ? before : mine;
    out.push({
      ...item,
      date: a.date, time: a.time,
      ...(o.end ? { end_date: b.date, end: b.time } : {}),
      ...(o.tz ? { tz: o.tz } : {}),
      ...(alert != null ? { alert } : {}),
    });
  }
  return out;
}
