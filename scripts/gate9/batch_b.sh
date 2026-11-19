#!/usr/bin/env bash
# gate8 (b): lost terminal / signals (gate6 scripts), m-rm 20x, m-rw, removed CDIR, gate7 repro tests; each in its own ns
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; T=$S/bin/tests
nst() { local t=$S/tmp/b.$$.$RANDOM; mkdir -p $t; TMPDIR=$t taskset -c "$CPUS" unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
echo "--- signals (b_inner)"; timeout -s KILL 1800 scripts/ns8r.sh scripts/g8_b_inner.sh 1
echo "--- pty (reg_e7)"; timeout -s KILL 1800 scripts/ns8r.sh scripts/g8_reg_e7_inner.sh 1
echo "--- m-rm: procfix3_item0 removal test x20"; p=0; f=0
for i in $(seq 1 20); do o=$(nst $T/procfix3_item0-c661aeb7ba9b6582 --exact campaign_directory_removed_during_a_resume_is_never_exit_0 --nocapture --test-threads=1 2>&1); if echo "$o" | grep -q '1 passed'; then p=$((p+1)); else f=$((f+1)); echo "RUN $i FAILED: $(echo "$o" | grep -E 'panicked|code|removal' | head -3)"; fi; echo "$o" | grep -o 'removal after [0-9]* ms: code [0-9]*' | sort | uniq -c | sed "s/^/  run $i: /"; done
echo "m-rm RESULT pass=$p fail=$f"
for t in m_rw_a_removed_external_work_dir_is_a_typed_end_with_a_true_message mj2_a_session_in_another_time_namespace_never_hides_a_marked_process b1_a_failed_recovery_read_stops_typed_and_decides_nothing an_unreadable_intent_log_never_gives_a_clean_end m_nr_run_dirs_that_cannot_be_created_end_typed mj_b_settle_reads_nothing_from_a_run_dir_that_is_not_ours m3a_full_intent_log_uses_the_tmpfs_fallback_and_the_restart_reads_it; do
  for k in 1 2 3; do o=$(nst $T/procfix7-a96b621a713ff383 --exact $t --test-threads=1 2>&1); echo "procfix7 $t run $k: $(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed')"; done; done
o=$(nst $T/procfix7-a96b621a713ff383 --exact b1_preload_fault_on_the_second_manifest_open --ignored --test-threads=1 2>&1); echo "procfix7 b1_preload (ignored): $(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed')"
for k in 1 2; do o=$(nst $T/gate7_repro-afc4b9bc5949795c --test-threads=1 2>&1); echo "gate7_repro run $k: $(echo "$o" | grep -o 'test result: [a-zA-Z]*. [0-9]* passed; [0-9]* failed')"; done
echo "=== done $(date -u +%FT%TZ)"
