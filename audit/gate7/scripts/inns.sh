#!/usr/bin/env bash
# inns.sh SCRIPT ARGS... : run a gate5 inner script in a fresh private user+pid+mount namespace pinned to CPUS, with a
# private TMPDIR. exec chain bash -> taskset -> unshare (same pid): a SIGKILL from an outer `timeout` hits unshare, whose
# --kill-child then SIGKILLs the namespace's pid 1, which tears the whole namespace down.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
t=$S/tmp/ns.$$.$RANDOM; mkdir -p "$t"
TMPDIR=$t exec taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- $BASH_BIN "$@"
