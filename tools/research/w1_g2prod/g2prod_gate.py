#!/usr/bin/env python
"""Gate table of the g2prod main session: union vs off, same binary and session (read-only).

usage: g2prod_gate.py SHA [RUNS_DIR] [--json OUT.json]

Per control (C-5F Ordered W24, C-5F Ready W24, C-HOT-sub r1a12 Ready W12) and
metric, the ratio of means union/off over the repeats, with the min-max of all
cross ratios and the delta-method SE (n small: indicative):
- record_s: sum of native record wall seconds (G2' plan time included);
- run_s: schedstat on-CPU seconds of the walk threads;
- uw: useful-work units (handoff 0.1.2): native calls x per-class cost, the
  class being the Apply owner or Route, the cost the record seconds per native
  call of that class in the off arm of the same session (pooled over repeats);
  a G2' residual call is charged the full class cost (over-charges union);
- instr: perf instructions:u of the whole process; instr_per_native;
- apply_calls, route_natives, natives (Apply calls + Route natives);
- scheduled (discovered domains), peak_pending, growth (least-squares slope of
  pending vs natives up to the peak), disc_per_native;
- plan_s: G2' plan seconds (engine telemetry).
Context per run: foreign busy CPUs (mean), schedstat run delay.
"""
import json
import math
import sys
from pathlib import Path

ARGS = sys.argv[1:]
JSON_OUT = None
if "--json" in ARGS:
    i = ARGS.index("--json")
    JSON_OUT = ARGS[i + 1]
    del ARGS[i:i + 2]
SHA = ARGS[0]
RUNS = Path(ARGS[1]) if len(ARGS) > 1 else Path("/common/dev/rustred/TMP/w1/g2prod/runs")
CONTROLS = [("C-5F Ordered W24", "five-finite", "c5f-ord-{arm}-{r}-" + SHA),
            ("C-5F Ready W24", "five-finite", "c5f-rdy-{arm}-{r}-" + SHA),
            ("C-HOT-sub r1a12 Ready W12", "hot", "hotsub-{arm}-{r}-" + SHA)]
REPS = ("r1", "r2", "r3")
KEYS = ("record_s", "run_s", "uw", "instr", "instr_per_native", "apply_calls", "route_natives", "natives",
        "scheduled", "peak_pending", "growth", "disc_per_native")


def load(d):
    out = {}
    for name in ("metrics", "g2stats", "pending", "audit", "verify"):
        p = d / f"{name}.json"
        try:
            out[name] = json.load(open(p)) if p.exists() and p.stat().st_size else {}
        except json.JSONDecodeError:
            out[name] = {}
    return out if out["metrics"] and out["g2stats"] else None


def base(r):
    m, g, p = r["metrics"], r["g2stats"], r["pending"]
    rec, perf = m.get("recorder") or {}, m.get("perf") or {}
    calls = g.get("native_calls_by_phase", {})
    apply_calls, route = calls.get("Apply", 0), calls.get("Route", 0)
    instr = perf.get("instructions:u") if isinstance(perf.get("instructions:u"), float) else None
    natives = apply_calls + route
    tele = m.get("g2_index_telemetry") or {}
    return {"record_s": sum(g.get("record_seconds_by_phase", {}).values()),
            "route_s": g.get("record_seconds_by_phase", {}).get("Route", 0.0),
            "run_s": rec.get("schedstat_run_seconds"), "delay_s": rec.get("schedstat_run_delay_seconds"),
            "foreign": rec.get("foreign_busy_cpus_mean"), "instr": instr,
            "instr_per_native": instr / natives if instr and natives else None,
            "apply_calls": apply_calls, "route_natives": route, "natives": natives,
            "scheduled": int(m["scheduled_nodes"]) if m.get("scheduled_nodes") else None,
            "peak_pending": p.get("peak_pending_domains"),
            "growth": p.get("pending_growth_per_completion_slope_to_peak"),
            "disc_per_native": p.get("discovered_domains_per_native"),
            "plan_s": tele.get("plan_seconds"), "owners": g.get("apply_by_owner") or {},
            "g2": g.get("g2") or {}}


def costs(offs):
    owner, route_calls, route_s = {}, 0, 0.0
    for b in offs:
        route_calls += b["route_natives"]
        route_s += b["route_s"]
        for o, v in b["owners"].items():
            c = owner.setdefault(o, [0, 0.0])
            c[0] += v["native_calls"]
            c[1] += v["record_seconds"]
    return {o: (s / c if c else 0.0) for o, (c, s) in owner.items()}, (route_s / route_calls if route_calls else 0.0)


def useful(b, owner_cost, route_cost):
    return b["route_natives"] * route_cost + sum(v["native_calls"] * owner_cost.get(o, 0.0)
                                                 for o, v in b["owners"].items())


def ok(r):
    m, v = r["metrics"], r.get("verify") or {}
    return (m.get("exit_code") == 0 and str(m.get("frontiers")) == "0" and r["audit"].get("audit") == "PASS"
            and (not v or (v.get("verdict") == "PASS"
                           and v.get("roots_independently_verified") == v.get("roots_total"))))


def stats(xs):
    n = len(xs)
    mean = sum(xs) / n
    sd = math.sqrt(sum((x - mean) ** 2 for x in xs) / (n - 1)) if n > 1 else 0.0
    return mean, sd, n


def ratio(a, b):
    if not a or not b or None in a or None in b or not all(a):
        return None, "-"
    (ma, sa, na), (mb, sb, nb) = stats(a), stats(b)
    r = mb / ma
    se = r * math.sqrt((sb / mb) ** 2 / nb + (sa / ma) ** 2 / na) if mb else 0.0
    cross = [y / x for x in a for y in b]
    return r, f"{r:.3f} ({min(cross):.3f}-{max(cross):.3f}; se {se:.3f})"


def main():
    dump = []
    print("| Control | n union/off | all drained, 0 frontiers, audit+oracle PASS | " + " | ".join(KEYS) + " |")
    print("|---|---|---|" + "---:|" * len(KEYS))
    context = []
    for name, fam, tmpl in CONTROLS:
        off = [x for x in (load(RUNS / tmpl.format(arm="off", r=r) / fam) for r in REPS) if x]
        on = [x for x in (load(RUNS / tmpl.format(arm="union", r=r) / fam) for r in REPS) if x]
        if not off or not on:
            continue
        offb, onb = [base(x) for x in off], [base(x) for x in on]
        oc, rc = costs(offb)
        for b in offb + onb:
            b["uw"] = useful(b, oc, rc)
        good = all(ok(x) for x in off + on)
        cells, ratios = [], {}
        for k in KEYS:
            ratios[k], cell = ratio([b[k] for b in offb], [b[k] for b in onb])
            cells.append(cell)
        print(f"| {name} | {len(on)}/{len(off)} | {'yes' if good else 'NO'} | " + " | ".join(cells) + " |")
        for arm, bs in (("off", offb), ("union", onb)):
            for b in bs:
                context.append(f"| {name} | {arm} | {b['record_s']:,.1f} | {b['run_s']} | {b['foreign']} | "
                               f"{b['delay_s']} | {b['apply_calls']:,} / {b['route_natives']:,} | {b['scheduled']:,} | "
                               f"{b['peak_pending']} | {b['plan_s']} | {b['g2'].get('residual', 0):,} / "
                               f"{b['g2'].get('full_cover', 0):,} / {b['g2'].get('anchor_links', 0):,} |")
        dump.append({"control": name, "n_union": len(on), "n_off": len(off), "all_ok": good, "ratios": ratios,
                     "off": [{k: v for k, v in b.items() if k != "owners"} for b in offb],
                     "union": [{k: v for k, v in b.items() if k != "owners"} for b in onb]})
    print()
    print("| Control | Arm | record_s | run_s | foreign busy CPUs | run delay s | Apply calls / Route natives | "
          "scheduled | peak pending | plan_s | G2' residual / full cover / anchor links |")
    print("|---|---|---:|---:|---:|---:|---|---:|---:|---:|---|")
    for line in context:
        print(line)
    if JSON_OUT:
        json.dump(dump, open(JSON_OUT, "w"), indent=1)


if __name__ == "__main__":
    main()
