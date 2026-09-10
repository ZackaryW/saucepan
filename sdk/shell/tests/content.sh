#!/bin/sh
set -eu
. "$1"
root=$2
export SAUCEPAN_TEST_ROOT="$root/store"
export SAUCEPAN_TEST_KEY=0707070707070707070707070707070707070707070707070707070707070707
export SAUCEPAN_MARKER="$root/proof.json"
saucepan_path "$3" > "$root/path.json"
saucepan_history "$4" > "$root/history.json"
saucepan_snapshot "$4" "$5" > "$root/snapshot.json"
saucepan_mirror "$3" "$root/mirror with spaces" > "$root/mirror.json"
if saucepan_mirror "$3" "$root/mirror with spaces" > "$root/occupied.out" 2> "$root/occupied.err"; then exit 93; else test "$?" -eq 1; fi
unset SAUCEPAN_MARKER
export SAUCEPAN_APP=ordinary
saucepan_register ordinary > "$root/ordinary-proof.json"
saucepan_view > "$root/ordinary.json"
