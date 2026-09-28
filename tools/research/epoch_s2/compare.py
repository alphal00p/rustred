#!/usr/bin/env python
"""epoch-s2 gate 3: natives, domains and edges of epoch vs legacy Ready (and Ordered) per control.

Parses the top level of each result.json properly (the files are pretty-printed with sorted keys
and two-space indentation; the multi-GB `domains` array is skipped line by line, every other
top-level value is decoded with json). Edges are reported with and without self-edges (IMP-16:
self-edges are excluded from every edges gate): epoch self-edges from the result's
`epoch.self_edges`, legacy self-edges counted from the CP5 edge segments of the run's checkpoint
(latest.json; (u32 source, u32 target) pairs after a 32-byte header).

usage: compare.py EPOCH_LABEL READY_LABEL [ORDERED_ROOT] [--out FILE]
"""
import argparse
import json
import re
from array import array
from pathlib import Path

R = Path("/common/dev/rustred/TMP")
TOP = re.compile(r'^  "([^"]+)": ?(.*)$')
KEYS = {"status", "stop_reason", "native_processed_nodes", "scheduled_nodes", "descendant_closure",
        "delegation", "epoch", "publication_policy"}


def top_level(path, keys=KEYS):
    """The requested top-level values of a pretty-printed result.json."""
    out, current, buffer = {}, None, []

    def close():
        if current is not None:
            text = "\n".join(buffer).strip()
            if text.endswith(","):
                text = text[:-1]
            out[current] = json.loads(text)

    with open(path, encoding="utf-8") as handle:
        for line in handle:
            if line.startswith('  "'):
                match = TOP.match(line.rstrip("\n"))
                if match:
                    close()
                    current = match.group(1) if match.group(1) in keys else None
                    buffer = [match.group(2)] if current is not None else []
                    continue
            if line.startswith("}"):
                break
            if current is not None:
                buffer.append(line.rstrip("\n"))
    close()
    return out


def cp5_self_edges(checkpoint):
    """Self-edges in the CP5 dependency-edge segments of a legacy run (None without a checkpoint)."""
    latest = checkpoint / "latest.json"
    if not latest.is_file():
        return None
    manifest = json.loads(latest.read_text())
    count = 0
    for segment in manifest["sections"]["edges"]["segments"]:
        data = (checkpoint / segment["file"]).read_bytes()[32:]
        pairs = array("I")
        pairs.frombytes(data)
        count += sum(1 for i in range(0, len(pairs), 2) if pairs[i] == pairs[i + 1])
    return count


def row(directory):
    path = directory / "result.json"
    if not path.exists():
        return {}
    values = top_level(path)
    edges = values.get("descendant_closure", {}).get("dependency_edges")
    epoch = values.get("epoch") or {}
    if epoch:
        self_edges = epoch.get("self_edges")
    else:
        self_edges = cp5_self_edges(directory / "checkpoint")
    return {
        "status": values.get("status"),
        "stop_reason": values.get("stop_reason"),
        "natives": values.get("native_processed_nodes"),
        "domains": values.get("scheduled_nodes"),
        "edges": edges,
        "self_edges": self_edges,
        "edges_excl_self": None if edges is None or self_edges is None else edges - self_edges,
        "aliases": values.get("delegation", {}).get("delegated_publications"),
        "antichain_folded": (epoch.get("counters") or {}).get("antichain_folded"),
    }


def pct(a, b):
    try:
        return round(100.0 * (float(a) - float(b)) / float(b), 2)
    except (TypeError, ValueError, ZeroDivisionError):
        return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("epoch")
    parser.add_argument("ready")
    parser.add_argument("ordered", nargs="?", default="fable51-controls/int-ref-g3")
    parser.add_argument("--families", default="fg,bmw,h,x,four-all,four-all-p5,five-finite")
    parser.add_argument("--epoch-root", default="epoch-s2/runs")
    parser.add_argument("--out")
    args = parser.parse_args()
    table = []
    for family in args.families.split(","):
        e = row(R / args.epoch_root / args.epoch / family)
        if not e:
            continue
        r = row(R / "epoch-s2/runs" / args.ready / family)
        o = row(R / args.ordered / family)
        entry = {"family": family, "epoch_run": str(R / args.epoch_root / args.epoch / family),
                 "ready_run": str(R / "epoch-s2/runs" / args.ready / family),
                 "ordered_run": str(R / args.ordered / family)}
        for name in ("natives", "domains", "edges", "edges_excl_self", "self_edges", "aliases"):
            entry[name] = {"epoch": e.get(name), "ready": r.get(name), "ordered": o.get(name),
                           "epoch_vs_ready_pct": pct(e.get(name), r.get(name)),
                           "epoch_vs_ordered_pct": pct(e.get(name), o.get(name))}
        entry["antichain_folded"] = e.get("antichain_folded")
        entry["status"] = {"epoch": e.get("status"), "ready": r.get("status"), "ordered": o.get("status")}
        entry["stop_reason"] = {"epoch": e.get("stop_reason"), "ready": r.get("stop_reason"),
                                "ordered": o.get("stop_reason")}
        table.append(entry)
    text = json.dumps(table, indent=1)
    if args.out:
        Path(args.out).write_text(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
