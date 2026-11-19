#!/usr/bin/env bash
# gate8 ns8u.sh SCRIPT ARGS... : HOST-LIKE: an outer root-mapped user+pid+mount namespace with a PRIVATE /dev/shm, then a
# NESTED user namespace mapping uid/gid 1000 to the outer root and leaving its own root unmapped: every process of the
# script is capability-less (host-like non-dumpable / ptrace semantics). Same pid namespace (pid 1 = the outer shell).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9
BASH_BIN=/home/vhirschi/.nix-profile/bin/bash
: "${CPUS:?CPUS must be set by unit8.sh}"
t=$S/tmp/nsu.$$.$RANDOM; mkdir -p "$t"
TMPDIR=$t exec taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- $BASH_BIN -c 'mount -t tmpfs -o size=32m,mode=1777 tmpfs /dev/shm && touch /dev/shm/.gate9-private && unshare --map-user=1000 --map-group=1000 -U -- "$0" "$@"; rc=$?; exit $rc' $BASH_BIN "$@"
