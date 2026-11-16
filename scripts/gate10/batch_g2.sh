#!/usr/bin/env bash
# gate10 (g) + m9-ns probe, one after the other in this unit
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; cd $S
echo "##### NS probe (both /proc enumerations truncated)"; env EXTRA_RES=',"unproven_end_limit_s":60' scripts/matrix10.sh $S/cells_NS.txt $S/logs/matrix_NS.txt 3; cat $S/logs/matrix_NS.txt
echo "##### manifest variants"; bash scripts/batch_gmf.sh
echo "##### zombie-leader churn variants"; bash scripts/batch_gchurn.sh
echo "=== done $(date -u +%FT%TZ)"
