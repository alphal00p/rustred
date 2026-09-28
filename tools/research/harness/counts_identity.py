#!/usr/bin/env python3
"""Per-native result identity between harness runs over the same fixture.

  counts_identity.py BASE_RUN_OR_GROUP RUN_OR_GROUP...

For every native (fixture index `i`) the canonical JSON of its per-effect
counts, native stats and error kind is hashed; every pass of every run must give
the base's value (a group directory's members are merged, e.g. parallelccd).
With SINK=count the counts are the successor/record counts of the inspection
stream, so this checks that a configuration change (replicas, allocator,
placement) did not change what the inspection produced.
"""
import hashlib
import json
import os
import sys

DROP_STATS = ("elapsed", "seconds", "nanos", "_ns", "time")


def members(path):
    if os.path.exists(os.path.join(path, "receipt.json")):
        return [path]
    return sorted(os.path.join(path, m) for m in os.listdir(path)
                  if os.path.exists(os.path.join(path, m, "receipt.json")))


def clean(v):
    if isinstance(v, dict):
        return {k: clean(x) for k, x in v.items() if not any(d in k for d in DROP_STATS)}
    return v


def digests(run):
    out = {}
    for m in members(run):
        with open(os.path.join(m, "natives.jsonl")) as f:
            for line in f:
                r = json.loads(line)
                key = json.dumps([r.get("counts"), clean(r.get("stats")), r.get("error_kind")], sort_keys=True)
                out.setdefault(r["i"], set()).add(hashlib.blake2b(key.encode(), digest_size=16).hexdigest())
    return out


def main():
    base_path, runs = sys.argv[1], sys.argv[2:]
    base = digests(base_path)
    unstable = sum(1 for v in base.values() if len(v) > 1)
    print(f"{base_path}: {len(base)} natives, {unstable} with differing results across passes")
    ok = True
    for run in runs:
        cur = digests(run)
        common = [i for i in cur if i in base]
        differ = [i for i in common if cur[i] != base[i]]
        print(f"{run}: natives {len(cur)}, compared {len(common)}, identical {len(common) - len(differ)}, "
              f"differ {len(differ)}{' e.g. ' + str(differ[:5]) if differ else ''}")
        ok &= not differ
    print("PASS" if ok else "FAIL")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
