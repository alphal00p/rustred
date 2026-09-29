#!/usr/bin/env python
"""epoch-s2: collect the gate verdicts of one fix-round tag from the run logs into one JSON.

usage: gate_summary.py TAG [--out FILE]
Reads TMP/epoch-s2/runs/{controls.log, fixround-TAG.log, gate4-TAG/identity.log, cp5-resume-TAG/log,
identity-TAG-*.json} and TMP/epoch-s2/gate3-TAG-*.json; every verdict is taken from the mechanical lines those
scripts write (ORACLE ... PASS/FAIL, GATE4 ..., GATE cp5-resume-..., "identical": true/false).
"""
import json
import re
import sys
from pathlib import Path

S = Path("/common/dev/rustred/TMP/epoch-s2")
RUNS = S / "runs"


def main():
    tag = sys.argv[1]
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else None
    summary = {"tag": tag, "oracle": {}, "identity": {}, "gate4": {}, "cp5_resume": {}, "gate3": {}}
    oracle = re.compile(r"ORACLE (\S+) (PASS|FAIL) verdict=(\S*) .* roots=(\S*) wall=(\S*) bin=(\S+)")
    for line in (RUNS / "controls.log").read_text().splitlines():
        match = oracle.search(line)
        if match and f"/{tag}-" in match.group(1):
            run = match.group(1).split("/runs/")[-1]
            summary["oracle"][run] = {"gate": match.group(2), "verdict": match.group(3),
                                      "roots": match.group(4), "wall": match.group(5), "binary": match.group(6)}
    for path in sorted(RUNS.glob(f"identity-{tag}-*.json")):
        report = json.loads(path.read_text())
        summary["identity"][path.stem] = {"identical": report["identical"], "records": report.get("records"),
                                          "lockstep_b": report.get("lockstep_b"), "workers": report.get("workers"),
                                          "differences": report["differences"][:5], "receipt": str(path)}
    log = RUNS / f"gate4-{tag}" / "identity.log"
    if log.exists():
        for line in log.read_text().splitlines():
            match = re.search(r"GATE4 (\S+) (PASS|FAIL) (.*)", line)
            if match:
                summary["gate4"][match.group(1)] = {"gate": match.group(2), "detail": match.group(3)}
    log = RUNS / f"cp5-resume-{tag}" / "log"
    if log.exists():
        for line in log.read_text().splitlines():
            match = re.search(r"GATE (cp5-resume-\S+) (PASS|FAIL) (.*)", line)
            if match:
                summary["cp5_resume"][match.group(1)] = {"gate": match.group(2), "detail": match.group(3)}
    for path in sorted(S.glob(f"gate3-{tag}-*.json")):
        summary["gate3"][path.stem] = json.loads(path.read_text())
    text = json.dumps(summary, indent=1)
    if out:
        Path(out).write_text(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
