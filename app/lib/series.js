// Editing events: what the event sheet shows, and the records to write when
// it is saved or an event is deleted, including "only this one" or "all" for
// a repeating event.
//
// A change to one occurrence is a record of its own (`recurrence_of` the
// series, `recurrence_date` the occurrence it replaces); deleting one
// occurrence adds its date to the series' `exdates`.

import { addDays, pad } from '../../uuob/ui/dates.js';
import { parseRule, ruleText, supported, describe } from './recur.js';
import { fromLocal, daysBetween, zonedInstant, localInstant } from './zones.js';

/** What the app adds to an event on screen (lib/instances.js, the sheet).
 *  Everything else belongs to the record, including fields a newer version
 *  of the app writes: an edit here keeps them. */
const ADDED = ['id', 'container', 'calendar', 'key', 'occurrence', 'series', 'row', 'continues', 'goesOn', 'readonly', 'repeat', 'repeatWas', 'changed', 'extra'];
/** What the event sheet sets; the rest of a record (people, links …) stays as it was. */
const SHEET = ['title', 'notes', 'location', 'all_day', 'date', 'end_date', 'start', 'end', 'tz', 'rrule', 'exdates', 'recurrence_of', 'recurrence_date', 'uid'];
export function record(e) {
  const r = {};
  for (const k of Object.keys(e)) if (!ADDED.includes(k) && e[k] !== undefined && e[k] !== null && e[k] !== '' && !(Array.isArray(e[k]) && !e[k].length)) r[k] = e[k];
  if (r.all_day) (delete r.start, delete r.end);
  if (r.end_date && r.end_date <= r.date) delete r.end_date;
  if (r.id) delete r.id;
  return r;
}

const mins = (t) => Number(t.slice(0, 2)) * 60 + Number(t.slice(3, 5));
const hhmm = (m) => `${pad(Math.floor(m / 60) % 24)}:${pad(m % 60)}`;
const WEEKDAYS = ['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'];
const weekdayOf = (date) => WEEKDAYS[new Date(`${date}T12:00:00Z`).getUTCDay()];

/** The sheet's repeat settings from a rule. Rules the sheet can't show in
 *  full are kept as they are ("Custom"). */
export function repeatFrom(rrule, date) {
  const none = { freq: 'NONE', interval: 1, days: [weekdayOf(date)], ends: 'never', until: addDays(date, 90), count: 10 };
  if (!rrule) return none;
  const r = parseRule(rrule);
  const simple =
    supported(rrule) &&
    Object.keys(r).every((k) => ['FREQ', 'INTERVAL', 'COUNT', 'UNTIL', 'WKST', ...(r.FREQ === 'WEEKLY' ? ['BYDAY'] : [])].includes(k)) &&
    (!r.BYDAY || r.BYDAY.every((d) => WEEKDAYS.includes(d)));
  if (!simple) return { ...none, freq: 'CUSTOM', raw: rrule, label: describe(rrule) };
  const u = r.UNTIL;
  return {
    freq: r.FREQ,
    interval: Number(r.INTERVAL) || 1,
    days: r.BYDAY ?? [weekdayOf(date)],
    ends: r.COUNT ? 'count' : u ? 'until' : 'never',
    until: u ? `${u.slice(0, 4)}-${u.slice(4, 6)}-${u.slice(6, 8)}` : none.until,
    count: Number(r.COUNT) || 10,
    ...(r.WKST ? { wkst: r.WKST } : {}),
  };
}

/** The rule text for the sheet's repeat settings (null for "Never"). */
export function ruleFrom(rep, ev) {
  if (!rep || rep.freq === 'NONE') return null;
  if (rep.freq === 'CUSTOM') return rep.raw;
  const r = { FREQ: rep.freq };
  if (rep.interval > 1) r.INTERVAL = String(rep.interval);
  if (rep.freq === 'WEEKLY') {
    const days = rep.days?.length ? rep.days : [weekdayOf(ev.date)];
    if (days.length > 1 || days[0] !== weekdayOf(ev.date)) r.BYDAY = WEEKDAYS.filter((d) => days.includes(d));
  }
  if (rep.ends === 'count') r.COUNT = String(Math.max(1, Number(rep.count) || 1));
  if (rep.ends === 'until' && rep.until) {
    if (ev.all_day) r.UNTIL = rep.until.replaceAll('-', '');
    else {
      // With a start time the end has to be a moment too: the end of that day, in UTC.
      const ms = ev.tz ? zonedInstant(rep.until, '23:59', ev.tz) : localInstant(rep.until, '23:59');
      r.UNTIL = new Date(ms).toISOString().replace(/[-:]/g, '').replace(/\.\d+/, '').slice(0, 15) + 'Z';
    }
  }
  if (rep.wkst) r.WKST = rep.wkst;
  return ruleText(r);
}

/**
 * The event sheet for an occurrence on screen (from `expand`), or for a new
 * event (no `id`). `events` are all stored events, to find a changed
 * occurrence's series.
 */
export function editorFor(o, events = []) {
  const series = o.series ?? (o.recurrence_of ? events.find((m) => m.id === o.recurrence_of && m.container === o.container) : null) ?? null;
  const repeat = repeatFrom(series?.rrule ?? o.rrule, series?.date ?? o.date);
  return {
    id: o.id, container: o.container, calendar: o.calendar ?? o.container,
    title: o.title ?? '', notes: o.notes ?? '', location: o.location ?? '',
    all_day: !!o.all_day, date: o.date, end_date: o.end_date ?? o.date,
    start: o.start ?? '09:00', end: o.end ?? '10:00',
    repeat, repeatWas: JSON.stringify(repeat),
    series, occurrence: o.occurrence ?? o.recurrence_date ?? null, changed: !!o.recurrence_of,
    uid: o.uid, readonly: !!o.readonly,
    // What the sheet doesn't show (people, links, a newer app's fields), kept on save.
    extra: Object.fromEntries(Object.entries(record(o.recurrence_of ? o : (series ?? o))).filter(([k]) => !SHEET.includes(k))),
  };
}

/** Whether saving needs "only this one or all?" first. A change of calendar
 *  or of the repeat rule can only be made to the whole series. */
export function asksScope(ed) {
  if (!ed.series) return false;
  return ed.calendar === ed.series.container && !repeatChanged(ed);
}
const repeatChanged = (ed) => JSON.stringify(ed.repeat) !== ed.repeatWas;

/** The times from the sheet, made consistent (an end before the start moves). */
function times(ed) {
  const e = { title: ed.title.trim() || 'New event', notes: ed.notes, location: ed.location, all_day: ed.all_day, date: ed.date };
  let last = ed.end_date && ed.end_date > ed.date ? ed.end_date : ed.date;
  if (ed.all_day) return { ...e, end_date: last };
  let { start, end } = ed;
  if (last === ed.date && mins(end) <= mins(start)) {
    // "22:00 to 01:00" on one date means overnight; anything else gets an hour.
    if (mins(start) >= 18 * 60 && mins(end) < 6 * 60) last = addDays(ed.date, 1);
    else end = hhmm(Math.min(mins(start) + 60, 23 * 60 + 59));
  }
  return { ...e, start, end, end_date: last };
}

/**
 * The records to write when the sheet is saved: a list of
 * { put: container, id, value } and { delete: container, id }.
 * `scope` is 'this' or 'all' for an occurrence of a repeating event.
 */
export function planSave(ed, scope, events = []) {
  const t = times(ed);
  const ops = [];
  const s = ed.series;
  if (!s) {
    const value = record({ ...ed.extra, ...t, rrule: ruleFrom(ed.repeat, t), uid: ed.uid });
    ops.push({ put: ed.calendar, id: ed.id, value });
    if (ed.id && ed.container && ed.container !== ed.calendar) ops.push({ delete: ed.container, id: ed.id });
    return ops;
  }
  if (scope === 'this') {
    const id = ed.changed ? ed.id : `${s.id}.${ed.occurrence.replaceAll('-', '')}`;
    // In the series' time zone, like the rest of it (the sheet shows this device's time).
    let when = t;
    if (s.tz && !t.all_day) {
      const a = fromLocal(t.date, t.start, s.tz);
      const b = fromLocal(t.end_date, t.end, s.tz);
      when = { ...t, date: a.date, start: a.time, end_date: b.date, end: b.time, tz: s.tz };
    }
    ops.push({ put: s.container, id, value: record({ ...ed.extra, ...when, recurrence_of: s.id, recurrence_date: ed.occurrence, uid: s.uid }) });
    return ops;
  }
  // All of them: the series moves by as much as this occurrence was moved.
  const tz = s.tz;
  const a = t.all_day ? { date: t.date, time: null } : fromLocal(t.date, t.start, tz);
  const shift = daysBetween(ed.occurrence, a.date);
  const date = addDays(s.date, shift);
  const master = { ...record(s), title: t.title, notes: t.notes, location: t.location, all_day: t.all_day, date };
  if (t.all_day) {
    master.end_date = addDays(date, daysBetween(t.date, t.end_date));
    delete master.tz;
  } else {
    const b = fromLocal(t.end_date, t.end, tz);
    Object.assign(master, { start: a.time, end: b.time, end_date: addDays(date, daysBetween(a.date, b.date)) });
    if (s.all_day) delete master.tz; // was all-day: now in this device's time
  }
  // An untouched rule is kept word for word (Google's own spelling of it).
  master.rrule = repeatChanged(ed) ? (ruleFrom(ed.repeat, master) ?? undefined) : s.rrule;
  if (shift && master.exdates) master.exdates = master.exdates.map((x) => addDays(x, shift));
  const value = record(master);
  const others = events.filter((e) => e.recurrence_of === s.id && e.container === s.container);
  const timingChanged = shift !== 0 || value.rrule !== s.rrule || !value.rrule;
  const to = ed.calendar ?? s.container;
  ops.push({ put: to, id: s.id, value });
  for (const o of others) {
    const drop = timingChanged || (ed.changed && o.id === ed.id);
    if (drop) ops.push({ delete: o.container, id: o.id });
    else if (to !== s.container) ops.push({ put: to, id: o.id, value: record(o) }, { delete: o.container, id: o.id });
  }
  if (to !== s.container) ops.push({ delete: s.container, id: s.id });
  return ops;
}

/** The records to write when an event is deleted (`scope` as for saving). */
export function planDelete(ed, scope, events = []) {
  const s = ed.series;
  if (!s) return [{ delete: ed.container, id: ed.id }];
  if (scope === 'this') {
    const ops = [{ put: s.container, id: s.id, value: record({ ...s, exdates: [...new Set([...(s.exdates ?? []), ed.occurrence])].sort() }) }];
    if (ed.changed) ops.push({ delete: s.container, id: ed.id });
    return ops;
  }
  return [
    { delete: s.container, id: s.id },
    ...events.filter((e) => e.recurrence_of === s.id && e.container === s.container).map((e) => ({ delete: e.container, id: e.id })),
  ];
}
