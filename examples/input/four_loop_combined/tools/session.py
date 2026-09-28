#!/usr/bin/env python
"""Session driver for queued combined four-loop walks (socket 1 or any fixed CPU set).

Run it UNDER the lock that covers its CPUs, e.g. for socket 1:

  flock -w 14400 /common/dev/rustred/TMP/locks/socket1.lock python session.py QUEUE.json NAME

QUEUE.json is a list of items {family, label, command, policy, workers, timeout, cpus} with the optional
keys stop_natives (-> run_c4l.py --stop-natives), budget_seconds (duration assumed for the budget check
instead of timeout; for items with an early native stop), binary (default: the frozen legacy binary
4a17f9c7) and arm (a free-form tag such as "legacy" or "candidate", written to the log).

The driver logs the SHA-256 of QUEUE.json first: for a C-4L-comb-R comparison the queue IS the
pre-registration (arms, widths, n, order, CPUs, caps), so it must be written before the session starts and
must not be edited afterwards. Items run in queue order while the hold budget allows (C4L_BUDGET, default
3,420 s: an item starts only if elapsed + budget_seconds|timeout + 120 s <= budget). Items not started are
written to NAME.remaining.json. C4L_YIELD_FILE: stop starting items once this path exists (lets a
neighbouring lane take the lock). C4L_SESSION_DIR: where NAME.log, NAME.remaining.json and the per-run logs
go (default /common/dev/rustred/TMP/c4l-s2).

Successor of TMP/c4l-s2/session.py (sha256 451149b5..., sessions B2 and C); the changes are the queue
digest, the per-item binary and arm, sys.executable and the committed runner next to this file.
"""
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(os.environ.get("C4L_SESSION_DIR", "/common/dev/rustred/TMP/c4l-s2"))
RUNNER = Path(__file__).resolve().with_name("run_c4l.py")
BIN = "/common/dev/rustred/TMP/fable51-controls/bin/rustred-4a17f9c7"
BUDGET = int(os.environ.get("C4L_BUDGET", "3420"))
YIELD = os.environ.get("C4L_YIELD_FILE")


def log(name, msg):
    line = f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} {msg}"
    print(line, flush=True)
    with open(HERE / f"{name}.log", "a") as f:
        f.write(line + "\n")


def main():
    queue_path = Path(sys.argv[1])
    name = sys.argv[2]
    HERE.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256(queue_path.read_bytes()).hexdigest()
    queue = json.loads(queue_path.read_bytes())
    t0 = time.time()
    avail = [l for l in open('/proc/meminfo') if l.startswith('MemAvailable')][0].split()[1]
    log(name, f"lock acquired; queue {queue_path} sha256 {digest}; {len(queue)} items; MemAvailable_kB={avail}")
    remaining = []
    for i, item in enumerate(queue):
        elapsed = time.time() - t0
        if elapsed + item.get("budget_seconds", item["timeout"]) + 120 > BUDGET:
            remaining = queue[i:]
            log(name, f"budget: {len(remaining)} items deferred (elapsed {elapsed:.0f} s)")
            break
        if YIELD and os.path.exists(YIELD):
            remaining = queue[i:]
            log(name, f"yield: {YIELD} exists; {len(remaining)} items deferred (elapsed {elapsed:.0f} s)")
            break
        binary = item.get("binary", BIN)
        log(name, f"start {item['family']} {item['label']} arm={item.get('arm')} binary={binary} "
                  f"policy={item['policy']} W={item['workers']} cpus={item['cpus']} timeout={item['timeout']} "
                  f"stop_natives={item.get('stop_natives')}")
        cmd = [sys.executable, str(RUNNER), "--binary", binary, "--command", item["command"],
               "--family", item["family"], "--label", item["label"], "--cpus", item["cpus"],
               "--policy", item["policy"], "--workers", str(item["workers"]),
               "--timeout-seconds", str(item["timeout"])]
        if item.get("stop_natives"):
            cmd += ["--stop-natives", str(item["stop_natives"])]
        with open(HERE / f"run-{item['label']}-{item['family']}.log", "w") as out:
            rc = subprocess.call(cmd, stdout=out, stderr=subprocess.STDOUT)
        log(name, f"end {item['family']} {item['label']} rc={rc} elapsed_session={time.time() - t0:.0f}s")
    json.dump(remaining, open(HERE / f"{name}.remaining.json", "w"), indent=1)
    log(name, f"lock released (hold {time.time() - t0:.0f} s)")


if __name__ == "__main__":
    main()
