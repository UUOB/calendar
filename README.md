# Calendar

**Your calendars in one place, kept on your own devices.**

**[Download](https://github.com/jachymri/uuob-releases)** for Android and Mac · Built for **[UUOB](https://github.com/UUOB/uuob)**

## Built for UUOB

Calendar has no server and no account. Your events live in [UUOB](https://github.com/UUOB/uuob), on your
own devices. They are end-to-end encrypted, so only your devices (and people you share with) can read them, and
UUOB keeps them in sync.

1. Install UUOB, then Calendar.
2. Open Calendar. UUOB asks *"Calendar wants to use your network"*. Tap **Allow**.
3. Your events are now on all your linked devices.

No UUOB on this device? In UUOB on another device choose **Apps → Connect with a code**, and type the code
into Calendar. [Full setup guide →](https://github.com/UUOB/uuob/blob/main/docs/setup.md)

## Features

- **Week and month views**, and a day view on phones.
- **Several calendars**, each with its own colour. Hide any of them.
- **Repeating events**: change one day or the whole series.
- **Bring your calendars along**: import `.ics` files, or subscribe to a calendar link (Google, holidays,
  school, work). Links refresh by themselves.
- **Save a calendar as an `.ics` file** to take it anywhere.
- **Invitations**: see who's coming and mark your own answer (Going, Maybe or No).
- **Join calls** with one tap when an event has a meeting link.
- **Alerts**: one default for all events, or a different one per event, on your phone and computer.
- **Your tasks** from Tasks, next to your events, if you allow it.
- **Share a calendar** with someone, so they can view it or edit it.
- **Up next widget** on Android: today's timeline at a glance.

## For developers

```
app/             the app (Svelte) and its shell (Tauri v2, in app/src-tauri)
uuob/            UUOB's repository, as a git submodule: the app SDK (uuob/ui/uuob.js and
                 uuob/crates/uuob-sdk), the connect screen and design tokens (uuob/ui),
                 in-app updates (uuob/crates/uuob-update) and the build scripts (uuob/docker)
```

```bash
git clone --recurse-submodules https://github.com/UUOB/calendar.git && cd calendar
```

Everything runs in UUOB's dev container (`uuob/docker/dev.sh` builds it on first use):

```bash
uuob/docker/dev.sh sh -c 'npm ci && npm run build'   # the front end
uuob/docker/dev.sh cargo build                       # the desktop app (Linux)
uuob/docker/android.sh uuob/docker/apk.sh            # a signed APK for arm64 phones, in dist/
gh workflow run mac.yml                              # the Mac app, built on GitHub
```

To publish a new version, raise `version` in `app/src-tauri/tauri.conf.json`, commit, push and run
`uuob/docker/release.sh` (see [UUOB for developers](https://github.com/UUOB/uuob/blob/main/docs/developers.md), *Updates*).

To use a newer UUOB SDK: `git -C uuob pull origin main`, then commit the `uuob` folder.

## License

[GNU Affero General Public License, version 3 or later](LICENSE).
