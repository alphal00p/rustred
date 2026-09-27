#!/usr/bin/env python
"""Markdown tables for the W0.8 RESULTS.md from knob run directories.

  results_tables.py order  --reference ARM LABEL_GLOB...   # order A/B table
  results_tables.py build  --reference ARM LABEL_GLOB...   # build A/B table

Order table per family and arm: natives, mistakes/native, transfers/native,
peak unreserved, peak pending, inspector CPU, root AUC (over the reference
arm's inspector CPU), cpu50, wall; each with the ratio of means to the
reference arm. Build table: inspector CPU ms/native mean [min..max] and ratio,
traversal, foreign load, peak RSS, n.

Ratios use the reference arm's runs from the same session(s) as the arm's
runs (session_map.py; sessions/<name>.log). An arm whose runs come from
several sessions is compared with the reference runs of those sessions
pooled; the build table also prints the per-session ratios and whether
every arm run beat every same-session reference run ("disjoint"). A cell
marked "pooled ref" had no shared session and falls back to all reference
runs.
"""
import argparse
import fnmatch
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import session_map  # noqa: E402
import summarize  # noqa: E402
from ab_table import arm_of  # noqa: E402


def collect(runs_dir, globs, reference, sessions_dir=session_map.SESSIONS):
    smap = session_map.load(sessions_dir)
    runs = sorted(r for r in Path(runs_dir).glob("*/*") if (r / "metrics.json").exists()
                  and any(fnmatch.fnmatch(r.parent.name, g) for g in globs))
    horizon = defaultdict(list)
    for r in runs:
        if arm_of(r.parent.name) == reference:
            m = json.loads((r / "metrics.json").read_text())
            if m.get("inspector_cpu_seconds"):
                horizon[r.name].append(m["inspector_cpu_seconds"])
    groups = defaultdict(list)
    for r in runs:
        h = statistics.mean(horizon[r.name]) if horizon.get(r.name) else None
        row = summarize.row(r, h)
        row["session"] = smap.get((r.parent.name, r.name))
        groups[(r.name, arm_of(r.parent.name))].append(row)
    return groups


def mean(rows, key):
    vals = [r[key] for r in rows if isinstance(r.get(key), (int, float))]
    return (statistics.mean(vals), min(vals), max(vals), len(vals)) if vals else None


def cell(rows, ref, key, fmt="{:.4g}", ratio=True, span=False):
    m = mean(rows, key)
    if m is None:
        return "-"
    text = fmt.format(m[0])
    if span and m[3] > 1:
        text += " [" + fmt.format(m[1]) + ".." + fmt.format(m[2]) + "]"
    if ratio and ref and rows is not ref:
        ref, pooled = session_map.same_session(rows, ref)
        r = mean(ref, key)
        if r and r[0]:
            text += f" (x{m[0] / r[0]:.3f}" + (", pooled ref)" if pooled else ")")
    return text


def session_detail(rows, ref, key):
    """Same-session reference n, per-session ratios, disjointness."""
    if not ref or rows is ref:
        sessions = sorted({str(r.get("session")) for r in rows})
        return f"{len(rows)} ({', '.join(sessions)})", "-", "-"
    same, pooled = session_map.same_session(rows, ref)
    per = []
    for s in sorted({r.get("session") for r in rows}, key=str):
        a = mean([r for r in rows if r.get("session") == s], key)
        b = mean([r for r in ref if r.get("session") == s], key)
        if a and b and b[0]:
            per.append(f"{s}: x{a[0] / b[0]:.3f} ({a[3]} vs {b[3]})")
    m, r = mean(rows, key), mean(same, key)
    disjoint = "-"
    if m and r:
        disjoint = "yes" if m[2] < r[1] else ("yes (slower)" if m[1] > r[2] else "no")
    return (f"{len(same)}" + (" pooled" if pooled else ""), "; ".join(per) or "-", disjoint)


def order_table(groups, reference):
    families = sorted({f for f, _ in groups})
    out = []
    for family in families:
        ref = groups.get((family, reference), [])
        out.append(f"\n**{family}** (reference `{reference}`)\n")
        out.append("| arm | n | exit | frontiers | natives | mistakes/native | transfers/native | peak unreserved | peak pending | inspector CPU s | root AUC | cpu50 s | wall s | foreign load |")
        out.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
        for (fam, arm), rows in sorted(groups.items()):
            if fam != family:
                continue
            exits = ",".join(str(r["exit"]) + ("T" if r.get("timed_out") else "") for r in rows)
            fr = ",".join(str(r["frontiers"]) for r in rows)
            out.append("| " + " | ".join([
                arm, str(len(rows)), exits, fr,
                cell(rows, ref, "natives", "{:.0f}"),
                cell(rows, ref, "mistakes_per_native", "{:.4f}"),
                cell(rows, ref, "transfers_per_native", "{:.4f}"),
                cell(rows, ref, "peak_unreserved", "{:.0f}"),
                cell(rows, ref, "peak_pending", "{:.0f}"),
                cell(rows, ref, "inspector_cpu", "{:.1f}"),
                cell(rows, ref, "auc", "{:.3f}"),
                cell(rows, ref, "cpu50", "{:.1f}"),
                cell(rows, ref, "wall", "{:.1f}"),
                cell(rows, None, "foreign_load", "{:.2f}", ratio=False),
            ]) + " |")
    return "\n".join(out)


def build_table(groups, reference):
    out = ["| family | arm | n | inspector CPU ms/native mean [min..max] (ratio to same-session reference) |"
           " same-session reference n | per-session ratio (arm n vs ref n) | disjoint |"
           " traversal s | foreign load | peak RSS GB |",
           "|---|---|---|---|---|---|---|---|---|---|"]
    for (fam, arm), rows in sorted(groups.items()):
        ref = groups.get((fam, reference), [])
        ref_n, per, disjoint = session_detail(rows, ref, "inspector_ms_per_native")
        out.append("| " + " | ".join([
            fam, arm, str(len(rows)),
            cell(rows, ref, "inspector_ms_per_native", "{:.4f}", span=True),
            ref_n, per, disjoint,
            cell(rows, ref, "traversal", "{:.1f}"),
            cell(rows, None, "foreign_load", "{:.2f}", ratio=False),
            cell(rows, ref, "peak_rss_gb", "{:.2f}"),
        ]) + " |")
    return "\n".join(out)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("kind", choices=("order", "build"))
    p.add_argument("--runs", default=str(summarize.RUNS))
    p.add_argument("--sessions", default=str(session_map.SESSIONS))
    p.add_argument("--reference", required=True)
    p.add_argument("labels", nargs="+")
    args = p.parse_args()
    groups = collect(args.runs, args.labels, args.reference, args.sessions)
    print(order_table(groups, args.reference) if args.kind == "order"
          else build_table(groups, args.reference))


if __name__ == "__main__":
    main()
