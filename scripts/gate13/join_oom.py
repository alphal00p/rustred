import json, sys
seg, oomf = sys.argv[1], sys.argv[2]
oom = set(l.strip() for l in open(oomf) if l.strip())
unit, fin, jobsig, cont = {}, {}, {}, {}
for line in open(seg):
    r = json.loads(line); e = r["event"]; b = e.get("body") or {}
    if e["kind"] == "job":
        k = (b["job"], b["attempt"]); ev = b["ev"]
        if ev.get("ev") == "launch" and ev.get("scope_unit"):
            unit[k] = ev["scope_unit"] + ".scope"
        if ev.get("ev") == "finished":
            fin[k] = (ev.get("class"), ev.get("signal"))
    if e["kind"] == "signal_sent" and b.get("by", "").startswith("job signal"):
        jobsig.setdefault(b.get("job"), []).append((b.get("signal"), b.get("pid"), r["utc"]))
    if e["kind"] == "note" and b.get("text", "").startswith("job_containment "):
        j = json.loads(b["text"][len("job_containment "):]); cont[(j["job"], j["attempt"])] = j["containment"]
rows = {}
for k, u in unit.items():
    if u in oom:
        c = fin.get(k, ("?", None))[0]
        rows.setdefault(c, []).append(k)
print("oom-kill failed scopes:", sum(len(v) for v in rows.values()), {c: len(v) for c, v in rows.items()})
km = [k for k, v in fin.items() if v[0] == "killed_memory"]
print("killed_memory ends:", len(km), "of which scope failed oom-kill:", sum(unit.get(k) in oom for k in km))
print("job-signal SIGKILL records:", sum(len(v) for v in jobsig.values()), "jobs", sorted(jobsig))
for k in sorted(km):
    print("  killed_memory", k, "scope", "FAILED oom-kill" if unit.get(k) in oom else "not failed", "containment", cont.get(k))
