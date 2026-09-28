#!/usr/bin/env python
"""Analyse an M1 run directory written by m1_resume_profile.py.

Writes <run>/analysis.json and prints it. Sections:
  timeline       launch -> restore -> traversal -> stop request -> exit
  duty           coordinator_duty shares (session, early, late, perf windows)
  throughput     natives, committed domains, discovered domains, delegated per hour
  ram            RSS per discovered domain; marginal bytes per domain above the
                 restored count (fit over heartbeats after T+60 s)
  threads        per thread class: CPUs used, user/system split, run delay,
                 voluntary/involuntary switches, migrations (whole run + windows)
  foreign        foreign busy CPUs on the pinned set (own CPU subtracted)
  perf_stat      IPC and miss rates per thread class and window
  perf           per perf.data: DSO split, flat top symbols, and (coordinator)
                 caller attribution by duty bucket (see perf_attribution.py)
"""
import argparse
import csv
import json
import statistics
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import perf_attribution  # noqa: E402

BUCKETS = ("ordered_commit_seconds", "preparation_seconds", "dispatch_seconds", "poll_seconds",
           "publication_seconds", "progress_json_seconds", "closure_refresh_seconds",
           "checkpoint_seconds", "ready_service_seconds", "wait_seconds")


def load_jsonl(path):
    rows = []
    if not path.exists():
        return rows
    for line in open(path):
        line = line.strip()
        if line:
            try:
                rows.append(json.loads(line))
            except ValueError:
                pass
    return rows


def duty_between(a, b):
    da, db = a["coordinator_duty"], b["coordinator_duty"]
    wall = db["coordinator_elapsed_seconds"] - da["coordinator_elapsed_seconds"]
    out = {"coordinator_wall_seconds": wall}
    total = 0.0
    for key in BUCKETS:
        d = db.get(key, 0.0) - da.get(key, 0.0)
        out[key.replace("_seconds", "_share")] = d / wall if wall > 0 else None
        total += d
    out["untimed_share"] = 1 - total / wall if wall > 0 else None
    pa, pb = a.get("admission_preparation") or {}, b.get("admission_preparation") or {}
    records = pb.get("prepared_batch_records", 0) - pa.get("prepared_batch_records", 0)
    batches = pb.get("parallel_batches", 0) - pa.get("parallel_batches", 0)
    requests = pb.get("speculative_admission_requests", 0) - pa.get("speculative_admission_requests", 0)
    checks = pb.get("speculative_containment_checks", 0) - pa.get("speculative_containment_checks", 0)
    commit = pb.get("ordered_commit_wall_seconds", 0) - pa.get("ordered_commit_wall_seconds", 0)
    prep = pb.get("preparation_wall_seconds", 0) - pa.get("preparation_wall_seconds", 0)
    out.update({"prepared_batch_records": records, "parallel_batches": batches,
                "commit_us_per_prepared_record": 1e6 * commit / records if records else None,
                "prep_us_per_parallel_batch": 1e6 * prep / batches if batches else None,
                "speculative_admission_requests": requests,
                "speculative_checks_per_request": checks / requests if requests else None})
    # W1.1 SoA kernel timers (binaries from d9163195 on; absent on 4a17f9c7/7eed68fc).
    # coordinator_duty.admission_kernel: coordinator scan wall, NESTED inside ordered_commit_seconds.
    # admission_preparation.speculative_kernel: helper scan wall summed over helpers (inside preparation).
    ka, kb = da.get("admission_kernel") or {}, db.get("admission_kernel") or {}
    if isinstance(kb, dict) and kb:
        k = {}
        for key in ("forward_scans", "forward_candidates", "forward_word_rejections", "forward_lane_rejections",
                    "forward_exact_tests", "forward_scan_seconds", "reverse_scans", "reverse_candidates",
                    "reverse_word_rejections", "reverse_lane_rejections", "reverse_exact_tests",
                    "reverse_scan_seconds", "reverse_prepared_candidates", "reverse_examined_candidates"):
            if kb.get(key) is not None:
                k[key] = kb[key] - (ka.get(key) or 0)
        scan = k.get("forward_scan_seconds", 0.0) + k.get("reverse_scan_seconds", 0.0)
        commit_bucket = db.get("ordered_commit_seconds", 0.0) - da.get("ordered_commit_seconds", 0.0)
        k["scan_seconds"] = scan
        k["scan_share_of_coordinator_wall"] = scan / wall if wall > 0 else None
        k["scan_share_of_ordered_commit"] = scan / commit_bucket if commit_bucket > 0 else None
        for side in ("forward", "reverse"):
            cand = k.get(f"{side}_candidates")
            k[f"{side}_ns_per_candidate"] = 1e9 * k.get(f"{side}_scan_seconds", 0.0) / cand if cand else None
        # From 9e1c2175 the reverse timer spans the whole retirement call and its denominator is every
        # examined candidate (helper-decided included); d9163195 divided by commit-decided IDs only.
        examined = k.get("reverse_examined_candidates")
        if examined:
            k["reverse_retire_ns_per_examined_candidate"] = 1e9 * k.get("reverse_scan_seconds", 0.0) / examined
        out["admission_kernel"] = k
    sa, sb = pa.get("speculative_kernel") or {}, pb.get("speculative_kernel") or {}
    if isinstance(sb, dict) and sb:
        key = "scan_wall_seconds_summed_over_helpers"
        scan = (sb.get(key) or 0.0) - (sa.get(key) or 0.0)
        cand = (sb.get("candidates") or 0) - (sa.get("candidates") or 0)
        out["speculative_kernel"] = {
            key: scan, "candidates": cand,
            "ns_per_candidate": 1e9 * scan / cand if cand else None,
            "helper_cpus_equivalent": scan / wall if wall > 0 else None,
            "per_request": cand / requests if requests else None}
    return out


def rate_between(a, b):
    dt = b["observed_unix_time"] - a["observed_unix_time"]
    de = (b.get("elapsed_seconds") or 0) - (a.get("elapsed_seconds") or 0)
    out = {"observed_seconds": dt, "native_elapsed_seconds": de}
    for key in ("expanded", "committed_domains", "discovered", "committed_events", "delegated_publications",
                "native_publications", "transferred_obligations", "pending_native_publications"):
        if a.get(key) is not None and b.get(key) is not None:
            out[key + "_delta"] = b[key] - a[key]
            out[key + "_per_hour"] = (b[key] - a[key]) / de * 3600 if de > 0 else None
    if a.get("slot_busy_seconds_sum") is not None and b.get("slot_busy_seconds_sum") is not None and de > 0:
        out["mean_busy_inspector_slots"] = (b["slot_busy_seconds_sum"] - a["slot_busy_seconds_sum"]) / de
    if a.get("closure_initial_closed") is not None and b.get("closure_initial_closed") is not None:
        out["initial_closed_start"], out["initial_closed_end"] = a["closure_initial_closed"], b["closure_initial_closed"]
    return out


def nearest(rows, t, key="observed_unix_time", after=True):
    cands = [r for r in rows if (r[key] >= t if after else r[key] <= t)]
    if not cands:
        return None
    return min(cands, key=lambda r: abs(r[key] - t))


def reclassify(threads):
    """run1's harness classified helpers by the untruncated name; comm is 15 bytes."""
    for row in threads.values():
        if row.get("class") == "other" and row.get("comm", "").startswith("owner-admission"):
            row["class"] = "admission_helper"
    return threads


def thread_delta(s0, s1):
    t0, t1 = s0["threads"], s1["threads"]
    wall = s1["unix_time"] - s0["unix_time"]
    classes = {}
    for tid, r1 in t1.items():
        r0 = t0.get(tid)
        if r0 is None:
            continue
        c = classes.setdefault(r1["class"], {"threads": 0, "cpu_s": 0.0, "utime_s": 0.0, "stime_s": 0.0,
                                              "run_delay_s": 0.0, "voluntary": 0, "involuntary": 0,
                                              "migrations": 0})
        c["threads"] += 1
        c["cpu_s"] += (r1.get("se.sum_exec_runtime", 0) - r0.get("se.sum_exec_runtime", 0)) / 1000.0
        c["utime_s"] += r1.get("utime_s", 0) - r0.get("utime_s", 0)
        c["stime_s"] += r1.get("stime_s", 0) - r0.get("stime_s", 0)
        c["run_delay_s"] += (r1.get("run_delay_ns", 0) - r0.get("run_delay_ns", 0)) / 1e9
        c["voluntary"] += r1.get("nr_voluntary_switches", 0) - r0.get("nr_voluntary_switches", 0)
        c["involuntary"] += r1.get("nr_involuntary_switches", 0) - r0.get("nr_involuntary_switches", 0)
        c["migrations"] += r1.get("se.nr_migrations", 0) - r0.get("se.nr_migrations", 0)
    for c in classes.values():
        c["cpus"] = c["cpu_s"] / wall
        c["cpu_share_per_thread"] = c["cpu_s"] / wall / c["threads"]
        c["run_delay_share_per_thread"] = c["run_delay_s"] / wall / c["threads"]
        c["voluntary_per_s"] = c["voluntary"] / wall
        c["involuntary_per_s"] = c["involuntary"] / wall
        c["migrations_per_s"] = c["migrations"] / wall
        c["system_share"] = c["stime_s"] / (c["utime_s"] + c["stime_s"]) if c["utime_s"] + c["stime_s"] else None
    return {"wall_seconds": wall, "classes": classes}


def parse_perf_stat(path, comm_class):
    """perf stat -x, --per-thread output: comm-tid,value,unit,event,..."""
    per_class = {}
    threads_per_class = {}
    seen = set()
    if not path.exists():
        return None
    for row in csv.reader(open(path)):
        if len(row) < 4 or row[0].startswith("#"):
            continue
        thread, value, _unit, event = row[0], row[1], row[2], row[3]
        try:
            value = float(value)
        except ValueError:
            continue
        tid = thread.rsplit("-", 1)[-1]
        cls = comm_class.get(tid, "other")
        if tid not in seen and value > 0:
            seen.add(tid)
            threads_per_class[cls] = threads_per_class.get(cls, 0) + 1
        d = per_class.setdefault(cls, {})
        d[event] = d.get(event, 0.0) + value
    out = {}
    for cls, d in per_class.items():
        cyc, ins = d.get("cycles:u", 0), d.get("instructions:u", 0)
        dram = d.get("ls_dmnd_fills_from_sys.dram_io_near:u", 0) + d.get("ls_dmnd_fills_from_sys.dram_io_far:u", 0)
        out[cls] = {"threads": threads_per_class.get(cls, 0), "counters": d, "ipc": ins / cyc if cyc else None,
                    "cache_misses_per_kinstr": 1000 * d.get("cache-misses:u", 0) / ins if ins else None,
                    "dtlb_misses_per_kinstr": 1000 * d.get("dTLB-load-misses:u", 0) / ins if ins else None,
                    "dram_demand_fills_per_kinstr": 1000 * dram / ins if ins else None,
                    "dram_far_share": d.get("ls_dmnd_fills_from_sys.dram_io_far:u", 0) / dram if dram else None}
    return out


def perf_flat(perf, data, top=40):
    out = {}
    for sort in ("dso", "symbol"):
        res = subprocess.run([perf, "report", "-i", str(data), "--stdio", "--no-children", "--sort", sort,
                              "--percent-limit", "0.3", "-g", "none"], capture_output=True, text=True)
        rows = []
        for line in res.stdout.splitlines():
            line = line.rstrip()
            if not line or line.lstrip().startswith("#"):
                continue
            parts = line.split(None, 1)
            if parts and parts[0].endswith("%"):
                rows.append([float(parts[0][:-1]), parts[1].strip() if len(parts) > 1 else ""])
        out[sort] = rows[:top]
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("run", type=Path)
    p.add_argument("--perf", required=True)
    p.add_argument("--skip-perf", action="store_true")
    p.add_argument("--end", type=float, help="override the end of the measured interval (unix time), "
                   "e.g. when the run was contaminated before its stop request")
    args = p.parse_args()
    run = args.run
    tl = json.load(open(run / "timeline.json"))
    timeline, windows = tl["timeline"], tl["windows"]
    if args.end is not None:
        timeline["measured_end_override_unix_time"] = args.end
        timeline["stop_request_unix_time"] = min(args.end, timeline.get("stop_request_unix_time") or args.end)
    meta = json.load(open(run / "meta.json"))
    hb = load_jsonl(run / "heartbeats.jsonl")
    duty_rows = [r for r in hb if r.get("coordinator_duty")]
    samples = load_jsonl(run / "samples.jsonl")
    for s in samples:
        reclassify(s.get("threads") or {})
    out = {"run": str(run), "binary": meta["binary"], "binary_sha256": meta["binary_sha256"],
           "cpus": meta["cpus"], "foreign_busy_cpus_before": meta.get("foreign_busy_cpus_before")}
    launch = timeline["launch_unix_time"]
    t0 = timeline.get("traversal_unix_time")
    stop = timeline.get("stop_request_unix_time")
    out["timeline"] = {k: (v - launch if isinstance(v, float) and k.endswith("unix_time") else v)
                       for k, v in timeline.items() if k not in ("host_meminfo_after",)}
    restore = [m for m in tl.get("markers", []) + hb if m.get("event") == "checkpoint_restored" and m.get("restore")]
    if restore:
        out["restore_report"] = restore[0].get("restore")
    # Duty and throughput over the measured interval [T, stop).
    pre_stop = [r for r in duty_rows if stop is None or r["observed_unix_time"] <= stop]
    if len(pre_stop) >= 2:
        a, b = pre_stop[0], pre_stop[-1]
        out["duty"] = {"session_T_to_stop": duty_between(a, b),
                       "session_totals_at_stop": duty_between({"coordinator_duty": {k: 0.0 for k in BUCKETS} | {"coordinator_elapsed_seconds": 0.0}}, b)}
        out["throughput"] = {"T_to_stop": rate_between(a, b)}
        # 5-minute slices
        slices = []
        start = a
        for r in pre_stop:
            if r["observed_unix_time"] - start["observed_unix_time"] >= 300:
                slices.append({"from_T_seconds": start["observed_unix_time"] - a["observed_unix_time"],
                               "duty": duty_between(start, r), "rate": rate_between(start, r)})
                start = r
        # The trailing remainder (run2: 297 s) is kept when it spans at least 2 min.
        tail = pre_stop[-1]
        if tail is not start and tail["observed_unix_time"] - start["observed_unix_time"] >= 120:
            slices.append({"from_T_seconds": start["observed_unix_time"] - a["observed_unix_time"],
                           "partial": True, "duty": duty_between(start, tail), "rate": rate_between(start, tail)})
        out["slices_5min"] = slices
    for tag, win in windows.items():
        if "end_unix_time" not in win:
            continue
        a = nearest(duty_rows, win["start_unix_time"], after=False) or nearest(duty_rows, win["start_unix_time"])
        b = nearest(duty_rows, win["end_unix_time"], after=True) or nearest(duty_rows, win["end_unix_time"], after=False)
        if a and b and a is not b:
            out.setdefault("duty", {})[f"window_{tag}"] = duty_between(a, b)
            out.setdefault("throughput", {})[f"window_{tag}"] = rate_between(a, b)
    # RAM
    ram = {}
    if duty_rows:
        first = [r for r in hb if t0 and r["observed_unix_time"] >= t0 and r.get("rss_bytes") and r.get("discovered")]
        last = [r for r in hb if (stop is None or r["observed_unix_time"] <= stop) and r.get("rss_bytes") and r.get("discovered")]
        if first and last:
            f, l = first[0], last[-1]
            ram["at_T"] = {"rss_bytes": f["rss_bytes"], "discovered": f["discovered"],
                           "bytes_per_domain": f["rss_bytes"] / f["discovered"]}
            ram["at_stop"] = {"rss_bytes": l["rss_bytes"], "discovered": l["discovered"],
                              "bytes_per_domain": l["rss_bytes"] / l["discovered"]}
            dd = l["discovered"] - f["discovered"]
            ram["marginal_T_to_stop_bytes_per_domain"] = (l["rss_bytes"] - f["rss_bytes"]) / dd if dd else None
            fit = [r for r in last if r["observed_unix_time"] >= t0 + 60]
            if len(fit) > 10:
                xs = [r["discovered"] for r in fit]
                ys = [r["rss_bytes"] for r in fit]
                mx, my = statistics.fmean(xs), statistics.fmean(ys)
                sxx = sum((x - mx) ** 2 for x in xs)
                if sxx > 0:
                    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
                    ss_res = sum((y - (my + slope * (x - mx))) ** 2 for x, y in zip(xs, ys))
                    ss_tot = sum((y - my) ** 2 for y in ys)
                    ram["fit_after_T+60s"] = {"bytes_per_domain": slope, "r2": 1 - ss_res / ss_tot if ss_tot else None,
                                              "points": len(fit), "discovered_range": [min(xs), max(xs)]}
        # Samples taken while the native exits carry a reduced status (run2: the
        # second-to-last has only Threads, the last is empty); skip missing fields.
        hwm = [s["status"]["VmHWM"] for s in samples
               if s.get("status") and s["status"].get("VmHWM") is not None]
        if hwm:
            ram["peak_vmhwm_bytes"] = max(hwm) * 1024
        rss_restore = [s for s in samples if s.get("status") and s["status"].get("VmRSS") is not None
                       and timeline.get("restored_unix_time")
                       and s["unix_time"] <= timeline["restored_unix_time"] + 5]
        if rss_restore:
            ram["rss_at_restore_bytes"] = rss_restore[-1]["status"]["VmRSS"] * 1024
        ram["status_samples_without_vm_fields"] = sum(
            1 for s in samples if isinstance(s.get("status"), dict)
            and (s["status"].get("VmHWM") is None or s["status"].get("VmRSS") is None))
        mem = [s["mem_available"] for s in samples if s.get("mem_available")]
        if mem:
            ram["host_memavailable_min_bytes"] = min(mem)
        numa = run / "numa.json"
        if numa.exists():
            doc = json.load(open(numa))
            pages = doc.get("pages_by_node", {})
            total = sum(pages.values())
            if total:
                ram["numa_pages_by_node"] = pages
                ram["numa_socket1_share"] = sum(v for k, v in pages.items() if int(k) >= 4) / total
                ram["numa_read_seconds"] = doc.get("seconds")
    out["ram"] = ram
    # Threads
    snaps = {}
    for path in run.glob("sched-*.json"):
        snaps[path.stem[len("sched-"):]] = json.load(open(path))
        reclassify(snaps[path.stem[len("sched-"):]]["threads"])
    live = [s for s in samples if s.get("threads")]
    if t0 and live:
        s_first = min((s for s in live if s["unix_time"] >= t0), key=lambda s: s["unix_time"], default=None)
        s_last = max((s for s in live if stop is None or s["unix_time"] <= stop), key=lambda s: s["unix_time"], default=None)
        if s_first and s_last and s_last is not s_first:
            out["threads"] = {"T_to_stop": thread_delta(s_first, s_last)}
            foreign = [s["foreign_busy_cpus_estimate"] for s in live
                       if s.get("foreign_busy_cpus_estimate") is not None and s_first["unix_time"] <= s["unix_time"] <= s_last["unix_time"]]
            if foreign:
                out["foreign"] = {"mean_busy_cpus": statistics.fmean(foreign), "max_busy_cpus": max(foreign),
                                  "p90_busy_cpus": sorted(foreign)[int(0.9 * (len(foreign) - 1))]}
    for tag in windows:
        if f"{tag}-start" in snaps and f"{tag}-end" in snaps:
            out.setdefault("threads", {})[f"window_{tag}"] = thread_delta(snaps[f"{tag}-start"], snaps[f"{tag}-end"])
    # perf stat
    comm_class = {}
    for s in snaps.values():
        for tid, r in s["threads"].items():
            comm_class[tid] = r["class"]
    for tag in windows:
        stat = parse_perf_stat(run / f"perf-stat-{tag}.csv", comm_class)
        if stat:
            out.setdefault("perf_stat", {})[tag] = stat
    # perf record
    if not args.skip_perf:
        perf_out = {}
        for data in sorted(run.glob("perf-*.data")):
            if data.stat().st_size == 0:
                perf_out[data.name] = {"empty": True}
                continue
            entry = perf_flat(args.perf, data)
            if "coordinator" in data.name:
                entry["attribution"] = perf_attribution.attribute(args.perf, data, role="coordinator")
            else:
                entry["attribution"] = perf_attribution.attribute(args.perf, data, role="worker")
            perf_out[data.name] = entry
            # Stand-alone copy, named like run1's hand-made attr-<window>-<class>.json files.
            attr_path = run / ("attr-" + data.stem[len("perf-"):] + ".json")
            json.dump(entry["attribution"], open(attr_path, "w"), indent=1)
        out["perf"] = perf_out
    json.dump(out, open(run / "analysis.json", "w"), indent=1)
    print(json.dumps(out, indent=1)[:20000])


if __name__ == "__main__":
    main()
