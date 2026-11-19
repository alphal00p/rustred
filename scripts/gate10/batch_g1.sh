#!/usr/bin/env bash
# gate10 (g)/(2): gate9's reproduction tests and procfix9 (incl. ignored) on 3550b2ac, one binary at a time
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; T=$S/bin/tests; R=$S/scripts/r10.sh
$R g1_gate9_ledger_create $T/gate9_ledger_create --include-ignored --test-threads=1 --nocapture
$R g1_gate9_churn $T/gate9_churn --include-ignored --test-threads=1 --nocapture
$R g1_gate9_orig_ledger_lock $T/gate9_orig_ledger_lock --include-ignored --test-threads=1
$R g1_gate9_pf3 $T/gate9_pf3 --include-ignored --test-threads=1
$R g1_procfix9 $T/procfix9 --test-threads=1 --nocapture
$R g1_procfix9_ign_probe $T/procfix9 --ignored --exact g9_b2_fork_churner_crashed_session_fast_probe --nocapture --test-threads=1
$R g1_procfix9_ign_g9churn $T/procfix9 --ignored --exact g9_b2_gate9_churner_never_gives_exit_0 --nocapture --test-threads=1
$R g1_procfix9_ign_preload $T/procfix9 --ignored --exact g9_b1_gate9_preload_injector_never_destroys_the_ledger --nocapture --test-threads=1
