#!/usr/bin/env python3
"""Reads the probe's trace of random draws (hooks.c, struct draw).

    python3 tools/probe/draws.py TRACE              # counts and the busiest callers
    python3 tools/probe/draws.py TRACE --list       # every draw
    python3 tools/probe/draws.py TRACE OTHER        # where two runs part
    python3 tools/probe/draws.py TRACE --replica DRAWS [--functions TSV]
                                                    # where the replica parts

With --replica, DRAWS is what `anstoss-3-1 --script ... --draws DRAWS`
wrote: a line per draw of the range, the result and the replica's line
that drew. The original's draws of the main generator with a range of
two or more are set against them in order (below 2 neither generator
moves); the first that differs is shown with what led up to it, the
original's callers named by the decompile's functions.tsv where given.

A record is 48 bytes: the caller's return address, the range (0 for rand()),
the result, the source, the thread (in the order threads first drew) and
the eight return addresses above the caller, by the game's frame pointers
(0 past the last found).
"""

import argparse
import struct
import sys
from collections import Counter
from pathlib import Path

RECORD = struct.Struct("<IIiBBH8I")
SOURCES = ["main", "second", "third", "rand", "rand-activesim"]


def read(path: Path) -> list[tuple]:
    data = path.read_bytes()
    if len(data) % RECORD.size:
        sys.exit(f"{path}: {len(data)} bytes, not whole records")
    return [RECORD.unpack_from(data, i) for i in range(0, len(data), RECORD.size)]


def show(index: int, draw: tuple, name=None) -> str:
    caller, argument, result, source, thread = draw[:5]
    kind = SOURCES[source] if source < len(SOURCES) else str(source)
    where = name or (lambda a: f"{a:#09x}")
    stack = " < ".join(where(a) for a in draw[6:] if a)
    return (f"{index:9} {kind:14} thread {thread} range {argument:6} -> {result:<6} "
            f"{where(caller)}" + (f" < {stack}" if stack else ""))


def main() -> int:
    args = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    args.add_argument("trace", type=Path)
    args.add_argument("other", type=Path, nargs="?")
    args.add_argument("--list", action="store_true")
    args.add_argument("--replica", type=Path)
    args.add_argument("--functions", type=Path)
    args.add_argument("--context", type=int, default=8)
    args = args.parse_args()
    draws = read(args.trace)
    if args.replica:
        return compare(draws, args.replica, args.functions, args.context)
    if args.other:
        other = read(args.other)
        for i, (a, b) in enumerate(zip(draws, other)):
            if a != b:
                print(f"the runs part at draw {i}:\n  {show(i, a)}\n  {show(i, b)}")
                return 1
        if len(draws) != len(other):
            print(f"the same {min(len(draws), len(other))} draws, then one run has more "
                  f"({len(draws)} against {len(other)})")
            return 1
        print(f"the same {len(draws)} draws")
        return 0
    if args.list:
        name = naming(args.functions) if args.functions else None
        for i, draw in enumerate(draws):
            print(show(i, draw, name))
        return 0
    print(f"{len(draws)} draws")
    for source, count in sorted(Counter(d[3] for d in draws).items()):
        print(f"  {SOURCES[source]:14} {count}")
    print("busiest callers:")
    for (source, caller), count in Counter((d[3], d[0]) for d in draws).most_common(15):
        print(f"  {caller:#09x} {SOURCES[source]:14} {count}")
    return 0


MAIN = 0


def naming(functions: Path | None):
    """The original's function an address lies in, by functions.tsv."""
    if not functions:
        return lambda caller: f"{caller:#09x}"
    import bisect
    rows = []
    for line in functions.read_text(encoding="latin-1").splitlines():
        fields = line.split("\t")
        if len(fields) > 3 and len(fields[0]) == 8:
            try:
                rows.append((int(fields[0], 16), fields[2]))
            except ValueError:
                pass
    rows.sort()
    starts = [r[0] for r in rows]

    def name(caller: int) -> str:
        i = bisect.bisect_right(starts, caller) - 1
        return f"{caller:#09x} {rows[i][1]}" if i >= 0 else f"{caller:#09x}"

    return name


def compare(draws: list[tuple], replica: Path, functions: Path | None, context: int) -> int:
    original = [d for d in draws if d[3] == MAIN and d[1] >= 2]
    ours = []
    for line in replica.read_text().splitlines():
        fields = line.split("\t")
        if len(fields) == 3:
            ours.append((int(fields[0]), int(fields[1]), fields[2]))
    name = naming(functions)
    for i, (o, r) in enumerate(zip(original, ours)):
        if (o[1], o[2]) != (r[0], r[1]):
            print(f"the replica parts at draw {i} (of {len(original)} and {len(ours)}):")
            for k in range(max(0, i - context), min(i + 3, len(original), len(ours))):
                o, r = original[k], ours[k]
                mark = ">>" if k == i else "  "
                print(f"{mark}{k:8}  original {o[1]:6} -> {o[2]:<6} {name(o[0]):40}"
                      f"  replica {r[0]:6} -> {r[1]:<6} {r[2]}")
            print("the original's draw there, with its callers:")
            print(show(i, original[i], name))
            return 1
    n = min(len(original), len(ours))
    if len(original) != len(ours):
        print(f"the same {n} draws, then " + (
            f"the original draws on ({len(original)} against {len(ours)}), next:\n"
            f"{show(n, original[n], name)}" if len(original) > n else
            f"the replica draws on ({len(ours)} against {len(original)}), next at {ours[n][2]}"))
        return 1
    print(f"the same {n} draws")
    return 0


if __name__ == "__main__":
    sys.exit(main())
