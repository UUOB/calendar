# Calendar on native screens: what has to keep working

Taken from the Svelte app (`app/Calendar.svelte`, `AlertPick.svelte`,
`app/lib/*`, `uuob/ui/Settings.svelte`, Android's `Alert.kt`, `Upcoming.kt`,
`UpNext.kt`) before it is replaced. Ticked once checked on the new app.
L = Linux window against a seeded UUOB; A = Android emulator with UUOB;
T = a test in `core/` (`tests/oracle`: the web version's own answers, 3180 of
them in four time zones). The Mac app is written (in the web look) but not yet built.

## The library (the same answers as app/lib)
- [x] Time zones: an event in another zone moves to this device's time on each date; skipped and repeated hours (T)
- [x] Repeats: daily/weekly/monthly/yearly, INTERVAL, COUNT, UNTIL, BYDAY with ordinals, BYMONTHDAY, BYMONTH, BYSETPOS, WKST; others show once (T)
- [x] .ics import: Google, Outlook (unknown zone names), all-day, DURATION, changed and cancelled occurrences, people and answers, meeting links; ids `ics-<fnv>` from the UID, also with accents and emoji (T)
- [x] .ics export Google, Apple and Outlook can import, folded at 75 bytes (T)
- [x] Saving and deleting: only this one / all of them, moving the series, another calendar, fields the sheet doesn't show kept (T)
- [x] The event card: Join (Meet, Zoom, Teams, Webex, Jitsi, Whereby), notes without the joining block, links, people and your answer (T)
- [x] Upcoming as the web version writes it (T)

## Data (the same records as the web version)
- [x] `events` in each calendar's container; the first calendar in the app's first container, its name and colour in `calendars/personal` (the web version on the emulator read events the native one wrote)
- [x] Each added calendar's `calendar/info` `{color, created, subscribed?, …}` (the rest kept when edited) (a calendar subscribed in the native app showed in the web one)
- [x] `prefs`: `alerts {before}`, `me {emails}`, `views {narrow, wide}`, `calendar {show_tasks, hidden}` (the whole record each time) (Linux: `calendar.hidden`, `views`, `alerts` and `me` seen in UUOB)
- [x] `subscriptions/<calendar> {url, refreshed}` and `answers/<event> {answer}` and `alerts/<event> {before: minutes | false}` in the first container (Linux: `subscriptions {url, refreshed}` and `alerts {before: 45}` seen in UUOB; answers by the core tests)
- [x] Old calendars (records in `calendars`) move into containers keyed by their old id, with their events (`created` 1 when unknown) (Linux: an old "Old Club" with its event moved on opening)
- [x] `upcoming` (14 days, declined ones left out, your own alert or the default, the calendar's colour) (matches the web version's output in 3180 cases over 4 zones; written on the emulator)
- [x] Old and new side by side on Android (emulator: the web version read the events and the subscribed calendar the native one wrote; the native one installs back over it)

## Views
- [x] Day, 2 days, 4 days, Week, Month
- [x] The switch on top offers 2 or 3 views, chosen in Settings, phones and computers each their own (`prefs/views`)
- [x] Previous / Today / Next; the title shows month and year
- [x] Hour grid: overlapping events side by side, title / time / place as room allows, the now line, all-day row with pieces that continue, repeat mark
- [x] Drag on the grid to make an event (a finger rests 350 ms first) (Linux)
- [x] Month: 6 weeks, 3 events per day and "+N more", tapping a day number opens the day view, tapping a day makes an event
- [x] Week starts Monday or Sunday (Settings, all apps); times 14:30 or 2:30 PM (Linux)
- [x] An app left open overnight moves on to the new day (Linux: rolled over to Friday at midnight)

## Calendars (sidebar)
- [x] Show/hide each calendar (kept in `prefs/calendar`) (Linux)
- [x] New calendar, edit name and colour, share, delete or leave; the first calendar stays private (Linux: new, rename, recolour, delete; sharing goes through UUOB and was not tried by hand)
- [x] Import .ics files (choose a new or existing calendar per file), with the result: events, repeats it can't show, zones read in the calendar's own (Linux: a repeating event in New York time and an all-day one, into a new calendar)
- [x] Subscribe to a link (webcal/http/https), named from the file or the link; read again after 3 hours when the app opens; Update now; "Follows a calendar link, updated …" (Linux and Android: a public HTTPS holiday calendar; on Linux it was read again when the app opened 4 hours later, and Update now read it at once)
- [x] Save as .ics file (Linux: time zone and repeat rule kept)
- [x] Tasks with dates (asks to read Tasks' data) (Linux: asks once, then timed tasks on the grid and dated ones in the month)
- [x] Phones: the list as a sheet (Android)

## Events
- [x] The card: when, repeat, calendar, Join, place, link, alert, people (folded), notes with links, joining details folded, Going / Maybe / No
- [x] The sheet: title, all-day, start/end dates and times, repeat (never/daily/weekly with days/monthly/yearly/custom, every N, until a date or N times), alert, calendar, place, notes
- [x] Only this one / All of them, for saving and deleting a repeating event (Linux: the records for one occurrence)
- [x] Alert picker: default, none, the usual times, Custom… (minutes, hours, days, up to a week) (Linux: Custom… 45 minutes before, stored as `before: 45`)

## Settings
- [x] Time and week start (all apps), Views, Default alert, Invitations (your addresses), About (Linux)

## Android
- [x] Alerts as Calendar's own notification, only from UUOB (emulator: delivered at the alert time; a forged broadcast was ignored)
- [x] The Up next widget: today and tomorrow, hours from now, all-day chips, the now line, light and dark; its list from UUOB only (emulator, light only: placed on the home screen, it drew the next events and the now line; its list arrived from UUOB, a forged one was ignored; all-day chips and dark not checked)
- [x] Installs over the web version and keeps the data (emulator)
