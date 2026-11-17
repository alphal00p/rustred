"""procfix7 round 9 (gate13 G13-M1 / G13-M2): host step 1's recorded events REPLAYED UNDER THE ROUND-9 RULES and graded
by a given grader module. A simulation [inferred], stated rule by rule; the recorded run itself stays read-only.

Usage: python3 -I -B replay_r9.py GRADER_DIR FIXTURE_DIR OOM_UNITS_FILE
  GRADER_DIR      directory holding grade_host_step.py / p8_units.py (imported unchanged)
  FIXTURE_DIR     host step 1's recorded files (fixtures/host-step-1: c/ledger, run.log, scopes.peak, units.seen,
                  CPU_LEDGER.extract.md)
  OOM_UNITS_FILE  the rr-<id>-* scopes the user journal shows "Failed with result 'oom-kill'" (39)

Rules applied to the recorded ledger (a writable temporary copy):
 (M1) every attempt noted "marker_only ... requested but not verified" (the 25 that died 74-126 ms after their scope
      started, OOM-killed INSIDE it per the journal) -> job_containment "scope_acknowledged_ended" from the kernel's
      record of its leader's cgroup at the exit (the exact unit, " (deleted)": the form the real-systemd check of round 9
      read on this host for a scope removed after its process exited);
 (M2) every attempt whose scope failed with oom-kill (39 = 30 recorded "lost" + 9 killed_memory) -> a `scope_result`
      note (failed / oom-kill), then the reset by its exact name: none of them is loaded after the run (units.after
      and SCOPES_AFTER empty); the 30 SIGKILL ends without a kill reason of ours also get `scope_oom_kill` (they would be
      killed_memory; their recorded classes and the requeues that followed are NOT re-simulated: P1-P8 do not read them);
 (W)  the 3 unproven_end notes of the round-7 walk: variant A keeps them (the walk of that build), variant B drops them
      (the round-8 walk, gate12 G12-M1 + 8b (c'), assumed to prove those lists).
"""

import json
import os
import shutil
import stat
import sys
import tempfile

GRADER, FIX, OOMF = sys.argv[1:4]
sys.path.insert(0, GRADER)
import grade_host_step as G  # noqa: E402

CID = "host-fake-1.3cc7bd03c55c83b7"
ROOT = "/sys/fs/cgroup/user.slice/user-1002.slice/user@1002.service/app.slice"
oom = {x.strip() for x in open(OOMF) if x.strip()}


def writable_copy():
    d = tempfile.mkdtemp(prefix="r9-replay-")
    for base, dirs, files in os.walk(FIX):
        rel = os.path.relpath(base, FIX)
        os.makedirs(os.path.join(d, rel), exist_ok=True)
        for f in files:
            dst = os.path.join(d, rel, f)
            shutil.copyfile(os.path.join(base, f), dst)
            os.chmod(dst, stat.S_IRUSR | stat.S_IWUSR)
    for f in ("stopped.txt", "units.before", "units.after"):
        open(os.path.join(d, f), "w").close()
    return d


def rewrite(d, keep_unproven):
    seg = os.path.join(d, "c", "ledger", "segment-000001.jsonl")
    recs = [json.loads(l) for l in open(seg)]
    unit, fin = {}, {}
    for r in recs:
        e = r["event"]
        b = e.get("body") or {}
        if e["kind"] == "job":
            k, ev = (b["job"], b["attempt"]), b["ev"]
            if ev.get("ev") == "launch" and ev.get("scope_unit"):
                unit[k] = ev["scope_unit"]
            if ev.get("ev") == "finished":
                fin[k] = ev
    out, n = [], {"m1": 0, "result": 0, "oomclass": 0, "unproven_dropped": 0}
    for r in recs:
        e = r["event"]
        b = e.get("body") or {}
        t = b.get("text", "") if e["kind"] == "note" else ""
        if t.startswith("unproven_end ") and not keep_unproven:
            n["unproven_dropped"] += 1
            continue
        if t.startswith("job_containment ") and '"containment":"marker_only"' in t:
            j = json.loads(t[len("job_containment "):])
            u = unit[(j["job"], j["attempt"])]
            j["containment"] = "scope_acknowledged_ended"
            j["why"] = (f"scope {u} existed: the kernel's record of the job's leader at its exit names it ({ROOT}/{u}.scope, "
                        "removed since); the job ended before its cgroup was checked")
            b["text"] = "job_containment " + json.dumps(j, separators=(",", ":"))
            n["m1"] += 1
        if e["kind"] == "job" and b["ev"].get("ev") == "finished":
            k = (b["job"], b["attempt"])
            u = unit.get(k)
            if u and u + ".scope" in oom:
                note = {"job": k[0], "attempt": k[1], "unit": u + ".scope", "active": "failed", "result": "oom-kill",
                        "when": "ended"}
                out.append({"event": {"kind": "note", "body": {"text": "scope_result " + json.dumps(note, separators=(",", ":"))}}})
                n["result"] += 1
                if b["ev"].get("signal") == 9 and b["ev"].get("class") == "lost":
                    out.append({"event": {"kind": "note", "body": {"text": "scope_oom_kill " + json.dumps(
                        {"job": k[0], "attempt": k[1], "unit": u}, separators=(",", ":"))}}})
                    n["oomclass"] += 1
        out.append(r)
    with open(seg, "w") as f:
        f.write("".join(json.dumps(r) + "\n" for r in out))
    return n


for name, keep in (("A (round-7 walk's 3 unproven_end kept)", True), ("B (round-8 walk: unproven_end dropped)", False)):
    d = writable_copy()
    try:
        n = rewrite(d, keep)
        res = G.grade(d, CID, 0, 0, 0, 0, "--run", os.path.join(d, "CPU_LEDGER.extract.md"), 0, "/nonexistent")
    finally:
        shutil.rmtree(d)
    print(f"== REPLAY UNDER THE ROUND-9 RULES, variant {name}: {n}")
    print("   " + " ".join(f"{k} {v}" for k, v, _ in res))
    for k, v, why in res:
        print(f"   {k} {v}: {why[:260]}")
