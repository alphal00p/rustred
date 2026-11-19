#!/usr/bin/env bash
# gate10 mutrun10.sh MUT... : for each mutation: apply in the source copy S/mut, build the targeted tests (light_build), run
# each binary as pid 1 of a private ns (private /dev/shm), restore with git checkout in S/mut's own scratch repo, verify clean.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; M=$S/mut; cd $M; export XDG_RUNTIME_DIR=$S/xdg
LB=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/implementer_ops/light_build.sh
nst() { local t=$S/tmp/m.$$.$RANDOM; mkdir -p $t; TMPDIR=$t timeout -s KILL 1500 unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=64m tmpfs /dev/shm && touch /dev/shm/.gate10-private && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/tmp/?*) rm -r -- "$t";; esac; return $rc; }
TESTS="--lib --test procfix9 --test procfix9_token --test procfix9_lockowner --test gate9_ledger_create --test gate9_churn --test gate10_sites --test ledgerfix2_ledger --test proc_cpu_policy --test ledger_lock --test audit_ledger2_faults"
for mut in "$@"; do
  echo "######## $mut ($(date -u +%T))"
  [ $mut = M0 ] || python3 -I $S/scripts/mutate10.py $mut || { echo "apply failed"; git checkout -q -- crates; continue; }
  $LB gate10mut test -p rustred-campaign $TESTS --no-run > $S/logs/mut_$mut.build.txt 2>&1 || { echo "BUILD FAILED"; grep -E '^error' -A5 $S/logs/mut_$mut.build.txt | head -12; git checkout -q -- crates; continue; }
  for exe in $(grep -oE '\(/common/build-cache[^)]*\)' $S/logs/mut_$mut.build.txt | tr -d '()'); do
    name=$(basename $exe | sed 's/-[0-9a-f]*$//')
    case $name in rustred_campaign) filt="--skip tui:: --skip closeloop::";; *) filt="";; esac
    o=$(nst $exe $filt --test-threads=1 2>&1); r=$(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed' | tail -1)
    echo "  $name: $r"; echo "$o" | grep -E '^test .* FAILED$|^---- ' | head -6 | sed 's/^/    /'
  done
  git checkout -q -- crates; [ -z "$(git status --porcelain crates)" ] && echo "  restored clean" || echo "  NOT CLEAN"
done
echo "=== done $(date -u +%FT%TZ)"
