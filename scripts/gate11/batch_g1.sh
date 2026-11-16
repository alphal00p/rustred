#!/usr/bin/env bash
# gate11 batch g1: gate10's reproductions on fdec15da (each test binary pid 1 of a private ns, private /dev/shm)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11
exe() { grep "^$1 test " $S/logs/executables.txt | awk '{print $3}'; }
$S/scripts/r11.sh g1_gate10_listing $(exe gate10_listing) --test-threads=1
$S/scripts/r11.sh g1_gate10_sites $(exe gate10_sites) --test-threads=2
$S/scripts/r11.sh g1_procfix10 $(exe procfix10) --test-threads=2 --include-ignored
$S/scripts/r11.sh g1_procfix10_dirs $(exe procfix10_dirs) --test-threads=2
