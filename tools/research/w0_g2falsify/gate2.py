#!/usr/bin/env python
"""W0 G2' falsifier, fix round: gate table with load-insensitive work metrics (read-only).

usage: gate2.py [RUNS_DIR] [--json OUT.json]

For each control (C-5F Ordered/Ready W24, C-HOT-sub r1a12 Ready W12), binary
session and G2' arm, against the flag-off arm of the same binary and session:

- record_s: sum of native record seconds (wall time per record; the registered
  W4.2 gate metric, includes the G2' plan time);
- run_s: schedstat on-CPU seconds of the walk threads;
- plan_s: G2' plan time inside record_s (anchor scan and point assignment of
  every planned job, found or not; engine report w0_g2_donly);
- uw_owner: useful-work units (handoff 0.1.2): native calls x per-class cost,
  the class being the Apply owner (13 owners on C-5F) or Route, the cost the
  record seconds per native call of that class in the flag-off arm of the same
  session (mean over its repeats). G2' residual inspections are charged the
  full class cost (they are smaller domains, so this over-charges the G2' arm)
  and the G2' plan time is not native work (reported as plan_s);
- uw_class: the same with the three Apply owner classes of the lane
  (000011001001011, 011101110111000, other) and Route;
- instr: perf stat instructions:u of the whole walk process (all threads,
  user space; B4 session only); instr_per_native = instr / (Apply native
  calls + Route natives);
- apply_calls, route_natives, scheduled, peak_pending.

Ratios are ratio of means over the repeats; brackets give the min-max of all
on/off cross ratios; "se" is the delta-method standard error of the ratio of
means from the sample SDs (n small: indicative only).
Registered gate (master plan W4.2): drained, 0 frontiers, audit (and
g2verify) PASS, record_s ratio <= 0.5, scheduled and peak pending ratios <= 1.
"""
import json
import math
import sys
from pathlib import Path

ARGS = [a for a in sys.argv[1:]]
JSON_OUT = None
if "--json" in ARGS:
    JSON_OUT = ARGS[ARGS.index("--json") + 1]
    del ARGS[ARGS.index("--json"):ARGS.index("--json") + 2]
RUNS = Path(ARGS[0] if ARGS else "/common/dev/rustred/TMP/w0/g2falsify/runs")
DUMP = []
CLASSES = ("000011001001011", "011101110111000", "other")
NAME = {"m1": "1 anchor", "m2": "<=2 anchors", "u": "union, any committed anchor (u)",
        "n": "union, full-native anchors only (n)"}
CONTROLS = [
    ("C-5F Ordered W24", "five-finite", "c5f-ord-{arm}-{r}-{sha}"),
    ("C-5F Ready W24", "five-finite", "c5f-rdy-{arm}-{r}-{sha}"),
    ("C-HOT-sub r1a12 Ready W12", "hot", "hotsub-r1a12-{arm}-{r}-{sha}"),
]
SESSIONS = [
    ("b04d00b2", ("m1", "m2", "u"), ("r1", "r2")),
    ("B4", ("u", "n"), ("r1", "r2", "r3", "r4")),
]
KEYS = ("record_s", "run_s", "uw_owner", "uw_class", "instr", "instr_per_native", "apply_calls",
        "route_natives", "scheduled", "peak_pending")


def b4_sha():
    shas = sorted({p.name.rsplit("-", 1)[1] for p in RUNS.glob("c4l-n-r1-*")})
    return shas[-1] if shas else None


def load(d):
    out = {}
    for name in ("metrics", "g2stats-v2", "g2stats", "pending", "audit", "g2verify"):
        p = d / f"{name}.json"
        try:
            out[name] = json.load(open(p)) if p.exists() and p.stat().st_size else {}
        except json.JSONDecodeError:
            out[name] = {}
    if out["g2stats-v2"]:
        # B3 runs: the fix-round g2stats (anchor kinds, per owner) without --union.
        for key in ("g2_anchor_reference_kinds", "apply_by_owner"):
            out["g2stats"].setdefault(key, out["g2stats-v2"].get(key))
    # Runs without post-processing (no g2stats yet) are skipped.
    return out if out["metrics"] and out["g2stats"] else None


def base(r):
    m, g, p = r["metrics"], r["g2stats"], r["pending"]
    cls = g.get("apply_by_owner_class", {})
    rec = m.get("recorder") or {}
    perf = m.get("perf") or {}
    apply_calls = sum(cls[k]["native_calls"] for k in CLASSES) if cls else None
    route = g.get("native_by_phase", {}).get("Route", {}) if g else {}
    instr = perf.get("instructions:u")
    instr = instr if isinstance(instr, float) else None
    natives = (apply_calls or 0) + (route.get("records") or 0)
    return {
        "record_s": sum(v["record_seconds"] for v in g.get("native_by_phase", {}).values()) if g else None,
        "plan_s": sum(c.get("plan_seconds", 0.0) for c in
                      ((m.get("w0_g2_donly") or {}).get("by_owner_class") or {}).values())
        if isinstance(m.get("w0_g2_donly"), dict) else 0.0,
        "run_s": rec.get("schedstat_run_seconds"),
        "delay_s": rec.get("schedstat_run_delay_seconds"),
        "foreign": rec.get("foreign_busy_cpus_mean"),
        "foreign_frac": rec.get("foreign_busy_fraction_mean"),
        "instr": instr,
        "instr_per_native": instr / natives if instr and natives else None,
        "ipc": perf.get("ipc_u"),
        "apply_calls": apply_calls,
        "route_natives": route.get("records"),
        "route_s": route.get("record_seconds"),
        "scheduled": int(m["scheduled_nodes"]) if m.get("scheduled_nodes") else None,
        "peak_pending": p.get("peak_pending_domains"),
        "owners": g.get("apply_by_owner"),
        "classes": {k: (cls[k]["native_calls"], cls[k]["apply_record_seconds"]) for k in CLASSES} if cls else None,
    }


def costs(offs):
    """Per-class seconds per native call from the flag-off runs (pooled)."""
    owner, klass = {}, {}
    route_calls = sum(b["route_natives"] or 0 for b in offs)
    route_s = sum(b["route_s"] or 0 for b in offs)
    if all(b["owners"] for b in offs):
        for b in offs:
            for o, v in b["owners"].items():
                c = owner.setdefault(o, [0, 0.0])
                c[0] += v["native_calls"]
                c[1] += v["record_seconds"]
    if all(b["classes"] for b in offs):
        for b in offs:
            for k, (calls, secs) in b["classes"].items():
                c = klass.setdefault(k, [0, 0.0])
                c[0] += calls
                c[1] += secs
    per = lambda d: {k: (s / c if c else 0.0) for k, (c, s) in d.items()}
    return (per(owner) if owner else None), (per(klass) if klass else None), (route_s / route_calls if route_calls else 0)


def useful(b, owner_cost, class_cost, route_cost):
    r = (b["route_natives"] or 0) * route_cost
    uo = uc = None
    if owner_cost is not None and b["owners"]:
        uo = r + sum(v["native_calls"] * owner_cost.get(o, 0.0) for o, v in b["owners"].items())
        missing = [o for o in b["owners"] if o not in owner_cost and b["owners"][o]["native_calls"]]
        if missing:
            uo = None
    if class_cost is not None and b["classes"]:
        uc = r + sum(calls * class_cost[k] for k, (calls, _) in b["classes"].items())
    return uo, uc


def ok(r):
    m = r["metrics"]
    v = r.get("g2verify") or {}
    return (m.get("exit_code") == 0 and str(m.get("frontiers")) == "0" and r["audit"].get("audit") == "PASS"
            and (not v or v.get("verdict") == "PASS"))


def stats(xs):
    n = len(xs)
    mean = sum(xs) / n
    sd = math.sqrt(sum((x - mean) ** 2 for x in xs) / (n - 1)) if n > 1 else 0.0
    return mean, sd, n


def ratio_cell(a, b):
    if not a or not b or None in a or None in b or not all(a):
        return None, "-"
    ma, sa, na = stats(a)
    mb, sb, nb = stats(b)
    r = mb / ma
    se = r * math.sqrt((sb / mb) ** 2 / nb + (sa / ma) ** 2 / na) if mb else 0.0
    cross = [y / x for x in a for y in b]
    return r, f"{r:.3f} ({min(cross):.3f}-{max(cross):.3f}; se {se:.3f})"


def main():
    sessions = []
    for sha, arms, reps in SESSIONS:
        if sha == "B4":
            sha = b4_sha()
            if sha is None:
                continue
        sessions.append((sha, arms, reps))
    print("| Control | Binary | Arm | n on/off | drained, 0 frontiers, audit/g2verify PASS | "
          + " | ".join(f"{k}" for k in KEYS) + " | Registered gate |")
    print("|---|---|---|---|---|" + "---:|" * len(KEYS) + "---|")
    context = []
    for cname, fam, tmpl in CONTROLS:
        for sha, arms, reps in sessions:
            off = [load(RUNS / tmpl.format(arm="off", r=r, sha=sha) / fam) for r in reps]
            off = [x for x in off if x]
            if not off:
                continue
            offb = [base(x) for x in off]
            oc, cc, rc = costs(offb)
            for b in offb:
                b["uw_owner"], b["uw_class"] = useful(b, oc, cc, rc)
            ms = [b["record_s"] for b in offb]
            context.append(f"| {cname} | {sha} | off | {len(offb)} | "
                           + f"{stats(ms)[0]:,.1f} +- {stats(ms)[1]:,.1f} (CV {stats(ms)[1] / stats(ms)[0]:.3f}) | "
                           + ", ".join(f"{b['foreign']:.1f}" for b in offb if b["foreign"] is not None) + " | "
                           + ", ".join(f"{b['delay_s']:,.0f}" for b in offb if b["delay_s"] is not None) + " | "
                           + ", ".join(f"{b['ipc']:.3f}" for b in offb if b["ipc"]) + " | - |")
            for arm in arms:
                on = [load(RUNS / tmpl.format(arm=arm, r=r, sha=sha) / fam) for r in reps]
                on = [x for x in on if x]
                if not on:
                    continue
                onb = [base(x) for x in on]
                for b in onb:
                    b["uw_owner"], b["uw_class"] = useful(b, oc, cc, rc)
                good = all(ok(x) for x in off + on)
                cells, ratio = [], {}
                for k in KEYS:
                    ratio[k], cell = ratio_cell([b[k] for b in offb], [b[k] for b in onb])
                    cells.append(cell)
                gate = ("PASS" if good and (ratio.get("record_s") or 9) <= 0.5 and (ratio.get("scheduled") or 9) <= 1
                        and (ratio.get("peak_pending") or 9) <= 1 else "FAIL")
                DUMP.append({"control": cname, "binary": sha, "arm": arm, "n_on": len(on), "n_off": len(off),
                             "all_ok": good, "gate": gate, "ratios": ratio,
                             "off_record_s": [b["record_s"] for b in offb], "on_record_s": [b["record_s"] for b in onb],
                             "on_plan_s": [b["plan_s"] for b in onb],
                             "off_foreign": [b["foreign"] for b in offb], "on_foreign": [b["foreign"] for b in onb],
                             "off_delay": [b["delay_s"] for b in offb], "on_delay": [b["delay_s"] for b in onb],
                             "off_instr_per_native": [b["instr_per_native"] for b in offb],
                             "on_instr_per_native": [b["instr_per_native"] for b in onb]})
                print(f"| {cname} | {sha} | {NAME[arm]} | {len(on)}/{len(off)} | {'yes' if good else 'NO'} | "
                      + " | ".join(cells) + f" | {gate} |")
                ms = [b["record_s"] for b in onb]
                plan = [b["plan_s"] or 0 for b in onb]
                context.append(f"| {cname} | {sha} | {arm} | {len(onb)} | "
                               + f"{stats(ms)[0]:,.1f} +- {stats(ms)[1]:,.1f} (CV {stats(ms)[1] / stats(ms)[0]:.3f}) | "
                               + ", ".join(f"{b['foreign']:.1f}" for b in onb if b["foreign"] is not None) + " | "
                               + ", ".join(f"{b['delay_s']:,.0f}" for b in onb if b["delay_s"] is not None) + " | "
                               + ", ".join(f"{b['ipc']:.3f}" for b in onb if b["ipc"]) + " | "
                               + f"{stats(plan)[0]:.1f} |")
    print()
    print("Per-arm context: record_s mean +- SD (CV), foreign busy CPUs (mean per run), schedstat run delay s per run, "
          "IPC (instructions:u / cycles:u) per run, G2' plan s (mean).")
    print()
    print("| Control | Binary | Arm | n | record_s mean +- SD (CV) | foreign busy CPUs | run delay s | IPC | plan_s |")
    print("|---|---|---|---:|---|---|---|---|---:|")
    for line in context:
        print(line)
    if JSON_OUT:
        json.dump(DUMP, open(JSON_OUT, "w"), indent=1)


if __name__ == "__main__":
    main()
