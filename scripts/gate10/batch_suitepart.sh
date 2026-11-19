#!/usr/bin/env bash
# gate10 (i): a SUITE PART beside the healthy campaigns (each binary pid 1 of a private ns, --test-threads=4)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; T=$S/bin/tests
for b in procfix3_sup procfix5 procfix6 sup_faults sup_ops proc_launch proc_signals procfix3_proc fake_child_signals; do
  $S/scripts/r10.sh suite_$b $T/$b --test-threads=4 | head -1
done
echo "=== done $(date -u +%FT%TZ)"
