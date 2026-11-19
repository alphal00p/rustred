#!/usr/bin/env bash
# gate9: after the mutation run: build my gate9 tests in the (clean) source copy S/mut, run gate9_churn (expected to FAIL on
# 9876cba2), then the ORIGINAL ledger_lock test 200x at the unit's CPU quota with 5 suite binaries looping as load.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; M=$S/mut; cd $M
until grep -q '=== done' $S/logs/mutrun.txt; do sleep 20; done
[ -z "$(git status --porcelain crates)" ] || { echo "mut copy not clean: abort"; exit 1; }
W=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate9/crates/rustred-campaign/tests
cp $W/gate9_churn.rs $W/gate9_churn.c $W/gate9_orig_ledger_lock.rs $W/gate9_ledger_create.rs $W/gate9_g9fault.c crates/rustred-campaign/tests/
LB=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/implementer_ops/light_build.sh
XDG_RUNTIME_DIR=$S/xdg $LB gate9mut test -p rustred-campaign --test gate9_churn --test gate9_orig_ledger_lock --test ledger_basic --test ledger_crash --test ledger_rotation --test sup_ops --test procfix5 --no-run > $S/logs/followon_build.txt 2>&1 || { echo BUILD FAILED; tail -20 $S/logs/followon_build.txt; exit 1; }
exe() { grep -oE "\(/common/build-cache[^)]*/$1-[0-9a-f]+\)" $S/logs/followon_build.txt | tr -d '()' | head -1; }
nsrun() { local t=$S/tmp/fo.$$.$RANDOM; mkdir -p $t; TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m tmpfs /dev/shm && exec "$0" "$@"' "$@"; local rc=$?; [ -n "$t" ] && case "$t" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/?*) rm -r -- "$t";; esac; return $rc; }
echo "--- gate9_churn (ignored, expected to FAIL on 9876cba2)"
o=$(nsrun $(exe gate9_churn) --ignored --nocapture --test-threads=1 2>&1); echo "$o" | grep -E "panicked|exit 0 with|test result" | cut -c1-260
echo "--- ORIGINAL ledger_lock crashed_supervisor test x200 under load ($(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max))"
STOP=$S/tmp/fo.stop.$$
load() { while [ ! -e $STOP ]; do nsrun $1 --test-threads=4 > /dev/null 2>&1; done; }
for b in ledger_basic ledger_crash ledger_rotation sup_ops procfix5; do load $(exe $b) & done
pass=0; fail=0; L=$(exe gate9_orig_ledger_lock)
for i in $(seq 1 200); do
  out=$(nsrun $L --exact crashed_supervisor_with_live_child_is_taken_over_and_recorded --test-threads=1 2>&1)
  if echo "$out" | grep -q 'test result: ok. 1 passed'; then pass=$((pass+1)); else fail=$((fail+1)); echo "RUN $i FAILED: $(echo "$out" | grep -E 'panicked|left:|right:|carries' | head -3 | tr '\n' ' ')"; fi
done
touch $STOP; wait; case "$STOP" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/tmp/fo.stop.*) rm -f -- "$STOP";; esac
echo "ORIGINAL ledger_lock RESULT runs=200 pass=$pass fail=$fail"
echo "=== done $(date -u +%FT%TZ)"
