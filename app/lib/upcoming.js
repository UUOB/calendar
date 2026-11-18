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

/** The alert choices offered first, in minutes before the start (null: no
 *  alert). Any other number of minutes can be chosen too, up to MAX_BEFORE. */
export const BEFORE = [null, 0, 5, 10, 30, 60, 1440];
export const DEFAULT_BEFORE = 10;
/** The earliest an alert can ring: a week before. An event joins the list
 *  (and its alert is armed) when a copy of Calendar runs in the DAYS days
 *  before it, so a week leaves a week for that. */
export const MAX_BEFORE = 7 * 1440;

/** "None", "At the start", "1 hour 30 minutes before", "2 days before". */
export function beforeLabel(m) {
  if (m == null) return 'None';
  if (m <= 0) return 'At the start';
  const n = (x, one) => `${x} ${one}${x === 1 ? '' : 's'}`;
  const [d, h, min] = [Math.floor(m / 1440), Math.floor((m % 1440) / 60), m % 60];
  return [d && n(d, 'day'), h && n(h, 'hour'), min && n(min, 'minute')].filter(Boolean).join(' ') + ' before';
}

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
