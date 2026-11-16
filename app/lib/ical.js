// iCalendar (.ics) files: reading what Google, Apple and Outlook export or
// publish, and writing calendars they can import.
//
// An event record here looks like:
//   { title, notes, location, all_day, date, end_date?, start?, end?, tz?,
//     rrule?, exdates?, recurrence_of?, recurrence_date?, uid? }
// `date`..`end_date` are the first and last day (all-day: inclusive);
// `start`/`end` are "HH:MM" on `date` and `end_date` (or `date`). Times are in
// `tz` when it is set, otherwise in this device's time.

import { addDays, pad } from '../../uuob/ui/dates.js';
import { knownZone, zonedWall, zonedInstant, localInstant } from './zones.js';
import { supported } from './recur.js';

// ---------------------------------------------------------------- reading

/** Lines of the file, with folded lines joined back. */
function lines(text) {
  return text.replace(/\r\n?/g, '\n').replace(/\n[ \t]/g, '').split('\n').filter((l) => l.trim());
}

/** "DTSTART;TZID=Europe/Prague:20261116T090000" -> { name, params, value }. */
function property(line) {
  let i = 0;
  let quoted = false;
  for (; i < line.length; i++) {
    const c = line[i];
    if (c === '"') quoted = !quoted;
    else if (c === ':' && !quoted) break;
  }
  const head = line.slice(0, i);
  const value = line.slice(i + 1);
  const [name, ...rest] = head.split(';');
  const params = {};
  for (const p of rest) {
    const eq = p.indexOf('=');
    if (eq > 0) params[p.slice(0, eq).toUpperCase()] = p.slice(eq + 1).replace(/^"|"$/g, '');
  }
  return { name: name.toUpperCase(), params, value };
}

const unescape = (s) => s.replace(/\\([nN]|[,;\\])/g, (_, c) => (c === 'n' || c === 'N' ? '\n' : c));

/** A short stable id for a record, from an iCalendar UID (any characters, any length). */
export function recordId(...parts) {
  const s = parts.join('|');
  // Two 32-bit FNV-1a hashes with different seeds: 64 bits, plenty for one calendar.
  let a = 0x811c9dc5;
  let b = 0x01000193 ^ 0x5bd1e995;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    a = Math.imul(a ^ c, 0x01000193) >>> 0;
    b = Math.imul(b ^ c, 0x5bd1e995) >>> 0;
  }
  return `ics-${a.toString(16).padStart(8, '0')}${b.toString(16).padStart(8, '0')}`;
}

/**
 * A date or date-time value as { date, time?, tz? }. All-day values have no
 * time. A "Z" value is in UTC; a TZID the platform doesn't know (Outlook's
 * "Central European Standard Time") falls back to `fallbackTz`, then to this
 * device's time.
 */
function when(p, fallbackTz, warn) {
  const v = p.value.trim();
  const date = `${v.slice(0, 4)}-${v.slice(4, 6)}-${v.slice(6, 8)}`;
  if (p.params.VALUE === 'DATE' || v.length === 8) return { date };
  const time = `${v.slice(9, 11)}:${v.slice(11, 13)}`;
  if (v.endsWith('Z')) return { date, time, tz: 'UTC' };
  const tzid = p.params.TZID?.replace(/^\//, '');
  if (tzid) {
    if (knownZone(tzid)) return { date, time, tz: tzid };
    warn.zones.add(tzid);
    if (fallbackTz) return { date, time, tz: fallbackTz };
  }
  return { date, time };
}

/** Moves a { date, time, tz } to another zone (or keeps it when the zone is the same). */
function into(w, tz) {
  if (!w.time || (w.tz ?? null) === (tz ?? null)) return w;
  const ms = instant(w);
  if (tz) return { ...zonedWall(ms, tz), tz };
  const d = new Date(ms);
  return { date: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`, time: `${pad(d.getHours())}:${pad(d.getMinutes())}` };
}

const instant = (w) => (w.tz ? zonedInstant(w.date, w.time, w.tz) : localInstant(w.date, w.time));

/** "P1DT2H30M" / "PT45M" / "P1W" -> minutes. */
function duration(s) {
  const m = s.match(/^([+-])?P(?:(\d+)W)?(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?)?$/);
  if (!m) return null;
  const mins = (+m[2] || 0) * 10080 + (+m[3] || 0) * 1440 + (+m[4] || 0) * 60 + (+m[5] || 0);
  return m[1] === '-' ? -mins : mins;
}

/** A wall date and time plus minutes. */
function plus(w, minutes) {
  const [y, m, d] = w.date.split('-').map(Number);
  const [h, mi] = w.time.split(':').map(Number);
  const t = new Date(Date.UTC(y, m - 1, d, h, mi + minutes));
  return { date: t.toISOString().slice(0, 10), time: t.toISOString().slice(11, 16) };
}

const minutesBetween = (a, b) => {
  const ms = (w) => {
    const [y, m, d] = w.date.split('-').map(Number);
    const [h, mi] = w.time.split(':').map(Number);
    return Date.UTC(y, m - 1, d, h, mi);
  };
  return Math.round((ms(b) - ms(a)) / 60000);
};

/**
 * Reads an .ics file. Returns
 *   { name, events, skipped, unreadableRules, zones }
 * `events` are records ready to save (each with its `id`); repeating events
 * come with their changed occurrences as records of their own. `name` is the
 * calendar's name if the file has one.
 */
export function parseIcs(text) {
  const warn = { zones: new Set() };
  let name = null;
  let calTz = null;
  const raw = []; // VEVENTs as lists of properties
  let depth = [];
  let cur = null;
  for (const line of lines(text)) {
    const p = property(line);
    if (p.name === 'BEGIN') {
      depth.push(p.value.toUpperCase());
      if (p.value.toUpperCase() === 'VEVENT' && depth.length === 2) cur = [];
      continue;
    }
    if (p.name === 'END') {
      if (p.value.toUpperCase() === 'VEVENT' && depth.length === 2 && cur) (raw.push(cur), (cur = null));
      depth.pop();
      continue;
    }
    if (depth.length === 1 && p.name === 'X-WR-CALNAME') name = unescape(p.value).trim() || null;
    if (depth.length === 1 && p.name === 'X-WR-TIMEZONE' && knownZone(p.value.trim())) calTz = p.value.trim();
    if (depth.length === 2 && cur) cur.push(p);
  }

  const masters = new Map(); // uid -> record
  const changed = []; // [uid, recurrence { date, time, tz }, record]
  const cancelled = []; // [uid, recurrence]
  let skipped = 0;
  let unreadableRules = 0;

  for (const props of raw) {
    const one = (n) => props.find((p) => p.name === n);
    const text = (n) => (one(n) ? unescape(one(n).value).trim() : '');
    const dtstart = one('DTSTART');
    if (!dtstart) {
      skipped++;
      continue;
    }
    const uid = text('UID') || `${text('SUMMARY')}|${dtstart.value}`;
    const start = when(dtstart, calTz, warn);
    const rid = one('RECURRENCE-ID') ? when(one('RECURRENCE-ID'), calTz, warn) : null;
    if (text('STATUS').toUpperCase() === 'CANCELLED') {
      if (rid) cancelled.push([uid, rid]);
      else skipped++;
      continue;
    }

    const e = { title: text('SUMMARY') || 'Untitled', notes: text('DESCRIPTION'), location: text('LOCATION'), uid };
    if (!start.time) {
      // All-day: DTEND is the day after the last day.
      let after = one('DTEND') ? when(one('DTEND'), calTz, warn).date : null;
      if (!after && one('DURATION')) after = addDays(start.date, Math.max(1, Math.round((duration(one('DURATION').value.trim()) ?? 1440) / 1440)));
      after ??= addDays(start.date, 1);
      const last = after > start.date ? addDays(after, -1) : start.date;
      Object.assign(e, { all_day: true, date: start.date, ...(last > start.date ? { end_date: last } : {}) });
    } else {
      let mins = 60;
      if (one('DTEND')) {
        const end = into(when(one('DTEND'), calTz, warn), start.tz);
        mins = minutesBetween(start, end);
      } else if (one('DURATION')) mins = duration(one('DURATION').value.trim()) ?? 60;
      else mins = 0;
      const end = plus(start, Math.max(0, mins));
      Object.assign(e, { all_day: false, date: start.date, start: start.time, end: end.time, ...(end.date > start.date ? { end_date: end.date } : {}) });
      if (start.tz) e.tz = start.tz;
    }

    const rule = one('RRULE')?.value.trim();
    if (rule && !rid) {
      e.rrule = rule.replace(/^RRULE:/i, '');
      if (!supported(e.rrule)) unreadableRules++;
      const ex = new Set();
      for (const p of props.filter((p) => p.name === 'EXDATE')) {
        for (const v of p.value.split(',')) ex.add(into(when({ ...p, value: v }, calTz, warn), start.tz).date);
      }
      if (ex.size) e.exdates = [...ex].sort();
    }

    if (rid) changed.push([uid, rid, e]);
    else masters.set(uid, e);
  }

  // Changed and cancelled occurrences belong to their series.
  for (const [uid, rid] of cancelled) {
    const m = masters.get(uid);
    if (!m) continue;
    const d = into(rid, m.tz).date;
    m.exdates = [...new Set([...(m.exdates ?? []), d])].sort();
  }
  const events = [];
  for (const [uid, e] of masters) events.push({ ...e, id: recordId(uid) });
  for (const [uid, rid, e] of changed) {
    const m = masters.get(uid);
    if (!m) {
      events.push({ ...e, id: recordId(uid, rid.date) }); // its series isn't in the file: keep it as a plain event
      continue;
    }
    const d = into(rid, m.tz).date;
    events.push({ ...e, id: recordId(uid, d), recurrence_of: recordId(uid), recurrence_date: d });
  }
  return { name, events, skipped, unreadableRules, zones: [...warn.zones] };
}

// ---------------------------------------------------------------- writing

const escape = (s) => String(s).replace(/\\/g, '\\\\').replace(/\n/g, '\\n').replace(/,/g, '\\,').replace(/;/g, '\\;');

/** Lines longer than 75 bytes are folded, as the format asks. */
function fold(line) {
  const bytes = new TextEncoder().encode(line);
  if (bytes.length <= 75) return line;
  const out = [];
  let chunk = '';
  let size = 0;
  for (const ch of line) {
    const n = new TextEncoder().encode(ch).length;
    if (size + n > (out.length ? 74 : 75)) {
      out.push(chunk);
      chunk = '';
      size = 0;
    }
    chunk += ch;
    size += n;
  }
  out.push(chunk);
  return out.join('\r\n ');
}

const d8 = (date) => date.replaceAll('-', '');
const dt = (date, time) => `${d8(date)}T${time.replace(':', '')}00`;

function stamp(name, e, date, time) {
  if (e.all_day) return `${name};VALUE=DATE:${d8(date)}`;
  if (e.tz === 'UTC') return `${name}:${dt(date, time)}Z`;
  if (e.tz) return `${name};TZID=${e.tz}:${dt(date, time)}`;
  return `${name}:${dt(date, time)}`;
}

/** An .ics file of `events`, which Google, Apple and Outlook can import. */
export function writeIcs(events, name) {
  const now = new Date().toISOString().replace(/[-:]/g, '').replace(/\.\d+/, '');
  const uidOf = new Map(events.map((e) => [e.id, e.uid || `${e.id}@uuob`]));
  const out = ['BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//UUOB//Calendar//EN', 'CALSCALE:GREGORIAN', 'METHOD:PUBLISH'];
  if (name) out.push(`X-WR-CALNAME:${escape(name)}`);
  for (const e of events) {
    const uid = e.recurrence_of ? (uidOf.get(e.recurrence_of) ?? `${e.recurrence_of}@uuob`) : uidOf.get(e.id);
    out.push('BEGIN:VEVENT', `UID:${escape(uid)}`, `DTSTAMP:${now}`);
    if (e.all_day) {
      out.push(stamp('DTSTART', e, e.date), stamp('DTEND', e, addDays(e.end_date ?? e.date, 1)));
    } else {
      out.push(stamp('DTSTART', e, e.date, e.start), stamp('DTEND', e, e.end_date ?? e.date, e.end));
    }
    if (e.rrule) out.push(`RRULE:${e.rrule}`);
    for (const x of e.exdates ?? []) out.push(stamp('EXDATE', e, x, e.start));
    if (e.recurrence_of && e.recurrence_date) {
      const master = events.find((m) => m.id === e.recurrence_of);
      out.push(stamp('RECURRENCE-ID', master ?? e, e.recurrence_date, (master ?? e).start));
    }
    out.push(`SUMMARY:${escape(e.title ?? '')}`);
    if (e.location) out.push(`LOCATION:${escape(e.location)}`);
    if (e.notes) out.push(`DESCRIPTION:${escape(e.notes)}`);
    out.push('END:VEVENT');
  }
  out.push('END:VCALENDAR');
  return out.map(fold).join('\r\n') + '\r\n';
}
