// Run with: npm test (several time zones; see package.json).
// Every file here is made up for the test; no real calendar is in this repository.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { occurrences, supported, describe } from './recur.js';
import { parseIcs, writeIcs, recordId } from './ical.js';
import { expand, byDay } from './instances.js';
import { toLocal, fromLocal } from './zones.js';

const occ = (rrule, date, from, to, extra = {}) => occurrences({ date, rrule, all_day: true, ...extra }, from, to);

test('daily, every other day, with COUNT', () => {
  assert.deepEqual(occ('FREQ=DAILY;INTERVAL=2;COUNT=3', '2026-03-28', '2026-01-01', '2026-12-31'), ['2026-03-28', '2026-03-30', '2026-04-01']);
});

test('weekly on several days, through a daylight saving change', () => {
  assert.deepEqual(occ('FREQ=WEEKLY;BYDAY=MO,WE', '2026-10-21', '2026-10-21', '2026-11-02'), ['2026-10-21', '2026-10-26', '2026-10-28', '2026-11-02']);
});

test('every second week counts weeks from WKST', () => {
  assert.deepEqual(occ('FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU;WKST=SU', '1997-08-05', '1997-08-01', '1997-08-31'), ['1997-08-05', '1997-08-17', '1997-08-19', '1997-08-31']);
  assert.deepEqual(occ('FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU;WKST=MO', '1997-08-05', '1997-08-01', '1997-08-31'), ['1997-08-05', '1997-08-10', '1997-08-19', '1997-08-24']);
});

test('monthly on the 2nd Tuesday and the last Friday', () => {
  assert.deepEqual(occ('FREQ=MONTHLY;BYDAY=2TU', '2026-01-13', '2026-01-01', '2026-04-30'), ['2026-01-13', '2026-02-10', '2026-03-10', '2026-04-14']);
  assert.deepEqual(occ('FREQ=MONTHLY;BYDAY=-1FR', '2026-01-30', '2026-01-01', '2026-03-31'), ['2026-01-30', '2026-02-27', '2026-03-27']);
});

test('monthly on the 31st skips short months; -1 is the last day', () => {
  assert.deepEqual(occ('FREQ=MONTHLY', '2026-01-31', '2026-01-01', '2026-05-31'), ['2026-01-31', '2026-03-31', '2026-05-31']);
  assert.deepEqual(occ('FREQ=MONTHLY;BYMONTHDAY=-1', '2026-01-31', '2026-01-01', '2026-04-30'), ['2026-01-31', '2026-02-28', '2026-03-31', '2026-04-30']);
});

test('the last weekday of the month with BYSETPOS', () => {
  assert.deepEqual(occ('FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1', '2026-01-30', '2026-01-01', '2026-03-31'), ['2026-01-30', '2026-02-27', '2026-03-31']);
});

test('yearly on 29 February only in leap years', () => {
  assert.deepEqual(occ('FREQ=YEARLY', '2024-02-29', '2024-01-01', '2032-12-31'), ['2024-02-29', '2028-02-29', '2032-02-29']);
});

test('yearly Thanksgiving: 4th Thursday of November', () => {
  assert.deepEqual(occ('FREQ=YEARLY;BYMONTH=11;BYDAY=4TH', '2025-11-27', '2025-01-01', '2027-12-31'), ['2025-11-27', '2026-11-26', '2027-11-25']);
});

test('UNTIL as a date and as a UTC time', () => {
  assert.deepEqual(occ('FREQ=DAILY;UNTIL=20260103', '2026-01-01', '2026-01-01', '2026-01-31'), ['2026-01-01', '2026-01-02', '2026-01-03']);
  const e = { date: '2026-01-01', start: '09:00', all_day: false, tz: 'Europe/Prague', rrule: 'FREQ=DAILY;UNTIL=20260103T080000Z' };
  assert.deepEqual(occurrences(e, '2026-01-01', '2026-01-31'), ['2026-01-01', '2026-01-02', '2026-01-03']);
  e.rrule = 'FREQ=DAILY;UNTIL=20260103T075900Z'; // one minute before the 9:00 Prague start
  assert.deepEqual(occurrences(e, '2026-01-01', '2026-01-31'), ['2026-01-01', '2026-01-02']);
});

test('EXDATE leaves a date out; a window far from the start still works', () => {
  assert.deepEqual(occ('FREQ=WEEKLY', '2020-01-06', '2026-11-16', '2026-11-30', { exdates: ['2026-11-23'] }), ['2026-11-16', '2026-11-30']);
});

test('rules this calendar cannot show are reported', () => {
  assert.equal(supported('FREQ=WEEKLY;BYDAY=MO'), true);
  assert.equal(supported('FREQ=HOURLY'), false);
  assert.equal(supported('FREQ=YEARLY;BYWEEKNO=20'), false);
  assert.deepEqual(occ('FREQ=YEARLY;BYWEEKNO=20', '2026-05-11', '2026-01-01', '2030-01-01'), ['2026-05-11']);
  assert.equal(describe('FREQ=WEEKLY;BYDAY=MO,WE'), 'Weekly on Mon, Wed');
  assert.equal(describe('FREQ=MONTHLY;BYDAY=-1FR'), 'Monthly on last Fri');
});

test('time zones: 9:00 in New York is the same instant on both sides of DST', () => {
  const winter = toLocal('2026-01-14', '09:00', 'America/New_York');
  const back = fromLocal(winter.date, winter.time, 'America/New_York');
  assert.deepEqual(back, { date: '2026-01-14', time: '09:00' });
  assert.deepEqual(toLocal('2026-07-01', '12:00', 'UTC'), toLocalExpected('2026-07-01T12:00:00Z'));
});
const toLocalExpected = (isoUtc) => {
  const d = new Date(isoUtc);
  const p = (n) => String(n).padStart(2, '0');
  return { date: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, time: `${p(d.getHours())}:${p(d.getMinutes())}` };
};

const SAMPLE = [
  'BEGIN:VCALENDAR',
  'PRODID:-//Google Inc//Google Calendar 70.9054//EN',
  'VERSION:2.0',
  'X-WR-CALNAME:Sample\\, test',
  'X-WR-TIMEZONE:Europe/Prague',
  'BEGIN:VTIMEZONE', 'TZID:Europe/Prague', 'BEGIN:STANDARD', 'DTSTART:19701025T030000', 'END:STANDARD', 'END:VTIMEZONE',
  'BEGIN:VEVENT',
  'DTSTART;TZID=Europe/Prague:20261019T090000',
  'DTEND;TZID=Europe/Prague:20261019T100000',
  'RRULE:FREQ=WEEKLY;BYDAY=MO',
  'EXDATE;TZID=Europe/Prague:20261102T090000',
  'UID:weekly-1@example.test',
  'SUMMARY:Stand-up',
  'DESCRIPTION:Line one\\nLine two\\, with comma and a very long text that will be folded acro',
  ' ss two lines',
  'BEGIN:VALARM', 'ACTION:DISPLAY', 'SUMMARY:Not an event', 'TRIGGER:-PT10M', 'END:VALARM',
  'END:VEVENT',
  'BEGIN:VEVENT',
  'DTSTART;TZID=Europe/Prague:20261109T113000',
  'DTEND;TZID=Europe/Prague:20261109T123000',
  'RECURRENCE-ID;TZID=Europe/Prague:20261109T090000',
  'UID:weekly-1@example.test',
  'SUMMARY:Stand-up (moved)',
  'END:VEVENT',
  'BEGIN:VEVENT',
  'RECURRENCE-ID;TZID=Europe/Prague:20261116T090000',
  'DTSTART;TZID=Europe/Prague:20261116T090000',
  'UID:weekly-1@example.test',
  'STATUS:CANCELLED',
  'END:VEVENT',
  'BEGIN:VEVENT',
  'DTSTART;VALUE=DATE:20261224',
  'DTEND;VALUE=DATE:20261227',
  'UID:holiday@example.test',
  'SUMMARY:Holidays',
  'END:VEVENT',
  'BEGIN:VEVENT',
  'DTSTART:20261120T210000Z',
  'DURATION:PT4H',
  'UID:night@example.test',
  'SUMMARY:Night train',
  'LOCATION:Platform 3\\; north',
  'END:VEVENT',
  'BEGIN:VEVENT',
  'DTSTART;TZID=Central European Standard Time:20261121T080000',
  'DTEND;TZID=Central European Standard Time:20261121T090000',
  'UID:outlook@example.test',
  'SUMMARY:From Outlook',
  'END:VEVENT',
  'BEGIN:VEVENT',
  'DTSTART;VALUE=DATE:20261201',
  'UID:cancelled@example.test',
  'STATUS:CANCELLED',
  'SUMMARY:Gone',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\r\n');

test('reads a Google-style export', () => {
  const r = parseIcs(SAMPLE);
  assert.equal(r.name, 'Sample, test');
  assert.equal(r.skipped, 1);
  assert.equal(r.unreadableRules, 0);
  assert.deepEqual(r.zones, ['Central European Standard Time']);
  const by = Object.fromEntries(r.events.map((e) => [e.title, e]));
  const s = by['Stand-up'];
  assert.equal(s.id, recordId('weekly-1@example.test'));
  assert.deepEqual([s.date, s.start, s.end, s.tz, s.rrule], ['2026-10-19', '09:00', '10:00', 'Europe/Prague', 'FREQ=WEEKLY;BYDAY=MO']);
  assert.deepEqual(s.exdates, ['2026-11-02', '2026-11-16']);
  assert.equal(s.notes, 'Line one\nLine two, with comma and a very long text that will be folded across two lines');
  const moved = by['Stand-up (moved)'];
  assert.deepEqual([moved.recurrence_of, moved.recurrence_date, moved.start], [s.id, '2026-11-09', '11:30']);
  assert.deepEqual([by.Holidays.all_day, by.Holidays.date, by.Holidays.end_date], [true, '2026-12-24', '2026-12-26']);
  const night = by['Night train'];
  assert.deepEqual([night.date, night.start, night.end_date, night.end, night.tz, night.location], ['2026-11-20', '21:00', '2026-11-21', '01:00', 'UTC', 'Platform 3; north']);
  assert.equal(by['From Outlook'].tz, 'Europe/Prague'); // unknown zone: the calendar's own
  assert.ok(!by['Not an event']);
});

test('on screen: series, moved occurrence, holidays and the night train', () => {
  const events = parseIcs(SAMPLE).events;
  const list = expand(events, '2026-10-19', '2026-11-30');
  const standups = list.filter((e) => e.title.startsWith('Stand-up')).map((e) => `${e.occurrence}:${e.title}`);
  assert.deepEqual(standups, ['2026-10-19:Stand-up', '2026-10-26:Stand-up', '2026-11-23:Stand-up', '2026-11-30:Stand-up', '2026-11-09:Stand-up (moved)']);
  assert.equal(new Set(list.map((e) => e.key)).size, list.length);
  const days = byDay(expand(events, '2026-12-23', '2026-12-27'), '2026-12-23', '2026-12-27');
  assert.deepEqual([...days].map(([d, x]) => x.allDay.length), [0, 1, 1, 1, 0]);
  assert.equal(days.get('2026-12-25').allDay[0].continues, true);
  // The night train, 21:00-01:00 UTC, splits into two pieces on this device's clock.
  const n = expand(events, '2026-11-19', '2026-11-22').find((e) => e.title === 'Night train');
  const d = byDay([n], '2026-11-19', '2026-11-22');
  const pieces = [...d.values()].flatMap((x) => x.timed);
  assert.ok(pieces.length >= 1 && pieces.length <= 2);
  assert.equal(pieces.at(-1).end, n.end);
});

test('written .ics reads back the same', () => {
  const events = parseIcs(SAMPLE).events;
  const again = parseIcs(writeIcs(events, 'Sample, test'));
  assert.equal(again.name, 'Sample, test');
  const strip = (es) => es.map(({ uid, ...e }) => e).sort((a, b) => a.id.localeCompare(b.id));
  assert.deepEqual(strip(again.events), strip(events));
  // Events made in this app (no uid, no tz) survive too.
  const own = [{ id: 'abc', title: 'Lunch; with, Ann', all_day: false, date: '2026-11-18', start: '12:00', end: '13:00', rrule: 'FREQ=WEEKLY;COUNT=3', exdates: ['2026-11-25'] }];
  const back = parseIcs(writeIcs(own)).events[0];
  assert.deepEqual([back.title, back.date, back.start, back.end, back.rrule, back.exdates, back.tz], ['Lunch; with, Ann', '2026-11-18', '12:00', '13:00', 'FREQ=WEEKLY;COUNT=3', ['2026-11-25'], undefined]);
  for (const line of writeIcs(events).split('\r\n')) assert.ok(new TextEncoder().encode(line).length <= 75, line);
});

// ------------------------------------------------------------ editing
import { editorFor, planSave, planDelete, asksScope, ruleFrom, repeatFrom } from './series.js';

const weekly = { id: 's1', container: 'c1', title: 'Gym', all_day: false, date: '2026-11-02', start: '18:00', end: '19:00', rrule: 'FREQ=WEEKLY;BYDAY=MO', exdates: ['2026-11-09'] };
const onScreen = (events, d) => expand(events, d, d).find((o) => o.date === d);

test('a new event saves as one record; an end before the start gets an hour', () => {
  const ed = editorFor({ date: '2026-11-20', start: '10:00', end: '09:00', calendar: 'c1' });
  ed.title = 'Dentist';
  const [op] = planSave(ed, null);
  assert.deepEqual(op, { put: 'c1', id: undefined, value: { title: 'Dentist', all_day: false, date: '2026-11-20', start: '10:00', end: '11:00' } });
  const late = editorFor({ date: '2026-11-20', start: '22:00', end: '01:00', calendar: 'c1' });
  assert.equal(planSave(late, null)[0].value.end_date, '2026-11-21'); // overnight, not shortened
});

test('repeat settings round trip; Google\'s own rule is kept word for word', () => {
  const rep = repeatFrom('FREQ=WEEKLY;BYDAY=MO', '2026-11-02');
  assert.deepEqual([rep.freq, rep.days], ['WEEKLY', ['MO']]);
  assert.equal(ruleFrom({ ...rep, days: ['MO', 'TH'] }, { date: '2026-11-02' }), 'FREQ=WEEKLY;BYDAY=MO,TH');
  assert.equal(repeatFrom('FREQ=MONTHLY;BYDAY=-1FR', '2026-01-30').freq, 'CUSTOM');
  const ed = editorFor(onScreen([weekly], '2026-11-16'), [weekly]);
  ed.start = '18:30';
  ed.end = '19:30';
  assert.equal(asksScope(ed), true);
  const [op] = planSave(ed, 'all', [weekly]);
  assert.equal(op.value.rrule, 'FREQ=WEEKLY;BYDAY=MO');
  assert.deepEqual([op.value.date, op.value.start, op.value.exdates], ['2026-11-02', '18:30', ['2026-11-09']]);
});

test('only this one: a changed occurrence, which then replaces it on screen', () => {
  const ed = editorFor(onScreen([weekly], '2026-11-16'), [weekly]);
  ed.title = 'Gym (late)';
  ed.start = '20:00';
  ed.end = '21:00';
  const ops = planSave(ed, 'this', [weekly]);
  assert.equal(ops.length, 1);
  const changed = { ...ops[0].value, id: ops[0].id, container: ops[0].put };
  assert.deepEqual([changed.recurrence_of, changed.recurrence_date, changed.start], ['s1', '2026-11-16', '20:00']);
  const list = expand([weekly, changed], '2026-11-16', '2026-11-16');
  assert.deepEqual(list.map((o) => o.title), ['Gym (late)']);
  // Opened again, it still belongs to the series.
  const again = editorFor(list[0], [weekly, changed]);
  assert.equal(again.series.id, 's1');
  assert.deepEqual(planDelete(again, 'this', [weekly, changed]).map((o) => o.put ?? `-${o.id}`), ['c1', `-${changed.id}`]);
});

test('delete only this one adds the date to the series; all removes its changes too', () => {
  const ed = editorFor(onScreen([weekly], '2026-11-23'), [weekly]);
  assert.deepEqual(planDelete(ed, 'this', [weekly])[0].value.exdates, ['2026-11-09', '2026-11-23']);
  const changed = { id: 's1.20261116', container: 'c1', recurrence_of: 's1', recurrence_date: '2026-11-16', title: 'x', date: '2026-11-16', start: '20:00', end: '21:00' };
  assert.deepEqual(planDelete(ed, 'all', [weekly, changed]).map((o) => o.id), ['s1', 's1.20261116']);
});

test('moving the whole series a day later moves its exceptions; another calendar moves it all', () => {
  const ed = editorFor(onScreen([weekly], '2026-11-16'), [weekly]);
  ed.date = '2026-11-17';
  ed.end_date = '2026-11-17';
  const [op] = planSave(ed, 'all', [weekly]);
  assert.deepEqual([op.value.date, op.value.exdates], ['2026-11-03', ['2026-11-10']]);
  const ed2 = editorFor(onScreen([weekly], '2026-11-16'), [weekly]);
  ed2.calendar = 'c2';
  assert.equal(asksScope(ed2), false);
  assert.deepEqual(planSave(ed2, 'all', [weekly]).map((o) => (o.put ? `+${o.put}` : `-${o.delete}`)), ['+c2', '-c1']);
});

test('a series in another time zone keeps its zone when its time changes', () => {
  const ny = { id: 'n', container: 'c1', title: 'Call', all_day: false, date: '2026-11-02', start: '09:00', end: '10:00', tz: 'America/New_York', rrule: 'FREQ=WEEKLY' };
  const o = onScreen([ny], expand([ny], '2026-11-16', '2026-11-18').find((x) => x.occurrence === '2026-11-16').date);
  const ed = editorFor(o, [ny]);
  const [op] = planSave(ed, 'all', [ny]); // saved without changes
  assert.deepEqual([op.value.date, op.value.start, op.value.end, op.value.tz], ['2026-11-02', '09:00', '10:00', 'America/New_York']);
});

test('a changed occurrence keeps its series\' time zone', () => {
  const ny = { id: 'n', container: 'c1', title: 'Call', all_day: false, date: '2026-11-02', start: '09:00', end: '10:00', tz: 'America/New_York', rrule: 'FREQ=WEEKLY' };
  const o = expand([ny], '2026-11-08', '2026-11-11').find((x) => x.occurrence === '2026-11-09');
  const ed = editorFor(o, [ny]);
  ed.title = 'Call (one off)';
  const [op] = planSave(ed, 'this', [ny]);
  assert.deepEqual([op.value.date, op.value.start, op.value.end, op.value.tz], ['2026-11-09', '09:00', '10:00', 'America/New_York']);
  // and it still shows at the series' time, in place of that occurrence
  const list = expand([ny, { ...op.value, id: op.id, container: 'c1' }], '2026-11-08', '2026-11-11');
  assert.deepEqual(list.map((x) => [x.title, x.start]), [['Call (one off)', o.start]]);
});

// ------------------------------------------------------------ invitations
import { meeting, notesOf, linkify, peopleOf, peopleSummary, answerOf, asksAnswer, plain } from './details.js';

const INVITE = [
  'BEGIN:VCALENDAR',
  'X-WR-CALNAME:alex@example.test',
  'BEGIN:VEVENT',
  'DTSTART:20261116T190000Z',
  'DTEND:20261116T200000Z',
  'RRULE:FREQ=WEEKLY;BYDAY=MO',
  'UID:plan@example.test',
  'SUMMARY:Team planning',
  'ORGANIZER;CN=Sam Example:mailto:sam@example.test',
  'ATTENDEE;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;CN=Sam Example:mailto:sam@example.test',
  'ATTENDEE;CUTYPE=INDIVIDUAL;PARTSTAT=TENTATIVE;CN=kim@example.test:mailto:kim@example.test',
  'ATTENDEE;CUTYPE=INDIVIDUAL;PARTSTAT=DECLINED;CN="Lee, J.":mailto:lee@example.test',
  'ATTENDEE;CUTYPE=INDIVIDUAL;PARTSTAT=NEEDS-ACTION:mailto:alex@example.test',
  'ATTENDEE;CUTYPE=ROOM;PARTSTAT=ACCEPTED;CN=Room 2:mailto:room2@resource.example.test',
  'X-GOOGLE-CONFERENCE:https://meet.google.com/abc-defg-hij',
  'DESCRIPTION:Agenda: https://example.test/agenda.\\n\\n-::~:~::~:~:~:~:~:~:~:~::~:~::-\\nJoin with Google Meet: https://meet.google.com/abc-defg-hij\\nOr dial: (US) +1 555-0100 PIN: 123456#\\n\\nPlease do not edit this section.\\n-::~:~::~:~:~:~:~:~:~:~::~:~::-',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\r\n');

test('invitations: organizer, people and answers, the meeting link and your address', () => {
  const r = parseIcs(INVITE);
  assert.equal(r.owner, 'alex@example.test');
  const e = r.events[0];
  assert.deepEqual(e.organizer, { name: 'Sam Example', email: 'sam@example.test' });
  assert.deepEqual(e.attendees.map((a) => [a.name, a.email, a.answer]), [
    ['Sam Example', 'sam@example.test', 'yes'],
    [undefined, 'kim@example.test', 'maybe'],
    ['Lee, J.', 'lee@example.test', 'no'],
    [undefined, 'alex@example.test', null],
  ]);
  assert.equal(e.conference, 'https://meet.google.com/abc-defg-hij');
  // They survive being written and read again, and saving the event from the sheet.
  assert.deepEqual(parseIcs(writeIcs([e])).events[0].attendees, e.attendees);
  const ed = editorFor({ ...e, container: 'c1' });
  ed.title = 'Team planning (new room)';
  const saved = planSave(ed, null, [])[0].value;
  assert.deepEqual([saved.attendees, saved.organizer, saved.conference], [e.attendees, e.organizer, e.conference]);
  // An occurrence of the series keeps them too, for "only this one".
  const o = expand([{ ...e, container: 'c1' }], '2026-11-23', '2026-11-24')[0]; // Monday 19:00 UTC: Tuesday in Tokyo
  const ed2 = editorFor(o, [{ ...e, container: 'c1' }]);
  assert.deepEqual(planSave(ed2, 'this', [])[0].value.attendees, e.attendees);
  // Fields a newer app version writes are kept by an edit here.
  const later = editorFor({ id: 'x', container: 'c1', title: 'A', date: '2026-11-18', start: '09:00', end: '10:00', alerts: [10] });
  assert.deepEqual(planSave(later, null, [])[0].value.alerts, [10]);
});

test('the card: join link, notes without the joining block, people and your answer', () => {
  const e = parseIcs(INVITE).events[0];
  assert.deepEqual(meeting(e), { url: 'https://meet.google.com/abc-defg-hij', name: 'Google Meet', host: 'meet.google.com' });
  assert.deepEqual(meeting({ location: 'Zoom: https://us02web.zoom.us/j/123?pwd=x' }).name, 'Zoom');
  assert.equal(meeting({ notes: 'Lunch at https://example.test' }), null);
  const n = notesOf(e.notes);
  assert.equal(n.text, 'Agenda: https://example.test/agenda.');
  assert.match(n.joining, /^Join with Google Meet/);
  assert.doesNotMatch(n.joining, /do not edit/);
  // A divider that never closes doesn't hide the rest of the notes.
  assert.deepEqual(notesOf('Bring snacks\n____________________________\nParking is behind the hall'), { text: 'Bring snacks\n____________________________\nParking is behind the hall', joining: '' });
  assert.deepEqual(linkify(n.text), [{ text: 'Agenda: ' }, { text: 'example.test/agenda', href: 'https://example.test/agenda' }, { text: '.' }]);
  assert.equal(plain('Hi<br>see <a href="https://x.test">https://x.test</a> &amp; more'), 'Hi\nsee https://x.test & more');

  const mine = new Set(['alex@example.test']);
  const people = peopleOf(e, mine);
  assert.deepEqual(people.map((p) => [p.email, p.organizer, p.you]), [
    ['sam@example.test', true, false],
    ['kim@example.test', false, false],
    ['lee@example.test', false, false],
    ['alex@example.test', false, true],
  ]);
  assert.equal(peopleSummary(people), '4 people · 1 yes · 1 maybe · 1 no');
  assert.equal(answerOf(e, mine, null), 'invited');
  assert.equal(answerOf(e, mine, 'maybe'), 'maybe');
  assert.equal(peopleOf(e, mine, 'yes').find((p) => p.you).answer, 'yes');
  assert.equal(answerOf(e, new Set(), null), null); // not knowing you: no guess
  assert.equal(asksAnswer(e, mine), true);
  assert.equal(asksAnswer(e, new Set(['sam@example.test'])), false); // you organize it
  assert.equal(answerOf({ title: 'Lunch' }, mine, null), null);
});

test('coming up: the next two weeks, on the event\'s own clock, with alerts', async () => {
  const { upcomingFor } = await import('./upcoming.js');
  const { localInstant } = await import('./zones.js');
  const now = localInstant('2026-03-02', '08:00');
  const events = [
    { id: 'standup', container: 'work', title: 'Standup', date: '2026-03-02', start: '09:30', end: '09:45', rrule: 'FREQ=WEEKLY', location: 'Room 2' },
    { id: 'call', container: 'work', title: 'Call', date: '2026-03-02', start: '17:00', end: '18:00', tz: 'America/New_York', location: 'https://meet.example.test/x' },
    { id: 'gone', container: 'work', title: 'Gone', date: '2026-03-02', start: '07:00', end: '07:30' }, // over
    { id: 'now', container: 'work', title: 'Going on', date: '2026-03-02', start: '07:30', end: '08:30' },
    { id: 'trip', container: 'work', title: 'Trip', date: '2026-03-04', end_date: '2026-03-05', all_day: true },
    { id: 'no', container: 'work', title: 'Declined', date: '2026-03-03', start: '12:00', end: '13:00' },
  ];
  // The alerts you chose for single events: 30 minutes before the call, none for the one going on.
  const own = (o) => ({ call: 30, now: null })[o.id];
  const list = upcomingFor(events, 10, { now, skip: (o) => o.id === 'no', own, color: (o) => (o.id === 'trip' ? 'green' : undefined) });
  // Today and the next two weeks' standups; nothing over or declined.
  assert.deepEqual(list.filter((a) => a.title === 'Standup').map((a) => a.date), ['2026-03-02', '2026-03-09', '2026-03-16']);
  assert.deepEqual(list.find((a) => a.title === 'Standup'), { id: 'work~standup~2026-03-02', title: 'Standup', body: 'Room 2', color: undefined, date: '2026-03-02', time: '09:30', end_date: '2026-03-02', end: '09:45', alert: 10 });
  // In New York's time, wherever this runs: every device writes the same. A link isn't a place.
  const call = list.find((a) => a.title === 'Call');
  assert.deepEqual([call.date, call.time, call.end, call.tz, call.alert, call.body], ['2026-03-02', '17:00', '18:00', 'America/New_York', 30, undefined]);
  assert.deepEqual(list.find((a) => a.title === 'Going on').alert, undefined);
  assert.deepEqual(list.find((a) => a.title === 'Trip'), { id: 'work~trip~2026-03-04', title: 'Trip', body: undefined, color: 'green', date: '2026-03-04', end_date: '2026-03-05' });
  assert.equal(list.length, 6);
  // No default alert: only the events with one of their own ring.
  assert.deepEqual(upcomingFor(events, null, { now, own }).filter((a) => a.alert != null).map((a) => a.title), ['Call']);
});

test('alert labels, for any number of minutes', async () => {
  const { beforeLabel, MAX_BEFORE } = await import('./upcoming.js');
  assert.equal(beforeLabel(null), 'None');
  assert.equal(beforeLabel(0), 'At the start');
  assert.equal(beforeLabel(1), '1 minute before');
  assert.equal(beforeLabel(90), '1 hour 30 minutes before');
  assert.equal(beforeLabel(1440), '1 day before');
  assert.equal(beforeLabel(2 * 1440 + 125), '2 days 2 hours 5 minutes before');
  assert.equal(MAX_BEFORE, 7 * 1440);
});
