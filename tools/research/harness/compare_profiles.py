#!/usr/bin/env python3
"""Compare per-native symbol costs between harness profiles (perf_native_symbols.py --json outputs).

  compare_profiles.py BASE.json RUN.json [RUN2.json ...] [--kind self|inclusive] [--top N]

Rows are sorted by the growth (RUN - BASE, per native) in the last RUN; the table
shows each symbol's per-native period in every profile, the growth and its share
of the total growth of native period per native.
"""
import json
import sys


def main():
    args = sys.argv[1:]
    kind, top = "self", 40
    files = []
    while args:
        if args[0] == "--kind":
            kind, args = args[1], args[2:]
        elif args[0] == "--top":
            top, args = int(args[1]), args[2:]
        else:
            files.append(args[0])
            args = args[1:]
    profs = [json.load(open(f)) for f in files]
    key = "self_per_native" if kind == "self" else "inclusive_per_native"
    base, last = profs[0], profs[-1]
    growth_total = last["period_native_per_native"] - base["period_native_per_native"]
    syms = set()
    for p in profs:
        syms.update(p[key])
    rows = []
    for s in syms:
        vals = [p[key].get(s, 0.0) for p in profs]
        rows.append((vals[-1] - vals[0], s, vals))
    rows.sort(reverse=True)
    hdr = " | ".join(f"{p['run'].rstrip('/').split('/')[-1]}" for p in profs)
    print(f"{kind}: native period per native: " + ", ".join(f"{p['period_native_per_native']:.3e}" for p in profs)
          + f"; growth {growth_total:.3e}")
    print(f"| symbol | {hdr} | growth | share of growth |")
    print("|---|" + "---:|" * (len(profs) + 2))
    for g, s, vals in rows[:top]:
        print(f"| `{s[:110]}` | " + " | ".join(f"{v:.3e}" for v in vals)
              + f" | {g:.3e} | {100 * g / growth_total:.1f}% |")


if __name__ == "__main__":
    main()
