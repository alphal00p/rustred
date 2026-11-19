#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
$S/scripts/r10.sh g10_sites $S/bin/tests/gate10_sites --test-threads=1 --nocapture
grep -h 'G10\|^test \|panicked' $S/logs/g10_sites.txt | cut -c1-260
