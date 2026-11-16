#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; cd $S
echo "##### gate10 pid reuse between scan and proof"
for d in none other none other; do $S/scripts/ns8r.sh $S/scripts/g10_pidreuse_inner.sh pr-$d-$RANDOM $d 2>&1 | grep RESULT; done
echo "##### gate9 decoy families"
bash scripts/batch_F.sh 2>&1 | cut -c1-240
