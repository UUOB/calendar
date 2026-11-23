#!/usr/bin/env bash
# Screenshots of Calendar on the Mac, for checking how it looks (mac.yml):
# connected to a UUOB with made-up demo data; Week, Month and Year with a day's
# hint, Settings with the Custom alert, a button's hint, a click outside
# Settings, light and dark; then a small window. A button a version doesn't have
# is simply left out (the run for the version before uses the same script).
#   mac/shots.sh <Calendar.app> <uuob binary> <seeded data dir> <out dir>
set -uo pipefail
name=Calendar exe=$1/Contents/MacOS/Calendar UUOB=$2 DATA=$3 out=$4
mkdir -p "$out"
. "$(dirname "$0")/../sdk/kit/mac/shots-lib.sh"

mode=start
uuob_up
app_up
for mode in light dark; do
  [ $mode = dark ] && { se 'tell appearance preferences to set dark mode to true'; sleep 2; }
  click "Week"; shot week
  click "Month" && shot month
  # Year joins the switch from Settings (a version without it leaves this out).
  cmd ","; click "Year"; esc
  if click "Year"; then
    shot year
    # A day's hint: the first squares sit near the top left of the page.
    pointer 340 230; sleep 1; shot year-hint
  fi
  click "Week"
  # The side panel's foot: Settings and the toggle on one line, the toggle's hint.
  hover "Hide sidebar (⌃⌘S)" && shot sidebar-hint
  cmd ","; shot settings
  # Custom… puts a time in the menu's place: 15:00 is 15 hours before.
  if click "~before"; then
    key "Cus"; enter; shot alert-custom
    key "15:00"; enter; shot alert-set
  fi
  # A click on the window beside Settings closes it.
  se "tell process \"$name\" to click at {1040, 700}"; sleep 1.5; shot outside-click
  esc
done

# A small window (dark): the rail, and Year stacked.
mode=small
se "tell process \"$name\" to set size of front window to {520, 620}"; sleep 2
shot week
click "Year" && shot year
app_down
uuob_down
