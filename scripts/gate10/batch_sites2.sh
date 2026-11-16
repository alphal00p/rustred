#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
$S/scripts/r10.sh g10_sites2 $S/bin/tests/gate10_sites --test-threads=1 --nocapture g10_site_tui g10_site_closeloop
grep -h 'G10\|^test \|panicked' $S/logs/g10_sites2.txt | cut -c1-260
