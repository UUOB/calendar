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
- **Alerts**: one default for all events, or a different one per event, at any time before it you choose,
  on your phone and computer.
- **Your tasks** from Tasks, next to your events, if you allow it.
- **Share a calendar** with someone, so they can view it or edit it.
- **Views**: a day, 2 days, 4 days, a week or the month. In Settings you choose which two or three the
  switch on top offers.
- **Up next widget** on Android: today and tomorrow side by side, at a glance.

## For developers

Calendar is native on each system, over one Rust core, like UUOB itself:

```
core/            Calendar without its screens (Rust): its data, rules and the calls to UUOB, for all three
                 systems; Swift and Kotlin reach it through UniFFI
android/         the Android app (Jetpack Compose, Material 3)
mac/             the Mac app (SwiftUI, macOS 14 or newer)
linux/           the Linux app (GTK 4 and Libadwaita 1.5, through Relm4)
sdk/             UUOB's SDK for apps, as a git submodule (UUOB-sdk): the client (sdk/client) and the
                 controls and connect screen every app shares (sdk/kit: android, mac, linux)
uuob/            UUOB's repository, as a git submodule: the build scripts (uuob/docker)
app/             the earlier web version (Svelte in Tauri), kept until the native one replaces it
```

```bash
git clone --recurse-submodules https://github.com/UUOB/calendar.git && cd calendar
```

Everything builds in UUOB's containers (`uuob/docker`; each builds its image on first use):

```bash
uuob/docker/linux.sh cargo build -p calendar-linux     # the Linux app, target/debug/uuob-calendar
uuob/docker/linux.sh cargo test                       # the core's tests
uuob/docker/android.sh uuob/docker/apk.sh             # a signed APK for arm64 phones, in dist/
gh workflow run mac.yml                               # the Mac app, built on GitHub (or mac/build.sh on a Mac)
```

To publish a new version, raise `version` in `Cargo.toml`, commit, push and run `uuob/docker/release.sh`
(see [UUOB for developers](https://github.com/UUOB/uuob/blob/main/docs/developers.md), *Updates*).

To use a newer SDK or UUOB: `git -C sdk pull origin main` (or `uuob`), then commit that folder.

## License

[GNU Affero General Public License, version 3 or later](LICENSE).
