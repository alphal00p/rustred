#!/usr/bin/env bash
# gate8 mutation check: each mutation in the worktree source (UNCOMMITTED), build, run the tests that should notice it
# (each test binary in its own private namespace + private /dev/shm), restore with git checkout of that file, verify clean.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9
W=$WS/wt/ac-audit-gate8; C=crates/rustred-campaign/src; cd $W || exit 1
git diff --quiet -- crates || { echo "worktree not clean: abort"; exit 1; }
nst() { local t=$S/tmp/mu.$$.$RANDOM; mkdir -p $t; TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=64m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
exe() { grep "Executable $1" $S/logs/mut_build.txt | sed 's/.*(\(.*\))/\1/' | tail -1; }
run1() { local bin=$1; shift; local o; o=$(nst $bin "$@" --test-threads=1 2>&1); echo "    $(basename $bin | cut -d- -f1) $*: $(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed' | tail -1)"; echo "$o" | grep -m2 'panicked' | cut -c1-220 | sed 's/^/      /'; }
for m in "M1 $C/supervisor/mod.rs" "M2 $C/supervisor/mod.rs" "M3 $C/supervisor/mod.rs" "M4 $C/supervisor/mod.rs" "M5 $C/launch/guard.rs" "M6 $C/supervisor/mod.rs"; do
  set -- $m; id=$1; f=$2
  echo "=== $id ($(date -u +%FT%TZ))"; python3 -I $S/scripts/mutate8.py $f $id || continue
  nice -n 10 $WS/plan/auto-campaign/implementer_ops/light_build.sh gate8 test -p rustred-campaign --lib --test procfix7 --test gate7_repro --no-run > $S/logs/mut_build.txt 2>&1; b=$?
  echo "  build rc=$b"
  if [ $b = 0 ]; then
    L=$(exe 'unittests src/lib.rs'); P7=$(exe tests/procfix7.rs); G7=$(exe tests/gate7_repro.rs)
    case $id in
      M1) run1 $P7 --exact b1_a_failed_recovery_read_stops_typed_and_decides_nothing;;
      M2) run1 $P7 --exact mj2_a_session_in_another_time_namespace_never_hides_a_marked_process; run1 $L resources::lineage;;
      M3) run1 $P7 --exact mj1_an_unproven_end_is_named_its_lease_released_and_the_run_ends_8;;
      M4) run1 $G7 --exact g7_fallback_intent_log_of_another_campaign_is_never_recorded;;
      M5) run1 $L launch::guard; run1 $P7 --exact m3a_full_intent_log_uses_the_tmpfs_fallback_and_the_restart_reads_it; run1 $G7 --exact g7_fallback_intent_log_of_another_campaign_is_never_recorded;;
      M6) run1 $L resources; run1 $P7 --exact mj_a_non_dumpable_descendant_of_a_job_is_named_and_never_proved_away; run1 $P7 --exact an_unreadable_intent_log_never_gives_a_clean_end;;
    esac
  fi
  git checkout -- $f; git diff --quiet -- crates && echo "  restored: worktree clean" || { echo "  RESTORE FAILED: abort"; exit 1; }
done
echo "=== done $(date -u +%FT%TZ)"
