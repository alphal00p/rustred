#!/usr/bin/env bash
# batch_crash.sh FIRST LAST : seeds FIRST..LAST, modes rotated
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
M=("ndaemon daemon" "ndchain chainroot" "ndorphan flipexec" "ndaemon ndchain dropexec" "ndaemon ndorphan daemon" "ndchain flipexec chainroot")
for s in $(seq $1 $2); do m=${M[$((s % 6))]}; echo "=== innsu g7_crash_inner.sh $s $m ($(date -u +%FT%TZ))"; timeout -s KILL 3000 scripts/innsu.sh scripts/g7_crash_inner.sh $s $m; done
echo "=== done $(date -u +%FT%TZ)"
