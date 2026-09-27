"""Read-only scan of a pretty-printed owner-domain-walk result.json 'domains' array.
Usage: python pilot_scan.py OUT.json RESULT.json
"""
import heapq
import json
import math
import os
import re
import sys
from multiprocessing import Pool

KEYS8 = {"successors", "events", "term_visits", "zero_terms", "shift_groups", "boundary_cells", "selected_pieces",
         "native_operations", "conditional_successors", "same_support_successors", "strict_subsupport_successors",
         "sign_splits", "coalescing_additions", "zero_sector_groups", "apply_domains", "route_domains", "masks_examined"}
KEYS10 = {"cells", "coordinate_cells", "pieces", "predicates", "rules", "split_operations", "terminal_checks"}
NAMES = ["count", "seconds", "accepted"] + sorted(KEYS8) + ["m_" + k for k in sorted(KEYS10)]
IDX = {n: i for i, n in enumerate(NAMES)}
pat = re.compile(rb'^( *)"([a-z_]+)": ?(.*?),?\n$')


def sbin(x):
    return -99 if x <= 0 else math.floor(4 * math.log10(x))


def cbin(x):
    return -1 if x <= 0 else int(math.log2(x))


def add(d, k, vals):
    cur = d.get(k)
    if cur is None:
        d[k] = list(vals)
    else:
        for i, v in enumerate(vals):
            cur[i] += v


def work(args):
    path, start, end = args
    acc = {"phase": {}, "sec_hist": {}, "succ_hist": {}, "rank": {}, "top": [], "records": 0}
    top = []
    with open(path, "rb") as f:
        f.seek(start)
        if start:
            f.readline()
        pos = f.tell()
        # advance to a record start
        rec = None
        while True:
            line = f.readline()
            if not line:
                break
            pos += len(line)
            if line == b"    {\n":
                if pos - len(line) > end:
                    break
                rec = {"_v": [0] * len(NAMES)}
                sect = None
                continue
            if rec is None:
                continue
            if line.startswith(b"    }"):
                v = rec["_v"]
                ph = rec.get("phase", "?")
                if rec.get("kind") == "native_inspection":
                    v[0] = 1
                    s = v[1]
                    succ = v[IDX["successors"]] if ph == "Apply" else v[IDX["accepted"]]
                    add(acc["phase"], ph, v)
                    add(acc["sec_hist"], f"{ph}|{sbin(s)}", [1, s, succ])
                    add(acc["succ_hist"], f"{ph}|{cbin(succ)}", [1, s, succ])
                    if ph == "Apply":
                        add(acc["rank"], str(rec.get("rank")), [1, s, succ])
                    item = (s, rec.get("id"), ph, rec.get("rank"), rec.get("apow"), succ, v[IDX["term_visits"]], v[IDX["selected_pieces"]])
                    if len(top) < 200:
                        heapq.heappush(top, item)
                    elif s > top[0][0]:
                        heapq.heapreplace(top, item)
                    acc["records"] += 1
                else:
                    add(acc["phase"], "other:" + str(rec.get("kind")), [1])
                rec = None
                continue
            m = pat.match(line)
            if not m:
                continue
            ind = len(m.group(1))
            key = m.group(2).decode()
            val = m.group(3)
            if ind == 6:
                sect = key
                if key == "phase":
                    rec["phase"] = val.strip(b'"').decode()
                elif key == "record_kind":
                    rec["kind"] = val.strip(b'"').decode()
                elif key == "seconds":
                    rec["_v"][1] = float(val)
                elif key == "accepted_events":
                    rec["_v"][2] = int(val)
                elif key == "rank":
                    rec["rank"] = val.decode()
                elif key == "id":
                    rec["id"] = val.decode()
            elif ind == 8 and sect == "stats" and key in KEYS8:
                rec["_v"][IDX[key]] = int(val)
            elif ind == 8 and sect == "power_bounds" and key == "max_positive_power":
                rec["apow"] = val.decode()
            elif ind == 10 and sect == "stats" and key in KEYS10:
                rec["_v"][IDX["m_" + key]] = int(val)
    acc["top"] = top
    return acc


def main():
    out, path = sys.argv[1], sys.argv[2]
    size = os.path.getsize(path)
    n = max(1, size // (128 << 20))
    step = size // n
    tasks = [(path, i * step, size if i == n - 1 else (i + 1) * step - 1) for i in range(n)]
    acc = {"phase": {}, "sec_hist": {}, "succ_hist": {}, "rank": {}, "top": [], "records": 0}
    with Pool(int(os.environ.get("NPROC", "32"))) as pool:
        for part in pool.imap_unordered(work, tasks):
            for key in ("phase", "sec_hist", "succ_hist", "rank"):
                for k, v in part[key].items():
                    add(acc[key], k, v)
            acc["records"] += part["records"]
            acc["top"] = heapq.nlargest(200, acc["top"] + part["top"], key=lambda t: t[0])
    acc["names"] = NAMES
    json.dump(acc, open(out, "w"))
    print("records", acc["records"])


if __name__ == "__main__":
    main()
