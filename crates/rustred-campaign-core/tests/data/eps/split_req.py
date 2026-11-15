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
    by.setdefault(v["chart"], []).append({"table": q["table"], "offset": q["offset"], "rule": q["rule"], "keys": q["keys"]})
for c, reqs in by.items():
    with open(f"{outbase}.{c}", "w") as o:
        for r in reqs: o.write(json.dumps(r) + "\n")
print(" ".join(sorted(by)))
