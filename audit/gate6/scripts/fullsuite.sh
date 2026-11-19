#!/usr/bin/env bash
# fullsuite.sh LOG : whole-crate rustred-campaign tests (debug, --no-fail-fast) with LIGHT_PIDNS=1 (each test binary in
# unshare -Urpf --mount-proc). "Unpinned": affinity = every host CPU except the reserved ranges of pick_free.py / rule 6
# (so the tests see >= 16 allowed CPUs); concurrency bounded by --test-threads=4 (cargo -j 4). CPUS (from withcpus.sh)
# is the ledger budget held while it runs.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
cd $WS/wt/audit-gate6 || exit 2
ulimit -c 0
MASK=0-127,163,176-191,216-257,261,263-270,272-287,320-334,336-341,343-361,364-377,379-383
echo "budget CPUs (flock): $CPUS; affinity mask: $MASK; start $(date -u +%FT%TZ)"
LIGHT_PIDNS=1 taskset -c "$MASK" $WS/plan/auto-campaign/implementer_ops/light_build.sh gate6 test -p rustred-campaign --no-fail-fast -- --test-threads=4
rc=$?; echo "cargo test rc=$rc end $(date -u +%FT%TZ)"; exit $rc
