// The answers of the web version's calendar library (app/lib), kept as
// fixtures for the Rust core (core/), so both write and read events the same
// way. Run in each of the four time zones, at a fixed moment:
//   for tz in UTC Europe/Prague America/New_York Pacific/Auckland; do
//     TZ=$tz node core/tests/oracle/generate.mjs > core/tests/oracle/$(echo $tz | tr / _).json
//   done
// Each case is { fn, args, out }; core/tests/oracle.rs runs the same calls.
// (app/lib goes once the native app has replaced it; these answers stay.)

const NOW = Date.UTC(2026, 10, 19, 12, 0, 0); // Thu 2026-11-19 12:00 UTC
const Real = Date;
globalThis.Date = class extends Real {
  constructor(...a) {
    a.length ? super(...a) : super(NOW);
  }
  static now() {
    return NOW;
  }
};

const lib = '../../../app/lib/';
const zones = await import(lib + 'zones.js');
const recur = await import(lib + 'recur.js');
const ical = await import(lib + 'ical.js');
const series = await import(lib + 'series.js');
const instances = await import(lib + 'instances.js');
const details = await import(lib + 'details.js');
const upcoming = await import(lib + 'upcoming.js');
const native = await import(lib + 'native.js');

const cases = [];
const plainOf = (v) => {
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [k, plainOf(x)]));
  if (v instanceof Set) return [...v].map(plainOf);
  if (Array.isArray(v)) return v.map(plainOf);
  if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).filter(([, x]) => x !== undefined).map(([k, x]) => [k, plainOf(x)]));
  return v === undefined ? null : v;
};
const add = (fn, args, f) => {
  let out;
  try {
    out = plainOf(f(...args));
  } catch (e) {
    out = { error: String(e?.message ?? e) };
  }
  cases.push({ fn, args: plainOf(args), out });
};

// ------------------------------------------------------------------ zones
const ZONES = ['UTC', 'Europe/Prague', 'America/New_York', 'Pacific/Auckland', 'Asia/Kolkata', 'Australia/Lord_Howe'];
const WALLS = [
  ['2026-01-14', '09:00'], ['2026-07-01', '12:00'], ['2026-03-29', '02:30'], ['2026-10-25', '02:30'],
  ['2026-03-08', '02:30'], ['2026-11-01', '01:30'], ['2026-09-27', '02:30'], ['2026-04-05', '02:30'],
  ['2026-12-31', '23:59'], ['2026-02-28', '00:00'], ['2026-10-04', '02:15'],
];
for (const tz of ZONES) for (const [d, t] of WALLS) add('zonedInstant', [d, t, tz], zones.zonedInstant);
for (const [d, t] of WALLS) add('localInstant', [d, t], zones.localInstant);
const MS = [NOW, Date.UTC(2026, 2, 29, 0, 59), Date.UTC(2026, 2, 29, 1, 0), Date.UTC(2026, 9, 25, 0, 30), Date.UTC(2026, 9, 25, 1, 30), Date.UTC(2026, 2, 8, 7, 0), Date.UTC(2026, 10, 1, 6, 30), Date.UTC(2026, 8, 26, 14, 0), Date.UTC(2026, 0, 1, 0, 0)];
for (const ms of MS) {
  add('localWall', [ms], zones.localWall);
  for (const tz of ZONES) add('zonedWall', [ms, tz], zones.zonedWall);
}
for (const tz of [...ZONES, null]) for (const [d, t] of WALLS) {
  add('toLocal', [d, t, tz], zones.toLocal);
  add('fromLocal', [d, t, tz], zones.fromLocal);
}
for (const [a, b] of [['2026-01-01', '2026-12-31'], ['2026-03-28', '2026-03-30'], ['2026-11-19', '2026-11-01'], ['2024-02-28', '2024-03-01']]) add('daysBetween', [a, b], zones.daysBetween);
for (const z of ['Europe/Prague', 'UTC', 'Central European Standard Time', '', 'Mars/Olympus', 'America/Argentina/Buenos_Aires']) add('knownZone', [z], zones.knownZone);

// ------------------------------------------------------------------ recur
const RULES = [
  'FREQ=DAILY;INTERVAL=2;COUNT=3', 'FREQ=WEEKLY;BYDAY=MO,WE', 'FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU;WKST=SU', 'FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,SU;WKST=MO',
  'FREQ=MONTHLY;BYDAY=2TU', 'FREQ=MONTHLY;BYDAY=-1FR', 'FREQ=MONTHLY', 'FREQ=MONTHLY;BYMONTHDAY=-1', 'FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1',
  'FREQ=YEARLY', 'FREQ=YEARLY;BYMONTH=11;BYDAY=4TH', 'FREQ=DAILY;UNTIL=20260103', 'FREQ=DAILY;UNTIL=20260103T080000Z', 'FREQ=DAILY;UNTIL=20260103T075900Z',
  'FREQ=WEEKLY', 'FREQ=HOURLY', 'FREQ=YEARLY;BYWEEKNO=20', 'RRULE:FREQ=WEEKLY;BYDAY=FR;COUNT=5', 'FREQ=MONTHLY;BYMONTHDAY=1,15;COUNT=6',
  'FREQ=YEARLY;BYDAY=1MO', 'FREQ=DAILY;BYDAY=SA,SU', 'FREQ=DAILY;BYMONTH=12', 'FREQ=MONTHLY;INTERVAL=3;BYMONTH=1,2,3', 'freq=weekly;byday=mo',
  'FREQ=WEEKLY;BYDAY=MO;UNTIL=20261231T225959Z', 'FREQ=MONTHLY;BYDAY=5FR', 'FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29', 'FREQ=DAILY;COUNT=1', '',
];
for (const r of RULES) {
  add('parseRule', [r], recur.parseRule);
  add('supported', [r], recur.supported);
  add('describe', [r], recur.describe);
  add('ruleText', [recur.parseRule(r)], recur.ruleText);
}
const OCC = [
  [{ date: '2026-03-28', rrule: RULES[0], all_day: true }, '2026-01-01', '2026-12-31'],
  [{ date: '2026-10-21', rrule: RULES[1], all_day: true }, '2026-10-21', '2026-11-02'],
  [{ date: '1997-08-05', rrule: RULES[2], all_day: true }, '1997-08-01', '1997-08-31'],
  [{ date: '1997-08-05', rrule: RULES[3], all_day: true }, '1997-08-01', '1997-08-31'],
  [{ date: '2026-01-13', rrule: RULES[4], all_day: true }, '2026-01-01', '2026-04-30'],
  [{ date: '2026-01-30', rrule: RULES[5], all_day: true }, '2026-01-01', '2026-03-31'],
  [{ date: '2026-01-31', rrule: RULES[6], all_day: true }, '2026-01-01', '2026-05-31'],
  [{ date: '2026-01-31', rrule: RULES[7], all_day: true }, '2026-01-01', '2026-04-30'],
  [{ date: '2026-01-30', rrule: RULES[8], all_day: true }, '2026-01-01', '2026-03-31'],
  [{ date: '2024-02-29', rrule: RULES[9], all_day: true }, '2024-01-01', '2032-12-31'],
  [{ date: '2025-11-27', rrule: RULES[10], all_day: true }, '2025-01-01', '2027-12-31'],
  [{ date: '2026-01-01', rrule: RULES[11], all_day: true }, '2026-01-01', '2026-01-31'],
  [{ date: '2026-01-01', start: '09:00', all_day: false, tz: 'Europe/Prague', rrule: RULES[12] }, '2026-01-01', '2026-01-31'],
  [{ date: '2026-01-01', start: '09:00', all_day: false, tz: 'Europe/Prague', rrule: RULES[13] }, '2026-01-01', '2026-01-31'],
  [{ date: '2026-01-01', start: '09:00', all_day: false, rrule: RULES[12] }, '2026-01-01', '2026-01-31'],
  [{ date: '2020-01-06', rrule: RULES[14], all_day: true, exdates: ['2026-11-23'] }, '2026-11-16', '2026-11-30'],
  [{ date: '2026-05-11', rrule: RULES[16], all_day: true }, '2026-01-01', '2030-01-01'],
  [{ date: '2026-11-06', start: '10:00', all_day: false, rrule: RULES[17] }, '2026-11-01', '2026-12-31'],
  [{ date: '2026-11-01', rrule: RULES[18], all_day: true }, '2026-11-01', '2027-06-30'],
  [{ date: '2026-03-02', rrule: RULES[19], all_day: true }, '2026-01-01', '2029-12-31'],
  [{ date: '2026-11-19', rrule: RULES[20], all_day: true }, '2026-11-19', '2026-12-06'],
  [{ date: '2026-11-30', rrule: RULES[21], all_day: true }, '2026-11-30', '2027-12-03'],
  [{ date: '2026-01-15', rrule: RULES[22], all_day: true }, '2026-01-01', '2028-12-31'],
  [{ date: '2026-11-16', start: '23:30', all_day: false, tz: 'America/New_York', rrule: RULES[24] }, '2026-11-01', '2027-01-31'],
  [{ date: '2026-01-30', rrule: RULES[25], all_day: true }, '2026-01-01', '2026-12-31'],
  [{ date: '2024-02-29', rrule: RULES[26], all_day: true }, '2024-01-01', '2033-01-01'],
  [{ date: '2026-11-19', rrule: RULES[27], all_day: true }, '2026-01-01', '2026-12-31'],
  [{ date: '2026-11-19', all_day: true }, '2026-11-01', '2026-11-30'],
  [{ date: '2026-11-19', all_day: true }, '2026-12-01', '2026-12-30'],
  [{ date: '2026-01-05', rrule: 'FREQ=DAILY', all_day: true }, '2026-01-01', '2045-12-31'],
];
for (const [e, from, to] of OCC) add('occurrences', [e, from, to], recur.occurrences);

// ------------------------------------------------------------------ ical
const SAMPLE = [
  'BEGIN:VCALENDAR', 'PRODID:-//Google Inc//Google Calendar 70.9054//EN', 'VERSION:2.0', 'X-WR-CALNAME:Sample\\, test', 'X-WR-TIMEZONE:Europe/Prague',
  'BEGIN:VTIMEZONE', 'TZID:Europe/Prague', 'BEGIN:STANDARD', 'DTSTART:19701025T030000', 'END:STANDARD', 'END:VTIMEZONE',
  'BEGIN:VEVENT', 'DTSTART;TZID=Europe/Prague:20261019T090000', 'DTEND;TZID=Europe/Prague:20261019T100000', 'RRULE:FREQ=WEEKLY;BYDAY=MO',
  'EXDATE;TZID=Europe/Prague:20261102T090000', 'UID:weekly-1@example.test', 'SUMMARY:Stand-up',
  'DESCRIPTION:Line one\\nLine two\\, with comma and a very long text that will be folded acro', ' ss two lines',
  'BEGIN:VALARM', 'ACTION:DISPLAY', 'SUMMARY:Not an event', 'TRIGGER:-PT10M', 'END:VALARM', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;TZID=Europe/Prague:20261109T113000', 'DTEND;TZID=Europe/Prague:20261109T123000', 'RECURRENCE-ID;TZID=Europe/Prague:20261109T090000',
  'UID:weekly-1@example.test', 'SUMMARY:Stand-up (moved)', 'END:VEVENT',
  'BEGIN:VEVENT', 'RECURRENCE-ID;TZID=Europe/Prague:20261116T090000', 'DTSTART;TZID=Europe/Prague:20261116T090000', 'UID:weekly-1@example.test', 'STATUS:CANCELLED', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;VALUE=DATE:20261224', 'DTEND;VALUE=DATE:20261227', 'UID:holiday@example.test', 'SUMMARY:Holidays', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART:20261120T210000Z', 'DURATION:PT4H', 'UID:night@example.test', 'SUMMARY:Night train', 'LOCATION:Platform 3\\; north', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;TZID=Central European Standard Time:20261121T080000', 'DTEND;TZID=Central European Standard Time:20261121T090000', 'UID:outlook@example.test', 'SUMMARY:From Outlook', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;VALUE=DATE:20261201', 'UID:cancelled@example.test', 'STATUS:CANCELLED', 'SUMMARY:Gone', 'END:VEVENT',
  'END:VCALENDAR',
].join('\r\n');
const INVITE = [
  'BEGIN:VCALENDAR', 'VERSION:2.0', 'X-WR-CALNAME:ana.novak@example.test',
  'BEGIN:VEVENT', 'DTSTART:20261125T140000Z', 'DTEND:20261125T150000Z', 'UID:invite-1@google.test', 'SUMMARY:Planning',
  'ORGANIZER;CN=Ben Ortiz:mailto:ben@example.test',
  'ATTENDEE;CN=Ana Novák;PARTSTAT=NEEDS-ACTION:mailto:ana.novak@example.test',
  'ATTENDEE;CN=ben@example.test;PARTSTAT=ACCEPTED:mailto:ben@example.test',
  'ATTENDEE;CUTYPE=ROOM;CN=Room 2;PARTSTAT=ACCEPTED:mailto:room2@resource.test',
  'ATTENDEE;PARTSTAT=TENTATIVE:mailto:cy@example.test',
  'ATTENDEE;CN="Dee, D.";PARTSTAT=DECLINED:mailto:dee@example.test',
  'X-GOOGLE-CONFERENCE:https://meet.google.com/abc-defg-hij', 'URL:https://example.test/event/1',
  'DESCRIPTION:Agenda: see https://docs.example.test/x.\\n\\n-::~:~::~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~::~:~::-\\nJoin with Google Meet: https://meet.google.com/abc-defg-hij\\nPlease do not edit this section.\\n-::~:~::~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~:~::~:~::-',
  'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;VALUE=DATE:20261128', 'DURATION:P3D', 'UID:Ünïcødé-😀-uid@example.test', 'SUMMARY:Trip ✈️', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;TZID=/America/New_York:20261101T013000', 'DTEND;TZID=America/New_York:20261101T030000', 'UID:fold@example.test', 'SUMMARY:Fold hour', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;TZID=Europe/Prague:20260329T023000', 'DTEND;TZID=Europe/Prague:20260329T033000', 'UID:gap@example.test', 'SUMMARY:Gap hour', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART:20261130T220000', 'DTEND:20261201T013000', 'UID:floating@example.test', 'SUMMARY:Floating overnight', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;TZID=Asia/Kolkata:20261210T090000', 'UID:noend@example.test', 'SUMMARY:No end', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;VALUE=DATE:20261205', 'UID:allday-one@example.test', 'SUMMARY:One day', 'END:VEVENT',
  'BEGIN:VEVENT', 'SUMMARY:No start', 'UID:nostart@example.test', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;TZID=Europe/Prague:20261201T100000', 'DTEND;TZID=Europe/Prague:20261201T110000', 'RRULE:FREQ=HOURLY;COUNT=3', 'UID:hourly@example.test', 'SUMMARY:Hourly', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART:20261202T090000Z', 'DTEND:20261202T100000Z', 'RECURRENCE-ID:20261202T090000Z', 'UID:orphan@example.test', 'SUMMARY:Orphan change', 'END:VEVENT',
  'BEGIN:VEVENT', 'DTSTART;TZID=Europe/Prague:20261203T090000', 'DTEND;TZID=Europe/Prague:20261203T100000', 'RRULE:FREQ=DAILY;COUNT=5',
  'EXDATE:20261204T080000Z,20261205T080000Z', 'UID:exdates@example.test', 'SUMMARY:Exdates in UTC', 'END:VEVENT',
  'END:VCALENDAR',
].join('\n');
const OUTLOOK = [
  'BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:Microsoft Exchange Server 2010',
  'BEGIN:VEVENT', 'DTSTART;TZID="W. Europe Standard Time":20261123T100000', 'DTEND;TZID="W. Europe Standard Time":20261123T110000',
  'UID:040000008200E00074C5B7101A82E008', 'SUMMARY;LANGUAGE=en-US:Review', 'LOCATION:https://teams.microsoft.com/l/meetup-join/abc',
  'DESCRIPTION:Hi\\n________________________________________________________________________________\\nMicrosoft Teams meeting\\nJoin: https://teams.microsoft.com/l/meetup-join/abc\\n________________________________________________________________________________\\nThanks',
  'END:VEVENT', 'END:VCALENDAR',
].join('\r\n');
for (const f of [SAMPLE, INVITE, OUTLOOK, 'not a calendar', 'BEGIN:VCALENDAR\r\nEND:VCALENDAR']) add('parseIcs', [f], ical.parseIcs);
for (const u of [['weekly-1@example.test'], ['weekly-1@example.test', '2026-11-09'], ['Ünïcødé-😀-uid@example.test'], [''], ['a', 'b', 'c']]) add('recordId', u, ical.recordId);
const parsed = ical.parseIcs(SAMPLE).events.concat(ical.parseIcs(INVITE).events, ical.parseIcs(OUTLOOK).events);
add('writeIcs', [parsed, 'Sample, test'], ical.writeIcs);
add('writeIcs', [[{ id: 'e1', title: 'Made here', notes: 'Line 1\nLine 2; semi, comma \\ slash', location: 'Café Žižkov', all_day: false, date: '2026-11-20', start: '18:00', end: '19:30' }, { id: 'e2', title: 'Long '.repeat(30), all_day: true, date: '2026-11-21', end_date: '2026-11-23' }], null], ical.writeIcs);

// ------------------------------------------------------------------ series
const weekly = { id: 'w1', container: 'calendar', title: 'Stand-up', all_day: false, date: '2026-11-02', start: '09:00', end: '09:15', rrule: 'FREQ=WEEKLY;BYDAY=MO', exdates: ['2026-11-16'], location: 'Room', organizer: { email: 'x@y.z' }, future_field: 7 };
const prague = { id: 'p1', container: 'calendar.abc', title: 'Call', all_day: false, date: '2026-10-19', start: '17:00', end: '18:00', tz: 'Europe/Prague', rrule: 'FREQ=WEEKLY;BYDAY=MO', uid: 'call@x' };
const changedOne = { id: 'w1.20261109', container: 'calendar', title: 'Stand-up (late)', all_day: false, date: '2026-11-09', start: '10:00', end: '10:15', recurrence_of: 'w1', recurrence_date: '2026-11-09' };
const allday = { id: 'a1', container: 'calendar', title: 'Trip', all_day: true, date: '2026-11-27', end_date: '2026-11-29' };
const plainEv = { id: 'e1', container: 'calendar', title: 'Dentist', notes: 'n', location: '', all_day: false, date: '2026-11-25', start: '22:00', end: '23:00', url: 'https://x.test' };
const EVENTS = [weekly, prague, changedOne, allday, plainEv];
for (const e of [weekly, prague, changedOne, allday, plainEv, { ...plainEv, start: undefined, notes: '', exdates: [], end_date: '2026-11-25' }]) add('record', [e], series.record);
for (const [r, d] of [[null, '2026-11-19'], ['FREQ=WEEKLY;BYDAY=MO,WE', '2026-11-16'], ['FREQ=DAILY;COUNT=4', '2026-11-19'], ['FREQ=MONTHLY;BYDAY=2TU', '2026-11-10'], ['FREQ=WEEKLY;UNTIL=20261231T225959Z;WKST=SU', '2026-11-19'], ['FREQ=YEARLY;INTERVAL=2;UNTIL=20301119', '2026-11-19'], ['FREQ=WEEKLY;BYDAY=1MO', '2026-11-02']]) add('repeatFrom', [r, d], series.repeatFrom);
const REPS = [
  [{ freq: 'NONE' }, { date: '2026-11-19' }],
  [{ freq: 'DAILY', interval: 1, days: ['TH'], ends: 'never', until: '2027-01-01', count: 10 }, { date: '2026-11-19', all_day: false, start: '09:00' }],
  [{ freq: 'WEEKLY', interval: 2, days: ['TH'], ends: 'count', until: '2027-01-01', count: '7' }, { date: '2026-11-19' }],
  [{ freq: 'WEEKLY', interval: 1, days: ['MO', 'TH'], ends: 'until', until: '2026-12-31', count: 10 }, { date: '2026-11-19', all_day: false, start: '09:00' }],
  [{ freq: 'WEEKLY', interval: 1, days: ['TH'], ends: 'until', until: '2026-12-31', count: 10, wkst: 'SU' }, { date: '2026-11-19', all_day: false, start: '09:00', tz: 'America/New_York' }],
  [{ freq: 'MONTHLY', interval: 1, days: [], ends: 'until', until: '2027-03-01', count: 10 }, { date: '2026-11-19', all_day: true }],
  [{ freq: 'CUSTOM', raw: 'FREQ=MONTHLY;BYDAY=2TU' }, { date: '2026-11-10' }],
  [{ freq: 'YEARLY', interval: 1, days: [], ends: 'count', count: 0 }, { date: '2026-11-19' }],
];
for (const [rep, ev] of REPS) add('ruleFrom', [rep, ev], series.ruleFrom);
// What the screen shows: occurrences of the stored events.
const shownEvents = instances.expand(EVENTS, '2026-10-26', '2026-12-06');
const pick = (key) => shownEvents.find((o) => o.key === key);
const EDITOR_FOR = [
  ['calendar/w1/2026-11-23'], ['calendar/w1.20261109'], ['calendar.abc/p1/2026-11-23'], ['calendar/a1/2026-11-27'], ['calendar/e1/2026-11-25'],
];
for (const [key] of EDITOR_FOR) add('editorFor', [pick(key) ?? { missing: key }, EVENTS], series.editorFor);
add('editorFor', [{ date: '2026-11-19', start: '09:00', end: '10:00', calendar: 'calendar' }, []], series.editorFor);
// The sheet changed in various ways, then saved / deleted.
const sheet = (key, change = {}, repeatChange = null) => {
  const ed = series.editorFor(pick(key), EVENTS);
  Object.assign(ed, change);
  if (repeatChange) ed.repeat = { ...ed.repeat, ...repeatChange };
  return ed;
};
const SAVES = [
  [sheet('calendar/e1/2026-11-25', { title: 'Dentist!', start: '22:00', end: '01:00' }), null],
  [sheet('calendar/e1/2026-11-25', { start: '10:00', end: '09:00' }), null],
  [sheet('calendar/e1/2026-11-25', { calendar: 'calendar.abc' }), null],
  [sheet('calendar/e1/2026-11-25', {}, { freq: 'WEEKLY', days: ['WE'] }), null],
  [sheet('calendar/w1/2026-11-23', { title: 'Only this', start: '11:00', end: '11:30' }), 'this'],
  [sheet('calendar/w1/2026-11-23', { title: 'All moved', date: '2026-11-24', end_date: '2026-11-24' }), 'all'],
  [sheet('calendar/w1/2026-11-23', { title: 'Same day' }), 'all'],
  [sheet('calendar/w1/2026-11-23', {}, { interval: 2 }), 'all'],
  [sheet('calendar/w1/2026-11-23', { calendar: 'calendar.abc' }), 'all'],
  [sheet('calendar/w1.20261109', { title: 'Changed again' }), 'this'],
  [sheet('calendar/w1.20261109', { title: 'All from changed' }), 'all'],
  [sheet('calendar.abc/p1/2026-11-23', { start: '18:00', end: '19:00' }), 'this'],
  [sheet('calendar.abc/p1/2026-11-23', { start: '18:00', end: '19:00' }), 'all'],
  [sheet('calendar.abc/p1/2026-11-23', { all_day: true }), 'all'],
  [sheet('calendar/a1/2026-11-27', { end_date: '2026-11-30' }), null],
  [{ ...series.editorFor({ date: '2026-11-19', start: '09:00', end: '10:00', calendar: 'calendar' }), title: '  ' }, null],
];
for (const [ed, scope] of SAVES) {
  add('asksScope', [ed], series.asksScope);
  add('planSave', [ed, scope, EVENTS], series.planSave);
}
for (const [key, scope] of [['calendar/e1/2026-11-25', null], ['calendar/w1/2026-11-23', 'this'], ['calendar/w1/2026-11-23', 'all'], ['calendar/w1.20261109', 'this'], ['calendar/w1.20261109', 'all']]) {
  add('planDelete', [series.editorFor(pick(key), EVENTS), scope, EVENTS], series.planDelete);
}

// ------------------------------------------------------------------ instances
const ICS_EVENTS = ical.parseIcs(SAMPLE).events.concat(ical.parseIcs(INVITE).events).map((e) => ({ ...e, container: 'calendar.ics' }));
const ALL = EVENTS.concat(ICS_EVENTS);
for (const [from, to] of [['2026-11-16', '2026-11-22'], ['2026-10-26', '2026-12-06'], ['2026-11-19', '2026-11-19'], ['2026-03-23', '2026-04-05'], ['2026-10-19', '2026-11-08'], ['2026-11-30', '2026-12-03']]) {
  add('expand', [ALL, from, to], instances.expand);
  add('byDay', [instances.expand(ALL, from, to), from, to], instances.byDay);
}
for (const e of ALL) {
  add('lastDay', [e], instances.lastDay);
  if (!e.all_day) add('length', [e], instances.length);
}

// ------------------------------------------------------------------ details
const DETAILS = ical.parseIcs(INVITE).events.concat(ical.parseIcs(OUTLOOK).events, [
  { title: 'Zoom', location: 'https://us02web.zoom.us/j/123?pwd=x', notes: '' },
  { title: 'HTML notes', notes: '<p>Hello <b>there</b></p><a href="https://x.test/a">https://x.test/a</a><br>Line &amp; more<ul><li>one</li><li>two</li></ul>' },
  { title: 'Link text', notes: '<a href="https://x.test/b">Docs</a> and <a href="mailto:a@b.c">mail</a>' },
  { title: 'Plain', notes: 'Call tel:+420123456789 or mail mailto:a@b.c, see www.example.test or https://example.test/path?q=1.' },
  { title: 'Unclosed', notes: 'Before\n-::~:~::~:~::-\nNever closes' },
  { title: 'Jitsi', conference: 'https://meet.jit.si/MyRoom', notes: 'x' },
  { title: 'Bad url', location: 'http://[bad', notes: '' },
]);
const MINE = ['ana.novak@example.test'];
for (const e of DETAILS) {
  add('meeting', [e], details.meeting);
  add('plain', [e.notes], details.plain);
  add('notesOf', [e.notes], details.notesOf);
  add('linkify', [details.notesOf(e.notes).text], details.linkify);
  for (const ans of [null, 'maybe']) {
    add('peopleOf', [e, MINE, ans], (e, m, a) => details.peopleOf(e, new Set(m), a));
    add('answerOf', [e, MINE, ans], (e, m, a) => details.answerOf(e, new Set(m), a));
  }
  add('peopleSummary', [details.peopleOf(e, new Set(MINE))], details.peopleSummary);
  add('asksAnswer', [e, MINE], (e, m) => details.asksAnswer(e, new Set(m)));
  add('asksAnswer', [e, ['ben@example.test']], (e, m) => details.asksAnswer(e, new Set(m)));
}
for (const s of ['a@b.co', 'not an email', ' x@y.z ', 'a@b']) add('isEmail', [s], details.isEmail);

// ------------------------------------------------------------------ upcoming
for (const m of [null, 0, 5, 60, 90, 1440, 1530, 2 * 1440 + 61, 10080, -3]) add('beforeLabel', [m], upcoming.beforeLabel);
const own = new Map([['w1', null], ['e1', 30], ['w1.20261109', 5]]);
const no = new Set(['ics-declined']);
add('upcomingFor', [ALL, 10, { ownKeys: [...own], skipIds: [...no] }], (events, before, o) =>
  upcoming.upcomingFor(events, before, {
    skip: (x) => o.skipIds.includes(x.id),
    own: (x) => {
      for (const k of [x.recurrence_of ? x.id : null, x.recurrence_of ?? x.series?.id ?? x.id]) if (k && own.has(k)) return own.get(k);
    },
    color: (x) => (x.container === 'calendar' ? 'blue' : 'green'),
  }),
);
add('upcomingFor', [ALL, null, {}], (events, before) => upcoming.upcomingFor(events, before));

// ------------------------------------------------------------------ native
for (const u of ['webcal://example.test/cal.ics', 'http://example.test/a.ics', 'example.test/x', 'https://localhost/x', '', 'webcals://p01-calendars.icloud.com/published/2/abc', '  https://calendar.google.com/calendar/ical/x%40gmail.com/private-abc/basic.ics ']) add('calendarUrl', [u], native.calendarUrl);

console.log(JSON.stringify({ tz: process.env.TZ ?? null, now: NOW, cases }, null, 1));
