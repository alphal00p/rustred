#!/usr/bin/env bash
# gate12 mutrun12.sh MUT... : per mutation: apply in the gate12 worktree (uncommitted), build the named tests (light_build),
# run the named test as pid 1 of a private ns (private /dev/shm), restore (git checkout of crates/rustred-campaign/src).
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12; W=$WS/wt/ac-audit-gate12; cd $W
LB=$WS/plan/auto-campaign/implementer_ops/light_build.sh
source $WS/ops/env.sh >/dev/null 2>&1; export TMPDIR=$S/tmp/mut; mkdir -p $TMPDIR; unset LIGHT_SCCACHE
nst() { local t=$S/tmp/m.$$.$RANDOM; mkdir -p $t; TMPDIR=$t timeout -s KILL 1800 unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=64m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12/tmp/m.*) rm -r -- "$t";; esac; return $rc; }
sel() { case $1 in
  M0) echo "rustred_campaign=launch::cgroup::tests::the_recorded_scope_backstop;procfix11=m11_mut_m4 m11_mut_m9;procfix11_seed=m11_mut_m6";;
  M4) echo "procfix11=m11_mut_m4";;
  M6) echo "procfix11_seed=m11_mut_m6";;
  M8) echo "rustred_campaign=launch::cgroup::tests::the_recorded_scope_backstop";;
  M9) echo "procfix11=m11_mut_m9";;
  esac; }
[ -z "$(git status --porcelain crates/rustred-campaign/src)" ] || { echo "src not clean: abort"; exit 3; }
for mut in "$@"; do
  echo "######## $mut ($(date -u +%T))"
  [ $mut = M0 ] || python3 -I $S/scripts/mutate12.py $mut || { echo "apply failed"; git checkout -q -- crates/rustred-campaign/src; continue; }
  git diff --stat crates/rustred-campaign/src | tail -1
  XDG_RUNTIME_DIR=/run/user/1002 nice -n 10 $LB gate12 test -p rustred-campaign --lib --test procfix11 --test procfix11_seed --no-run > $S/logs/mut12_$mut.build.txt 2>&1 || { echo "BUILD FAILED"; grep -E '^error' -A5 $S/logs/mut12_$mut.build.txt | head -12; git checkout -q -- crates/rustred-campaign/src; continue; }
  IFS=';' read -ra items <<< "$(sel $mut)"
  for it in "${items[@]}"; do
    name=${it%%=*}; filt=${it#*=}
    exe=$(grep -oE '\(/common/build-cache[^)]*/'"$name"'-[0-9a-f]*\)' $S/logs/mut12_$mut.build.txt | tr -d '()' | head -1)
    [ -n "$exe" ] || { echo "  $name: no executable"; continue; }
    o=$(nst $exe $filt --test-threads=1 2>&1); r=$(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed' | tail -1)
    echo "  $name [$filt]: $r"; echo "$o" | grep -E '^test .* (FAILED|ok)$' | head -8 | sed 's/^/    /'; echo "$o" | grep -E "panicked|assert" | head -3 | cut -c1-300 | sed 's/^/    /'
  done
  git checkout -q -- crates/rustred-campaign/src; [ -z "$(git status --porcelain crates/rustred-campaign/src)" ] && echo "  restored clean" || echo "  NOT CLEAN"
done
echo "=== done $(date -u +%FT%TZ)"
