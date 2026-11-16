#!/usr/bin/env bash
# gate9: N runs of gate9_pf3's sweep test (pid 1 of a private ns, private /dev/shm, private TMPDIR), 3 at a time
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; PAR=${2:-3}
one() { local t=$S/tmp/p3.$$.$RANDOM; mkdir -p $t; o=$(TMPDIR=$t timeout -s KILL 300 unshare -Urpf --mount-proc --kill-child -- $S/bin/tests/gate9_pf3 --exact sweep_kills_only_marked_members_and_never_a_decoy --nocapture --test-threads=1 2>&1); case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac
  echo "PF3B $(echo "$o" | grep -o 'test result: [a-zA-Z]*') $(echo "$o" | grep -o 'GATE9 killed [0-9]*') $(echo "$o" | grep -A3 panicked | tr '\n' ' ' | cut -c1-200)"; }
for i in $(seq 1 $1); do one & [ $((i % ${PAR:-3})) = 0 ] && wait; done; wait
