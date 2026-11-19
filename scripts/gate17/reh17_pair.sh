#!/usr/bin/env bash
# gate17 item 1 + 5: the triple stop (FAKE_TRIPLE_STOP=70, stall 60:240, 60 x 20-40 s, HELPERS=2) with the e07c7d88 release
# binaries of my scratch freeze package and with gate16's 041e7fe8 release binaries (S16/bin/rel, read only); scripts =
# the package's. Each in its own scope (CPUQuota 200 %, MemoryMax 8G, nice 10).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
F=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17/frozen/20261118T1113Z-host-step
run() { tag=$1; rel=$2
  systemd-run --user --scope --unit=gate17-reh-$tag -q -p CPUQuota=200% -p MemoryMax=8G -p MemorySwapMax=0 -- \
    env HTDIR=$F RELDIR=$rel HELPERS=2 FAKE_MANAGER_STALL=60:240 FAKE_TRIPLE_STOP=70 nice -n 10 bash $S/scripts/reh17.sh $tag 60 '[20000,40000]' > $S/logs/reh17_$tag.txt 2>&1
  echo "$tag rc=$?"; }
run xstop $F/bin & a=$!
run xstopOld /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate16/bin/rel & b=$!
wait $a; wait $b
