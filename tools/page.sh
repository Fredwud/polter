#!/bin/sh
# This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
# the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# The pictures on Polter's page (README.md, art/page/), made from LANTERN's bot run with Polter's
# own commands: the same build makes the same files. Run it after a change that shows.
#   sh tools/page.sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release -p lantern
out="$PWD/art/page"
mkdir -p "$out"
tmp=$(mktemp -d)
cd carts/lantern
game=../../target/release/lantern
run=haunts/bot_run.haunt
# the run from the second spark to the open door, twenty frames a second
$game clip --haunt $run --frame 181 --frames 440 --step 3 --out "$out/lantern.gif" > /dev/null
$game peek --haunt $run --frames 560 --step 40 --scale 1 --columns 5 --dir "$tmp/peek" > /dev/null
cp "$tmp/peek/contact_sheet.png" "$out/contact_sheet.png"
$game listen --haunt $run --frames 560 --dir "$tmp/listen" > /dev/null
cp "$tmp/listen/ears.png" "$out/ears.png"
$game workshop --haunt $run --frames 300 --out "$out/workshop.png" --size 1280,720 > /dev/null
rm -r "$tmp"
ls -l "$out"
