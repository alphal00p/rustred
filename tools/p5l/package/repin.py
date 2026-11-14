#!/usr/bin/env python3
"""repin.py RUN_DIR NEEDLE [PERIOD]: while RUN_DIR/rc is absent, every PERIOD seconds (300) pin the threads of MY process
whose command line contains NEEDLE one per CPU of RUN_DIR/cpus (round robin; busy threads first). The kernel does not
balance a multi-NUMA CPU set (p5l-library-3, 2026-11-12: 13 of 16 threads on one CPU). Affinity only: no signal is sent,
and only processes of this user whose executable is p5l_library are touched."""
import os
import sys
import time

run, needle = sys.argv[1], sys.argv[2]
period = int(sys.argv[3]) if len(sys.argv) > 3 else 300
uid = os.getuid()
while not os.path.exists(os.path.join(run, 'rc')):
    try:
        cpus = [int(x) for x in open(os.path.join(run, 'cpus')).read().strip().split(',')]
    except OSError:
        time.sleep(30)
        continue
    for pid in os.listdir('/proc'):
        if not pid.isdigit():
            continue
        try:
            if os.stat(f'/proc/{pid}').st_uid != uid or not os.readlink(f'/proc/{pid}/exe').endswith('p5l_library'):
                continue
            if needle not in open(f'/proc/{pid}/cmdline').read().replace('\0', ' '):
                continue
            tids = os.listdir(f'/proc/{pid}/task')
            stats = []
            for tid in tids:
                f = open(f'/proc/{pid}/task/{tid}/stat').read().rsplit(')', 1)[1].split()
                stats.append((f[0] != 'R', int(tid)))
            stats.sort()
            for i, (_, tid) in enumerate(stats):
                os.sched_setaffinity(tid, {cpus[i % len(cpus)]})
        except (OSError, ValueError, IndexError):
            continue
    time.sleep(period)
