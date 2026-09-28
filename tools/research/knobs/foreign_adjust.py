#!/usr/bin/env python
"""Foreign-load sensitivity check for the W0.8 build A/B tables.

  foreign_adjust.py PREFIX REFERENCE_ARM [--at 0.10] [--runs DIR]
  e.g. foreign_adjust.py c6 c6-release

Per family, fits one pooled within-arm linear model of the relative
inspector CPU ms/native against the run's foreign load (the fraction of
the run CPUs' busy time not used by the run, from knob_run.py), then
reports each arm's mean ratio to the reference arm with every run moved
to the same foreign load (--at). Each arm is compared with the reference
runs of the same session(s) (session_map.py), as in results_tables.py.
This is an estimate [E]: it only tests whether the arms' ratios are an
artefact of different foreign load; it does not turn runs above the 10 %
void threshold into valid timings, and it extrapolates the fitted slope
outside the measured foreign-load range when --at lies below it.
"""
import argparse
import collections
import json
import re
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import session_map  # noqa: E402

RUNS = Path("/common/dev/rustred/TMP/w0/knobs/runs")


def load(runs, prefix, sessions_dir=session_map.SESSIONS):
    smap = session_map.load(sessions_dir)
    data = collections.defaultdict(list)
    for path in sorted(Path(runs).glob(f"{prefix}-*/*/metrics.json")):
        label, family = path.parent.parent.name, path.parent.name
        arm = re.sub(r"-r\d+$", "", label)
        m = json.loads(path.read_text())
        ms, foreign = m.get("inspector_cpu_ms_per_native"), m.get("foreign_load_fraction")
        if ms and foreign is not None:
            data[(family, arm)].append((foreign, ms, smap.get((label, family))))
    return data


def main():
    p = argparse.ArgumentParser()
    p.add_argument("prefix")
    p.add_argument("reference")
    p.add_argument("--at", type=float, default=0.10)
    p.add_argument("--runs", default=str(RUNS))
    p.add_argument("--sessions", default=str(session_map.SESSIONS))
    args = p.parse_args()
    data = load(args.runs, args.prefix, args.sessions)
    print(f"| family | slope (relative ms/native per unit foreign load) | n runs | foreign load range |"
          f" arm ratios to `{args.reference}` at foreign load {args.at:.2f} [E] |")
    print("|---|---|---|---|---|")
    for family in sorted({f for f, _ in data}):
        arms = sorted(a for f, a in data if f == family)
        xs, ys, all_x = [], [], []
        for arm in arms:
            pts = data[(family, arm)]
            mx = statistics.mean(x for x, _, _ in pts)
            my = statistics.mean(y for _, y, _ in pts)
            for x, y, _ in pts:
                xs.append(x - mx)
                ys.append((y - my) / my)
                all_x.append(x)
        sxx = sum(x * x for x in xs)
        slope = sum(x * y for x, y in zip(xs, ys)) / sxx if sxx else 0.0
        def adjusted(pts):
            return statistics.mean(y * (1 - slope * (x - args.at)) for x, y, _ in pts)

        ref_pts = data.get((family, args.reference), [])
        parts = []
        for a in arms:
            if a == args.reference or not ref_pts:
                continue
            sessions = {s for _, _, s in data[(family, a)]}
            same = [p for p in ref_pts if p[2] in sessions]
            parts.append(f"{a.removeprefix(args.prefix + '-')} x{adjusted(data[(family, a)]) / adjusted(same or ref_pts):.3f}"
                         + ("" if same else " (pooled ref)"))
        ratios = ", ".join(parts) or "-"
        print(f"| {family} | {slope:+.2f} | {len(all_x)} | {min(all_x):.2f}..{max(all_x):.2f} | {ratios} |")


if __name__ == "__main__":
    main()
