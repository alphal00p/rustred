#!/usr/bin/env python
"""Summarise an ab_session.sh session per arm and family: inspector CPU per
native (ms; mean, per-run min..max and whether the arm's runs are disjoint
from the reference arm's), instructions and cycles per native and IPC (perf,
user space, whole process), wall, peak RSS, per-run foreign load (min..max),
the ratio of means against the reference arm of the same session, and the
strict and oracle verdicts per run.

Reading rules (handoff 0.1 item 7): CPU levers are read in instructions per
native (load-insensitive) and IPC/cycles (load-sensitive); an arm whose
inspector-CPU runs overlap the reference arm's is "not distinguishable" at
this n, whatever its ratio of means.

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
        gate = run / family / "oracle" / "oracle-gate.json"
        oracle = json.loads(gate.read_text()).get("gate") if gate.exists() else None
        perf = m.get("perf") or {}
        out.append({"run": run.name, "cpu": m.get("inspector_cpu_ms_per_native"),
                    "wall": m.get("whole_command_seconds"), "rss": m.get("peak_rss_bytes"),
                    "foreign": m.get("foreign_load_fraction"), "exit": m.get("exit_code"),
                    "natives": m.get("native_processed_nodes"), "strict": verdict, "oracle": oracle,
                    "ins": perf.get("instructions_u_per_native"), "cyc": perf.get("cycles_u_per_native"),
                    "ipc": perf.get("ipc_u"),
                    "foreign_busy_cpus_mean": (m.get("recorder") or {}).get("foreign_busy_cpus_mean")})
    return out


def avg(data, key):
    values = [r[key] for r in data if r[key] is not None]
    return mean(values) if values else None


summary = {"tag": args.tag, "reference": args.ref, "families": {}}
lines = ["| family | arm | n | inspector ms/native mean [min..max] | x ref | runs vs ref | instr/native (M) | x ref | "
         "IPC | x ref | wall s | peak RSS GB | x ref | foreign load per run | strict | oracle |",
         "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
for family in args.families:
    ref = rows(args.ref, family)
    base = {k: avg(ref, k) for k in ("cpu", "wall", "rss", "ins", "ipc")} if ref else {}
    summary["families"][family] = {}
    for arm in [args.ref, *args.arms]:
        data = rows(arm, family)
        if not data:
            continue
        means = {k: avg(data, k) for k in ("cpu", "wall", "rss", "foreign", "ins", "cyc", "ipc")}
        ratio = {k: (means[k] / base[k] if base.get(k) and means[k] is not None else None)
                 for k in ("cpu", "wall", "rss", "ins", "ipc")}
        cpus = [r["cpu"] for r in data]
        ref_cpus = [r["cpu"] for r in ref]
        if arm == args.ref:
            relation = "reference"
        elif max(cpus) < min(ref_cpus):
            relation = "all below"
        elif min(cpus) > max(ref_cpus):
            relation = "all above"
        else:
            relation = "overlapping: not distinguishable"
        foreign = [r["foreign"] for r in data if r["foreign"] is not None]
        summary["families"][family][arm] = {"n": len(data), "means": means, "ratios": ratio,
                                            "relation_to_reference_runs": relation, "per_run": data}
        fmt = lambda v, d=3: "-" if v is None else f"{v:.{d}f}"
        lines.append(
            f"| {family} | {arm} | {len(data)} | {means['cpu']:.4f} [{min(cpus):.4f}..{max(cpus):.4f}] | "
            f"{fmt(ratio['cpu'])} | {relation} | {fmt(means['ins'] and means['ins'] / 1e6, 2)} | {fmt(ratio['ins'])} | "
            f"{fmt(means['ipc'])} | {fmt(ratio['ipc'])} | {means['wall']:.1f} | {means['rss'] / 1e9:.3f} | "
            f"{fmt(ratio['rss'])} | {min(foreign):.2f}..{max(foreign):.2f} | "
            f"{'/'.join(str(r['strict']) for r in data)} | {'/'.join(str(r['oracle']) for r in data)} |")
out = root / f"{args.tag}-summary.json"
out.write_text(json.dumps(summary, indent=1))
print("\n".join(lines))
print(f"\n(json: {out})")
