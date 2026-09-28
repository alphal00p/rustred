#!/usr/bin/env python
"""Summarise an ab_session.sh session: per arm and family, mean inspector CPU
per native (ms), whole-command wall, peak RSS and foreign load over the
repeats, the ratio of means against the reference arm of the same session, and
the strict verdicts vs 4a17f9c7.

usage: ab_table.py TAG REF_ARM ARM... [--families fg bmw h x] [--runs-root DIR]
"""
import argparse
import json
from pathlib import Path
from statistics import mean

p = argparse.ArgumentParser()
p.add_argument("tag")
p.add_argument("ref")
p.add_argument("arms", nargs="+")
p.add_argument("--families", nargs="+", default=["fg", "bmw", "h", "x"])
p.add_argument("--runs-root", default="/common/dev/rustred/TMP/w1-ops/runs")
args = p.parse_args()
root = Path(args.runs_root)


def rows(arm, family):
    out = []
    for run in sorted(root.glob(f"{args.tag}-{arm}-r*")):
        metrics = run / family / "metrics.json"
        if not metrics.exists():
            continue
        m = json.loads(metrics.read_text())
        strict = run / family / "strict-vs-4a17f9c7.json"
        verdict = json.loads(strict.read_text()).get("verdict") if strict.exists() else None
        out.append({"run": run.name, "cpu": m.get("inspector_cpu_ms_per_native"),
                    "wall": m.get("whole_command_seconds"), "rss": m.get("peak_rss_bytes"),
                    "foreign": m.get("foreign_load_fraction"), "exit": m.get("exit_code"),
                    "natives": m.get("native_processed_nodes"), "strict": verdict})
    return out


summary = {"tag": args.tag, "reference": args.ref, "families": {}}
lines = [f"| family | arm | n | inspector ms/native | x ref | wall s | x ref | peak RSS GB | x ref | "
         f"foreign load | strict vs 4a17f9c7 |", "|---|---|---|---|---|---|---|---|---|---|---|"]
for family in args.families:
    ref = rows(args.ref, family)
    base = {k: mean(r[k] for r in ref) for k in ("cpu", "wall", "rss")} if ref else None
    summary["families"][family] = {}
    for arm in [args.ref, *args.arms]:
        data = rows(arm, family)
        if not data:
            continue
        means = {k: mean(r[k] for r in data) for k in ("cpu", "wall", "rss", "foreign")}
        ratio = {k: (means[k] / base[k] if base else None) for k in ("cpu", "wall", "rss")}
        verdicts = [r["strict"] for r in data]
        summary["families"][family][arm] = {"n": len(data), "means": means, "ratios": ratio,
                                            "per_run": data, "strict": verdicts}
        fmt = lambda v: "-" if v is None else f"{v:.3f}"
        lines.append(f"| {family} | {arm} | {len(data)} | {means['cpu']:.4f} | {fmt(ratio['cpu'])} | "
                     f"{means['wall']:.1f} | {fmt(ratio['wall'])} | {means['rss'] / 1e9:.3f} | {fmt(ratio['rss'])} | "
                     f"{means['foreign']:.2f} | {'/'.join(str(v) for v in verdicts)} |")
out = root / f"{args.tag}-summary.json"
out.write_text(json.dumps(summary, indent=1))
print("\n".join(lines))
print(f"\n(json: {out})")
