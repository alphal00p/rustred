#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
$S/scripts/r10.sh g10_listing $S/bin/tests/gate10_listing --ignored --test-threads=1 --nocapture
grep -h 'G10-B1\|panicked\|exit 0 with' $S/logs/g10_listing.txt | cut -c1-260 | head -8
