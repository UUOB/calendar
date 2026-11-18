<script>
  import { onMount, tick } from 'svelte';
  import Icon from '../uuob/ui/Icon.svelte';
  import Settings, { settings } from '../uuob/ui/Settings.svelte';
  import TimeField from '../uuob/ui/TimeField.svelte';
  import { uuob } from '../uuob/ui/uuob.js';
  import { today, iso, parse, addDays, pad, friendlyTime } from '../uuob/ui/dates.js';
  import { expand, byDay } from './lib/instances.js';
  import { editorFor, planSave, planDelete, asksScope } from './lib/series.js';
  import { daysBetween } from './lib/zones.js';
  import { parseIcs, writeIcs } from './lib/ical.js';
  import { record } from './lib/series.js';
  import { calendarUrl, fetchText, saveText, openLink } from './lib/native.js';
  import { meeting, notesOf, linkify, peopleOf, peopleSummary, answerOf, asksAnswer } from './lib/details.js';
  import { describe } from './lib/recur.js';
  import { upcomingFor, BEFORE, DEFAULT_BEFORE } from './lib/upcoming.js';

  // Every calendar is a container of its own, so it can be shared with other
  // people. The first calendar lives in the app's first container, which
  // stays private.
  const eventsIn = (calendar) => uuob.collection('events', calendar);
  const firstCal = uuob.collection('calendars'); // the first calendar's { name, color }
  const prefsC = uuob.collection('prefs'); // synced preferences, in our own container
  // Calendar links you subscribed to, by calendar. In the private first
  // container: a link like Google's secret address is a key to that calendar,
  // so it is not shared along with the calendar.
  const subsC = uuob.collection('subscriptions');
  // Your Going / Maybe / No to invitations, by event. Also in the private
  // first container: only you see it, and the organizer isn't told.
  const answersC = uuob.collection('answers');
  const alertsC = uuob.collection('alerts'); // your own alert for an event, like your answers
  const COLORS = ['blue', 'red', 'orange', 'yellow', 'green', 'teal', 'purple', 'pink'];
  const HOUR = 52; // px per hour

  let all = $state([]); // events from every container
  let containers = $state([]);
  let infos = $state([]); // each added calendar's { color, created }, kept inside it
  let first = $state({ name: 'Personal', color: 'blue' });
  const events = $derived(all.map((e) => ({ ...e, calendar: e.container })));
  const cals = $derived(
    containers
      .map((c) => (c.main ? { created: 0, ...c, ...first, id: c.id } : { created: 1, color: 'blue', ...infos.find((i) => i.container === c.id), ...c }))
      .sort((x, y) => y.main - x.main || x.created - y.created),
  );
  const canEdit = (id) => {
    const c = cals.find((c) => c.id === id);
    return c?.canWrite !== false && !c?.subscribed;
  };
  let hidden = $state(new Set());
  let mode = $state('week'); // 'week' | 'month'
  let cursor = $state(today());
  let narrow = $state(false);
  let viewing = $state(null); // the event whose card is open
  let peopleOpen = $state(false);
  let joiningOpen = $state(false);
  let addMenu = $state(false); // the sidebar's + : new calendar, import, subscribe
  let settingsOpen = $state(false);
  let alertBefore = $state(DEFAULT_BEFORE); // the default alert, minutes before an event (null: none)
  let alertsKnown = $state(false);
  let eventsKnown = $state(false); // every calendar has delivered its events
  let hourTick = $state(0); // moves the coming-up window on
  let answers = $state(new Map()); // event id -> 'yes' | 'maybe' | 'no'
  let ownAlerts = $state(new Map()); // event id -> minutes before | null (none)
  let editor = $state(null); // event being edited / created (see lib/series.js)
  let scopeAsk = $state(null); // 'save' | 'delete': only this occurrence, or all?
  let calSheet = $state(null);
  let calsOpen = $state(false); // phones: the calendar list, shown as a sheet
  let importSheet = $state(null); // { items: [{ name, r, target }], busy, done, total, result }
  let subSheet = $state(null); // { url, name, color, busy, error }
  let subs = $state([]); // [{ id: calendar, url, refreshed }]
  let fileEl = $state();
  let note = $state(''); // a short message after export / refresh
  let showTasks = $state(false);
  let tasks = $state([]);
  let drag = $state(null);
  let now = $state(new Date());
  let gridEl = $state();
  // Width of the hour grid's scrollbar (0 with overlay scrollbars). The rows
  // above the grid don't scroll, so they pad by this much to stay aligned.
  let scrollbar = $state(0);
  let gridWidth = $state(800); // to guess how many lines an event's title needs
  $effect(() => {
    const el = gridEl;
    if (!el) return;
    const measure = () => ((scrollbar = el.offsetWidth - el.clientWidth), (gridWidth = el.clientWidth));
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    measure();
    return () => ro.disconnect();
  });

  // Follows the clock, so an app left open overnight rolls over to the new day.
  const t = $derived(iso(now));
  let shownToday = today();
  $effect(() => {
    if (t === shownToday) return;
    if (cursor === shownToday) cursor = t; // keep following "today" if we were on it
    shownToday = t;
  });
  // Monday, or Sunday if the user chose so in Settings.
  const weekStart = (d) => addDays(d, -((parse(d).getDay() + (settings.sunday_first ? 0 : 6)) % 7));
  const days = $derived(narrow ? [cursor] : Array.from({ length: 7 }, (_, i) => addDays(weekStart(cursor), i)));
  const monthDays = $derived.by(() => {
    const first = iso(new Date(parse(cursor).getFullYear(), parse(cursor).getMonth(), 1));
    const start = weekStart(first);
    return Array.from({ length: 42 }, (_, i) => addDays(start, i));
  });
  const calOf = (e) => cals.find((c) => c.id === e.calendar) ?? cals[0];
  const colorOf = (e) => `var(--c-${calOf(e)?.color ?? 'blue'})`;
  const shown = $derived(events.filter((e) => !hidden.has(e.calendar)));
  // What each day on screen shows: occurrences of repeating events, events
  // longer than a day on each of their days (lib/instances.js).
  const range = $derived(mode === 'month' ? [monthDays[0], monthDays[41]] : [days[0], days[days.length - 1]]);
  const instances = $derived(expand(shown, range[0], range[1]));
  const dayMap = $derived(byDay(instances, range[0], range[1]));

  // Invitations. Google names your own calendar by your address, so the
  // files and links you add tell which guest is you (kept with your prefs).
  let addresses = $state([]);
  const mine = $derived(new Set(addresses));
  const answerKey = (e) => (e.recurrence_of ? e.id : (e.series?.id ?? e.id));
  const chosen = (e) => answers.get(answerKey(e)) ?? null;
  const answerFor = (e) => answerOf(e, mine, chosen(e));
  function setAnswer(e, a) {
    answers = new Map(answers).set(answerKey(e), a);
    answersC.put({ answer: a }, answerKey(e));
  }
  /** "2:30–4:00 PM", "14:30–16:00" */
  function span(a, b) {
    const x = friendlyTime(a);
    const y = friendlyTime(b === '24:00' ? '00:00' : b);
    const suffix = x.match(/\s?[AP]M$/i)?.[0];
    return suffix && y.endsWith(suffix.trim()) ? `${x.slice(0, -suffix.length)}–${y}` : `${x}–${y}`;
  }
  const blockTime = (e) => (e.continues ? `until ${friendlyTime(e.end === '24:00' ? '00:00' : e.end)}` : span(e.start, e.end));
  /** About how many lines a title takes in an event block (13px bold is about 7px a letter). */
  const titleLines = (title, lanes) => Math.ceil(title.length / Math.max(4, Math.floor(((gridWidth - 56) / days.length / lanes - 23) / 7.2)));
  const place = (e) => (e.location && !/^https?:\/\//i.test(e.location.trim()) ? e.location : '');
  function whenText(e) {
    const day = (d) => parse(d).toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long' });
    const last = e.end_date && e.end_date > e.date ? e.end_date : e.date;
    if (e.all_day) return last > e.date ? `${day(e.date)} – ${day(last)}` : `${day(e.date)} · all day`;
    if (last > e.date) return `${day(e.date)}, ${friendlyTime(e.start)} – ${day(last)}, ${friendlyTime(e.end)}`;
    return `${day(e.date)} · ${span(e.start, e.end)}`;
  }
  const NONE = { allDay: [], timed: [] };
  const onDay = (d) => dayMap.get(d) ?? NONE;
  const tasksOn = (d) => (showTasks ? tasks.filter((x) => x.due === d && !x.done) : []);
  const mins = (hhmm) => (hhmm ? Number(hhmm.slice(0, 2)) * 60 + Number(hhmm.slice(3)) : 0);
  const hhmm = (m) => `${pad(Math.floor(m / 60) % 24)}:${pad(m % 60)}`;

  const title = $derived.by(() => {
    const d = parse(mode === 'week' && !narrow ? days[3] : cursor);
    return { month: d.toLocaleDateString(undefined, { month: 'long' }), year: d.getFullYear() };
  });

  // Lay out overlapping timed events side by side.
  function layout(list) {
    const timed = list.filter((e) => !e.all_day).sort((a, b) => mins(a.start) - mins(b.start) || mins(b.end) - mins(a.end));
    const out = [];
    let cluster = [];
    let clusterEnd = -1;
    const flush = () => {
      const lanes = [];
      for (const e of cluster) {
        let lane = lanes.findIndex((end) => end <= mins(e.start));
        if (lane < 0) lane = lanes.length;
        lanes[lane] = Math.max(mins(e.end), mins(e.start) + 15);
        out.push({ e, lane });
      }
      for (const o of out.slice(out.length - cluster.length)) o.lanes = lanes.length;
      cluster = [];
    };
    for (const e of timed) {
      if (mins(e.start) >= clusterEnd && cluster.length) flush();
      cluster.push(e);
      clusterEnd = Math.max(clusterEnd, mins(e.end));
    }
    if (cluster.length) flush();
    return out;
  }

  function move(dir) {
    if (mode === 'month') {
      const d = parse(cursor);
      cursor = iso(new Date(d.getFullYear(), d.getMonth() + dir, 1));
    } else cursor = addDays(cursor, dir * (narrow ? 1 : 7));
  }

  function newEvent(date, start = '09:00', end = null) {
    editor = {
      ...editorFor({
        date, start, end: end ?? hhmm(mins(start) + 60),
        calendar: (cals.find((c) => canEdit(c.id) && !hidden.has(c.id)) ?? cals.find((c) => canEdit(c.id)))?.id ?? uuob.self,
      }),
      alert: 'default', alertWas: 'default',
    };
  }

  // Drag on the grid to create an event. A finger has to rest for a moment
  // first, so scrolling the day never creates one; a mouse starts right away.
  let hold = null;
  function down(ev, date) {
    if (ev.button !== 0 || ev.target.closest('.event')) return;
    const el = ev.currentTarget;
    const rect = el.getBoundingClientRect();
    const m = Math.max(0, Math.min(1425, Math.round(((ev.clientY - rect.top) / HOUR) * 4) * 15));
    const start = () => {
      hold = null;
      drag = { date, from: m, to: m + 60, top: rect.top };
      try { el.setPointerCapture(ev.pointerId); } catch {}
    };
    if (ev.pointerType === 'mouse') return start();
    cancelHold();
    hold = { x: ev.clientX, y: ev.clientY, timer: setTimeout(start, 350) };
  }
  function cancelHold() {
    clearTimeout(hold?.timer);
    hold = null;
  }
  function moveDrag(ev) {
    if (hold && Math.hypot(ev.clientX - hold.x, ev.clientY - hold.y) > 8) cancelHold();
    if (!drag) return;
    const m = Math.round(((ev.clientY - drag.top) / HOUR) * 4) * 15;
    drag.to = Math.max(drag.from + 15, Math.min(1440, m));
  }
  function up() {
    cancelHold();
    if (!drag) return;
    const d = drag;
    drag = null;
    newEvent(d.date, hhmm(d.from), hhmm(Math.min(d.to, 1439)));
  }
  function cancelDrag() {
    cancelHold();
    drag = null;
  }
  // Once an event is being dragged out, the finger sizes it instead of scrolling.
  function sizing(node) {
    const stop = (e) => drag && e.preventDefault();
    node.addEventListener('touchmove', stop, { passive: false });
    return { destroy: () => node.removeEventListener('touchmove', stop) };
  }

  // Writes what lib/series.js worked out. Moving an event to another
  // calendar moves it to that calendar's container.
  // Returns the id of the first event written (the one the sheet was about).
  async function apply(ops) {
    let first = null;
    for (const o of ops) {
      if (o.put) first ??= (await eventsIn(o.put).put(o.value, o.id))?.id ?? o.id;
      else await eventsIn(o.delete).delete(o.id);
    }
    return first;
  }
  function saveEvent(scope) {
    if (editor.series && !scope) {
      if (asksScope(editor)) return void (scopeAsk = 'save');
      scope = 'all';
    }
    const ops = planSave(editor, scope, events);
    const { alert, alertWas } = editor;
    editor = scopeAsk = null;
    apply(ops).then((id) => id && alert !== alertWas && setOwnAlert(id, alert));
  }
  function deleteEvent(scope) {
    if (editor.series && !scope) return void (scopeAsk = 'delete');
    const ops = planDelete(editor, scope, events);
    editor = scopeAsk = null;
    apply(ops);
  }
  // Tapping an event shows its card; the card's pencil opens the sheet. A day
  // piece of a longer event shows the whole event.
  function open(e) {
    viewing = instances.find((o) => o.key === e.key) ?? e;
    peopleOpen = joiningOpen = false;
    linkError = '';
  }
  let linkError = $state('');
  const visit = (url) => ((linkError = ''), openLink(url).catch(() => (linkError = "That link couldn't be opened.")));
  function edit(e) {
    viewing = null;
    const alert = alertChoice(e);
    editor = { ...editorFor(e, events), alert, alertWas: alert };
  }
  // A new start date keeps the event's length in days.
  function moveStart(date) {
    if (!date) return;
    const span = Math.max(0, daysBetween(editor.date, editor.end_date ?? editor.date));
    editor.date = date;
    editor.end_date = addDays(date, span);
  }
  const WEEK = [['MO', 'M'], ['TU', 'T'], ['WE', 'W'], ['TH', 'T'], ['FR', 'F'], ['SA', 'S'], ['SU', 'S']];
  function toggleDay(d) {
    const days = editor.repeat.days.includes(d) ? editor.repeat.days.filter((x) => x !== d) : [...editor.repeat.days, d];
    if (days.length) editor.repeat.days = days;
  }
  const unit = (freq, n) => ({ DAILY: 'day', WEEKLY: 'week', MONTHLY: 'month', YEARLY: 'year' })[freq] + (n > 1 ? 's' : '');

  async function saveCal() {
    const { id, main, color, created } = calSheet;
    const name = calSheet.name.trim();
    if (!name) return;
    calSheet = null;
    if (main) return void (await firstCal.put({ name, color }, 'personal'));
    let cal = id;
    if (!cal) cal = await uuob.containers.create(name);
    else if (name !== cals.find((c) => c.id === id)?.name) await uuob.containers.rename(id, name);
    // The rest of the info (a link it follows, whose calendar it is) stays.
    const { container, id: _, ...info } = infos.find((i) => i.container === cal) ?? {};
    await uuob.collection('calendar', cal).put({ ...info, color, created: created ?? Date.now() }, 'info');
  }
  // Deletes the calendar everywhere; one someone shared with you is only left.
  async function deleteCal() {
    const { id } = calSheet;
    calSheet = null;
    await uuob.containers.delete(id);
    if (subs.some((x) => x.id === id)) await subsC.delete(id);
  }
  function share(c) {
    calSheet = null;
    uuob.containers.share(c.id).catch(() => {});
  }
  const sharedNote = (c) =>
    c.from
      ? `From ${c.from.name}${c.canWrite ? '' : ' · view only'}`
      : c.sharedWith?.length
        ? `Shared with ${c.sharedWith.map((p) => p.name).join(', ')}`
        : '';

  // Added calendars used to be records in the app's first container. Move
  // each into a container of its own (keyed by its old id, so every device
  // ends up with the same container).
  async function moveOldCalendars() {
    const old = (await firstCal.list()).filter((c) => c.id !== 'personal');
    if (!old.length) return;
    const loose = await eventsIn().list();
    for (const c of old) {
      const cal = await uuob.containers.create(c.name || 'Calendar', c.id).catch(() => uuob.containers.create(c.name || 'Calendar'));
      await uuob.collection('calendar', cal).put({ color: c.color ?? 'blue', created: c.created ?? 1 }, 'info');
      for (const { calendar, ...e } of loose.filter((e) => e.calendar === c.id)) {
        await eventsIn(cal).put(e);
        await eventsIn().delete(e.id);
      }
      await firstCal.delete(c.id);
    }
  }

  // ------------------------------------------------ import, export, subscribe
  // Every save refreshes what's on screen; while hundreds are written, the
  // screen waits and reads them once at the end.
  let stopEvents = null;
  function watchEvents() {
    stopEvents?.();
    stopEvents = uuob.all('events').subscribe((r, complete) => ((all = r), (eventsKnown ||= complete)));
  }
  async function quietly(work) {
    stopEvents?.();
    stopEvents = null;
    try {
      return await work();
    } finally {
      watchEvents();
    }
  }
  async function newCalendar(name, extra = {}) {
    const id = await uuob.containers.create(name.slice(0, 64) || 'Calendar');
    await uuob.collection('calendar', id).put({ color: COLORS[(cals.length + 1) % COLORS.length], created: Date.now(), ...extra }, 'info');
    return id;
  }

  async function readFiles(ev) {
    const files = [...(ev.currentTarget.files ?? [])];
    ev.currentTarget.value = '';
    if (!files.length) return;
    const items = [];
    for (const f of files) {
      try {
        const r = parseIcs(await f.text());
        items.push({ name: r.name ?? f.name.replace(/\.ics$/i, ''), r, target: 'new' });
      } catch {
        items.push({ name: f.name, error: 'This file could not be read as a calendar.' });
      }
    }
    calsOpen = false;
    importSheet = { items, busy: false, done: 0, total: items.reduce((n, i) => n + (i.r?.events.length ?? 0), 0), result: null };
  }

  async function runImport() {
    const s = importSheet;
    s.busy = true;
    try {
      await quietly(() => importAll(s));
    } catch (e) {
      s.busy = false;
      s.result = { events: s.done, unreadable: 0, zones: [], error: e?.message || 'The import stopped.' };
    }
  }
  async function importAll(s) {
    let events = 0;
    let unreadable = 0;
    const zones = new Set();
    for (const item of s.items.filter((i) => i.r?.events.length)) {
      const cal = item.target === 'new' ? await newCalendar(item.name) : item.target;
      await noteAddress(item.r.owner);
      for (const e of item.r.events) {
        await eventsIn(cal).put(record(e), e.id);
        s.done++;
        events++;
      }
      unreadable += item.r.unreadableRules;
      item.r.zones.forEach((z) => zones.add(z));
    }
    s.busy = false;
    s.result = { events, unreadable, zones: [...zones] };
  }

  /** Puts the link's events into its calendar: new and changed ones are
   *  written, ones no longer at the link are removed. */
  async function refresh(sub) {
    const r = parseIcs(await fetchText(sub.url));
    const keep = new Set(r.events.map((e) => e.id));
    const gone = all.filter((e) => e.container === sub.id && !keep.has(e.id));
    await quietly(async () => {
      for (const e of r.events) await eventsIn(sub.id).put(record(e), e.id);
      for (const e of gone) await eventsIn(sub.id).delete(e.id);
    });
    await subsC.put({ url: sub.url, refreshed: Date.now() }, sub.id);
    await noteAddress(r.owner);
    return r;
  }

  /** Remembers your address when a file or link is your own calendar
   *  (Google's are named by their owner's), so the card knows which guest is you. */
  // What is coming up, for UUOB's widget and alerts on your devices, written
  // again whenever events, answers or the setting change (and every hour, as
  // the window moves on).
  $effect(() => {
    if (!alertsKnown || !eventsKnown) return;
    void hourTick;
    const list = upcomingFor(events, alertBefore, { skip: (o) => answerFor(o) === 'no', own: ownAlert, color: (o) => calOf(o)?.color ?? 'blue' });
    const t = setTimeout(() => uuob.upcoming(list).catch(() => {}), 1500);
    return () => clearTimeout(t);
  });

  // Your own alert for an event (kept like your answers, so it is yours alone
  // and works for any calendar): for a whole repeating event, or one changed
  // occurrence of it. undefined: the default; null: none.
  function ownAlert(e) {
    for (const k of [e.recurrence_of ? e.id : null, e.recurrence_of ?? e.series?.id ?? e.id]) if (k && ownAlerts.has(k)) return ownAlerts.get(k);
  }
  /** The choice as the pickers show it: 'default', 'none' or minutes. */
  const alertChoice = (e) => {
    const m = ownAlert(e);
    return m === undefined ? 'default' : m === null ? 'none' : String(m);
  };
  function setOwnAlert(key, choice) {
    const m = new Map(ownAlerts);
    if (choice === 'default') (m.delete(key), alertsC.delete(key).catch(() => {}));
    else {
      const before = choice === 'none' ? null : Number(choice);
      m.set(key, before);
      alertsC.put({ before: before ?? false }, key).catch(() => {});
    }
    ownAlerts = m;
  }
  const defaultAlert = (m) => (m == null ? 'none' : (BEFORE.find(([x]) => x === m)?.[1] ?? `${m} minutes before`).toLowerCase());

  function setAlertBefore(v) {
    alertBefore = v === 'none' ? null : Number(v);
    prefsC.put({ before: alertBefore }, 'alerts').catch(() => {});
  }

  async function forgetAddress(a) {
    addresses = addresses.filter((x) => x !== a);
    await prefsC.put({ emails: addresses }, 'me');
  }
  async function noteAddress(owner) {
    if (!owner || addresses.includes(owner)) return;
    addresses = [...addresses, owner];
    await prefsC.put({ emails: addresses }, 'me');
  }

  async function subscribe() {
    const s = subSheet;
    const url = calendarUrl(s.url);
    if (!url) return void (s.error = 'That doesn\'t look like a calendar link.');
    s.busy = true;
    s.error = '';
    try {
      const r = parseIcs(await fetchText(url));
      const name = s.name.trim() || r.name || nameFromUrl(url);
      const id = await newCalendar(name, { subscribed: true });
      await subsC.put({ url, refreshed: 0 }, id);
      await refresh({ id, url });
      subSheet = null;
    } catch (e) {
      s.busy = false;
      s.error = e?.message || 'The link could not be read.';
    }
  }

  // ".../CzechHolidays.ics" -> "CzechHolidays"; Google's ".../basic.ics" says nothing, so the site's name.
  function nameFromUrl(url) {
    const u = new URL(url);
    const file = decodeURIComponent(u.pathname.split('/').filter(Boolean).pop() ?? '').replace(/\.ics$/i, '');
    return file && !/^(basic|calendar|feed|ical|public|full)$/i.test(file) ? file : u.hostname.replace(/^www\./, '');
  }

  async function refreshNow(id) {
    const sub = subs.find((x) => x.id === id);
    if (!sub) return;
    note = 'Updating…';
    try {
      const r = await refresh(sub);
      note = `Updated: ${r.events.length} events.`;
    } catch (e) {
      note = e?.message || 'The link could not be read.';
    }
  }

  // Calendar links are read again when the app opens, at most every few
  // hours, once both the links and the calendars are known (UUOB may take
  // a moment to be found).
  let refreshedOnce = false;
  let subsKnown = false;
  async function refreshDue() {
    if (refreshedOnce || !subsKnown || !containers.length) return;
    refreshedOnce = true;
    for (const sub of subs) {
      if (Date.now() - (sub.refreshed ?? 0) > 3 * 3600 * 1000 && containers.some((c) => c.id === sub.id)) await refresh(sub).catch(() => {});
    }
  }

  async function exportCal(c) {
    const list = all.filter((e) => e.container === c.id).map((e) => ({ ...record(e), id: e.id }));
    const file = `${c.name.replace(/[\\/:*?"<>|]+/g, ' ').trim() || 'Calendar'}.ics`;
    try {
      if (await saveText(file, writeIcs(list, c.name))) note = `Saved ${list.length} events as ${file}.`;
    } catch (e) {
      note = e?.message || 'The file could not be saved.';
    }
  }

  const ago = (ms) => {
    if (!ms) return 'not yet';
    const m = Math.round((Date.now() - ms) / 60000);
    return m < 1 ? 'just now' : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} h ago` : `${Math.round(m / 1440)} days ago`;
  };

  function toggleCal(id) {
    const h = new Set(hidden);
    h.has(id) ? h.delete(id) : h.add(id);
    hidden = h;
    savePrefs();
  }
  // Which calendars are hidden and whether tasks show, kept with your
  // preferences (the whole record each time: a save replaces it).
  let prefsLoaded = false;
  function savePrefs() {
    if (prefsLoaded) prefsC.put({ show_tasks: showTasks, hidden: [...hidden] }, 'calendar');
  }

  // Showing tasks needs read access to the Tasks app's container.
  let stopTasks = null;
  async function toggleTasks(save = true) {
    if (showTasks) {
      showTasks = false;
      stopTasks?.();
    } else {
      try {
        if (!(await uuob.requestAccess('tasks', 'read'))) return;
        showTasks = true;
        stopTasks = uuob.collection('tasks', 'tasks').subscribe((r) => (tasks = r));
      } catch {
        return; /* no Tasks app */
      }
    }
    if (save) savePrefs();
  }

  async function scrollToMorning() {
    await tick();
    if (gridEl) gridEl.scrollTop = HOUR * 7.5;
  }

  onMount(() => {
    const mq = matchMedia('(max-width: 720px)');
    narrow = mq.matches;
    mq.onchange = () => (narrow = mq.matches);
    const a = firstCal.subscribe((r) => {
      const p = r.find((c) => c.id === 'personal');
      if (p) first = { name: p.name, color: p.color };
    });
    watchEvents();
    const c = uuob.all('calendar').subscribe((r) => (infos = r));
    const d = uuob.containers.subscribe((r) => {
      containers = r;
      refreshDue();
    });
    const e = subsC.subscribe((r) => {
      subs = r;
      subsKnown = true;
      refreshDue();
    });
    const f = answersC.subscribe((r) => (answers = new Map(r.map((x) => [x.id, x.answer]))));
    const h = alertsC.subscribe((r) => (ownAlerts = new Map(r.map((x) => [x.id, typeof x.before === 'number' ? x.before : null]))));
    const g = prefsC.subscribe((r) => {
      addresses = r.find((x) => x.id === 'me')?.emails ?? [];
      const a = r.find((x) => x.id === 'alerts');
      alertBefore = a ? (a.before ?? null) : DEFAULT_BEFORE;
      alertsKnown = true;
    });
    moveOldCalendars().catch(() => {});
    prefsC.get('calendar').then((p) => {
      if (p?.hidden) hidden = new Set(p.hidden);
      prefsLoaded = true;
      if (p?.show_tasks) toggleTasks(false);
    }, () => (prefsLoaded = true));
    const tickNow = () => (now = new Date());
    const timer = setInterval(tickNow, 30000);
    const hourly = setInterval(() => hourTick++, 3600000);
    // Timers pause while the Mac sleeps or the window is hidden; catch up on return.
    const wake = () => document.visibilityState === 'visible' && tickNow();
    document.addEventListener('visibilitychange', wake);
    window.addEventListener('focus', tickNow);
    scrollToMorning();
    return () => (a(), stopEvents?.(), c(), d(), e(), f(), g(), h(), clearInterval(timer), clearInterval(hourly), document.removeEventListener('visibilitychange', wake), window.removeEventListener('focus', tickNow), stopTasks?.());
  });
</script>

<svelte:window onkeydown={(ev) => ev.key === 'Escape' && (addMenu ? (addMenu = false) : viewing ? (viewing = null) : null)} />

<div class="app" class:cals-open={calsOpen}>
  {#if calsOpen}<div class="cals-scrim" onclick={() => (calsOpen = false)} role="presentation"></div>{/if}
  <aside class="sidebar">
    <div class="cals-head">
      <span class="section-label">Calendars</span>
      <button class="icon-btn add-cal" class:on={addMenu} onclick={() => (addMenu = !addMenu)} aria-label="Add a calendar" aria-expanded={addMenu}><Icon name="plus" size={18} stroke={2.2} /></button>
      {#if addMenu}
        <div class="menu-scrim" onclick={() => (addMenu = false)} role="presentation"></div>
        <div class="menu" role="menu">
          <button role="menuitem" onclick={() => ((addMenu = calsOpen = false), (calSheet = { name: '', color: COLORS[(cals.length + 1) % COLORS.length] }))}><Icon name="plus" size={17} stroke={2.2} /> New calendar</button>
          <button role="menuitem" onclick={() => ((addMenu = false), fileEl.click())}><Icon name="download" size={17} /> Import a file</button>
          <button role="menuitem" onclick={() => ((addMenu = calsOpen = false), (subSheet = { url: '', name: '', busy: false, error: '' }))}><Icon name="link" size={17} /> Subscribe to a link</button>
        </div>
      {/if}
    </div>
    {#each cals as c (c.id)}
      <div class="cal-row">
        <button class="check" class:on={!hidden.has(c.id)} style="--ring:var(--c-{c.color})" onclick={() => toggleCal(c.id)} aria-label="Show {c.name}"></button>
        <span class="name">{c.name}</span>
        {#if c.subscribed}<span class="shared-mark" title="Subscribed to a link"><Icon name="link" size={15} /></span>{/if}
        {#if c.from || c.sharedWith?.length}<span class="shared-mark" title={sharedNote(c)}><Icon name="people" size={15} /></span>{/if}
        <button class="icon-btn sm" title={c.from ? 'About this calendar' : 'Edit'} onclick={() => ((calsOpen = false), ((note = ''), (calSheet = { ...c })))}><Icon name="edit" size={15} /></button>
      </div>
    {/each}
    <input type="file" accept=".ics,text/calendar" multiple hidden bind:this={fileEl} onchange={readFiles} />

    <div class="section-label other-apps">From other apps</div>
    <div class="cal-row">
      <button class="check" class:on={showTasks} style="--ring:var(--text-2)" onclick={() => toggleTasks()} aria-label="Show tasks"></button>
      <span class="name wraps">Tasks with dates</span>
      {#if !showTasks}<Icon name="lock" size={14} />{/if}
    </div>
    {#if !showTasks}<p class="caption">Asks to read your Tasks data.</p>{/if}

    <button class="btn ghost settings-btn" onclick={() => ((calsOpen = false), (settingsOpen = true))}><Icon name="settings" size={19} /> Settings</button>
  </aside>

  <section class="main">
    <header>
      <h1 class="large-title"><b>{title.month}</b> <span class="year">{title.year}</span></h1>
      <span class="spacer"></span>
      <button class="icon-btn only-narrow" onclick={() => (calsOpen = true)} aria-label="Calendars"><Icon name="menu" /></button>
      <div class="segmented">
        <button class:on={mode === 'week'} onclick={() => ((mode = 'week'), scrollToMorning())}>{narrow ? 'Day' : 'Week'}</button>
        <button class:on={mode === 'month'} onclick={() => (mode = 'month')}>Month</button>
      </div>
      <div class="nav">
        <button class="icon-btn" onclick={() => move(-1)} aria-label="Previous"><Icon name="left" /></button>
        <button class="btn sm" onclick={() => ((cursor = t), scrollToMorning())}>Today</button>
        <button class="icon-btn" onclick={() => move(1)} aria-label="Next"><Icon name="right" /></button>
      </div>
      <button class="btn primary new" onclick={() => newEvent(mode === 'month' ? t : days.includes(t) ? t : days[0])} aria-label="New event" title="New event"><Icon name="plus" size={20} stroke={2.2} /></button>
    </header>

    {#if mode === 'week'}
      <div class="week" style="--cols:{days.length};--sb:{scrollbar}px">
        <div class="heads">
          <div class="gutter"></div>
          {#each days as d}
            <button class="head" class:today={d === t} onclick={() => ((cursor = d), narrow || (mode = 'week'))}>
              <span class="wd">{parse(d).toLocaleDateString(undefined, { weekday: 'short' })}</span>
              <span class="dn">{parse(d).getDate()}</span>
            </button>
          {/each}
        </div>
        <div class="allday">
          <div class="gutter caption">all-day</div>
          {#each days as d}
            <div class="ad-cell">
              {#each onDay(d).allDay as e (e.key)}
                {@const a = answerFor(e)}
                <button class="pill {a ?? ''}" class:cont={e.continues} class:on={e.goesOn} style="--c:{colorOf(e)}" title={e.title} onclick={() => open(e)}>
                  {#if e.series}<svg class="rep" viewBox="0 0 24 24" aria-label="Repeats"><path d="M17 2l4 4-4 4M3 11V9a3 3 0 013-3h15M7 22l-4-4 4-4M21 13v2a3 3 0 01-3 3H3" /></svg>{/if}{e.continues && d !== days[0] ? '' : e.title}
                </button>
              {/each}
              {#each tasksOn(d).filter((x) => !x.time) as x (x.id)}
                <div class="pill task"><span class="tick"></span>{x.title}</div>
              {/each}
            </div>
          {/each}
        </div>
        <div class="grid scroll" bind:this={gridEl}>
          <div class="hours">
            {#each Array(24) as _, h}<div class="hour"><span>{h === 0 ? '' : friendlyTime(`${pad(h)}:00`)}</span></div>{/each}
          </div>
          {#each days as d}
            <div class="col" class:today={d === t} style="height:{HOUR * 24}px" use:sizing onpointerdown={(ev) => down(ev, d)} onpointermove={moveDrag} onpointerup={up} onpointercancel={cancelDrag} oncontextmenu={(ev) => (hold || drag) && ev.preventDefault()} role="presentation">
              {#each Array(24) as _, h}<div class="line" style="top:{h * HOUR}px"></div>{/each}
              <!-- Each event shows as much as fits: its title wraps over the lines it needs, the time
                   goes below when there's a line left for it, then the place if there's room. -->
              {#each layout(onDay(d).timed) as { e, lane, lanes } (e.key)}
                {@const top = (mins(e.start) / 60) * HOUR}
                {@const h = Math.max(22, ((mins(e.end) - mins(e.start)) / 60) * HOUR - 2)}
                {@const a = answerFor(e)}
                {@const where = h >= 76 ? place(e) : ''}
                {@const room = Math.max(1, Math.floor((h - 5 - (where ? 15 : 0)) / 15))}
                {@const timeLine = h >= 38 && titleLines(e.title, lanes) < room}
                <button class="event {a ?? ''}" class:short={h < 38} style="--c:{colorOf(e)};top:{top}px;height:{h}px;left:calc({(lane / lanes) * 100}% + 2px);width:calc({100 / lanes}% - 4px)" onclick={() => open(e)}>
                  <span class="et" style="-webkit-line-clamp:{timeLine ? room - 1 : room}">{e.title}{#if h < 38} <span class="tm">{friendlyTime(e.start)}</span>{/if}</span>
                  {#if timeLine}<span class="em">{blockTime(e)}</span>{/if}
                  {#if where}<span class="em">{where}</span>{/if}
                </button>
              {/each}
              {#each tasksOn(d).filter((x) => x.time) as x (x.id)}
                <div class="task-mark" style="top:{(mins(x.time) / 60) * HOUR}px"><span class="tick"></span>{x.title}</div>
              {/each}
              {#if drag?.date === d}
                <div class="event ghost" style="top:{(drag.from / 60) * HOUR}px;height:{((drag.to - drag.from) / 60) * HOUR}px;left:2px;width:calc(100% - 4px)">
                  <span class="em">{friendlyTime(hhmm(drag.from))} – {friendlyTime(hhmm(Math.min(drag.to, 1439)))}</span>
                </div>
              {/if}
              {#if d === t}
                <div class="now" style="top:{((now.getHours() * 60 + now.getMinutes()) / 60) * HOUR}px"></div>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="month">
        {#each monthDays.slice(0, 7) as d}<div class="mh">{parse(d).toLocaleDateString(undefined, { weekday: narrow ? 'narrow' : 'short' })}</div>{/each}
        {#each monthDays as d}
          {@const list = [...onDay(d).allDay, ...[...onDay(d).timed].sort((a, b) => mins(a.start) - mins(b.start))]}
          <div class="cell" class:other={parse(d).getMonth() !== parse(cursor).getMonth()} onclick={() => newEvent(d)} role="presentation">
            <button class="num" class:today={d === t} onclick={(ev) => (ev.stopPropagation(), (cursor = d), (mode = 'week'), scrollToMorning())}>{parse(d).getDate()}</button>
            {#each list.slice(0, 3) as e (e.key)}
              {@const a = answerFor(e)}
              <button class="mev {a ?? ''}" class:solid={e.row} style="--c:{colorOf(e)}" onclick={(ev) => (ev.stopPropagation(), open(e))}>
                {#if !e.row}<span class="dot" style="background:var(--c)"></span>{/if}
                <span class="mt">{#if !e.row && !e.continues}<span class="mtime">{friendlyTime(e.start)}</span> {/if}{e.title}</span>
              </button>
            {/each}
            {#if list.length > 3}<span class="more caption">+{list.length - 3} more</span>{/if}
            {#each tasksOn(d).slice(0, 2) as x (x.id)}<div class="mev task"><span class="tick"></span><span class="mt">{x.title}</span></div>{/each}
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

{#if editor}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (editor = null)} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), saveEvent())}>
      <input class="field title-field" placeholder="New event" bind:value={editor.title} autofocus />
      <div class="form">
        <label class="toggle"><input type="checkbox" bind:checked={editor.all_day} /> <span>All-day</span></label>
        <div class="two" class:one={editor.all_day}>
          <label class="stack">Starts<input class="field" type="date" value={editor.date} onchange={(e) => moveStart(e.currentTarget.value)} required /></label>
          {#if !editor.all_day}<label class="stack"><span class="sr">Start time</span><TimeField bind:value={editor.start} aria-label="Start time" /></label>{/if}
        </div>
        <div class="two" class:one={editor.all_day}>
          <label class="stack">Ends<input class="field" type="date" bind:value={editor.end_date} min={editor.date} required /></label>
          {#if !editor.all_day}<label class="stack"><span class="sr">End time</span><TimeField bind:value={editor.end} aria-label="End time" /></label>{/if}
        </div>
        <label class="stack">Repeat
          <select class="field" bind:value={editor.repeat.freq}>
            <option value="NONE">Never</option>
            <option value="DAILY">Every day</option>
            <option value="WEEKLY">Every week</option>
            <option value="MONTHLY">Every month</option>
            <option value="YEARLY">Every year</option>
            {#if editor.repeat.raw}<option value="CUSTOM">{editor.repeat.label}</option>{/if}
          </select>
        </label>
        {#if editor.repeat.freq === 'WEEKLY'}
          <div class="weekdays" role="group" aria-label="On these days">
            {#each WEEK as [code, letter]}
              <button type="button" class="wday" class:on={editor.repeat.days.includes(code)} onclick={() => toggleDay(code)} aria-label={code}>{letter}</button>
            {/each}
          </div>
        {/if}
        {#if editor.repeat.freq !== 'NONE' && editor.repeat.freq !== 'CUSTOM'}
          <div class="two">
            <label class="stack">Every
              <span class="with-unit"><input class="field" type="number" min="1" max="99" bind:value={editor.repeat.interval} /><span>{unit(editor.repeat.freq, editor.repeat.interval)}</span></span>
            </label>
            <label class="stack">Until
              <select class="field" bind:value={editor.repeat.ends}>
                <option value="never">Forever</option>
                <option value="until">A date</option>
                <option value="count">A number of times</option>
              </select>
            </label>
          </div>
          {#if editor.repeat.ends === 'until'}
            <input class="field" type="date" bind:value={editor.repeat.until} min={editor.date} aria-label="Last date" />
          {:else if editor.repeat.ends === 'count'}
            <span class="with-unit"><input class="field" type="number" min="1" max="999" bind:value={editor.repeat.count} aria-label="Times" /><span>times</span></span>
          {/if}
        {/if}
        {#if !editor.all_day}
          <label class="stack">Alert
            <select class="field" bind:value={editor.alert}>{@render alertOptions(editor.alert)}</select>
          </label>
        {/if}
        <label class="stack">Calendar
          <select class="field" bind:value={editor.calendar}>
            {#each cals.filter((c) => canEdit(c.id) || c.id === editor.calendar) as c}<option value={c.id}>{c.name}</option>{/each}
          </select>
        </label>
        <input class="field" placeholder="Location" bind:value={editor.location} />
        <textarea class="field" placeholder="Notes" bind:value={editor.notes}></textarea>
      </div>
      <div class="actions">
        {#if editor.id}
          <button type="button" class="btn lg danger" onclick={() => deleteEvent()}>Delete</button>
        {:else}
          <button type="button" class="btn lg" onclick={() => (editor = null)}>Cancel</button>
        {/if}
        <button class="btn lg primary">{editor.id ? 'Save' : 'Add'}</button>
      </div>
    </form>
  </div>
{/if}

{#snippet alertOptions(current)}
  <option value="default">Default ({defaultAlert(alertBefore)})</option>
  {#each BEFORE as [m, label]}<option value={m === null ? 'none' : String(m)}>{m === null ? 'No alert' : label}</option>{/each}
  {#if !['default', 'none', ...BEFORE.map(([m]) => String(m))].includes(current)}<option value={current}>{current} minutes before</option>{/if}
{/snippet}

<Settings bind:open={settingsOpen} name="Calendar" week>
  <div class="group-label">Alerts</div>
  <div class="group">
    <div class="srow">
      <span class="label">Default alert</span>
      <select class="field alert-pick" aria-label="Default alert before events" value={alertBefore ?? 'none'} onchange={(e) => setAlertBefore(e.currentTarget.value)}>
        {#each BEFORE as [m, label]}<option value={m ?? 'none'}>{label}</option>{/each}
      </select>
    </div>
  </div>
  <p class="foot">For events with a time; each event can have its own. UUOB rings them on your phones and computers.</p>

  <div class="group-label">Invitations</div>
  <div class="group">
    {#each addresses as a (a)}
      <div class="srow addr">
        <span class="label">{a}</span>
        <button class="icon-btn" onclick={() => forgetAddress(a)} aria-label="Remove {a}" title="Remove"><Icon name="close" size={17} /></button>
      </div>
    {:else}
      <div class="srow"><span class="value">No address yet</span></div>
    {/each}
  </div>
  <p class="foot">Your addresses tell which guest is you. They're learned when you import or subscribe to your own Google calendar.</p>
</Settings>

{#if viewing}
  {@const e = viewing}
  {@const c = calOf(e)}
  {@const call = meeting(e)}
  {@const n = notesOf(e.notes)}
  {@const people = peopleOf(e, mine, chosen(e))}
  {@const mineAnswer = answerFor(e)}
  <div class="scrim" onclick={(ev) => ev.target === ev.currentTarget && (viewing = null)} role="presentation">
    <div class="sheet card" role="dialog" aria-label={e.title} style="--c:{colorOf(e)}">
      <div class="card-head">
        <h2 class="title card-title">{e.title}</h2>
        {#if canEdit(e.calendar)}<button class="icon-btn" onclick={() => edit(e)} aria-label="Edit" title="Edit"><Icon name="edit" size={19} /></button>{/if}
        <button class="icon-btn" onclick={() => (viewing = null)} aria-label="Close" title="Close"><Icon name="close" size={19} /></button>
      </div>
      <p class="when">{whenText(e)}</p>
      {#if e.series}<p class="caption">{describe(e.series.rrule)}</p>{/if}
      <p class="caption cal-line"><span class="dot" style="background:var(--c)"></span>{c?.name}{c?.subscribed ? ' · from a link' : c?.from ? ` · ${sharedNote(c)}` : ''}</p>

      {#if call}
        <button class="join" onclick={() => visit(call.url)}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM16 10l5-3v10l-5-3" /></svg>
          <span>Join {call.name}</span><span class="host">{call.host}</span>
        </button>
      {/if}
      {#if linkError}<p class="caption error">{linkError}</p>{/if}
      {#if place(e)}
        <div class="row"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 21s-7-6.2-7-11.5a7 7 0 0 1 14 0C19 14.8 12 21 12 21zM12 12a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z" /></svg><span class="wraps">{place(e)}</span></div>
      {/if}
      {#if e.url && e.url !== call?.url}
        <button class="row link" onclick={() => visit(e.url)}><Icon name="link" size={18} /><span class="wraps">{e.url.replace(/^https?:\/\/(www\.)?/, '')}</span></button>
      {/if}
      {#if !e.all_day}
        <label class="row">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15zM10 20.5a2 2 0 0 0 4 0" /></svg>
          <select class="row-pick" aria-label="Alert" value={alertChoice(e)} onchange={(ev) => setOwnAlert(answerKey(e), ev.currentTarget.value)}>{@render alertOptions(alertChoice(e))}</select>
        </label>
      {/if}
      {#if people.length}
        <button class="row" onclick={() => (peopleOpen = !peopleOpen)} aria-expanded={peopleOpen}>
          <Icon name="people" size={18} /><span class="wraps">{peopleSummary(people)}</span>
          <span class="chev" class:open={peopleOpen}><Icon name="chevron" size={16} /></span>
        </button>
        {#if peopleOpen}
          <ul class="people">
            {#each people as p}
              <li>
                <span class="st {p.answer ?? 'none'}" title={{ yes: 'Going', maybe: 'Maybe', no: 'Not going' }[p.answer] ?? 'No answer yet'}>{{ yes: '✓', maybe: '?', no: '✕' }[p.answer] ?? ''}</span>
                <span class="wraps">{p.name || p.email}</span>
                {#if p.organizer}<span class="caption">organizer</span>{/if}
                {#if p.you}<span class="caption">you</span>{/if}
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
      {#if n.text}
        <p class="notes">{#each linkify(n.text) as part}{#if part.href}<a href={part.href} onclick={(ev) => (ev.preventDefault(), visit(part.href))}>{part.text}</a>{:else}{part.text}{/if}{/each}</p>
      {/if}
      {#if n.joining}
        <button class="row small" onclick={() => (joiningOpen = !joiningOpen)} aria-expanded={joiningOpen}>
          <span>Joining details</span><span class="chev" class:open={joiningOpen}><Icon name="chevron" size={15} /></span>
        </button>
        {#if joiningOpen}
          <p class="notes caption">{#each linkify(n.joining) as part}{#if part.href}<a href={part.href} onclick={(ev) => (ev.preventDefault(), visit(part.href))}>{part.text}</a>{:else}{part.text}{/if}{/each}</p>
        {/if}
      {/if}
      {#if asksAnswer(e, mine)}
        <div class="segmented answer" role="group" aria-label="Your answer">
          {#each [['yes', 'Going'], ['maybe', 'Maybe'], ['no', 'No']] as [v, label]}
            <button class:on={mineAnswer === v} aria-pressed={mineAnswer === v} onclick={() => setAnswer(e, v)}>{label}</button>
          {/each}
        </div>
        <p class="caption answer-note">Your answer stays on your devices.</p>
      {/if}
    </div>
  </div>
{/if}

{#if scopeAsk && editor}
  <div class="scrim top" onclick={(e) => e.target === e.currentTarget && (scopeAsk = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">{scopeAsk === 'save' ? 'Change' : 'Delete'} a repeating event</h2>
      <p class="secondary">{scopeAsk === 'save' ? 'Change' : 'Delete'} only this one, on {parse(editor.occurrence).toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long' })}, or every time it repeats?</p>
      <div class="actions">
        <button type="button" class="btn lg" class:danger={scopeAsk === 'delete'} onclick={() => (scopeAsk === 'save' ? saveEvent('this') : deleteEvent('this'))}>Only this one</button>
        <button type="button" class="btn lg" class:primary={scopeAsk === 'save'} class:danger={scopeAsk === 'delete'} onclick={() => (scopeAsk === 'save' ? saveEvent('all') : deleteEvent('all'))}>All of them</button>
      </div>
      <button type="button" class="btn ghost wide back" onclick={() => (scopeAsk = null)}>Cancel</button>
    </div>
  </div>
{/if}

{#if calSheet?.from}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (calSheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">{calSheet.name}</h2>
      <p class="secondary">
        {calSheet.from.name} shares this calendar with you. {calSheet.canWrite ? 'You can both change it.' : 'You can view it, but not change it.'}
      </p>
      <button type="button" class="btn share-row" onclick={() => exportCal(calSheet)}><Icon name="share" size={18} /><span>Save as .ics file</span></button>
      {#if note}<p class="caption private">{note}</p>{/if}
      <div class="actions">
        <button type="button" class="btn lg danger" onclick={deleteCal}>Leave calendar</button>
        <button type="button" class="btn lg primary" onclick={() => (calSheet = null)}>Done</button>
      </div>
    </div>
  </div>
{:else if calSheet}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (calSheet = null)} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), saveCal())}>
      <h2 class="title">{calSheet.id ? 'Edit calendar' : 'New calendar'}</h2>
      <input class="field cal-name" placeholder="Name, e.g. Work" bind:value={calSheet.name} autofocus />
      <div class="swatches">
        {#each COLORS as c}
          <button type="button" class="swatch" class:on={calSheet.color === c} style="background:var(--c-{c})" onclick={() => (calSheet.color = c)} aria-label={c}></button>
        {/each}
      </div>
      {#if calSheet.subscribed}
        <p class="caption private">Follows a calendar link{subs.find((x) => x.id === calSheet.id) ? `, updated ${ago(subs.find((x) => x.id === calSheet.id).refreshed)}` : ''}. Its events change only at the link.</p>
        {#if subs.some((x) => x.id === calSheet.id)}
          <button type="button" class="btn share-row" onclick={() => refreshNow(calSheet.id)}><Icon name="download" size={18} /><span>Update now</span></button>
        {/if}
      {/if}
      {#if calSheet.id}
        <button type="button" class="btn share-row" onclick={() => exportCal(calSheet)}><Icon name="share" size={18} /><span>Save as .ics file</span></button>
      {/if}
      {#if note}<p class="caption private">{note}</p>{/if}
      {#if calSheet.main}
        <p class="caption private">Your first calendar stays private. To share events with someone, add a calendar and share that.</p>
      {:else if calSheet.id}
        <button type="button" class="btn share-row" onclick={() => share(calSheet)}>
          <Icon name="share" size={18} />
          <span>{calSheet.sharedWith?.length ? sharedNote(calSheet) : 'Share calendar…'}</span>
        </button>
      {/if}
      <div class="actions">
        {#if calSheet.id && !calSheet.main}
          <button type="button" class="btn lg danger" onclick={deleteCal}>Delete</button>
        {:else}
          <button type="button" class="btn lg" onclick={() => (calSheet = null)}>Cancel</button>
        {/if}
        <button class="btn lg primary" disabled={!calSheet.name.trim()}>Save</button>
      </div>
    </form>
  </div>
{/if}

{#if importSheet}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && !importSheet.busy && (importSheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Import</h2>
      {#if importSheet.result}
        {@const r = importSheet.result}
        <p class="secondary">Imported {r.events} {r.events === 1 ? 'event' : 'events'}.</p>
        {#if r.error}<p class="caption error">{r.error} Importing the file again picks up where it stopped.</p>{/if}
        {#if r.unreadable}<p class="caption">{r.unreadable} {r.unreadable === 1 ? 'event repeats' : 'events repeat'} in a way this calendar can't show yet; they appear only the first time.</p>{/if}
        {#if r.zones.length}<p class="caption">Times in {r.zones.join(', ')} were read in the calendar's own time zone.</p>{/if}
        <p class="caption">Importing the same file again updates these events instead of adding them twice.</p>
        <button type="button" class="btn lg primary wide back" onclick={() => (importSheet = null)}>Done</button>
      {:else}
        {#each importSheet.items as item}
          <div class="import-row">
            <div class="import-name"><b>{item.name}</b>
              {#if item.error}<span class="caption">{item.error}</span>
              {:else}<span class="caption">{item.r.events.filter((e) => !e.recurrence_of).length} events{item.r.events.some((e) => e.rrule) ? `, ${item.r.events.filter((e) => e.rrule).length} repeating` : ''}</span>{/if}
            </div>
            {#if item.r?.events.length}
              <select class="field" bind:value={item.target} disabled={importSheet.busy} aria-label="Import into">
                <option value="new">New calendar “{item.name}”</option>
                {#each cals.filter((c) => canEdit(c.id)) as c}<option value={c.id}>Into {c.name}</option>{/each}
              </select>
            {/if}
          </div>
        {/each}
        {#if importSheet.busy}
          <progress class="wide" max={importSheet.total} value={importSheet.done}></progress>
          <p class="caption">{importSheet.done} of {importSheet.total}…</p>
        {/if}
        <div class="actions">
          <button type="button" class="btn lg" disabled={importSheet.busy} onclick={() => (importSheet = null)}>Cancel</button>
          <button type="button" class="btn lg primary" disabled={importSheet.busy || !importSheet.total} onclick={runImport}>Import</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if subSheet}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && !subSheet.busy && (subSheet = null)} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), subscribe())}>
      <h2 class="title">Subscribe to a calendar</h2>
      <p class="secondary">Paste a calendar link (webcal:// or one ending in .ics), such as Google Calendar's “Secret address in iCal format”, a holiday or a team schedule. It stays up to date and can't be changed here.</p>
      <input class="field cal-name" placeholder="webcal://… or https://….ics" bind:value={subSheet.url} autofocus inputmode="url" autocomplete="off" autocapitalize="off" spellcheck="false" />
      <input class="field cal-name" placeholder="Name (optional)" bind:value={subSheet.name} />
      {#if subSheet.error}<p class="caption error">{subSheet.error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn lg" disabled={subSheet.busy} onclick={() => (subSheet = null)}>Cancel</button>
        <button class="btn lg primary" disabled={subSheet.busy || !subSheet.url.trim()}>{subSheet.busy ? 'Reading…' : 'Subscribe'}</button>
      </div>
    </form>
  </div>
{/if}

<style>
  .import-row { display: grid; gap: 8px; padding: 12px 0; border-bottom: 1px solid var(--line); }
  .import-name { display: flex; flex-direction: column; gap: 2px; }
  progress { margin-top: 16px; accent-color: var(--accent); }
  .error { color: var(--c-red); margin: 10px 0 0; }
  .shared-mark { color: var(--text-3); display: grid; }
  .share-row { width: 100%; height: 46px; justify-content: flex-start; gap: 10px; margin-top: 18px; color: var(--accent); }
  .share-row span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .private { margin: 16px 0 0; }
  .wide { width: 100%; }

  /* The sidebar's + and its menu. */
  .cals-head { position: relative; display: flex; align-items: center; justify-content: space-between; margin: 0 0 4px; padding-left: 8px; }
  .cals-head .section-label { margin: 0; }
  .add-cal { width: 30px; height: 30px; color: var(--accent); }
  .add-cal.on { background: var(--active); }
  .menu-scrim { position: fixed; inset: 0; z-index: 70; }
  .menu {
    position: absolute; right: 0; top: 34px; z-index: 71; min-width: 210px; padding: 6px; display: grid;
    background: var(--surface-2); border: 1px solid var(--line-strong); border-radius: 14px; box-shadow: var(--shadow-lg);
    animation: pop 0.2s var(--spring);
  }
  .menu button {
    display: flex; align-items: center; gap: 10px; height: 40px; padding: 0 12px; border: 0; border-radius: 9px;
    background: none; color: var(--text); font-size: 15px; font-weight: 500; text-align: left; cursor: pointer;
  }
  .menu button:hover { background: var(--hover); }
  .menu :global(svg) { color: var(--accent); }

  /* Invitations: not answered yet is an outline, maybe is striped, no is faded. */
  .event.invited, .pill.invited { background: var(--bg); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 70%, transparent); }
  .event.maybe, .pill.maybe {
    background: repeating-linear-gradient(135deg, color-mix(in srgb, var(--c) 26%, var(--bg)) 0 6px, color-mix(in srgb, var(--c) 12%, var(--bg)) 6px 12px);
  }
  .event.no, .pill.no, .mev.no { opacity: 0.45; }
  .event.no .et, .pill.no, .mev.no .mt { text-decoration: line-through; }
  .mev.invited .dot { background: transparent !important; box-shadow: inset 0 0 0 1.5px var(--c); }
  .mtime { color: var(--text-2); font-weight: 500; font-variant-numeric: tabular-nums; margin-right: 4px; }

  /* The event card. */
  .card { padding: 22px 22px 24px; }
  .card-head { display: flex; align-items: flex-start; gap: 4px; }
  .card-title { flex: 1; min-width: 0; overflow-wrap: anywhere; padding: 4px 6px 0 0; }
  .card-head .icon-btn { flex: none; }
  .when { margin: 10px 0 2px; font-size: 16px; font-weight: 500; }
  .card .caption { margin: 2px 0 0; }
  .cal-line { display: flex; align-items: center; gap: 7px; }
  .join {
    display: flex; align-items: center; gap: 10px; width: 100%; margin-top: 18px; padding: 0 16px; height: 50px; border: 0; border-radius: 14px;
    background: var(--accent); color: var(--on-accent, #fff); font-size: 16px; font-weight: 600; cursor: pointer; transition: filter 0.15s;
  }
  .join:hover { filter: brightness(1.08); }
  .join svg, .row svg { width: 20px; height: 20px; flex: none; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .join .host { margin-left: auto; font-size: 13px; font-weight: 500; opacity: 0.8; }
  .row {
    display: flex; align-items: center; gap: 12px; width: 100%; min-height: 46px; padding: 10px 2px; margin: 0;
    border: 0; border-bottom: 1px solid var(--line); background: none; color: var(--text); font-size: 15px; text-align: left;
  }
  button.row { cursor: pointer; }
  .card > .row:last-child { border-bottom: 0; }
  .row > :global(svg), .row > svg { color: var(--text-2); }
  .row.link { color: var(--accent); }
  .row-pick { flex: 1; min-width: 0; padding: 0; border: 0; background: none; color: var(--text); font: inherit; cursor: pointer; }
  .row.small { min-height: 38px; font-size: 14px; color: var(--text-2); border: 0; }
  .wraps { flex: 1; min-width: 0; overflow-wrap: anywhere; }
  .chev { display: grid; color: var(--text-3); transition: transform 0.18s var(--ease); }
  .chev.open { transform: rotate(90deg); }
  .people { list-style: none; margin: 4px 0 6px; padding: 0 0 8px 32px; border-bottom: 1px solid var(--line); display: grid; gap: 8px; font-size: 14px; }
  .people li { display: flex; align-items: center; gap: 10px; }
  .st { width: 18px; height: 18px; flex: none; border-radius: 50%; display: grid; place-items: center; font-size: 11px; font-weight: 700; box-shadow: inset 0 0 0 1.5px var(--text-3); color: var(--text-3); }
  .st.yes { box-shadow: none; background: color-mix(in srgb, var(--c-green) 22%, transparent); color: var(--c-green); }
  .st.maybe { box-shadow: none; background: color-mix(in srgb, var(--c-orange) 22%, transparent); color: var(--c-orange); }
  .st.no { box-shadow: none; background: color-mix(in srgb, var(--c-red) 20%, transparent); color: var(--c-red); }
  .notes { margin: 14px 0 0; font-size: 15px; line-height: 1.5; white-space: pre-wrap; overflow-wrap: anywhere; color: var(--text); }
  .notes.caption { margin: 2px 0 0; font-size: 13px; color: var(--text-2); }
  .notes a { color: var(--accent); text-decoration: none; }
  .notes a:hover { text-decoration: underline; }
  .answer { display: flex; width: 100%; margin-top: 20px; }
  .answer button { flex: 1; padding: 9px 0; font-size: 15px; }
  .answer-note { text-align: center; margin-top: 8px !important; }
  .scrim.top { z-index: 120; }
  .back { margin-top: 8px; }
  .sr { visibility: hidden; }
  .two.one { grid-template-columns: 1fr; }
  .weekdays { display: flex; justify-content: space-between; gap: 6px; }
  .wday { width: 38px; height: 38px; border-radius: 50%; border: 0; background: var(--surface-2); color: var(--text-2); font-weight: 650; cursor: pointer; transition: background 0.15s; }
  .wday:hover { background: var(--active); }
  .wday.on { background: var(--accent); color: #fff; }
  .with-unit { display: flex; align-items: center; gap: 10px; }
  .with-unit input { width: 90px; flex: none; }
  .with-unit span { color: var(--text-2); }
  .pill { display: flex; align-items: center; gap: 4px; min-height: 21px; }
  .rep { width: 11px; height: 11px; flex: none; fill: none; stroke: currentColor; stroke-width: 2.4; stroke-linecap: round; stroke-linejoin: round; }
  .pill.cont { border-top-left-radius: 0; border-bottom-left-radius: 0; margin-left: -3px; }
  .pill.on { border-top-right-radius: 0; border-bottom-right-radius: 0; margin-right: -3px; }
  .app { display: flex; height: 100vh; }
  .sidebar { width: 240px; flex: none; padding: 28px 16px 18px; border-right: 1px solid var(--line); background: var(--bg-elevated); display: flex; flex-direction: column; overflow-y: auto; }
  .settings-btn { margin-top: auto; align-self: flex-start; padding: 0 10px; color: var(--text-2); }
  .settings-btn:hover { color: var(--text); background: var(--hover); }
  .cals-open .settings-btn { margin-top: 20px; }
  .alert-pick { width: auto; max-width: 100%; }
  .addr { display: flex; align-items: center; gap: 8px; }
  .addr .label { flex: 1; font-size: 15px !important; }
  .cal-row { display: flex; align-items: center; gap: 12px; padding: 7px 8px; border-radius: 10px; color: var(--text-3); }
  .cal-row:hover { background: var(--hover); }
  .cal-row .name { flex: 1; min-width: 0; color: var(--text); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cal-row .name.wraps { white-space: normal; }
  .icon-btn.sm { width: 28px; height: 28px; opacity: 0; }
  .cal-row:hover .icon-btn.sm { opacity: 1; }
  .other-apps { margin-top: 28px; }
  .sidebar .caption { padding: 0 8px; margin: 4px 0 0; }

  .main { flex: 1; display: flex; flex-direction: column; min-width: 0; padding: 26px 28px 0; }
  header { display: flex; align-items: center; gap: 14px; margin-bottom: 18px; flex-wrap: wrap; }
  .large-title b { font-weight: 700; }
  .year { font-weight: 400; color: var(--text-2); }
  .nav { display: flex; align-items: center; gap: 2px; }
  .btn.sm { height: 32px; padding: 0 14px; font-size: 14px; }
  .new { width: 36px; height: 36px; padding: 0; }

  .week { flex: 1; display: flex; flex-direction: column; min-height: 0; }
  /* Pad by the grid's scrollbar width (--sb) so these columns line up with the grid below. */
  .heads, .allday { display: grid; grid-template-columns: 56px repeat(var(--cols), 1fr); padding-right: var(--sb, 0px); }
  .head { display: flex; align-items: baseline; justify-content: center; gap: 6px; padding: 6px 0 10px; border: 0; background: none; cursor: pointer; }
  .wd { font-size: 13px; color: var(--text-2); font-weight: 500; text-transform: uppercase; letter-spacing: 0.03em; }
  .dn { font: 600 20px/1 var(--font-display); width: 34px; height: 34px; display: inline-grid; place-items: center; border-radius: 50%; }
  .head.today .dn { background: var(--c-red); color: #fff; }
  .head.today .wd { color: var(--c-red); }
  .allday { border-bottom: 1px solid var(--line-strong); }
  .gutter { font-size: 11px; text-align: right; padding: 6px 8px 0 0; color: var(--text-3); }
  .ad-cell { display: grid; gap: 2px; padding: 3px 2px; border-left: 1px solid var(--line); min-height: 30px; align-content: start; }
  .pill {
    font-size: 12px; font-weight: 600; text-align: left; border: 0; border-radius: 6px; padding: 3px 8px; cursor: pointer;
    background: color-mix(in srgb, var(--c) 22%, transparent); color: color-mix(in srgb, var(--c), var(--text) 30%);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .pill.task, .mev.task, .task-mark { background: var(--surface-2); color: var(--text-2); display: flex; align-items: center; gap: 6px; cursor: default; }
  .tick { width: 10px; height: 10px; border-radius: 50%; border: 1.5px solid var(--text-3); flex: none; }

  .grid { flex: 1; display: grid; grid-template-columns: 56px repeat(var(--cols), 1fr); min-height: 0; position: relative; }
  .hours { position: relative; }
  .hour { height: 52px; position: relative; }
  .hour span { position: absolute; top: -7px; right: 8px; font-size: 11px; color: var(--text-3); white-space: nowrap; }
  .col { position: relative; border-left: 1px solid var(--line); touch-action: pan-y; user-select: none; -webkit-user-select: none; cursor: cell; }
  .col.today { background: color-mix(in srgb, var(--accent) 3%, transparent); }
  .line { position: absolute; left: 0; right: 0; border-top: 1px solid var(--line); pointer-events: none; }
  .event {
    position: absolute; border: 0; border-radius: 7px; padding: 3px 8px; text-align: left; overflow: hidden; cursor: pointer;
    background: color-mix(in srgb, var(--c) 24%, var(--bg)); border-left: 3px solid var(--c);
    color: color-mix(in srgb, var(--c), var(--text) 45%); display: flex; flex-direction: column; gap: 1px;
    transition: filter 0.15s, transform 0.12s var(--ease); z-index: 1;
  }
  .event:hover { filter: brightness(1.12); }
  .event:active { transform: scale(0.98); }
  .event.short { justify-content: center; padding: 2px 8px; }
  .event.ghost { --c: var(--accent); opacity: 0.85; pointer-events: none; border-style: dashed; }
  /* The title wraps (long course codes break too) over the lines the block has room for. */
  .et {
    font-size: 13px; font-weight: 650; line-height: 15px; flex: none; overflow: hidden; overflow-wrap: anywhere;
    display: -webkit-box; -webkit-box-orient: vertical;
  }
  .tm { font-size: 12px; font-weight: 500; opacity: 0.85; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .em { font-size: 12px; line-height: 15px; opacity: 0.85; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: none; font-variant-numeric: tabular-nums; }
  .task-mark { position: absolute; left: 2px; right: 2px; font-size: 12px; padding: 2px 6px; border-radius: 6px; z-index: 2; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .now { position: absolute; left: -5px; right: 0; height: 2px; background: var(--c-red); z-index: 3; pointer-events: none; }
  .now::before { content: ''; position: absolute; left: 0; top: -4px; width: 10px; height: 10px; border-radius: 50%; background: var(--c-red); }

  .month { flex: 1; display: grid; grid-template-columns: repeat(7, 1fr); grid-template-rows: auto repeat(6, 1fr); min-height: 0; padding-bottom: 20px; }
  .mh { font-size: 13px; color: var(--text-2); font-weight: 600; text-align: right; padding: 0 10px 8px; text-transform: uppercase; letter-spacing: 0.03em; }
  .cell { border-top: 1px solid var(--line); border-left: 1px solid var(--line); padding: 4px; display: flex; flex-direction: column; gap: 2px; min-height: 0; overflow: hidden; cursor: pointer; transition: background 0.15s; }
  .cell:hover { background: var(--hover); }
  .cell:nth-child(7n + 1) { border-left: 0; }
  .cell.other .num { color: var(--text-3); }
  .num { align-self: flex-end; font: 600 15px/1 var(--font-display); width: 30px; height: 30px; border-radius: 50%; border: 0; background: none; cursor: pointer; flex: none; }
  .num:hover { background: var(--active); }
  .num.today { background: var(--c-red); color: #fff; }
  .mev { display: flex; align-items: center; gap: 6px; border: 0; background: none; padding: 2px 6px; border-radius: 6px; font-size: 12px; font-weight: 550; cursor: pointer; text-align: left; min-width: 0; }
  .mev:hover { background: var(--active); }
  .mev.solid { background: color-mix(in srgb, var(--c) 22%, transparent); color: color-mix(in srgb, var(--c), var(--text) 30%); }
  .mt { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .more { padding: 0 6px; font-size: 11px; }

  .title-field { font-size: 22px; font-weight: 650; height: 56px; border: 0; background: transparent; padding: 0 2px; }
  .title-field:focus { box-shadow: none; background: transparent; }
  .form { display: grid; gap: 12px; margin-top: 8px; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .toggle { display: flex; align-items: center; gap: 10px; font-weight: 500; cursor: pointer; }
  .toggle input { width: 20px; height: 20px; accent-color: var(--accent); }
  .cal-name { margin-top: 16px; }
  .swatches { display: flex; justify-content: center; gap: 12px; margin: 18px 0 6px; flex-wrap: wrap; }
  .swatch { width: 32px; height: 32px; border-radius: 50%; border: 0; cursor: pointer; transition: transform 0.15s var(--spring); }
  .swatch:hover { transform: scale(1.1); }
  .swatch.on { outline: 2px solid var(--text-2); outline-offset: 3px; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }

  .only-narrow, .cals-scrim { display: none; }
  @media (hover: none) { .icon-btn.sm { opacity: 1; } }
  @media (max-width: 720px) {
    .sidebar { display: none; }
    .only-narrow { display: inline-grid; }
    .cals-open .cals-scrim { display: block; position: fixed; inset: 0; z-index: 50; background: rgba(0, 0, 0, 0.45); }
    .cals-open .sidebar {
      display: block; position: fixed; left: 0; right: 0; bottom: 0; z-index: 60; width: auto; max-height: 75vh; overflow-y: auto;
      border: 0; border-radius: var(--r-xl) var(--r-xl) 0 0; padding: 26px 16px calc(26px + env(safe-area-inset-bottom));
      box-shadow: var(--shadow-lg);
    }
    .main { padding: 18px 12px 0; }
    /* The title and + on the first line; the view and the dates below. */
    header { gap: 8px; }
    .large-title { font-size: 28px; flex: 1 1 calc(100% - 52px); min-width: 0; }
    header .new { order: 1; }
    header .only-narrow, header .segmented, header .nav { order: 2; }
    header .spacer { display: none; }
    header .nav { margin-left: auto; }
    .mh { text-align: center; padding: 0 0 6px; }
    .mev .mt { display: none; }
    .num { align-self: center; }
  }
</style>
