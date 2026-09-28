#!/usr/bin/env python3
"""Cycles per native by symbol from a frame-pointer perf profile of a harness run.

  perf script -i RUN/perf.data -F tid,period,ip,sym | perf_native_symbols.py RUN [--top N] [--json OUT]

Only samples whose call chain contains the native entry point
(`walking::inspection::inspect`) are counted, i.e. native inspection CPU on the
worker threads (prepare and harness bookkeeping are excluded). For each symbol:
self = period of samples whose leaf is the symbol; inclusive = period of samples
with the symbol anywhere in the chain (counted once per sample). Values are
divided by the run's native count (receipt.json section.natives), giving event
counts (cycles for cycles:u) per native. Symbols are shortened by dropping
generic arguments so that monomorphisations aggregate.
"""
import json
import os
import re
import sys
from collections import defaultdict

ENTRY = "walking::inspection::inspect"


def short(sym):
    s = sym.strip()
    s = re.sub(r"\+0x[0-9a-f]+$", "", s)
    # collapse generic arguments <...> (nested) to keep names comparable
    out, depth = [], 0
    for ch in s:
        if ch == "<":
            depth += 1
            if depth == 1:
                out.append("<")
            continue
        if ch == ">":
            depth -= 1
            if depth == 0:
                out.append("..>")
            continue
        if depth == 0:
            out.append(ch)
    return "".join(out)


def main():
    run = sys.argv[1]
    top = 60
    out_json = None
    args = sys.argv[2:]
    while args:
        if args[0] == "--top":
            top, args = int(args[1]), args[2:]
        elif args[0] == "--json":
            out_json, args = args[1], args[2:]
        else:
            sys.exit(__doc__)
    receipt = json.load(open(os.path.join(run, "receipt.json")))
    natives = receipt["section"]["natives"]
    self_p = defaultdict(int)
    incl_p = defaultdict(int)
    total = native_total = 0
    samples = native_samples = 0
    period = 0
    chain = []

    def flush():
        nonlocal total, native_total, samples, native_samples
        if not chain and period == 0:
            return
        samples += 1
        total += period
        if any(ENTRY in f for f in chain):
            native_samples += 1
            native_total += period
            if chain:
                self_p[short(chain[0])] += period
            for f in set(short(f) for f in chain):
                incl_p[f] += period

    for line in sys.stdin:
        if not line.strip():
            continue
        if line[0] in " \t" and not re.match(r"^\s*\d+\s+\d+\s*$", line):
            parts = line.strip().split(None, 1)
            if parts and parts[0].startswith("ffffffff"):
                continue  # kernel entry frame of a user-mode sample: attribute to the user frame
            chain.append(parts[1] if len(parts) == 2 else "[unknown]")
            continue
        # sample header: "<tid> <period>"
        flush()
        parts = line.split()
        period = int(parts[-1])
        chain = []
    flush()
    res = {"run": run, "natives": natives, "samples": samples, "native_samples": native_samples,
           "period_total": total, "period_native": native_total,
           "period_native_per_native": native_total / natives,
           "self_per_native": {k: v / natives for k, v in sorted(self_p.items(), key=lambda kv: -kv[1])[:400]},
           "inclusive_per_native": {k: v / natives for k, v in sorted(incl_p.items(), key=lambda kv: -kv[1])[:400]}}
    if out_json:
        json.dump(res, open(out_json, "w"), indent=0)
    print(f"{run}: natives {natives}, samples {samples} ({native_samples} native), "
          f"native period/native {native_total / natives:.4e}")
    print("self (period per native, share of native):")
    for k, v in list(res["self_per_native"].items())[:top]:
        print(f"  {v:12.4e} {100 * v * natives / native_total:6.2f}%  {k[:150]}")


if __name__ == "__main__":
    main()
