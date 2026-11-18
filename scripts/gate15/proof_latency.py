# proof latency per job of a campaign directory: ledger 'finished' utc minus the job's own last event (EVENTS.jsonl:
# job_started utc + the largest t). Read-only.
import json, sys, glob, os, datetime
def ts(u): return datetime.datetime.strptime(u.rstrip('Z'), "%Y-%m-%dT%H:%M:%S.%f").timestamp()
c = sys.argv[1]
fin = {}
for f in glob.glob(c + "/ledger/*.jsonl"):
    for line in open(f, errors="replace"):
        try: r = json.loads(line)
        except ValueError: continue
        e = r.get("event", {})
        if e.get("kind") == "job" and e["body"]["ev"].get("ev") == "finished":
            fin[(e["body"]["job"], e["body"]["attempt"])] = (ts(r["utc"]), e["body"]["ev"].get("class"))
lat = []
for d in glob.glob(c + "/jobs/*"):
    ev = os.path.join(d, "EVENTS.jsonl")
    if not os.path.exists(ev): continue
    start = None; tmax = 0.0; job = att = None
    for line in open(ev, errors="replace"):
        try: x = json.loads(line)
        except ValueError: continue
        if x.get("kind") == "job_started": start = ts(x["utc"]); job = x["job_id"]; att = x["attempt"]
        tmax = max(tmax, float(x.get("t", 0)))
    if start is None or (job, att) not in fin: continue
    lat.append((fin[(job, att)][0] - (start + tmax), job, fin[(job, att)][1]))
lat.sort()
v = [x[0] for x in lat]
def p(q): return v[min(len(v) - 1, int(round((len(v) - 1) * q)))] if v else float('nan')
print(f"jobs {len(v)}: proof latency p50 {p(.5):.3f} s p90 {p(.9):.3f} s p99 {p(.99):.3f} s max {p(1):.3f} s; slowest {lat[-3:] if lat else []}")
