#!/usr/bin/env python
"""Foreign-load sensitivity check for the W0.8 build A/B tables.

  foreign_adjust.py PREFIX REFERENCE_ARM [--at 0.10] [--runs DIR]
  e.g. foreign_adjust.py c6 c6-release

Per family, fits one pooled within-arm linear model of the relative
inspector CPU ms/native against the run's foreign load (the fraction of
the run CPUs' busy time not used by the run, from knob_run.py), then
reports each arm's mean ratio to the reference arm with every run moved
to the same foreign load (--at). This is an estimate [E]: it only tests
whether the arms' ratios are an artefact of different foreign load; it
does not turn runs above the 10 % void threshold into valid timings.
"""
import argparse
import collections
import json
import re
import statistics
from pathlib import Path

RUNS = Path("/common/dev/rustred/TMP/w0/knobs/runs")


def load(runs, prefix):
    data = collections.defaultdict(list)
    for path in sorted(Path(runs).glob(f"{prefix}-*/*/metrics.json")):
        label, family = path.parent.parent.name, path.parent.name
        arm = re.sub(r"-r\d+$", "", label)
        m = json.loads(path.read_text())
        ms, foreign = m.get("inspector_cpu_ms_per_native"), m.get("foreign_load_fraction")
        if ms and foreign is not None:
            data[(family, arm)].append((foreign, ms))
    return data


def main():
    p = argparse.ArgumentParser()
    p.add_argument("prefix")
    p.add_argument("reference")
    p.add_argument("--at", type=float, default=0.10)
    p.add_argument("--runs", default=str(RUNS))
    args = p.parse_args()
    data = load(args.runs, args.prefix)
    print(f"| family | slope (relative ms/native per unit foreign load) | n runs | foreign load range |"
          f" arm ratios to `{args.reference}` at foreign load {args.at:.2f} [E] |")
    print("|---|---|---|---|---|")
    for family in sorted({f for f, _ in data}):
        arms = sorted(a for f, a in data if f == family)
        xs, ys, all_x = [], [], []
        for arm in arms:
            pts = data[(family, arm)]
            mx = statistics.mean(x for x, _ in pts)
            my = statistics.mean(y for _, y in pts)
            for x, y in pts:
                xs.append(x - mx)
                ys.append((y - my) / my)
                all_x.append(x)
        sxx = sum(x * x for x in xs)
        slope = sum(x * y for x, y in zip(xs, ys)) / sxx if sxx else 0.0
        adjusted = {a: statistics.mean(y * (1 - slope * (x - args.at)) for x, y in data[(family, a)])
                    for a in arms}
        ref = adjusted.get(args.reference)
        ratios = ", ".join(f"{a.removeprefix(args.prefix + '-')} x{adjusted[a] / ref:.3f}"
                           for a in arms if a != args.reference) if ref else "-"
        print(f"| {family} | {slope:+.2f} | {len(all_x)} | {min(all_x):.2f}..{max(all_x):.2f} | {ratios} |")


if __name__ == "__main__":
    main()
