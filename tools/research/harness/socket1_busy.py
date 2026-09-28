#!/usr/bin/env python3
"""Mean busy % of a CPU range over N seconds, from /proc/stat (load gate of the W0.3 sessions).

  socket1_busy.py [SECONDS [FIRST_CPU [LAST_CPU]]]      default: 60 s, CPUs 128-223 (socket-1 nodes 4-6)

Busy = non-idle, non-iowait jiffies. Prints one integer percent.
"""
import sys
import time


def snap():
    t = {}
    for line in open("/proc/stat"):
        x = line.split()
        if x[0].startswith("cpu") and x[0] != "cpu":
            v = list(map(int, x[1:9]))
            t[int(x[0][3:])] = (sum(v), v[3] + v[4])
    return t


n = float(sys.argv[1]) if len(sys.argv) > 1 else 60
lo = int(sys.argv[2]) if len(sys.argv) > 2 else 128
hi = int(sys.argv[3]) if len(sys.argv) > 3 else 223
a = snap()
time.sleep(n)
b = snap()
cs = [c for c in range(lo, hi + 1) if c in a]
print(int(100 * sum(((b[c][0] - b[c][1]) - (a[c][0] - a[c][1])) / max(1, b[c][0] - a[c][0]) for c in cs) / len(cs)))
