#!/usr/bin/env bash
# batchC: proc_cpu_policy (the two tests procfix6 saw fail under a 4-CPU pin) pinned to this batch's CPUs, then with
# >= 16 allowed CPUs (the suite's broad mask); each in a private namespace like LIGHT_PIDNS=1.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6
PCP=/common/build-cache/vhirschi/6b/06499e14506925/debug/deps/proc_cpu_policy-72b29ad162b6ebda
MASK=0-127,163,176-191,216-257,261,263-270,272-287,320-334,336-341,343-361,364-377,379-383
t=$S/tmp/pcp.$$; mkdir -p $t
for m in "$CPUS" "$MASK"; do
  echo "=== proc_cpu_policy with affinity $m ($(taskset -c "$m" nproc) allowed CPUs) ($(date -u +%FT%TZ))"
  TMPDIR=$t timeout -s KILL 600 taskset -c "$m" unshare -Urpf --mount-proc --kill-child -- $PCP --test-threads=1 2>&1 | grep -E "^test |panicked|out of bounds|test result" | head -12
done
echo "=== batchC done $(date -u +%FT%TZ)"
