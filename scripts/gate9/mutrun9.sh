#!/usr/bin/env bash
# gate9 mutrun9.sh: for each mutation: apply in S/mut, build the targeted tests, run them (each binary in its own private
# namespace + private /dev/shm), restore with git checkout in S/mut's own scratch repo, verify clean.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; M=$S/mut; cd $M
LB=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/implementer_ops/light_build.sh
nst() { local t=$S/tmp/m.$$.$RANDOM; mkdir -p $t; TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
TESTS="--lib --test procfix8 --test gate8_lineage --test procfix7 --test ledger_lock --test gate7_lineage --test procfix7_lineage --test proc_audit_regressions --test proc_reaudit_regressions"
for mut in "$@"; do
  echo "######## $mut ($(date -u +%T))"
  [ $mut = M0 ] || python3 -I $S/scripts/mutate9.py $mut || { echo "apply failed"; git checkout -q -- crates; continue; }
  XDG_RUNTIME_DIR=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/xdg $LB gate9mut test -p rustred-campaign $TESTS --no-run > $S/logs/mut_$mut.build.txt 2>&1 || { echo "BUILD FAILED"; tail -5 $S/logs/mut_$mut.build.txt; git checkout -q -- crates; continue; }
  for exe in $(grep -oE '\(/common/build-cache[^)]*\)' $S/logs/mut_$mut.build.txt | tr -d '()'); do
    name=$(basename $exe | sed 's/-[0-9a-f]*$//')
    case $name in rustred_campaign) filt="resources:: launch:: supervisor:: lock::";; procfix7) filt="mj8_1 mj2 b1_a";; *) filt="";; esac
    o=$(nst $exe $filt --test-threads=2 2>&1); r=$(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed' | tail -1)
    echo "  $name [$filt]: $r"; echo "$o" | grep -E '^test .* FAILED$' | head -6 | sed 's/^/    /'
  done
  git checkout -q -- crates; [ -z "$(git status --porcelain crates)" ] && echo "  restored clean" || echo "  NOT CLEAN"
done
echo "=== done $(date -u +%FT%TZ)"
