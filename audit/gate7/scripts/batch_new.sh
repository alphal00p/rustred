#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
for a in "g7_fallback_inner.sh f1" "g7_barrier_inner.sh b1 always" "g7_barrier_inner.sh b2 once" "g7_mjb_inner.sh noreplace noreplace" "g7_mjb_inner.sh race race" \
         "g7_mjb_inner.sh killclaim killclaim" "g7_mjb_inner.sh oldcopy oldcopy" "g7_mjb_inner.sh rmwork rmwork" "g7_mjb_inner.sh killcreate killcreate"; do
  echo "=== inns $a ($(date -u +%FT%TZ))"; timeout -s KILL 3000 scripts/inns.sh scripts/$a
done
echo "=== batch_new done $(date -u +%FT%TZ)"
