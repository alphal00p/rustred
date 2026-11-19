#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
echo "##### (d) release"; timeout -s KILL 900 scripts/ns8r.sh scripts/g8_d_inner.sh d1 2>&1 | tail -20
echo "##### (c)"; VARS="x1 x2 x3 x4 x5" TAGSUF=g9 bash scripts/batch_c.sh 2>&1 | grep -E "^---|rc|RESULT|refus|fallback" | cut -c1-220
echo "##### (b)"; bash scripts/batch_b.sh 2>&1 | cut -c1-260
T=$S/bin/tests
nst() { local t=$S/tmp/b.$$.$RANDOM; mkdir -p $t; TMPDIR=$t taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
echo "##### procfix8 (incl. ignored g8 preload), gate8_lineage"
o=$(nst $T/procfix8-9f3b36a33d2248a2 --include-ignored --test-threads=1 2>&1); echo "procfix8: $(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed')"
o=$(nst $T/gate8_lineage-e6d83cce3ea12bc5 --include-ignored --test-threads=1 2>&1); echo "gate8_lineage: $(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed')"
echo "=== done $(date -u +%FT%TZ)"
