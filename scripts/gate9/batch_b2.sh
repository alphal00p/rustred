#!/usr/bin/env bash
# gate8 (b) part 2 (rerun after the mutation batch): CLEAN rebuild of the worktree, check that target/debug/rr_campaign is
# byte-identical to the clean build copied at the start (S/bin/debug), then the suite test binaries, each in its own ns.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9
W=$WS/wt/ac-audit-gate8; cd $W || exit 1
git diff --quiet -- crates || { echo "worktree not clean: abort"; exit 1; }
nice -n 10 $WS/plan/auto-campaign/implementer_ops/light_build.sh gate8 test -p rustred-campaign --test procfix3_item0 --test procfix7 --test gate7_repro --test gate8_lineage --no-run > $S/logs/b2_build.txt 2>&1 || { echo "build failed"; exit 1; }
a=$(sha256sum < $W/target/debug/rr_campaign | cut -c1-16); b=$(sha256sum < $S/bin/debug/rr_campaign | cut -c1-16)
echo "worktree rr_campaign $a, clean copy $b: $( [ "$a" = "$b" ] && echo IDENTICAL || echo DIFFERENT)"
[ "$a" = "$b" ] || { echo "abort: the worktree binary is not the clean build"; exit 1; }
exe() { grep "Executable $1" $S/logs/b2_build.txt | sed 's/.*(\(.*\))/\1/' | tail -1; }
I0=$(exe tests/procfix3_item0.rs); P7=$(exe tests/procfix7.rs); G7=$(exe tests/gate7_repro.rs); G8=$(exe tests/gate8_lineage.rs)
nst() { local t=$S/tmp/b2.$$.$RANDOM; mkdir -p $t; TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=64m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
res() { grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed' | tail -1; }
echo "--- m-rm: procfix3_item0 campaign_directory_removed_during_a_resume_is_never_exit_0 x20"; p=0; f=0
for i in $(seq 1 20); do o=$(nst $I0 --exact campaign_directory_removed_during_a_resume_is_never_exit_0 --nocapture --test-threads=1 2>&1)
  if echo "$o" | grep -q '1 passed'; then p=$((p+1)); else f=$((f+1)); echo "RUN $i FAILED: $(echo "$o" | grep -E 'panicked|code' | head -3)"; fi
  echo "  run $i: $(echo "$o" | grep -o 'removal after [0-9]* ms: code [0-9]*' | awk '{print $NF}' | sort | uniq -c | tr '\n' ' ')"; done
echo "m-rm RESULT pass=$p fail=$f"
for t in m_rw_a_removed_external_work_dir_is_a_typed_end_with_a_true_message mj2_a_session_in_another_time_namespace_never_hides_a_marked_process b1_a_failed_recovery_read_stops_typed_and_decides_nothing an_unreadable_intent_log_never_gives_a_clean_end m_nr_run_dirs_that_cannot_be_created_end_typed mj_b_settle_reads_nothing_from_a_run_dir_that_is_not_ours m3a_full_intent_log_uses_the_tmpfs_fallback_and_the_restart_reads_it; do
  for k in 1 2 3; do echo "procfix7 $t run $k: $(nst $P7 --exact $t --test-threads=1 2>&1 | res)"; done; done
echo "procfix7 b1_preload (ignored): $(nst $P7 --exact b1_preload_fault_on_the_second_manifest_open --ignored --test-threads=1 2>&1 | res)"
for k in 1 2; do echo "gate7_repro run $k: $(nst $G7 --test-threads=1 2>&1 | res)"; done
echo "gate8_lineage (incl. ignored): $(nst $G8 --include-ignored 2>&1 | res)"
echo "=== done $(date -u +%FT%TZ)"
