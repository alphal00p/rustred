#!/usr/bin/env bash
# gate10: the ignored every-helper-call property test, the zombie take-over timing, M3 later separately
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; cd $S
echo "##### lock take-over with N unreaped zombies"
for nz in 0 200 1000 3000; do $S/scripts/ns8r.sh $S/scripts/g10_ztake_inner.sh zt-$nz-$RANDOM $nz 2>&1 | grep RESULT; done
echo "##### procfix9 g9_fault_property_every_helper_call (ignored)"
$S/scripts/r10.sh prop_every $S/bin/tests/procfix9 --ignored --exact g9_fault_property_every_helper_call --nocapture --test-threads=1
grep -h 'PF9\|panicked' $S/logs/prop_every.txt | cut -c1-300 | tail -8
echo "=== done $(date -u +%FT%TZ)"
