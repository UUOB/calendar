// Repeating events: the part of iCalendar's RRULE that calendars use.
//
// A repeating event is stored once, with its rule (`rrule`, the RRULE text,
// e.g. "FREQ=WEEKLY;BYDAY=MO,WE;UNTIL=20270101"), the dates taken out of the
// series (`exdates`) and, as records of their own, the occurrences changed on
// their own (`recurrence_of` + `recurrence_date`). Occurrences are worked out
// only for the days on screen.
//
// Understood: FREQ=DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL, COUNT, UNTIL,
// BYDAY (also "2TU", "-1FR"), BYMONTHDAY (also negative), BYMONTH, BYSETPOS
// and WKST. Anything else (hourly rules, BYYEARDAY, BYWEEKNO …) is reported by
// `supported()`; such an event shows only its first occurrence.

import { zonedWall } from './zones.js';

const DAYS = ['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'];
const KNOWN = new Set(['FREQ', 'INTERVAL', 'COUNT', 'UNTIL', 'BYDAY', 'BYMONTHDAY', 'BYMONTH', 'BYSETPOS', 'WKST']);
const FREQS = new Set(['DAILY', 'WEEKLY', 'MONTHLY', 'YEARLY']);
const MAX = 5000; // occurrences looked at, at most, per event and screen

/** The rule as an object: { FREQ: 'WEEKLY', BYDAY: ['MO','WE'], … }. */
export function parseRule(text) {
  const r = {};
  for (const part of String(text ?? '').replace(/^RRULE:/i, '').split(';')) {
    const [k, v] = part.split('=');
    if (!k || v === undefined) continue;
    const key = k.trim().toUpperCase();
    const val = v.trim().toUpperCase();
    r[key] = ['BYDAY', 'BYMONTHDAY', 'BYMONTH', 'BYSETPOS'].includes(key) ? val.split(',').filter(Boolean) : val;
  }
  return r;
}

/** The rule back as RRULE text, in a stable order. */
export function ruleText(r) {
  const order = ['FREQ', 'INTERVAL', 'COUNT', 'UNTIL', 'BYMONTH', 'BYMONTHDAY', 'BYDAY', 'BYSETPOS', 'WKST'];
  const keys = [...order.filter((k) => k in r), ...Object.keys(r).filter((k) => !order.includes(k))];
  return keys.map((k) => `${k}=${Array.isArray(r[k]) ? r[k].join(',') : r[k]}`).join(';');
}

/** Whether this calendar can show every occurrence of the rule. */
export function supported(text) {
  const r = parseRule(text);
  return FREQS.has(r.FREQ) && Object.keys(r).every((k) => KNOWN.has(k));
}

// Dates as UTC midnights, so arithmetic never meets daylight saving.
const toUtc = (s) => {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(Date.UTC(y, m - 1, d));
};
const toIso = (d) => d.toISOString().slice(0, 10);
const daysIn = (y, m) => new Date(Date.UTC(y, m + 1, 0)).getUTCDate();

/** The end of the series as a wall date and time in the event's zone, from
 *  UNTIL (a date, or a UTC date-time with "Z"). */
function until(r, tz) {
  const u = r.UNTIL;
  if (!u) return null;
  const date = `${u.slice(0, 4)}-${u.slice(4, 6)}-${u.slice(6, 8)}`;
  if (u.length <= 8) return { date, time: '23:59' };
  const time = `${u.slice(9, 11)}:${u.slice(11, 13)}`;
  if (u.endsWith('Z')) {
    const ms = Date.UTC(+u.slice(0, 4), +u.slice(4, 6) - 1, +u.slice(6, 8), +u.slice(9, 11), +u.slice(11, 13));
    if (tz) return zonedWall(ms, tz);
    const d = new Date(ms); // a floating event: compare on this device's clock
    return { date: toIso(new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()))), time: `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}` };
  }
  return { date, time };
}

/** "2TU" -> { n: 2, day: 2 }; "FR" -> { n: 0, day: 5 }. */
const byday = (s) => {
  const m = s.match(/^([+-]?\d{1,2})?([A-Z]{2})$/);
  return m ? { n: m[1] ? Number(m[1]) : 0, day: DAYS.indexOf(m[2]) } : null;
};

/** Days of one month (y, m) matching BYMONTHDAY / BYDAY (with ordinals
 *  counted in the month), or the start's day of the month without either. */
function monthDays(y, m, r, startDay) {
  const n = daysIn(y, m);
  let days = null;
  if (r.BYMONTHDAY) {
    days = r.BYMONTHDAY.map(Number).map((d) => (d < 0 ? n + d + 1 : d)).filter((d) => d >= 1 && d <= n);
  }
  if (r.BYDAY) {
    const set = new Set();
    for (const b of r.BYDAY.map(byday).filter(Boolean)) {
      const all = [];
      for (let d = 1; d <= n; d++) if (new Date(Date.UTC(y, m, d)).getUTCDay() === b.day) all.push(d);
      if (b.n === 0) all.forEach((d) => set.add(d));
      else {
        const d = b.n > 0 ? all[b.n - 1] : all[all.length + b.n];
        if (d) set.add(d);
      }
    }
    days = days ? days.filter((d) => set.has(d)) : [...set];
  }
  if (!days) days = startDay <= n ? [startDay] : [];
  return [...new Set(days)].sort((a, b) => a - b);
}

/** Keep the BYSETPOS-th of one period's candidates (1 = first, -1 = last). */
function setpos(list, r) {
  if (!r.BYSETPOS) return list;
  return r.BYSETPOS.map(Number)
    .map((p) => (p > 0 ? list[p - 1] : list[list.length + p]))
    .filter(Boolean)
    .sort((a, b) => a - b);
}

/** Candidate dates of each period, in order, from the start onward. */
function* periods(start, r) {
  const interval = Math.max(1, Number(r.INTERVAL) || 1);
  const months = r.BYMONTH?.map(Number);
  const s = toUtc(start);
  const sy = s.getUTCFullYear();
  const sm = s.getUTCMonth();
  const sd = s.getUTCDate();
  const plainDays = r.BYDAY?.map(byday).filter(Boolean).map((b) => b.day);
  if (r.FREQ === 'DAILY') {
    for (let d = new Date(s); ; d = new Date(d.getTime() + interval * 86400000)) {
      if (months && !months.includes(d.getUTCMonth() + 1)) continue;
      if (plainDays && !plainDays.includes(d.getUTCDay())) continue;
      if (r.BYMONTHDAY && !monthDays(d.getUTCFullYear(), d.getUTCMonth(), { BYMONTHDAY: r.BYMONTHDAY }, 0).includes(d.getUTCDate())) continue;
      yield [d];
    }
  } else if (r.FREQ === 'WEEKLY') {
    const wkst = r.WKST ? DAYS.indexOf(r.WKST) : 1;
    const days = plainDays?.length ? plainDays : [s.getUTCDay()];
    const back = (s.getUTCDay() - wkst + 7) % 7;
    for (let w = new Date(s.getTime() - back * 86400000); ; w = new Date(w.getTime() + interval * 7 * 86400000)) {
      const list = [];
      for (let i = 0; i < 7; i++) {
        const d = new Date(w.getTime() + i * 86400000);
        if (days.includes(d.getUTCDay()) && (!months || months.includes(d.getUTCMonth() + 1))) list.push(d);
      }
      yield setpos(list, r);
    }
  } else if (r.FREQ === 'MONTHLY') {
    for (let i = 0; ; i += interval) {
      const y = sy + Math.floor((sm + i) / 12);
      const m = (sm + i) % 12;
      if (months && !months.includes(m + 1)) {
        yield [];
        continue;
      }
      yield setpos(monthDays(y, m, r, sd), r).map((d) => new Date(Date.UTC(y, m, d)));
    }
  } else if (r.FREQ === 'YEARLY') {
    for (let y = sy; ; y += interval) {
      const list = [];
      for (const m of months ? months.map((x) => x - 1) : [sm]) {
        // Without BYMONTH, "BYDAY=1MO" counts in the start's month, as calendars mean it.
        for (const d of monthDays(y, m, r, sd)) list.push(new Date(Date.UTC(y, m, d)));
      }
      yield setpos(list.sort((a, b) => a - b), r);
    }
  }
}

/**
 * Start dates ("YYYY-MM-DD", on the event's own clock) of the occurrences of
 * `event` that begin between `from` and `to` (inclusive). The first occurrence
 * is always the event's own date. `event.exdates` are left out here too.
 */
export function occurrences(event, from, to) {
  const start = event.date;
  if (!event.rrule || !supported(event.rrule)) return start >= from && start <= to ? [start] : [];
  const r = parseRule(event.rrule);
  const count = r.COUNT ? Number(r.COUNT) : Infinity;
  const end = until(r, event.tz);
  const time = event.all_day ? '00:00' : (event.start ?? '00:00');
  const ex = new Set(event.exdates ?? []);
  const out = [];
  let seen = 0;
  let looked = 0;
  // The start counts as the first occurrence, whether or not it fits the rule.
  const take = (d) => {
    seen++;
    if (d >= from && d <= to && !ex.has(d)) out.push(d);
  };
  take(start);
  outer: for (const list of periods(start, r)) {
    if (++looked > MAX * 4) break;
    for (const day of list) {
      const d = toIso(day);
      if (d <= start) continue;
      if (seen >= count || d > to) break outer;
      if (end && (d > end.date || (d === end.date && time > end.time))) break outer;
      take(d);
      if (out.length >= MAX) break outer;
    }
  }
  return out;
}

/** What the rule says, in a few words: "Every week on Mon, Wed", "Monthly" … */
export function describe(text) {
  if (!text) return 'Never';
  if (!supported(text)) return 'Custom';
  const r = parseRule(text);
  const n = Number(r.INTERVAL) || 1;
  const unit = { DAILY: 'day', WEEKLY: 'week', MONTHLY: 'month', YEARLY: 'year' }[r.FREQ];
  let s = n === 1 ? { DAILY: 'Daily', WEEKLY: 'Weekly', MONTHLY: 'Monthly', YEARLY: 'Yearly' }[r.FREQ] : `Every ${n} ${unit}s`;
  if (r.BYDAY) {
    const names = { SU: 'Sun', MO: 'Mon', TU: 'Tue', WE: 'Wed', TH: 'Thu', FR: 'Fri', SA: 'Sat' };
    const ord = { 1: '1st', 2: '2nd', 3: '3rd', 4: '4th', 5: '5th', '-1': 'last', '-2': '2nd last' };
    s += ' on ' + r.BYDAY.map((b) => {
      const x = byday(b);
      return x ? `${x.n ? `${ord[x.n] ?? x.n} ` : ''}${names[DAYS[x.day]]}` : b;
    }).join(', ');
  }
  if (r.COUNT) s += `, ${r.COUNT} times`;
  else if (r.UNTIL) s += `, until ${r.UNTIL.slice(6, 8)}.${r.UNTIL.slice(4, 6)}.${r.UNTIL.slice(0, 4)}`;
  return s;
}
