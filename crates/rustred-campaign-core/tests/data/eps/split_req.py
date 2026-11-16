# RecordRequest (import check fact_requests) -> rr_proof instantiate form (A) requests, ONE FILE PER CHART (frozen
# kernels group requests by rule index; an eps table restarts index per chart). Inspect-only helper.
import json, sys
src, outbase = sys.argv[1], sys.argv[2]
by = {}
for l in open(src):
    q = json.loads(l)
    with open(q["table"], "rb") as f:
        f.seek(q["offset"]); line = f.readline().rstrip(b"\n")
    v = json.loads(line)
    assert v["index"] == q["rule"], q
    import hashlib
    assert hashlib.sha256(line).hexdigest() == q["record_sha256"], q
    # engine-line7b reads an optional "chart" (and an offset always reads the exact record)
    by.setdefault(v["chart"], []).append({"table": q["table"], "offset": q["offset"], "rule": q["rule"],
                                          "chart": v["chart"], "keys": q["keys"]})
# child jobs of at most MAXK keys (env SPLIT_MAX_KEYS, default 500000): a record's keys may be split across jobs
import os
maxk = int(os.environ.get("SPLIT_MAX_KEYS", "500000"))
names = []
for c, reqs in sorted(by.items()):
    part, used, out = 0, 0, []
    def flush():
        global part, used, out
        if out:
            name = f"{c}.{part}"
            with open(f"{outbase}.{name}", "w") as o:
                for r in out: o.write(json.dumps(r) + "\n")
            names.append(name)
            part += 1; used = 0; out = []
    for r in reqs:
        keys = r["keys"]
        while keys:
            room = maxk - used
            if room <= 0:
                flush(); room = maxk
            take, keys = keys[:room], keys[room:]
            out.append(dict(r, keys=take)); used += len(take)
    flush()
print(" ".join(names))
