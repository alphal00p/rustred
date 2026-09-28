#!/usr/bin/env python3
"""Foreign-load monitor for a pinned walk process.

Waits for a process whose command line contains --match (e.g. the probe's
--out directory), then every --sample seconds appends one JSON row to OUT:
busy jiffies summed over the CPU set (/proc/stat), the process's own
utime+stime (/proc/PID/stat, all threads), and the interval's foreign share
= (busy - own) / (ncpus * interval). A final summary row ("summary": true)
covers the whole life of the process. Foreign load is what other processes
(other users, other lanes) ran on the same logical CPUs; SMT-sibling and L3
contention from other CPUs is not visible here.

Usage: loadmon.py --cpus 100-117,362-367 --match TMP/w0/inputs/probe-v3b --out FILE
"""
import argparse
import json
import os
import time


def cpu_list(spec):
    out = []
    for part in spec.split(","):
        a, _, b = part.partition("-")
        out.extend(range(int(a), int(b or a) + 1))
    return out


def busy_total(cpus):
    busy = total = 0
    for line in open("/proc/stat"):
        name, *fields = line.split()
        if name.startswith("cpu") and name[3:].isdigit() and int(name[3:]) in cpus:
            v = list(map(int, fields))
            total += sum(v[:8])
            busy += sum(v[:8]) - v[3] - v[4]
    return busy, total


def own_ticks(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            s = f.read()
    except OSError:
        return None
    fields = s[s.rindex(")") + 2:].split()
    return int(fields[11]) + int(fields[12])  # utime, stime (fields 14, 15)


def find(match, exe_hint):
    me = os.getpid()
    for d in os.listdir("/proc"):
        if not d.isdigit() or int(d) == me:
            continue
        try:
            cmd = open(f"/proc/{d}/cmdline", "rb").read().split(b"\0")
        except OSError:
            continue
        if not cmd or exe_hint.encode() not in cmd[0]:
            continue
        if any(match.encode() in a for a in cmd):
            return int(d)
    return None


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--cpus", required=True)
    p.add_argument("--match", required=True)
    p.add_argument("--exe", default="rustred", help="substring of argv[0] of the monitored process")
    p.add_argument("--out", required=True)
    p.add_argument("--sample", type=float, default=30.0)
    p.add_argument("--wait", type=float, default=600.0, help="give up if the process does not appear")
    args = p.parse_args(argv)
    cpus = set(cpu_list(args.cpus))
    hz = os.sysconf("SC_CLK_TCK")
    t_wait = time.time()
    pid = None
    while pid is None and time.time() - t_wait < args.wait:
        pid = find(args.match, args.exe)
        if pid is None:
            time.sleep(1)
    out = open(args.out, "a")
    if pid is None:
        out.write(json.dumps({"error": "process not found", "match": args.match}) + "\n")
        return 1
    b0, t0 = busy_total(cpus)
    o0 = own_ticks(pid) or 0
    w0 = time.time()
    prev = (b0, t0, o0, w0)
    last_own = o0
    while True:
        time.sleep(1.0)
        o = own_ticks(pid)
        alive = o is not None
        if alive:
            last_own = o  # polled every second so at most ~1 s of own time is lost at exit
        w = time.time()
        if alive and w - prev[3] < args.sample:
            continue
        b, t = busy_total(cpus)
        o = last_own
        pb, pt, po, pw = prev
        dbusy, dtot, down = b - pb, t - pt, o - po
        row = {"wall": round(w - w0, 1), "pid": pid, "busy_s": round(dbusy / hz, 2), "own_s": round(down / hz, 2),
               "foreign_s": round(max(0, dbusy - down) / hz, 2),
               "foreign_share": round(max(0, dbusy - down) / dtot, 4) if dtot else None,
               "own_share": round(down / dtot, 4) if dtot else None}
        out.write(json.dumps(row) + "\n")
        out.flush()
        prev = (b, t, o, w)
        if not alive:
            break
    dbusy, dtot, down = prev[0] - b0, prev[1] - t0, prev[2] - o0
    out.write(json.dumps({"summary": True, "cpus": args.cpus, "ncpus": len(cpus), "pid": pid,
                          "wall_s": round(prev[3] - w0, 1), "busy_s": round(dbusy / hz, 1),
                          "own_s": round(down / hz, 1), "foreign_s": round(max(0, dbusy - down) / hz, 1),
                          "foreign_share": round(max(0, dbusy - down) / dtot, 4) if dtot else None,
                          "own_share": round(down / dtot, 4) if dtot else None}) + "\n")
    out.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
