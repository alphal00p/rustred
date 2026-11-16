#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
for i in $(seq 1 $1); do timeout -s KILL 300 unshare -Urpf --mount-proc --kill-child -- /home/vhirschi/.nix-profile/bin/bash scripts/g9_pf3_inner.sh 2>&1 | grep PF3 & [ $((i % 3)) = 0 ] && wait; done; wait
