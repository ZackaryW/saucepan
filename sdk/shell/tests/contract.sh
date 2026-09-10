#!/bin/sh
set -eu
. "$1"
root=$2
export SAUCEPAN_TEST_ROOT="$root/store"
export SAUCEPAN_TEST_KEY=0707070707070707070707070707070707070707070707070707070707070707
saucepan_init > "$root/init.json"
saucepan_register 'app with spaces;$(literal)' > "$root/proof.json"
export SAUCEPAN_MARKER="$root/proof.json"
saucepan_acquire "$root/recipe.json" > "$root/acquired.json"
saucepan_view > "$root/view.json"
saucepan_verify "$root/view.json" > "$root/verified.json"
saucepan_configure --settings "$root/settings.json" > "$root/configured.json"
saucepan_view > "$root/configured-view.json"
if saucepan_verify "$root/view.json" > "$root/stale.out" 2> "$root/stale.err"; then exit 90; else test "$?" -eq 1; fi
export SAUCEPAN_TEST_KEY=0808080808080808080808080808080808080808080808080808080808080808
if saucepan_view > "$root/wrong.out" 2> "$root/wrong.err"; then exit 91; else test "$?" -eq 1; fi
unset SAUCEPAN_TEST_KEY
if saucepan_view > "$root/missing.out" 2> "$root/missing.err"; then exit 92; else test "$?" -eq 64; fi
unset SAUCEPAN_TEST_ROOT SAUCEPAN_MARKER
saucepan_shared_executable > "$root/executable.json"
