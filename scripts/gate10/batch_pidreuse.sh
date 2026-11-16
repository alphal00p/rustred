#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
for d in none other none other none other; do $S/scripts/ns8r.sh $S/scripts/g10_pidreuse_inner.sh pr2-$d-$RANDOM $d 2>&1 | grep RESULT; done
