#!/usr/bin/env python
"""Read-only census of CP5 edge segments: self loops, target age.
Usage: edges_census.py FILE MAX_EDGES"""
import sys
from array import array
from collections import Counter

path = sys.argv[1]
max_edges = int(sys.argv[2])
with open(path, "rb") as fh:
    head = fh.read(32)
    count = int.from_bytes(head[16:24], "little")
    first = int.from_bytes(head[24:32], "little")
    n = min(count, max_edges)
    a = array("I")
    a.fromfile(fh, 2 * n)
src = a[0::2]
dst = a[1::2]
selfl = 0
older = 0
younger = 0
per_source = Counter()
gap_hist = Counter()
sources_with_self = set()
for s, t in zip(src, dst):
    per_source[s] += 1
    if s == t:
        selfl += 1
        sources_with_self.add(s)
    elif t < s:
        older += 1
        d = s - t
        gap_hist["older<1e2" if d < 100 else "older<1e4" if d < 10**4 else "older<1e6" if d < 10**6 else "older>=1e6"] += 1
    else:
        younger += 1
        d = t - s
        gap_hist["younger<1e2" if d < 100 else "younger<1e4" if d < 10**4 else "younger<1e6" if d < 10**6 else "younger>=1e6"] += 1
deg = Counter()
for s, c in per_source.items():
    deg["1" if c == 1 else "<=4" if c <= 4 else "<=16" if c <= 16 else "<=64" if c <= 64 else "<=256" if c <= 256 else "<=1024" if c <= 1024 else ">1024"] += 1
print({"file": path, "segment_count": count, "first": first, "sampled": n,
       "self_loops": selfl, "older": older, "younger": younger,
       "sources": len(per_source), "sources_with_self_loop": len(sources_with_self),
       "gap_hist": dict(gap_hist), "outdeg_hist": dict(deg),
       "min_src": min(src), "max_src": max(src), "max_dst": max(dst)})
