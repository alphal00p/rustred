"""gate13 item 6, SIMULATION [inferred]: host step 1's events as the FROZEN 28e345e1 build would record them on this host's
systemd 261: (a) the 25 dead-before-check attempts -> scope_unverified_ended (systemd-run 261 never writes the line
SCOPE_ACK_PREFIX expects); (b) the 9 killed_memory attempts (our MemoryCap kill reason) keep their FAILED oom-kill scopes
loaded (no --collect; scope_result_class resets only without a kill reason): in units.after, SCOPES_AFTER 9; (c) the 3
unproven_end notes removed (round 8b c' assumed to work)."""
import json, os, shutil, sys, tempfile
F, FIXW, OOMF = sys.argv[1], sys.argv[2], sys.argv[3]
sys.path.insert(0, F)
import grade_host_step as G
CID = "host-fake-1.3cc7bd03c55c83b7"
seg0 = os.path.join(FIXW, "c", "ledger", "segment-000001.jsonl")
unit, cls = {}, {}
for l in open(seg0):
    e = json.loads(l)["event"]; b = e.get("body") or {}
    if e["kind"] == "job":
        ev = b["ev"]; k = (b["job"], b["attempt"])
        if ev.get("ev") == "launch" and ev.get("scope_unit"): unit[k] = ev["scope_unit"] + ".scope"
        if ev.get("ev") == "finished": cls[k] = ev.get("class")
oom = set(x.strip() for x in open(OOMF) if x.strip())
left = sorted(u for k, u in unit.items() if cls.get(k) == "killed_memory" and u in oom)
def rw(r):
    b = r["event"].get("body") or {}; t = b.get("text", "")
    if t.startswith("unproven_end "): return None
    if t.startswith("job_containment ") and '"containment":"marker_only"' in t:
        j = json.loads(t[len("job_containment "):]); u = j["why"].split(" ")[1]
        j["containment"] = "scope_unverified_ended"
        j["why"] = f"scope {u} requested; the job ended before its cgroup was checked and job.stderr holds no start acknowledgment"
        b["text"] = "job_containment " + json.dumps(j, separators=(",", ":"))
    return r
d = tempfile.mkdtemp(prefix="g13-real-")
shutil.copytree(os.path.join(FIXW, "c"), os.path.join(d, "c"))
for f in ("run.log", "scopes.peak", "units.seen"): shutil.copy(os.path.join(FIXW, f), d)
open(os.path.join(d, "stopped.txt"), "w").close(); open(os.path.join(d, "units.before"), "w").close()
open(os.path.join(d, "units.after"), "w").write("".join(u + "\n" for u in left))
seg = os.path.join(d, "c", "ledger", "segment-000001.jsonl")
lines = [rw(json.loads(l)) for l in open(seg)]
open(seg, "w").write("".join(json.dumps(r) + "\n" for r in lines if r is not None))
res = G.grade(d, CID, 0, 0, 0, len(left), "--run", os.path.join(FIXW, "CPU_LEDGER.extract.md"), 0, "/nonexistent")
print("== REALISTIC (frozen build on systemd 261): " + " ".join(f"{k} {v}" for k, v, _ in res))
for k, v, why in res:
    if v != "PASS": print(f"   {k} {v}: {why[:300]}")
shutil.rmtree(d)
