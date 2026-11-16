#!/usr/bin/env bash
# gate11: the PROCESS-LAYER part of the suite on fdec15da, one binary at a time (each pid 1 of a private ns, private /dev/shm,
# --test-threads=4, ignored tests stay ignored except where noted), result line per binary into logs/suiteP.txt
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; T=$S/bin/tests; out=$S/logs/suiteP.txt
for b in lib proc_launch proc_signals proc_memory proc_pdeathsig proc_cpu_policy proc_audit_regressions proc_reaudit_b proc_reaudit_regressions \
         procfix3_item0 procfix3_limits procfix3_proc procfix3_site_ledger procfix3_sup procfix5 procfix6 procfix7 procfix7_lineage procfix7_tripwire \
         procfix8 procfix8_tripwire procfix9 procfix9_lockowner procfix9_token procfix9_tripwire gate7_lineage gate7_repro gate8_flake gate8_lineage \
         gate9_churn gate9_ledger_create gate9_orig_ledger_lock gate9_pf3 sup_faults sup_i1 sup_identity sup_ops fake_child_signals fake_child_killdrill \
         ledger_lock int5_unconfirmed; do
  extra=""; [ $b = lib ] && extra="--skip tui:: --skip closeloop::"
  $S/scripts/r11.sh suite_$b $T/$b $extra --test-threads=4 > /dev/null 2>&1
  echo "$b: $(grep -E '^test result' $S/logs/suite_$b.txt | tail -1) $(grep -E '^test .* FAILED$' $S/logs/suite_$b.txt | head -3 | tr '\n' ' ')" >> $out
done
echo "=== suiteP done $(date -u +%FT%TZ)" >> $out
