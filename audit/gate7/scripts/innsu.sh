#!/usr/bin/env bash
# innsu.sh SCRIPT ARGS... : like inns.sh, but the namespace maps our uid to 1000 and leaves root UNMAPPED: every exec'd
# process is capability-less (host-like ptrace / non-dumpable semantics). Private user+pid+mount namespace.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
t=$S/tmp/nsu.$$.$RANDOM; mkdir -p "$t"
TMPDIR=$t exec taskset -c "$CPUS" unshare --map-user=1000 --map-group=1000 -pf --mount-proc --kill-child -- $BASH_BIN "$@"
