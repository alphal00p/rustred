#!/usr/bin/env bash
# gate11 mutrun11.sh MUT... : per mutation: apply in the source copy S/mut, build the targeted tests (light_build), run the
# selected tests of each binary as pid 1 of a private ns (private /dev/shm), restore (git checkout in S/mut's own repo).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; M=$S/mut; cd $M
LB=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/implementer_ops/light_build.sh
nst() { local t=$S/tmp/m.$$.$RANDOM; mkdir -p $t; TMPDIR=$t timeout -s KILL 2400 unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=64m tmpfs /dev/shm && touch /dev/shm/.gate11-private && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/tmp/?*) rm -r -- "$t";; esac; return $rc; }
# per mutation: "binary=filter filter ..." items separated by ';' (lib filters skip tui/closeloop)
sel() { case $1 in
  M0) echo "rustred_campaign=resources:: launch:: supervisor::;procfix10=g10_b1_truncated_root_listing_crashed_session g10_batch_rv g10_m10_1 g10_m4 g10_m5 g10_m6 g10_m10_5 g10_the_marker_only;gate9_churn=;procfix5=real_children;gate11_walk=g11_walk_e2e_reached";;
  M1|M2) echo "rustred_campaign=resources::procfs;procfix10=g10_b1_truncated_root_listing_crashed_session g10_b1_truncated_root_listing_live_session g10_b1_truncation_control;gate11_walk=g11_walk_e2e_reached";;
  M3) echo "rustred_campaign=resources::;gate9_churn=";;
  M4|M5|M6) echo "rustred_campaign=launch::sweep;procfix10=g10_batch_rv g10_m10_1 g10_m4 g10_m5 g10_m6 g10_b1_truncated_root_listing_crashed_session";;
  M7) echo "procfix10=g10_the_marker_only;procfix5=real_children";;
  M8) echo "rustred_campaign=launch::cgroup";;
  M9) echo "rustred_campaign=supervisor::;procfix10=g10_m5 g10_m4 g10_m10_1";;
  M10) echo "procfix10=g10_m10_5";;
  esac; }
for mut in "$@"; do
  echo "######## $mut ($(date -u +%T))"
  [ $mut = M0 ] || python3 -I $S/scripts/mutate11.py $mut || { echo "apply failed"; git checkout -q -- crates; continue; }
  XDG_RUNTIME_DIR=/run/user/1002 nice -n 10 $LB gate11mut test -p rustred-campaign --lib --test procfix10 --test gate9_churn --test procfix5 --test gate11_walk --no-run > $S/logs/mut11_$mut.build.txt 2>&1 || { echo "BUILD FAILED"; grep -E '^error' -A5 $S/logs/mut11_$mut.build.txt | head -12; git checkout -q -- crates; continue; }
  IFS=';' read -ra items <<< "$(sel $mut)"
  for it in "${items[@]}"; do
    name=${it%%=*}; filt=${it#*=}
    exe=$(grep -oE '\(/common/build-cache[^)]*/'"$name"'-[0-9a-f]*\)' $S/logs/mut11_$mut.build.txt | tr -d '()' | head -1)
    [ -n "$exe" ] || { echo "  $name: no executable"; continue; }
    extra=""; [ $name = rustred_campaign ] && extra="--skip tui:: --skip closeloop::"
    o=$(nst $exe $filt $extra --test-threads=2 2>&1); r=$(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed' | tail -1)
    echo "  $name [$filt]: $r"; echo "$o" | grep -E '^test .* FAILED$' | head -8 | sed 's/^/    /'
  done
  git checkout -q -- crates; [ -z "$(git status --porcelain crates)" ] && echo "  restored clean" || echo "  NOT CLEAN"
done
echo "=== done $(date -u +%FT%TZ)"
