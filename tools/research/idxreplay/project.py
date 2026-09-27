#!/usr/bin/env python
"""Gate 0.4(b) projection [E] from idxreplay `streams` output (real gen-7 streams).

Admission CPU per native at N domains, for a design D (resolution pipeline of
plan section 3.2 in front of a layer layout, or today's engine without it):

  A_D(N) = sum over request classes c of r_c * cost_{D,c}(74M) * f_{D,c} * (N/74M)^alpha
           + r_cheap * cheap_ns

r_c      requests per native reaching class c: layer hits and misses from the
         pipeline tally at MRU k (streams `pipeline` row, scope all); a miss
         pays a forward miss scan plus a reverse (retirement) scan. For today's
         engine r_c comes from the engine's committed counters instead
         (`streams-input` row: forward checks on contained and on new admissions).
cost     CPU per request (or per tested candidate for the engine-counter row)
         measured on the gen-7 snapshot index with one thread
         (`streams-layer-cost` rows).
f        thread factor: 1, a scalar (--thread-factor), or per (layout, set)
         from a static thread sweep of the same session
         (--thread-sweep FILE --threads T: cpu_ns_per_tested at T threads over
         1 thread, `throughput` rows).
alpha    growth exponent of the per-request scan cost with the index size;
         the same alpha is applied to every scanned class unless
         --class-alpha overrides it per cost part (keys: hits, miss_scans,
         reverse for the pipeline designs; forward_contained, forward_new,
         reverse for today's engine), e.g. the layout's own thinning
         exponents: --class-alpha hits=0.03,miss_scans=0.46,reverse=0.50.
cheap_ns CPU per request resolved in a cheap tier (exact-job, self, Local,
         MRU, exact-store, helper) [E, not measured].
Share of worker CPU = A / (A + native CPU per native); native CPU per native
from the trace's job headers (`native_ms_per_job` of the pipeline row, v4
streams output) or --native-ms, optionally times --native-scale (e.g. the
gate-0.3 in-process contention factor at 96 threads).

Usage: project.py STREAMS.jsonl [--k 16] [--native-ms X] [--native-scale S]
                  [--thread-factor F | --thread-sweep FILE --threads T]
                  [--alpha 0.44,0.65,0.82] [--class-alpha hits=A,miss_scans=B,reverse=C]
                  [--n1 1e9] [--cheap-ns 100] [--json OUT]
"""
import argparse
import json

SETS = {"hit-minid", "hit-firstfound", "miss", "reverse"}


def sweep_factors(path, threads):
    """cpu_ns_per_tested(threads) / cpu_ns_per_tested(1) per (layout, set) from throughput rows."""
    per = {}
    for line in open(path):
        r = json.loads(line)
        if r.get("kind") != "throughput" or r.get("set") not in SETS:
            continue
        per[(r["layout"], r["set"], r["threads"])] = r["cpu_ns_per_tested"]
    out = {}
    for (lay, st, th), v in per.items():
        if th == threads and (lay, st, 1) in per:
            out[(lay, st)] = v / per[(lay, st, 1)]
    if not out:
        raise SystemExit(f"no throughput rows at {threads} and 1 threads in {path}")
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("streams")
    p.add_argument("--k", type=int, default=16)
    p.add_argument("--native-ms", type=float, default=None)
    p.add_argument("--native-scale", type=float, default=1.0)
    p.add_argument("--thread-factor", type=float, default=None)
    p.add_argument("--thread-sweep", default=None)
    p.add_argument("--threads", type=int, default=90)
    p.add_argument("--alpha", default="0.44,0.65,0.82")
    p.add_argument("--class-alpha", default=None, help="per cost part alpha overrides, e.g. hits=0.03,miss_scans=0.46,reverse=0.50")
    p.add_argument("--n0", type=float, default=74156033)
    p.add_argument("--n1", type=float, default=1e9)
    p.add_argument("--cheap-ns", type=float, default=100.0, help="CPU ns per cheap-tier request [E]")
    p.add_argument("--json", default=None, help="append one JSON row per design to this file")
    p.add_argument("--tag", default="")
    a = p.parse_args()

    rows = [json.loads(line) for line in open(a.streams)]
    pipe = {(r["scope"], r["mru_k"]): r for r in rows if r.get("kind") == "pipeline"}
    cost = {(r["class"], r["layout"], r["set"]): r for r in rows if r.get("kind") == "streams-layer-cost"}
    inp = next(r for r in rows if r.get("kind") == "streams-input")
    pa = pipe[("all", a.k)]
    jobs = pa["jobs"]
    req_per_job = pa["requests"] / jobs
    native_ms = a.native_ms if a.native_ms is not None else pa.get("native_ms_per_job")
    if native_ms is None:
        raise SystemExit("no native_ms_per_job in the pipeline row (v3 output): pass --native-ms")
    native_ms *= a.native_scale
    layer_hit = pa["share_layer-hit"] * req_per_job
    miss = pa["share_miss"] * req_per_job
    cheap = pa["cheap_share"] * req_per_job
    alphas = [float(x) for x in a.alpha.split(",")]
    class_alpha = {}
    if a.class_alpha:
        for kv in a.class_alpha.split(","):
            k, v = kv.split("=")
            class_alpha[k.strip()] = float(v)
    scale = a.n1 / a.n0

    if a.thread_sweep:
        tf = sweep_factors(a.thread_sweep, a.threads)
        tf_label = f"per-set factors, {a.threads} vs 1 threads ({a.thread_sweep.rsplit('/', 1)[-1]})"
    elif a.thread_factor is not None:
        tf = None
        tf_label = f"scalar factor {a.thread_factor}"
    else:
        tf = None
        tf_label = "1 thread"

    def factor(lay, st):
        if tf is not None:
            return tf[(lay, st)]
        return a.thread_factor if a.thread_factor is not None else 1.0

    def us(cls, lay, st, field="cpu_ns_per_q"):
        r = cost.get((cls, lay, st))
        return None if r is None else r[field] / 1000.0 * factor(lay, st)

    print(
        f"[{a.tag}] k={a.k} jobs {jobs:,}  requests/job {req_per_job:.1f}  cheap {cheap:.1f}  layer-hit {layer_hit:.2f}"
        f"  miss {miss:.2f}  native ms/job {native_ms:.3f} (scale {a.native_scale})  threads: {tf_label}  cheap {a.cheap_ns:.0f} ns"
        + (f"  class alpha overrides {class_alpha} (a= applies to the other parts)" if class_alpha else "")
    )

    designs = [
        ("pipeline + L0 min-ID", "l0-stored", "hit-minid"),
        ("pipeline + L0 first-found", "l0-stored", "hit-firstfound"),
        ("pipeline + SoA-id min-ID", "soa-id", "hit-minid"),
        ("pipeline + SoA-id first-found", "soa-id", "hit-firstfound"),
        ("pipeline + SoA-pattern min-ID", "soa-pattern", "hit-minid"),
        ("pipeline + SoA-pattern first-found", "soa-pattern", "hit-firstfound"),
    ]
    results = []

    def report(name, scan_parts, fixed_us, parts, lay, hitset):
        scan_us = sum(scan_parts.values())
        a0 = scan_us + fixed_us
        share0 = a0 / (a0 + 1000 * native_ms)
        line = f"  {name:36} 74M {a0 / 1000:7.3f} ms/native = {100 * share0:5.1f}%"
        at1 = {}
        for al in alphas:
            a1 = sum(v * scale ** class_alpha.get(k, al) for k, v in scan_parts.items()) + fixed_us
            s1 = a1 / (a1 + 1000 * native_ms)
            at1[al] = (a1 / 1000, s1)
            line += f" | a={al}: {a1 / 1000:6.2f} ms {100 * s1:5.1f}%"
        print(line)
        results.append(
            {
                "tag": a.tag,
                "design": name,
                "layout": lay,
                "hit_set": hitset,
                "k": a.k,
                "threads": tf_label,
                "class_alpha": class_alpha,
                "native_ms": native_ms,
                "cheap_ns": a.cheap_ns,
                "parts_us_74M": parts,
                "admission_ms_74M": a0 / 1000,
                "share_74M": share0,
                "n1": a.n1,
                "at_n1": {str(al): {"admission_ms": v[0], "share": v[1]} for al, v in at1.items()},
            }
        )

    for name, lay, hitset in designs:
        h = us("engine-hit", lay, hitset)
        m = us("engine-miss", lay, "miss")
        rv = us("engine-miss", lay, "reverse")
        if None in (h, m, rv):
            print(f"  {name}: missing costs")
            continue
        parts = {"hits": layer_hit * h, "miss_scans": miss * m, "reverse": miss * rv, "cheap": cheap * a.cheap_ns / 1000.0}
        scan = {c: parts[c] for c in ("hits", "miss_scans", "reverse")}
        report(name, scan, parts["cheap"], parts, lay, hitset)

    # Today's engine: every committed request scans the index (min-ID), no
    # pipeline; forward checks from the engine's counters priced at the L0
    # per-candidate cost, plus a reverse scan per new admission.
    njobs = inp["jobs_records"]
    fh = us("engine-hit", "l0-stored", "hit-minid", "cpu_ns_per_tested")
    fm = us("engine-miss", "l0-stored", "miss", "cpu_ns_per_tested")
    rv = us("engine-miss", "l0-stored", "reverse")
    if None not in (fh, fm, rv):
        parts = {
            "forward_contained": inp["engine_forward_checks_contained"] / njobs * fh,
            "forward_new": inp["engine_forward_checks_new"] / njobs * fm,
            "reverse": inp["new"] / njobs * rv,
        }
        report("today's engine (counters x L0 cost)", dict(parts), 0.0, parts, "l0-stored", "engine")

    if a.json:
        with open(a.json, "a") as f:
            for r in results:
                f.write(json.dumps(r) + "\n")


if __name__ == "__main__":
    main()
