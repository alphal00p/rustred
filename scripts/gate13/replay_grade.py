"""gate13 item 6: the FROZEN grader (imported unchanged from the frozen directory) on a WRITABLE copy of its fixture,
exactly as test_grade_host_step.py does (as recorded; with the round-8b notes rewrite)."""
import json, os, shutil, sys, tempfile
F, FIXW = sys.argv[1], sys.argv[2]
sys.path.insert(0, F)
import grade_host_step as G
print("grader module:", G.__file__)
CID = "host-fake-1.3cc7bd03c55c83b7"
def out_dir(rewrite=None):
    d = tempfile.mkdtemp(prefix="g13-grade-")
    shutil.copytree(os.path.join(FIXW, "c"), os.path.join(d, "c"))
    for f in ("run.log", "scopes.peak", "units.seen"):
        shutil.copy(os.path.join(FIXW, f), d)
    for f in ("stopped.txt", "units.before", "units.after"):
        open(os.path.join(d, f), "w").close()
    if rewrite:
        seg = os.path.join(d, "c", "ledger", "segment-000001.jsonl")
        lines = [rewrite(json.loads(l)) for l in open(seg)]
        with open(seg, "w") as f:
            f.write("".join(json.dumps(r) + "\n" for r in lines if r is not None))
    return d
def run(d):
    return G.grade(d, CID, 0, 0, 0, 0, "--run", os.path.join(FIXW, "CPU_LEDGER.extract.md"), 0, "/nonexistent")
def rw(r):
    b = r["event"].get("body") or {}
    t = b.get("text", "")
    if t.startswith("unproven_end "):
        return None
    if t.startswith("job_containment ") and '"containment":"marker_only"' in t:
        j = json.loads(t[len("job_containment "):]); unit = j["why"].split(" ")[1]
        j["containment"] = "scope_acknowledged_ended"; j["why"] = f"scope {unit} existed per systemd-run's start acknowledgment"
        b["text"] = "job_containment " + json.dumps(j, separators=(",", ":"))
    return r
for name, f in (("AS RECORDED", None), ("WITH ROUND-8b NOTES", rw)):
    d = out_dir(f)
    res = run(d)
    print(f"== {name}: " + " ".join(f"{k} {v}" for k, v, _ in res))
    for k, v, why in res:
        print(f"   {k} {v}: {why[:230]}")
    shutil.rmtree(d)
