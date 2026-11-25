#!/usr/bin/env bash
# Screenshots of Calendar on the Mac, for checking how it looks (mac.yml):
# connected to a UUOB with made-up demo data; Week, Month and Year with a day's
# hint, Settings with the Custom alert, buttons' hints, a click outside
# Settings, light and dark; the side panel's edge dragged; then a small window.
# A button a version doesn't have is simply left out (the run for the version
# before uses the same script).
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
  cmd ","; pointer 512 400 scroll -15; click "Year"; esc
  if click "Year"; then
    shot year
    # A day's hint: the first squares sit near the top left of the page.
    pointer 330 220; sleep 1; shot year-hint
  fi
  click "Week"
  # The + button's hint, kept inside the window at its right edge.
  pointer 966 77; sleep 1; shot hint-add
  # The title rested on: today's date in numbers.
  pointer 340 77; sleep 1.5; shot title-date
  # The side panel's foot: Settings and the toggle on one line, the toggle's hint.
  # The toggle's hint (an icon has no words to find; the window sits at 12,31, 1000 wide, 680 high).
  pointer 222 681; sleep 1; shot sidebar-hint
  cmd ","; shot settings
  pointer 512 400 scroll -15; shot settings-end
  # Custom… puts a time in the menu's place: 15:00 is 15 hours before.
  if click "~before"; then
    key "Cus"; enter; shot alert-custom
    key "15:00"; enter; shot alert-set
  fi
  # A click on the window beside Settings closes it.
  # A real click beside the sheet, on the window it covers (the screen is 1024 wide).
  pointer 70 600 left; sleep 1.5; shot outside-click
  esc
done

# The side panel's edge dragged (dark): wider, to the rail, to nothing (its
# toggle beside the window's buttons), back out from the left edge; then
# ⌃⌘S folds it to the rail. A version without the edge shows the same each time.
mode=sidebar
pointer 252 400 drag 330 400; shot wider
pointer 330 400 drag 120 400; shot rail
pointer 92 400 drag 22 400; shot hidden
pointer 18 400 drag 290 400; shot back
se 'keystroke "s" using {control down, command down}'; sleep 1.5; shot toggled
se 'keystroke "s" using {control down, command down}'; sleep 1.5

# A small window (dark): the rail, and Year stacked.
mode=small
win "set size of front window to {520, 620}"; sleep 2
shot week
click "Year" && shot year
app_down
uuob_down
