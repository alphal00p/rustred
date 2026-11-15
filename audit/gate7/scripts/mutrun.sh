#!/usr/bin/env bash
# gate7 mutation check of procfix7's own tests, in the SOURCE COPY $S/mut (never the worktree): apply one mutation, build,
# run the tests that should notice it (namespaced: LIGHT_PIDNS=1), restore the file from the worktree (unchanged source).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; W=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate7
LB=/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/implementer_ops/light_build.sh
cd $S/mut
run() { env -u LIGHT_SCCACHE LIGHT_PIDNS=1 XDG_RUNTIME_DIR=$S/xdg nice -n 10 $LB gate7 test -p rustred-campaign "$@" 2>&1 | grep -E "^test |test result|error\[|panicked|FAILED" | grep -v "^test result: ok. 0 passed" | cut -c1-220; }
restore() { cp $W/crates/rustred-campaign/src/$1 $S/mut/crates/rustred-campaign/src/$1; cmp -s $W/crates/rustred-campaign/src/$1 $S/mut/crates/rustred-campaign/src/$1 && echo "restored $1"; }
for spec in "M1_lineage_targets|resources/lineage.rs|--lib -- lineage --test-threads=2|--test procfix7_lineage -- --test-threads=1" \
            "M2_claim_refusal|supervisor/workdir.rs|--lib -- mj_b_work_dir_claim|--test procfix7 -- mj_b_copy --test-threads=1" \
            "M3_m4a_evidence|supervisor/mod.rs|--lib -- m4_disk_attributed|--test procfix6 -- m4_sigxfsz --test-threads=1" \
            "M4_mx_check|supervisor/failpoint.rs|--lib -- m_x_failpoint|--test procfix7 -- m_x --test-threads=1" \
            "M5_settle_identity|supervisor/mod.rs|--test procfix7 -- mj_b_settle --test-threads=1"; do
  IFS='|' read -r name file t1 t2 <<< "$spec"
  echo "=== $name ($(date -u +%FT%TZ))"
  python3 -I $S/scripts/mutate.py $S/mut $name || { echo "mutation not applied"; continue; }
  echo "--- $t1"; run $t1
  [ -n "$t2" ] && { echo "--- $t2"; run $t2; }
  restore $file
done
echo "=== mutation run done $(date -u +%FT%TZ)"
