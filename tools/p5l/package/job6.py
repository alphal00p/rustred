#!/usr/bin/env python3
"""job6.py: run one job of lane vk5l-wp6 on N free CPUs under the site policy.

job6.py --tag TAG --ncpu N --mem 16G --max-seconds S --out DIR [--lane vk5l-wp6] -- CMD...

- CPUs from SCRATCHPAD/audit-closure/tools/pick_free.py (idle, flock free, no live ledger claim, not reserved,
  not in the foreign 5L lists); build-slot CPUs are excluded there too (pick.py RESERVED).
- every CPU's ops/locks/cpu.<n> flock is taken NON-BLOCKING and held (inherited by the job) for its life;
- one claim line and one release line APPENDED to ops/CPU_LEDGER.md (never rewritten);
- the job runs in a transient systemd scope (MemoryMax, MemorySwapMax=0) under ops/guarded_run.py
  (refuses to start / stops below the 50 GB MemAvailable floor), pinned with taskset;
- admission: MemAvailable - cap >= 50 GB (re-read before the start);
- DIR/{stdout.log,stderr.log,cpus,rc,guard/}.  A finished DIR (rc 0) is skipped.
"""
import argparse
import fcntl
import os
import re
import subprocess
import sys
import time

WS = '/shared/vhirschi/t3code/vakint_dev/p_integral_dev'
PICK = WS + '/SCRATCHPAD/audit-closure/tools/pick_free.py'
LEDGER = WS + '/ops/CPU_LEDGER.md'
NEVER = set(range(168, 176)) | set(range(288, 320)) | set(range(192, 216)) | {258, 259, 260, 262, 271, 335}


def utc():
    return time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())


def mem_avail_gb():
    for line in open('/proc/meminfo'):
        if line.startswith('MemAvailable:'):
            return int(line.split()[1]) / 1e6
    return 0.0


def cap_gb(text):
    text = text.strip().upper()
    if text.endswith('G'):
        return float(text[:-1])
    if text.endswith('M'):
        return float(text[:-1]) / 1e3
    return float(text) / 1e9


def ledger(line):
    with open(LEDGER, 'a') as handle:  # APPEND ONLY
        handle.write(line + '\n')


def pick(n):
    out = subprocess.run([sys.executable, '-I', PICK, str(n * 2)], capture_output=True, text=True).stdout
    cpus = [int(x) for x in re.findall(r'\d+', out) if int(x) not in NEVER]
    held = []
    for cpu in cpus:
        fd = os.open(f'{WS}/ops/locks/cpu.{cpu}', os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            os.close(fd)
            continue
        held.append((cpu, fd))
        if len(held) == n:
            break
    if len(held) < n:
        for _, fd in held:
            os.close(fd)
        return []
    return held


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--tag', required=True)
    ap.add_argument('--ncpu', type=int, default=1)
    ap.add_argument('--mem', default='8G')
    ap.add_argument('--max-seconds', type=int, default=3600)
    ap.add_argument('--out', required=True)
    ap.add_argument('--lane', default='vk5l-wp6')
    ap.add_argument('--wait', type=int, default=1800, help='seconds to wait for CPUs / memory')
    ap.add_argument('cmd', nargs=argparse.REMAINDER)
    a = ap.parse_args()
    cmd = a.cmd[1:] if a.cmd and a.cmd[0] == '--' else a.cmd
    os.makedirs(a.out, exist_ok=True)
    rcf = os.path.join(a.out, 'rc')
    if os.path.exists(rcf) and open(rcf).read().strip() == '0':
        print(f'skip {a.tag} (rc 0)')
        return 0
    if os.path.exists(rcf):
        os.remove(rcf)
    # Rule 15 (user, 2026-11-15): never wait for a pinned CPU. Only the memory admission waits (up to --wait);
    # if the picker cannot give the CPUs at once, the job starts UNPINNED: CPUQuota = 100 % x ncpu, AllowedCPUs =
    # every CPU except the reserved sets (NEVER above: 168-175, 288-319, build slots 192-215, slot 4).
    deadline = time.time() + a.wait
    held = []
    while True:
        if os.path.exists(WS + '/ops/PAUSE'):
            print('PAUSE present: not starting', file=sys.stderr)
            open(rcf, 'w').write('98\n')
            return 98
        if mem_avail_gb() - cap_gb(a.mem) >= 50:
            break
        if time.time() >= deadline:
            open(rcf, 'w').write('99\n')
            return 99
        time.sleep(20)
    held = pick(a.ncpu)
    unit = f'vk5l-wp6-{a.tag}-{os.getpid()}'
    if held:
        cpus = ','.join(str(c) for c, _ in held)
        open(os.path.join(a.out, 'cpus'), 'w').write(cpus + '\n')
        ledger(f'{utc()} ({a.lane}): + CPU {cpus} (flock) -> {a.tag} (MemoryMax {a.mem}). Released at its end.')
        scope = ['-p', f'MemoryMax={a.mem}', '-p', 'MemorySwapMax=0']
        pin = ['taskset', '-c', cpus]
    else:
        allowed = ','.join(str(c) for c in range(os.cpu_count() or 384) if c not in NEVER)
        cpus = f'unpinned:{a.ncpu}'
        open(os.path.join(a.out, 'cpus'), 'w').write(cpus + '\n')
        ledger(f'{utc()} ({a.lane}): unpinned CPUQuota {100 * a.ncpu}% (taskset: all CPUs minus the reserved sets) -> '
               f'{a.tag} (MemoryMax {a.mem}). Released at its end.')
        # AllowedCPUs is not applied in a user scope here (cpuset not delegated, checked 2026-11-15): taskset instead.
        scope = ['-p', f'MemoryMax={a.mem}', '-p', 'MemorySwapMax=0', '-p', f'CPUQuota={100 * a.ncpu}%']
        pin = ['taskset', '-c', allowed]
    full = ['systemd-run', '--user', '--scope', '-q', f'--unit={unit}'] + scope + [
            '--', sys.executable, WS + '/ops/guarded_run.py', '--out',
            os.path.join(a.out, 'guard'), '--min-avail-gb', '50', '--max-seconds', str(a.max_seconds), '--'] + pin + cmd
    open(os.path.join(a.out, 'cmd'), 'w').write(' '.join(full) + '\n')
    with open(os.path.join(a.out, 'stdout.log'), 'w') as so, open(os.path.join(a.out, 'stderr.log'), 'w') as se:
        rc = subprocess.run(full, stdout=so, stderr=se, pass_fds=[fd for _, fd in held]).returncode
    open(rcf, 'w').write(f'{rc}\n')
    ledger(f'{utc()} ({a.lane}): RELEASED {cpus} ({a.tag} ended rc {rc})')
    for _, fd in held:
        os.close(fd)
    return rc


if __name__ == '__main__':
    sys.exit(main())
