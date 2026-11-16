# From rr_proof instantiate facts (one file per chart): per key the FIRST record in FILE order with in_case, chart_ok and
# no exception branch: the engine's first match (per chart; a key of another chart has chart_ok false). Test data line:
# {"key":[..],"first":[chart,index]} or "first":null. Inspect-only helper.
import json, sys
meta = json.load(open(sys.argv[1])); base = sys.argv[2]; out = sys.argv[3]
import hashlib
# bind every answer to its record: the echoed record_sha256 must be the sha256 of the line at the requested offset
table = None
for c in meta["charts"]:
    for l in open(base + "." + c):
        table = json.loads(l)["table"]; break
raw = open(table, "rb").read()
sha_of = {}
for (off, c, i) in meta["records"]:
    end = raw.index(b"\n", off)
    sha_of[(c, i)] = hashlib.sha256(raw[off:end]).hexdigest()
ok = {}
for c in meta["charts"]:
    for l in open(base + "." + c + ".facts"):
        v = json.loads(l)
        if v.get("schema") != "rustred.proof-kernel.instantiate.v1": continue
        assert v["error"] is None, v
        assert v["record_sha256"] == sha_of[(c, v["rule"])], (c, v["rule"])
        if v["in_case"] and v["chart_ok"] and v["branch"] is None:
            ok[(c, v["rule"], tuple(v["key"]))] = True
keys = set()
for c in meta["charts"]:
    for l in open(base + "." + c):
        keys |= set(tuple(k) for k in json.loads(l)["keys"]); break
order = meta["records"]  # [offset, chart, index] in file order
n_first = 0
with open(out, "w") as o:
    for k in sorted(keys):
        first = None
        for (_, c, i) in order:
            if ok.get((c, i, k)):
                first = [c, i]; break
        n_first += first is not None
        o.write(json.dumps({"key": list(k), "first": first}, separators=(",", ":")) + "\n")
print(len(keys), "keys,", n_first, "with a first match")
