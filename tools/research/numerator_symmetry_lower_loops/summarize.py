#!/usr/bin/env python3
"""Check reproducibility and source immutability of the two native trials."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def resources(path: Path) -> dict:
    values = {}
    for line in path.read_text().splitlines():
        for prefix, key in (
            ("User time (seconds):", "user_s"),
            ("System time (seconds):", "system_s"),
            ("Maximum resident set size (kbytes):", "peak_RSS_KiB"),
        ):
            if line.strip().startswith(prefix):
                values[key] = float(line.strip().removeprefix(prefix))
    values["CPU_s"] = values["user_s"] + values["system_s"]
    return values


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    args = parser.parse_args()
    root = args.evidence.resolve(strict=True)
    manifest = json.loads((root / "inputs/manifest.json").read_text())
    inputs = []
    for entry in manifest["files"]:
        for kind in ("source", "prepared"):
            path = Path(entry[kind])
            inputs.append({"path": str(path), "unchanged": sha(path) == entry[kind + "_sha256"]})
    if not all(item["unchanged"] for item in inputs):
        raise SystemExit("A pinned source or prepared input changed")
    results = []
    for case in ("k6", "h", "fg", "bmw", "x"):
        trials = []
        for repeat in (1, 2):
            directory = root / f"run{repeat}"
            receipt = json.loads((directory / f"{case}.receipt.json").read_text())
            if receipt["returncode"]:
                raise SystemExit(f"{case} repeat {repeat} did not complete")
            summary = json.loads((directory / case / "summary.json").read_text())
            if any(stage["ordinary_source_rows"] or stage["unreduced_equations"] for stage in summary["stages"]):
                raise SystemExit(f"{case} repeat {repeat} has unisolated or incomplete source work")
            trials.append({"receipt": receipt, "summary": summary, "resources": resources(directory / f"{case}.time")})
        binary_files = ["baseline.rrbin"] + [f"rank-{rank}-{kind}.rrbin" for rank in range(3) for kind in ("provider", "session")]
        json_files = ["baseline-maps.json"] + [f"rank-{rank}-maps.json" for rank in range(3)]
        checks = [
            {"file": file, "identical": sha(root / "run1" / case / file) == sha(root / "run2" / case / file)}
            for file in binary_files + json_files
        ]
        if not all(check["identical"] for check in checks if check["file"].endswith(".json")):
            raise SystemExit(f"{case} exact canonical display maps differ between repeats")
        results.append({"case": case, "trials": trials, "determinism": checks})
    combined = [json.loads((root / f"run{repeat}/combined/summary.json").read_text()) for repeat in (1, 2)]
    if combined[0] != combined[1]:
        raise SystemExit("The combined exact-alias result differs between repeats")
    report = {
        "schema": "rustred.lower-loop-symmetry-results.v1",
        "inputs": inputs,
        "all_sources_unchanged": True,
        "cases": results,
        "combined": combined[0],
        "comparison_scope": "same saved native programs; SYM only; zero ordinary source rows",
        "master_minimality_claim": False,
    }
    with (root / "reproducibility.json").open("x") as stream:
        json.dump(report, stream, indent=2)
    for result in results:
        summaries = [trial["summary"] for trial in result["trials"]]
        wall = [trial["receipt"]["supervised_wall_s"] for trial in result["trials"]]
        cpu = [trial["resources"]["CPU_s"] for trial in result["trials"]]
        rss = [trial["resources"]["peak_RSS_KiB"] / 1024 for trial in result["trials"]]
        print(json.dumps({"case":result["case"],"raw":summaries[0]["raw_keys"],"before":summaries[0]["baseline_basis"],
            "after_R1":summaries[0]["stages"][1]["after"],"after_R2":summaries[0]["stages"][2]["after"],
            "wall_s":wall,"CPU_s":cpu,"peak_RSS_MiB":rss,"all_native_bytes_identical":all(c["identical"] for c in result["determinism"])}))
    print(json.dumps({"all_sources_unchanged": True, "combined": combined[0]}))


if __name__ == "__main__":
    main()
