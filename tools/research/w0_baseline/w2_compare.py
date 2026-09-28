#!/usr/bin/env python
"""W2 comparator: compare two M1-style run directories (m1_resume_profile.py output)
over the same matched window, computed from the raw files of BOTH runs with the same code.

Window (both runs): T = first traversal heartbeat (timeline.traversal_unix_time), end = the
last heartbeat observed at or before min(stop request, T + --window + --slack). Rates use the
native clock (elapsed_seconds), as m1_analyze.py does; with --window 1500 the reference run
(M1 run2) reproduces its analysis.json T_to_stop figures exactly.

Per run: timeline and restore, throughput (natives, obligations = natives + aliases =
committed domains, aliases, discovered), work volume (queue pending growth per completion,
native-pending per native, discovered per native/obligation), coordinator duty split incl.
the W1.1 kernel timers (coordinator_duty.admission_kernel, nested in ordered_commit;
admission_preparation.speculative_kernel, helper scan wall), admission rates (checks per
request, us per prepared record, us per batch), 5-min slices and their spread, thread CPU
and run delay by class, foreign busy CPUs (window, restore, before launch), RAM (per domain,
marginal fits after T+60 s and over the second half of the window, VmHWM), perf stat IPC and
instructions per native/obligation per window.

Comparison: comparator / reference ratios, the ">= 1.5x" gate figure (comparator obligations/h
over the matched window with its slice spread), and a stretch-matched view: the legacy engine
commits in ID order (committed count ~ contiguous publication watermark), so both runs' first
N committed domains after T are the same restored-pending ID range; rates over the reference's
committed range and over each reference slice's committed range remove the work-mix confound.

Usage: w2_compare.py REFERENCE_RUN COMPARATOR_RUN --out OUT.json [--window 1500]
Reads only; writes only --out.
"""
import argparse
import json
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import m1_analyze as m1  # noqa: E402

SLICE_KEYS = {
    "natives_per_hour": lambda s: s["rate"].get("native_publications_per_hour"),
    "obligations_per_hour": lambda s: s["rate"].get("committed_domains_per_hour"),
    "aliases_per_hour": lambda s: s["rate"].get("delegated_publications_per_hour"),
    "discovered_per_hour": lambda s: s["rate"].get("discovered_per_hour"),
    "checks_per_request": lambda s: s["duty"].get("speculative_checks_per_request"),
    "commit_us_per_prepared_record": lambda s: s["duty"].get("commit_us_per_prepared_record"),
    "prep_us_per_parallel_batch": lambda s: s["duty"].get("prep_us_per_parallel_batch"),
    "ordered_commit_share": lambda s: s["duty"].get("ordered_commit_share"),
    "preparation_share": lambda s: s["duty"].get("preparation_share"),
    "progress_json_share": lambda s: s["duty"].get("progress_json_share"),
    "kernel_scan_share_of_coordinator_wall":
        lambda s: (s["duty"].get("admission_kernel") or {}).get("scan_share_of_coordinator_wall"),
}


def spread(values):
    vals = [v for v in values if v is not None]
    if not vals:
        return None
    out = {"n": len(vals), "mean": statistics.fmean(vals), "min": min(vals), "max": max(vals)}
    if len(vals) > 1:
        out["sd"] = statistics.stdev(vals)
        out["cv"] = out["sd"] / out["mean"] if out["mean"] else None
    return out


def fit(rows):
    if len(rows) < 10:
        return None
    xs = [r["discovered"] for r in rows]
    ys = [r["rss_bytes"] for r in rows]
    mx, my = statistics.fmean(xs), statistics.fmean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx <= 0:
        return None
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    ss_res = sum((y - (my + slope * (x - mx))) ** 2 for x, y in zip(xs, ys))
    ss_tot = sum((y - my) ** 2 for y in ys)
    return {"bytes_per_domain": slope, "r2": 1 - ss_res / ss_tot if ss_tot else None, "points": len(rows),
            "discovered_range": [min(xs), max(xs)]}


def work_volume(rate):
    committed = rate.get("committed_domains_delta")
    natives = rate.get("native_publications_delta")
    discovered = rate.get("discovered_delta")
    pending_native = rate.get("pending_native_publications_delta")
    out = {}
    if committed:
        out["queue_pending_growth"] = discovered - committed
        out["queue_pending_growth_per_completion"] = (discovered - committed) / committed
        out["discovered_per_obligation"] = discovered / committed
    if natives:
        out["discovered_per_native"] = discovered / natives
        out["native_pending_growth_per_native"] = pending_native / natives if pending_native is not None else None
        out["alias_share_of_obligations"] = (committed - natives) / committed if committed else None
    return out


def row_at_committed(rows, committed):
    for r in rows:
        if r.get("committed_domains") is not None and r["committed_domains"] >= committed:
            return r
    return None


def summarize(run, window, slack, end_override=None):
    tl = json.load(open(run / "timeline.json"))
    timeline, windows = tl["timeline"], tl["windows"]
    meta = json.load(open(run / "meta.json"))
    hb = m1.load_jsonl(run / "heartbeats.jsonl")
    duty_rows = [r for r in hb if r.get("coordinator_duty")]
    samples = m1.load_jsonl(run / "samples.jsonl")
    for s in samples:
        m1.reclassify(s.get("threads") or {})
    launch = timeline["launch_unix_time"]
    t0 = timeline["traversal_unix_time"]
    stop = timeline.get("stop_request_unix_time")
    stop_source = "harness" if stop else None
    if stop is None and end_override is None:
        # A stop file written outside the harness (runA) leaves no stop_request in timeline.json;
        # take its own unix_time so the window never runs into the final save and exit.
        try:
            doc = json.load(open(run / "stop-request.json"))
            if isinstance(doc.get("unix_time"), (int, float)):
                stop, stop_source = float(doc["unix_time"]), "stop_file"
        except (OSError, ValueError):
            pass
    if end_override is not None:  # e.g. a stop file written outside the harness (runA)
        stop = min(end_override, stop) if stop else end_override
        stop_source = "end_override"
    end = t0 + window + slack if stop is None else min(stop, t0 + window + slack)
    rows = [r for r in duty_rows if t0 <= r["observed_unix_time"] <= end]
    a, b = rows[0], rows[-1]
    out = {"run": str(run), "binary": meta["binary"], "binary_sha256": meta["binary_sha256"],
           "cpus": meta["cpus"], "perf_cpus": meta.get("perf_cpus"),
           "walk_semantics_probe": meta.get("walk_semantics_probe", {}).get("stdout")}
    markers = tl.get("markers", [])
    restore = next((m.get("restore") for m in markers if m.get("event") == "checkpoint_restored"), None)
    saves = [m["record"]["checkpoint"] for m in markers
             if m.get("event") == "checkpoint_saved" and isinstance(m.get("record"), dict)]
    out["timeline"] = {
        "launch_to_restored_s": timeline.get("restored_unix_time", launch) - launch,
        "launch_to_T_s": t0 - launch,
        "T_to_stop_request_s": (stop - t0) if stop else None, "stop_source": stop_source,
        "stop_request_to_exit_s": (timeline["exit_unix_time"] - stop) if stop and timeline.get("exit_unix_time") else None,
        "stop_reason": timeline.get("stop_reason"), "exit_code": timeline.get("exit_code"),
        "wall_s": timeline.get("wall_seconds"),
        "saves": [{"generation": s.get("generation"), "save_seconds": s.get("save_seconds"),
                   "new_bytes": s.get("new_bytes"), "paused": s.get("paused")} for s in saves]}
    if restore:
        out["restore"] = {k: restore.get(k) for k in ("restore_seconds", "verify_seconds", "decode_seconds",
                                                       "validate_seconds", "closure_seconds", "rss_bytes",
                                                       "domains", "records", "dependency_edges",
                                                       "committed_domains")}
        out["restore"]["restore_plus_verify_note"] = "restore_seconds includes verify/decode/validate"
        out["restore"]["phase_seconds"] = restore.get("phase_seconds")
    rate = m1.rate_between(a, b)
    out["window"] = {"T_offset_s": 0.0, "observed_seconds": b["observed_unix_time"] - a["observed_unix_time"],
                     "native_seconds": rate["native_elapsed_seconds"], "heartbeats": len(rows),
                     "committed_start": a.get("committed_domains"), "committed_end": b.get("committed_domains"),
                     "discovered_start": a.get("discovered"), "discovered_end": b.get("discovered")}
    out["throughput"] = {
        "natives_per_hour": rate.get("native_publications_per_hour"),
        "obligations_per_hour": rate.get("committed_domains_per_hour"),
        "aliases_per_hour": rate.get("delegated_publications_per_hour"),
        "discovered_per_hour": rate.get("discovered_per_hour"),
        "natives": rate.get("native_publications_delta"), "obligations": rate.get("committed_domains_delta"),
        "aliases": rate.get("delegated_publications_delta"), "discovered": rate.get("discovered_delta"),
        "native_pending_delta": rate.get("pending_native_publications_delta"),
        "initial_closed_start": rate.get("initial_closed_start"), "initial_closed_end": rate.get("initial_closed_end"),
        "mean_busy_inspector_slots": rate.get("mean_busy_inspector_slots")}
    out["work_volume"] = work_volume(rate)
    out["duty"] = m1.duty_between(a, b)
    # 5-min slices from T (same rule as m1_analyze.py: >= 300 s observed; trailing part kept if >= 120 s)
    slices, start = [], a
    for r in rows:
        if r["observed_unix_time"] - start["observed_unix_time"] >= 300:
            slices.append({"from_T_seconds": start["observed_unix_time"] - a["observed_unix_time"],
                           "duty": m1.duty_between(start, r), "rate": m1.rate_between(start, r),
                           "committed_offsets": [start["committed_domains"] - a["committed_domains"],
                                                 r["committed_domains"] - a["committed_domains"]]})
            start = r
    if b is not start and b["observed_unix_time"] - start["observed_unix_time"] >= 120:
        slices.append({"from_T_seconds": start["observed_unix_time"] - a["observed_unix_time"], "partial": True,
                       "duty": m1.duty_between(start, b), "rate": m1.rate_between(start, b),
                       "committed_offsets": [start["committed_domains"] - a["committed_domains"],
                                             b["committed_domains"] - a["committed_domains"]]})
    out["slices_5min"] = [{"from_T_seconds": s["from_T_seconds"], "partial": s.get("partial", False),
                           "native_seconds": s["rate"]["native_elapsed_seconds"],
                           "committed_offsets": s["committed_offsets"],
                           **{k: f(s) for k, f in SLICE_KEYS.items()}} for s in slices]
    out["slices_5min_spread"] = {k: spread([f(s) for s in slices]) for k, f in SLICE_KEYS.items()}
    # Threads and foreign load over the window
    live = [s for s in samples if s.get("threads")]
    s_first = min((s for s in live if s["unix_time"] >= t0), key=lambda s: s["unix_time"], default=None)
    # Never end on a sample of the exiting process (most threads already gone): runA's first
    # analysis took one and reported 0.46 CPUs in total.
    full = 0.9 * len(s_first["threads"]) if s_first else 0
    s_last = max((s for s in live if s["unix_time"] <= end and len(s["threads"]) >= full),
                 key=lambda s: s["unix_time"], default=None)
    if s_first and s_last and s_last is not s_first:
        td = m1.thread_delta(s_first, s_last)
        out["threads"] = {"wall_seconds": td["wall_seconds"],
                          "classes": {c: {k: v[k] for k in ("threads", "cpus", "cpu_s", "system_share",
                                                             "run_delay_share_per_thread", "run_delay_s",
                                                             "voluntary_per_s", "involuntary_per_s",
                                                             "migrations_per_s")}
                                      for c, v in td["classes"].items()}}
        out["threads"]["total_cpus"] = sum(v["cpus"] for v in td["classes"].values())
        natives = rate.get("native_publications_delta")
        insp = td["classes"].get("inspector")
        if insp and natives:
            out["threads"]["inspector_cpu_ms_per_native"] = 1e3 * insp["cpu_s"] / natives
        adm = sum(td["classes"].get(c, {}).get("cpu_s", 0.0) for c in ("coordinator", "admission_helper"))
        if rate.get("committed_domains_delta"):
            out["threads"]["admission_cpu_ms_per_obligation"] = 1e3 * adm / rate["committed_domains_delta"]
    def foreign_between(lo, hi):
        vals = [s["foreign_busy_cpus_estimate"] for s in samples
                if s.get("foreign_busy_cpus_estimate") is not None and lo <= s["unix_time"] <= hi]
        if not vals:
            return None
        vs = sorted(vals)
        return {"n": len(vs), "mean": statistics.fmean(vs), "p10": vs[int(0.1 * (len(vs) - 1))],
                "median": statistics.median(vs), "p90": vs[int(0.9 * (len(vs) - 1))], "max": vs[-1]}
    out["foreign_busy_cpus"] = {
        "before_launch_5s": meta.get("foreign_busy_cpus_before"),
        "launch_to_restored": foreign_between(launch, timeline.get("restored_unix_time", launch)),
        "window": foreign_between(t0, end),
        "window_slices": [foreign_between(a["observed_unix_time"] + s["from_T_seconds"],
                                          a["observed_unix_time"] + s["from_T_seconds"] + 300)
                          for s in slices],
        "method": "(busy jiffies on CPUs 128-227 - own process CPU) / wall, 5-s samples; harness CPUs off socket 1"}
    # RAM
    ram_rows = [r for r in hb if t0 <= r["observed_unix_time"] <= end and r.get("rss_bytes") and r.get("discovered")]
    ram = {}
    if ram_rows:
        f, l = ram_rows[0], ram_rows[-1]
        ram["at_T"] = {"rss_bytes": f["rss_bytes"], "discovered": f["discovered"],
                       "bytes_per_domain": f["rss_bytes"] / f["discovered"]}
        ram["at_end"] = {"rss_bytes": l["rss_bytes"], "discovered": l["discovered"],
                         "bytes_per_domain": l["rss_bytes"] / l["discovered"]}
        dd = l["discovered"] - f["discovered"]
        ram["marginal_endpoints_bytes_per_domain"] = (l["rss_bytes"] - f["rss_bytes"]) / dd if dd else None
        ram["fit_after_T+60s"] = fit([r for r in ram_rows if r["observed_unix_time"] >= t0 + 60])
        ram["fit_second_half"] = fit([r for r in ram_rows if r["observed_unix_time"] >= t0 + window / 2])
    hwm = [s["status"]["VmHWM"] for s in samples if s.get("status") and s["status"].get("VmHWM") is not None]
    if hwm:
        ram["peak_vmhwm_bytes"] = max(hwm) * 1024
    rss = [s for s in samples if s.get("status") and s["status"].get("VmRSS") is not None
           and s["unix_time"] <= timeline.get("restored_unix_time", 0) + 5]
    if rss:
        ram["rss_at_restore_bytes"] = rss[-1]["status"]["VmRSS"] * 1024
    mem = [s["mem_available"] for s in samples if s.get("mem_available")]
    if mem:
        ram["host_memavailable_min_bytes"] = min(mem)
    numa = run / "numa.json"
    if numa.exists():
        doc = json.load(open(numa))
        pages = doc.get("pages_by_node", {})
        total = sum(pages.values())
        if total:
            ram["numa_node_shares"] = {k: v / total for k, v in sorted(pages.items())}
    out["ram"] = ram
    # perf stat per window, with instructions per native / per obligation over the window's heartbeats
    snaps = {}
    for path in run.glob("sched-*.json"):
        doc = json.load(open(path))
        m1.reclassify(doc["threads"])
        snaps[path.stem[len("sched-"):]] = doc
    comm_class = {tid: r["class"] for s in snaps.values() for tid, r in s["threads"].items()}
    perf = {}
    for tag, win in windows.items():
        stat = m1.parse_perf_stat(run / f"perf-stat-{tag}.csv", comm_class)
        if not stat or "end_unix_time" not in win:
            continue
        wa = m1.nearest(duty_rows, win["start_unix_time"], after=False) or m1.nearest(duty_rows, win["start_unix_time"])
        wb = m1.nearest(duty_rows, win["end_unix_time"], after=True) or m1.nearest(duty_rows, win["end_unix_time"], after=False)
        wr = m1.rate_between(wa, wb) if wa and wb and wa is not wb else {}
        entry = {"from_T_seconds": win["start_unix_time"] - t0, "classes": {}}
        natives, obligations = wr.get("native_publications_delta"), wr.get("committed_domains_delta")
        entry["natives"], entry["obligations"] = natives, obligations
        entry["heartbeat_native_seconds"] = wr.get("native_elapsed_seconds")
        for cls, v in stat.items():
            ins = v["counters"].get("instructions:u", 0.0)
            entry["classes"][cls] = {"threads": v["threads"], "ipc": v["ipc"], "instructions": ins,
                                     "cycles": v["counters"].get("cycles:u", 0.0),
                                     "dram_far_share": v["dram_far_share"],
                                     "dram_demand_fills_per_kinstr": v["dram_demand_fills_per_kinstr"]}
        insp = entry["classes"].get("inspector")
        if insp and natives:
            entry["inspector_instructions_per_native"] = insp["instructions"] / natives
            entry["inspector_cycles_per_native"] = insp["cycles"] / natives
        adm = sum(entry["classes"].get(c, {}).get("instructions", 0.0) for c in ("coordinator", "admission_helper"))
        if obligations:
            entry["admission_instructions_per_obligation"] = adm / obligations
        entry["note"] = ("perf stat window (60 s) vs heartbeat deltas over the nearest enclosing heartbeats "
                         "(~70 s): per-native figures carry that ~15% boundary mismatch; ratios between runs use "
                         "the same rule")
        perf[tag] = entry
    out["perf_stat"] = perf
    out["_rows"] = rows  # dropped before writing
    return out


def ratio(x, y):
    return x / y if x is not None and y else None


def main():
    p = argparse.ArgumentParser()
    p.add_argument("reference", type=Path)
    p.add_argument("comparator", type=Path)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--window", type=float, default=1500.0)
    p.add_argument("--slack", type=float, default=5.0, help="harness poll slack past T+window")
    p.add_argument("--comparator-end", type=float, help="unix time: cut the comparator window here (a stop "
                   "request made outside the harness is not in its timeline)")
    args = p.parse_args()
    ref = summarize(args.reference, args.window, args.slack)
    cmp_ = summarize(args.comparator, args.window, args.slack, args.comparator_end)
    if args.comparator_end is not None:
        cmp_["timeline"]["external_stop_unix_time"] = args.comparator_end
    ref_rows, cmp_rows = ref.pop("_rows"), cmp_.pop("_rows")
    doc = {"window_definition": f"[T, min(stop request, T + {args.window:g} s + {args.slack:g} s slack)] on the "
                                "harness clock; rates on the native clock; T = first traversal heartbeat",
           "reference": ref, "comparator": cmp_, "ratios_comparator_over_reference": {}}
    r = doc["ratios_comparator_over_reference"]
    for key in ("natives_per_hour", "obligations_per_hour", "aliases_per_hour", "discovered_per_hour"):
        r[key] = ratio(cmp_["throughput"][key], ref["throughput"][key])
    for key in ("speculative_checks_per_request", "commit_us_per_prepared_record", "prep_us_per_parallel_batch",
                "ordered_commit_share", "preparation_share"):
        r[key] = ratio(cmp_["duty"].get(key), ref["duty"].get(key))
    for key in ("queue_pending_growth_per_completion", "discovered_per_native", "native_pending_growth_per_native"):
        r[key] = ratio(cmp_["work_volume"].get(key), ref["work_volume"].get(key))
    if "restore" in ref and "restore" in cmp_:
        r["restore_seconds"] = ratio(cmp_["restore"]["restore_seconds"], ref["restore"]["restore_seconds"])
        r["launch_to_restored"] = ratio(cmp_["timeline"]["launch_to_restored_s"], ref["timeline"]["launch_to_restored_s"])
    for key in ("inspector_cpu_ms_per_native", "admission_cpu_ms_per_obligation", "total_cpus"):
        r[key] = ratio((cmp_.get("threads") or {}).get(key), (ref.get("threads") or {}).get(key))
    # Stretch-matched: the same absolute committed-domain range in both runs (the legacy engine commits in
    # ID order from the same restored state, so equal committed counts mean the same stretch of work).
    ca, ra = cmp_rows[0], ref_rows[0]
    lo = max(ra["committed_domains"], ca["committed_domains"])
    hi = min(ref_rows[-1]["committed_domains"], cmp_rows[-1]["committed_domains"])
    stretch = {"reference_committed_range": [ra["committed_domains"], ref_rows[-1]["committed_domains"]],
               "comparator_committed_range": [ca["committed_domains"], cmp_rows[-1]["committed_domains"]],
               "common_committed_range": [lo, hi]}
    r_lo, r_hi = row_at_committed(ref_rows, lo), row_at_committed(ref_rows, hi)
    c_lo, c_hi = row_at_committed(cmp_rows, lo), row_at_committed(cmp_rows, hi)
    if r_lo and r_hi and c_lo and c_hi and r_lo is not r_hi and c_lo is not c_hi:
        rr, cr = m1.rate_between(r_lo, r_hi), m1.rate_between(c_lo, c_hi)
        stretch["common_committed"] = hi - lo
        for name, rate in (("reference", rr), ("comparator", cr)):
            stretch[name] = {"native_seconds": rate["native_elapsed_seconds"],
                             "obligations_per_hour": rate.get("committed_domains_per_hour"),
                             "natives_per_hour": rate.get("native_publications_per_hour"),
                             "natives": rate.get("native_publications_delta"),
                             "obligations": rate.get("committed_domains_delta")}
        stretch["obligations_per_hour_ratio"] = ratio(cr.get("committed_domains_per_hour"),
                                                      rr.get("committed_domains_per_hour"))
        stretch["time_ratio_reference_over_comparator"] = ratio(rr["native_elapsed_seconds"],
                                                                cr["native_elapsed_seconds"])
        per = []
        for sl in ref["slices_5min"]:
            s_lo = ra["committed_domains"] + sl["committed_offsets"][0]
            s_hi = ra["committed_domains"] + sl["committed_offsets"][1]
            if s_lo < lo or s_hi > hi:
                continue
            rl, rh = row_at_committed(ref_rows, s_lo), row_at_committed(ref_rows, s_hi)
            cl, ch = row_at_committed(cmp_rows, s_lo), row_at_committed(cmp_rows, s_hi)
            if not (rl and rh and cl and ch) or rl is rh or cl is ch:
                continue
            rrate, crate = m1.rate_between(rl, rh), m1.rate_between(cl, ch)
            per.append({"reference_slice_from_T_seconds": sl["from_T_seconds"], "committed_range": [s_lo, s_hi],
                        "reference_obligations_per_hour": rrate.get("committed_domains_per_hour"),
                        "comparator_obligations_per_hour": crate.get("committed_domains_per_hour"),
                        "reference_native_seconds": rrate["native_elapsed_seconds"],
                        "comparator_native_seconds": crate["native_elapsed_seconds"],
                        "ratio": ratio(crate.get("committed_domains_per_hour"),
                                       rrate.get("committed_domains_per_hour"))})
        stretch["per_reference_slice"] = per
        stretch["per_reference_slice_ratio_spread"] = spread([x["ratio"] for x in per])
    doc["stretch_matched"] = stretch
    # Validity (orchestrator 2026-09-28): the comparator's heartbeat/progress JSON must cost what
    # run2's did; d9163195 walked the queue storage on every heartbeat (progress_json ~26%).
    pj_cmp = cmp_["duty"].get("progress_json_share")
    pj_slices = [s.get("progress_json_share") for s in cmp_["slices_5min"] if s.get("progress_json_share") is not None]
    window_ok = cmp_["window"]["native_seconds"] >= 0.95 * args.window
    doc["validity"] = {
        "progress_json_share_comparator": pj_cmp,
        "progress_json_share_comparator_max_slice": max(pj_slices) if pj_slices else None,
        "progress_json_share_reference": ref["duty"].get("progress_json_share"),
        "progress_json_threshold": 0.02,
        "progress_json_ok": pj_cmp is not None and pj_cmp < 0.02,
        "full_window": window_ok,
        "comparator_native_window_seconds": cmp_["window"]["native_seconds"],
        "valid": bool(pj_cmp is not None and pj_cmp < 0.02 and window_ok)}
    ob = cmp_["throughput"]["obligations_per_hour"]
    doc["gate_1p5x"] = {
        "metric": "obligations discharged per hour (natives + aliases = committed domains) over the matched window",
        "comparator_obligations_per_hour": ob,
        "comparator_slice_spread": cmp_["slices_5min_spread"]["obligations_per_hour"],
        "threshold_obligations_per_hour_at_1p5x": 1.5 * ob if ob else None,
        "reference_run2_obligations_per_hour": ref["throughput"]["obligations_per_hour"],
        "reference_slice_spread": ref["slices_5min_spread"]["obligations_per_hour"]}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    json.dump(doc, open(args.out, "w"), indent=1)
    brief = {"validity": doc["validity"], "ratios": r, "gate": doc["gate_1p5x"],
             "stretch": {k: v for k, v in stretch.items() if k != "per_reference_slice"}}
    print(json.dumps(brief, indent=1))


if __name__ == "__main__":
    main()
