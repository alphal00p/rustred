#!/usr/bin/env bash
# gate7 (e): procfix7's ignored closing-loop interruption test (20 seeds, escalation round) under this unit's CPUQuota 40 %,
# inside a private user+pid+mount namespace (L10); env.sh first, then a fresh TMPDIR (L22).
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7
source $WS/ops/env.sh >/dev/null 2>&1
export TMPDIR=$S/tmp/ecl-$$-$(date +%s); mkdir -p $TMPDIR; export XDG_RUNTIME_DIR=/nonexistent-gate7-xdg
echo "cpu.max of this cgroup: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null)"
unshare -Urpf --mount-proc --kill-child -- $S/bin/tests/procfix7_closeloop --ignored --test-threads=1 --nocapture
echo "e_cl rc=$? $(date -u +%FT%TZ)"
