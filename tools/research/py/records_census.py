#!/usr/bin/env python
"""Read-only census of CP5 record segments: inspection cost vs lattice volume.

Usage: records_census.py FILE [MAX_BYTES]
"""
import json
import sys
from collections import defaultdict

SCR = "/tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad"
CLASS = {}
for line in open(f"{SCR}/owner_class.txt"):
    m, c = line.split()
    CLASS[m] = c

INF = None


def dist(bounds, cap):
    """Counts of sums of independent coordinate ranges, truncated at cap.
    bounds: list of (lo, hi or None). Returns list counts[s] for s<=cap, or None if cap None and unbounded."""
    if cap is None:
        if any(hi is None for lo, hi in bounds):
            return None
        cap = sum(hi for lo, hi in bounds)
    counts = [0] * (cap + 1)
    counts[0] = 1
    for lo, hi in bounds:
        hi2 = cap if hi is None else min(hi, cap)
        new = [0] * (cap + 1)
        if lo > hi2:
            return [0] * (cap + 1)
        # prefix sums
        pre = [0] * (cap + 2)
        for s in range(cap + 1):
            pre[s + 1] = pre[s] + counts[s]
        for s in range(cap + 1):
            a = s - hi2
            b = s - lo
            if b < 0:
                continue
            a = max(a, 0)
            new[s] = pre[b + 1] - pre[a]
        counts = new
    return counts


def points(owner, lower, upper, rank, pb):
    act = [(lower[i], upper[i]) for i in range(len(owner)) if owner[i] == "1"]
    ina = [(lower[i], upper[i]) for i in range(len(owner)) if owner[i] != "1"]
    t = len(act)
    amax = pb.get("max_positive_power")
    dmin = pb.get("min_power_difference")
    dmax = pb.get("max_power_difference")
    acap = None if amax is None else amax - t  # local sum cap
    rcap = rank
    # D = A - R; if dmax given and A unbounded, R >= A - dmax... handle simple cases
    if acap is None and dmax is not None and rcap is not None:
        acap = rcap + dmax - t
    if rcap is None and dmin is not None and acap is not None:
        rcap = acap + t - dmin
    if acap is not None and acap < 0:
        return 0
    da = dist(act, acap)
    dr = dist(ina, rcap)
    if da is None or dr is None:
        return None
    total = 0
    for sa, ca in enumerate(da):
        if ca == 0:
            continue
        A = sa + t
        for sr, cr in enumerate(dr):
            if cr == 0:
                continue
            D = A - sr
            if dmin is not None and D < dmin:
                continue
            if dmax is not None and D > dmax:
                continue
            total += ca * cr
    return total


def main():
    path = sys.argv[1]
    max_bytes = int(sys.argv[2]) if len(sys.argv) > 2 else None
    agg = defaultdict(lambda: defaultdict(float))
    per_owner = defaultdict(lambda: defaultdict(float))
    hist_ratio = defaultdict(lambda: defaultdict(float))  # succ/points bucket -> seconds
    hist_points = defaultdict(lambda: defaultdict(float))
    rank_hist = defaultdict(lambda: defaultdict(float))
    fixed_hist = defaultdict(float)
    read = 0
    n = 0
    with open(path, "rb") as fh:
        for raw in fh:
            read += len(raw)
            if max_bytes and read > max_bytes:
                break
            r = json.loads(raw)
            kind = r.get("record_kind")
            if kind != "native_inspection":
                agg[kind]["count"] += 1
                continue
            n += 1
            ph = r["phase"]
            owner = r["owner"]
            cls = CLASS.get(owner, "uninstalled") if ph == "Apply" else "route"
            key = (ph, cls)
            s = r["stats"]
            sec = r["seconds"] or 0.0
            succ = s.get("successors", s.get("events", 0))
            if ph == "Route":
                succ = s.get("apply_domains", 0) + s.get("route_domains", 0)
            pts = points(owner, r["lower"], r["upper"], r["rank"], r["power_bounds"] or {})
            a = agg[key]
            a["count"] += 1
            a["seconds"] += sec
            a["successors"] += succ
            a["events"] += r.get("accepted_events", 0)
            if ph == "Apply":
                a["pieces"] += s["matching"]["pieces"]
                a["selected"] += s["selected_pieces"]
                a["shift_groups"] += s["shift_groups"]
                a["same_support"] += s["same_support_successors"]
                a["subsupport"] += s["strict_subsupport_successors"]
            if pts is None:
                a["inf_count"] += 1
                a["inf_seconds"] += sec
                a["inf_succ"] += succ
            else:
                a["points"] += pts
                a["fin_seconds"] += sec
                a["fin_succ"] += succ
                if ph == "Apply":
                    ratio = succ / pts if pts else float("inf")
                    b = "0" if pts == 0 else ("<0.1" if ratio < 0.1 else "<0.5" if ratio < 0.5 else "<1" if ratio < 1 else "<2" if ratio < 2 else "<5" if ratio < 5 else ">=5")
                    hist_ratio[cls][b] += sec
                    hist_ratio[cls][b + "#"] += 1
                    pb = "0" if pts == 0 else "1" if pts == 1 else "<=10" if pts <= 10 else "<=100" if pts <= 100 else "<=1e3" if pts <= 1000 else "<=1e4" if pts <= 10**4 else "<=1e5" if pts <= 10**5 else "<=1e6" if pts <= 10**6 else ">1e6"
                    hist_points[cls][pb] += sec
                    hist_points[cls][pb + "#"] += 1
                    hist_points[cls][pb + "succ"] += succ
            if ph == "Apply":
                po = per_owner[owner]
                po["count"] += 1
                po["seconds"] += sec
                po["successors"] += succ
                rank_hist[cls][r["rank"]] += sec
                rank_hist[cls][("n", r["rank"])] += 1
                nfixed = sum(1 for lo, hi in zip(r["lower"], r["upper"]) if hi is not None and lo == hi)
                fixed_hist[nfixed] += 1
    out = {
        "file": path,
        "bytes_read": read,
        "native": n,
        "agg": {f"{k[0]}|{k[1]}" if isinstance(k, tuple) else str(k): dict(v) for k, v in agg.items()},
        "ratio_hist_seconds": {k: dict(v) for k, v in hist_ratio.items()},
        "points_hist": {k: dict(v) for k, v in hist_points.items()},
        "rank_hist": {k: {str(kk): vv for kk, vv in v.items()} for k, v in rank_hist.items()},
        "fixed_coords_hist": {str(k): v for k, v in sorted(fixed_hist.items())},
        "top_owners": sorted(
            [(o, CLASS.get(o, "?"), dict(v)) for o, v in per_owner.items()],
            key=lambda x: -x[2]["seconds"],
        )[:20],
    }
    print(json.dumps(out, indent=1, default=str))


if __name__ == "__main__":
    main()
