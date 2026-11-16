#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; T=$S/bin/tests
nsrun() { local t=$S/tmp/lf.$$.$RANDOM; mkdir -p $t; TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
a=0; b=0; c=0; d=0
for i in $(seq 1 40); do o=$(nsrun $T/ledger_lock-3bcb60cfdd767ab2 --test-threads=4 2>&1); echo "$o" | grep -q "test result: ok" && a=$((a+1)) || { b=$((b+1)); echo "$o" | grep -E "^test .*FAILED|panicked" | head -2 | sed "s/^/  run $i (parallel): /"; }; done
for i in $(seq 1 40); do o=$(nsrun $T/ledger_lock-3bcb60cfdd767ab2 --exact second_lock_in_process_is_refused_and_release_allows_fresh --test-threads=1 2>&1); echo "$o" | grep -q "test result: ok" && c=$((c+1)) || { d=$((d+1)); echo "$o" | grep panicked | head -1 | sed "s/^/  run $i (alone): /"; }; done
echo "LOCKFLAKE whole binary --test-threads=4: pass $a fail $b; the test alone: pass $c fail $d"
