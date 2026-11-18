#!/bin/bash
# The original's new game of crates/a3-screens/scripts/new-game.txt, kept
# for the run diff of a new game (crates/a3-screens/src/script.rs,
# `a_new_game_against_the_oracles_first_turn`):
#
#   tools/oracle/new-game.sh
#
# Plays the oracle once, a few minutes, with rand() seeded 1
# (A3PROBE_SRAND): the setup and the job offers up to the first turn (the
# line-up, dialog 147), where it dumps runs/new-game/first-turn.an3, then on
# to day 2 with a dump of every new day (day-NNN.an3) and one at the end
# (final.an3). The saves the oracle's copy had are put back at the end.
set -uo pipefail

here=$(cd "$(dirname "$0")/../.." && pwd)
oracle=${ANSTOSS3_ORACLE:-"$HOME/.cache/anstoss-3.1/oracle"}
out="$oracle/runs/new-game"

if [ "${1:-}" = --inside ]; then
    set -e
    probe() { python3 "$here/tools/probe/client.py" "$@"; }
    driver() { "$here/target/debug/anstoss-3-1" --original "$@"; }
    windows() { local path; path=$(realpath "$1"); echo "Z:${path//\//\\}"; }
    script="$here/crates/a3-screens/scripts/new-game.txt"
    first=$(mktemp) rest=$(mktemp)
    trap 'rm -f "$first" "$rest"' EXIT
    { grep -v '^until' "$script"; echo "until dialog 147"; } > "$first"
    # The seed line is the replica's; the rest plays on from the turn.
    { grep -v '^until\|^seed' "$script"; echo "until day 2"; } > "$rest"
    mkdir -p "$out"
    rm -f "$out"/*.an3
    probe "dumps $(windows "$out")" > /dev/null
    driver "$first"
    probe "dump $(windows "$out")\\first-turn.an3" > /dev/null
    driver "$rest"
    probe "dumps off" "dump $(windows "$out")\\final.an3" > /dev/null
    exit 0
fi

saves="$oracle/game/saves"
aside="$oracle/saves-aside"
# One run at a time: the oracle has one saves dir, one display, one port.
exec 9> "$oracle/oracle.lock"
flock 9
(cd "$here" && cargo build -q -p a3-app) || exit 1
mkdir -p "$aside" "$oracle/runs"
mv -f "$saves"/*.an3 "$aside/" 2> /dev/null
status=75
for _ in 1 2 3; do
    A3PROBE_SRAND=1 "$here/tools/oracle/run.sh" "$here/tools/oracle/new-game.sh" --inside > "$oracle/runs/new-game.log" 2>&1
    status=$?
    # Once more if the game crashed (tools/oracle/run.sh).
    [ $status = 75 ] || break
done
rm -f "$saves"/*.an3
mv -f "$aside"/*.an3 "$saves/" 2> /dev/null
[ $status = 0 ] && [ -f "$out/first-turn.an3" ] || { echo "see $oracle/runs/new-game.log" >&2; exit 1; }
echo "$out/first-turn.an3"
