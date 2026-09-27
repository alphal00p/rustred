"""Read-only scan of CP5 records-*.jsonl segments (v2 campaign) -> inspection cost distribution.

Usage: python records_scan.py OUT.json FILE [FILE...]
Splits each file into byte ranges, parses native_inspection records, aggregates.
"""
import heapq
import json
import math
import os
import sys
from multiprocessing import Pool

APPLY_STATS = [
    "successors", "events", "term_visits", "zero_terms", "coalescing_additions", "cancelled_groups",
    "shift_groups", "boundary_cells", "sign_splits", "selected_pieces", "native_operations",
    "conditional_successors", "same_support_successors", "strict_subsupport_successors", "problems",
    "zero_sector_groups", "application_refinement_cells", "optional_coefficient_refusals",
]
MATCH_STATS = ["cells", "coordinate_cells", "pieces", "predicates", "rules", "split_operations", "terminal_checks"]
ROUTE_STATS = ["apply_domains", "route_domains", "masks_examined", "masks_pruned", "coordinate_cells", "events"]


def sbin(x):  # quarter-decade bins of seconds
    if x <= 0:
        return -99
    return math.floor(4 * math.log10(x))


def cbin(x):  # log2 bins of counts
    return -1 if x <= 0 else int(math.log2(x))


def new_acc():
    return {
        "delegated": {},
        "phase": {},
        "sec_hist": {},
        "succ_hist": {},
        "owner": {},
        "rank": {},
        "apow": {},
        "top": [],
        "lines": 0,
        "bad": 0,
    }


def add(d, k, vals):
    cur = d.get(k)
    if cur is None:
        d[k] = list(vals)
    else:
        for i, v in enumerate(vals):
            cur[i] += v


def work(args):
    path, start, end, seg = args
    acc = new_acc()
    top = []
    with open(path, "rb") as f:
        f.seek(start)
        if start:
            f.readline()
        pos = f.tell()
        while pos <= end:
            line = f.readline()
            if not line:
                break
            pos += len(line)
            acc["lines"] += 1
            if b'"record_kind":"native_inspection"' not in line:
                if b'delegated' in line:
                    ph = "Apply" if b'"phase":"Apply"' in line else "Route"
                    acc["delegated"][ph] = acc["delegated"].get(ph, 0) + 1
                continue
            try:
                r = json.loads(line)
            except Exception:
                acc["bad"] += 1
                continue
            ph = r["phase"]
            s = r.get("seconds") or 0.0
            st = r.get("stats") or {}
            ev = r.get("accepted_events", 0)
            partial = 0
            if ph == "Apply":
                vec = [1, s, ev] + [st.get(k, 0) for k in APPLY_STATS] + [st.get("matching", {}).get(k, 0) for k in MATCH_STATS] + [partial]
                succ = st.get("successors", 0)
            else:
                vec = [1, s, ev] + [st.get(k, 0) for k in ROUTE_STATS]
                succ = ev
            add(acc["phase"], f"{seg}|{ph}", vec)
            add(acc["phase"], f"ALL|{ph}", vec)
            add(acc["sec_hist"], f"{ph}|{sbin(s)}", [1, s, succ, ev])
            add(acc["succ_hist"], f"{ph}|{cbin(succ)}", [1, s, succ, ev])
            if ph == "Apply":
                add(acc["owner"], r["owner"], [1, s, succ, ev, st.get("term_visits", 0)])
                add(acc["rank"], str(r.get("rank")), [1, s, succ, ev])
                pb = r.get("power_bounds") or {}
                add(acc["apow"], str(pb.get("max_positive_power")), [1, s, succ, ev])
            item = (s, r.get("id"), seg, ph, r.get("owner"), r.get("rank"), r.get("power_bounds"), r.get("lower"), r.get("upper"), ev,
                    {k: st.get(k) for k in ("successors", "term_visits", "zero_terms", "selected_pieces", "shift_groups", "boundary_cells",
                                            "native_operations", "conditional_successors", "same_support_successors")},
                    (st.get("matching") or {}).get("cells"))
            if len(top) < 400:
                heapq.heappush(top, item)
            elif s > top[0][0]:
                heapq.heapreplace(top, item)
    acc["top"] = top
    return acc


def merge(a, b):
    for key in ("delegated",):
        for k, v in b[key].items():
            a[key][k] = a[key].get(k, 0) + v
    for key in ("phase", "sec_hist", "succ_hist", "owner", "rank", "apow"):
        for k, v in b[key].items():
            add(a[key], k, v)
    a["lines"] += b["lines"]
    a["bad"] += b["bad"]
    a["top"] = heapq.nlargest(400, a["top"] + b["top"], key=lambda t: t[0])


def main():
    out = sys.argv[1]
    files = sys.argv[2:]
    tasks = []
    for path in files:
        size = os.path.getsize(path)
        seg = os.path.basename(path)
        n = max(1, size // (256 << 20))
        step = size // n
        for i in range(n):
            s = i * step
            e = size if i == n - 1 else (i + 1) * step
            tasks.append((path, s, e - 1 if i < n - 1 else e, seg))
    acc = new_acc()
    with Pool(int(os.environ.get("NPROC", "32"))) as pool:
        for part in pool.imap_unordered(work, tasks):
            merge(acc, part)
    acc["apply_stat_names"] = ["count", "seconds", "accepted_events"] + APPLY_STATS + ["m_" + k for k in MATCH_STATS] + ["partial"]
    acc["route_stat_names"] = ["count", "seconds", "accepted_events"] + ROUTE_STATS
    acc["top"] = [list(t) for t in sorted(acc["top"], key=lambda t: -t[0])]
    with open(out, "w") as f:
        json.dump(acc, f)
    print("lines", acc["lines"], "bad", acc["bad"])


if __name__ == "__main__":
    main()
