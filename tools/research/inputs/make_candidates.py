#!/usr/bin/env python3
"""Build the W0.6 candidate helper sets with the entry-query planner.

Candidates (all keep the 116 physics roots of plan-v3 unchanged):
  v3       plan-v3 regression: A_max helpers on every owner with t >= 8
  I1       hybrid: owners in L* get the interim helper shape (largest root
           rank, A unbounded); every other owner keeps its plan-v3 helper
  I1b      envelope-sized bounded helpers for owners outside L*: rank and
           A_max raised to the observed per-owner envelope plus a margin;
           L* owners keep their plan-v3 helper
  I1+I1b   both

Inputs: --lstar (JSON list of L* owner masks), --envelope (cp5hop
envelope.tsv of the reference checkpoint: v2 generation 7), --margin-a,
--margin-r. Each candidate gets `helper-bounds.json` (I1b variants) and a
planner output directory; the checker runs on each.
"""
import argparse
import csv
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path("/common/dev/rustred")
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PLANNER = REPO / "examples/python/plan_renormalization_entry_queries.py"
CHECKER = REPO / "examples/python/check_renormalization_entry_queries.py"


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--lstar", type=Path, required=True)
    p.add_argument("--envelope", type=Path, required=True)
    p.add_argument("--margin-a", type=int, default=2)
    p.add_argument("--margin-r", type=int, default=2)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--executable", type=Path, default=ROOT / "TMP/fable51-controls/bin/rustred-4a17f9c7")
    p.add_argument("--selection", type=Path,
                   default=ROOT / "TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved/inputs/selection.json")
    p.add_argument("--only", default="v3,I1,I1b,I1+I1b")
    args = p.parse_args(argv)
    selection = json.loads(args.selection.read_text())
    masks = [o["mask"] for o in selection["owners"]]
    lstar = set(json.loads(args.lstar.read_text()))
    if lstar - set(masks):
        sys.exit("L* has unknown owners")
    t8 = {m for m in masks if m.count("1") >= 8}
    t7_outside = sorted(m for m in masks if m.count("1") < 8 and m not in lstar)
    if t7_outside:
        sys.exit(f"owners with t <= 7 outside L* keep unbounded plan-v3 helpers: refusing {t7_outside}")
    env = {}
    for row in csv.DictReader(open(args.envelope), delimiter="\t"):
        env[row["mask"]] = row
    envelope_bounds = {}
    for m in sorted(set(masks) - lstar):
        e = env[m]
        if int(e["unbounded_A"]) or int(e["unbounded_rank"]):
            sys.exit(f"owner {m} has unbounded A or rank in the reference envelope")
        envelope_bounds[m] = {"max_numerator_rank": int(e["max_finite_rank"]) + args.margin_r,
                              "max_positive_power": int(e["max_finite_A"]) + args.margin_a}
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "lstar.json").write_text(json.dumps(sorted(lstar), indent=1) + "\n")
    specs = {
        "v3": (sorted(t8), None),
        "I1": (sorted(t8 - lstar), None),
        "I1b": (sorted(t8), envelope_bounds),
        "I1+I1b": (sorted(t8 - lstar), envelope_bounds),
    }
    report = {}
    for name in args.only.split(","):
        positive, bounds = specs[name]
        tag = name.replace("+", "-")
        extra = []
        if bounds is not None:
            doc = {"schema": "rustred.helper-bounds.json.v1",
                   "provenance": {"envelope": str(args.envelope), "margin_a": args.margin_a,
                                  "margin_r": args.margin_r,
                                  "rule": "owners outside L*: max finite rank/A over Apply domains + margin"},
                   "owners": bounds}
            path = args.out / f"helper-bounds-{tag}.json"
            path.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n")
            extra = ["--helper-bounds-from", str(path)]
        plan_dir = args.out / f"plan-{tag}"
        command = [sys.executable, str(PLANNER), "--loops", "5", "--manifest", str(args.selection),
                   "--momenta", str(REPO / "examples/input/tide_five_loop_manifest.json"),
                   "--parent-witnesses", str(REPO / "examples/input/tide_five_loop_parent_vertices.json"),
                   "--gauge", "feynman", "--difference-set", "9,10",
                   "--classification", str(REPO / "examples/python/fixtures/tide_five_loop_skeleton_classification.json"),
                   "--helper-positive-power-owners", ",".join(positive), "--executable", str(args.executable),
                   "--output-directory", str(plan_dir), "--quiet"] + extra
        out = subprocess.run(command, capture_output=True, text=True)
        if out.returncode != 0:
            sys.exit(f"planner failed for {name}: {out.stderr}")
        check = subprocess.run([sys.executable, str(CHECKER), "--queries", str(plan_dir / "queries.json"),
                                "--receipt", str(plan_dir / "entry-plan-receipt.json")], capture_output=True, text=True)
        verdict = json.loads(check.stdout)
        report[name] = {"plan": str(plan_dir), "planner": json.loads(out.stdout), "checker_exit": check.returncode,
                        "checker_status": verdict.get("status"), "checker_failures": verdict.get("failures"),
                        "helpers": {q["owner"]: [q["max_numerator_rank"], q["power_bounds"]["max_positive_power"]]
                                    for q in json.loads((plan_dir / "queries.json").read_text())["queries"]
                                    if q["id"].startswith("owner-anchor-")}}
        report[name]["unbounded_A_helpers"] = sum(1 for v in report[name]["helpers"].values() if v[1] is None)
        (plan_dir / "checker.json").write_text(check.stdout)
    (args.out / "candidates.json").write_text(json.dumps(report, indent=1, sort_keys=True) + "\n")
    print(json.dumps({k: (v["checker_status"], v["planner"]["queries_sha256"][:12]) for k, v in report.items()}))


if __name__ == "__main__":
    sys.exit(main())
