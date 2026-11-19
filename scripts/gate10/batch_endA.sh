#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; cd $S
echo "##### (e) finish eB1"; $S/scripts/ns8r.sh $S/scripts/g10_e_finish_inner.sh eB1 2>&1 | grep -E 'finish|FINISH'
bash $S/scripts/batch_tail.sh
