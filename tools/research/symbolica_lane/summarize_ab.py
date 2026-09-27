#!/usr/bin/env python
"""Summarize native_seconds.py output for an ab_controls.py run tree.

Groups rows by (family, label) from the path .../<family>/<label>-r<k>/result.json
and prints, per family, each label's repeats and the relative difference of the
means against the reference label.

Usage: summarize_ab.py NATIVE_SECONDS.json [--ref ref] [--markdown]
"""
import argparse
import json
from collections import defaultdict
from pathlib import Path

FIELDS = (
    ("ms_per_native", "inspector ms/native"),
    ("process_cpu_ms_per_native", "process CPU ms/native"),
    ("traversal_seconds", "traversal s"),
    ("whole_command_seconds", "whole command s"),
    ("process_cpu_seconds", "process CPU s"),
    ("peak_rss_bytes", "peak RSS GB"),
)


def num(value):
    if value is None:
        return None
    return float(value)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("rows", type=Path)
    p.add_argument("--ref", default="ref")
    p.add_argument("--markdown", action="store_true")
    args = p.parse_args()
    rows = json.load(open(args.rows))
    groups = defaultdict(list)
    for row in rows:
        run_dir = Path(row["path"]).parent
        family = run_dir.parent.name
        label = run_dir.name.rsplit("-r", 1)[0]
        groups[(family, label)].append(row)
    families = sorted({f for f, _ in groups})
    labels = sorted({l for _, l in groups}, key=lambda l: (l != args.ref, l))
    out = []
    if args.markdown:
        out.append("| family | label | natives | " + " | ".join(n for _, n in FIELDS) + " |")
        out.append("|---|---|---:|" + "---:|" * len(FIELDS))
    for family in families:
        ref_means = {}
        for label in labels:
            reps = groups.get((family, label))
            if not reps:
                continue
            natives = {sum(r["native_inspections"].values()) for r in reps}
            cells = []
            for key, _ in FIELDS:
                vals = [num(r.get(key)) for r in reps if r.get(key) is not None]
                if not vals:
                    cells.append("-")
                    continue
                if key == "peak_rss_bytes":
                    vals = [v / 1e9 for v in vals]
                mean = sum(vals) / len(vals)
                text = " / ".join(f"{v:.4g}" for v in vals)
                if label == args.ref:
                    ref_means[key] = mean
                elif key in ref_means and ref_means[key]:
                    text += f" ({100 * (mean / ref_means[key] - 1):+.1f}%)"
                cells.append(text)
            nat = ",".join(str(n) for n in sorted(natives))
            if args.markdown:
                out.append(f"| {family} | {label} | {nat} | " + " | ".join(cells) + " |")
            else:
                out.append(f"{family:12s} {label:8s} natives={nat} " + "  ".join(
                    f"{name}={cell}" for (_, name), cell in zip(FIELDS, cells)))
    print("\n".join(out))


if __name__ == "__main__":
    main()
