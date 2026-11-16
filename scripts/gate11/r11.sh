#!/usr/bin/env bash
# gate11 r11.sh LOGPREFIX EXE [test args...] : run one test binary as pid 1 of a PRIVATE user+pid+mount namespace with a private
# tmpfs on /dev/shm and a private TMPDIR under the gate11 scratch; full output to $S/logs/LOGPREFIX.txt; prints the summary.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11
lp=$1; e=$2; shift 2
t=$S/tmp/rt.$$.$RANDOM; mkdir -p $t
TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=64m tmpfs /dev/shm && touch /dev/shm/.gate11-private && exec "$0" "$@"' "$e" "$@" > $S/logs/$lp.txt 2>&1
rc=$?
[ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/tmp/?*) rm -r -- "$t";; esac
echo "=== $lp rc=$rc $(date -u +%FT%TZ): $(grep -E '^test result' $S/logs/$lp.txt | tail -1)"
grep -E 'panicked|FAILED|failed|exit 0 with' $S/logs/$lp.txt | head -5
exit $rc
