#!/usr/bin/env bash
# gate17 item 5: (i) stall 90:240 within the default 900-s limit (60 x 20-40 s, HELPERS=2) and (ii) a stall beyond the
# limit with jobs running at the stop (16 x 200-240 s, HELPERS=2, stall 60:240, MANAGER_WAIT_LIMIT_S=60): the e07c7d88
# release binaries and scripts of my scratch freeze package. Each in its own scope (CPUQuota 200 %, MemoryMax 8G, nice 10).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
F=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17/frozen/20261118T1113Z-host-step
systemd-run --user --scope --unit=gate17-reh-stall -q -p CPUQuota=200% -p MemoryMax=8G -p MemorySwapMax=0 -- \
  env HTDIR=$F RELDIR=$F/bin HELPERS=2 FAKE_MANAGER_STALL=90:240 nice -n 10 bash $S/scripts/reh17.sh stall 60 '[20000,40000]' > $S/logs/reh17_stall.txt 2>&1 &
a=$!
systemd-run --user --scope --unit=gate17-reh-limitdrain -q -p CPUQuota=200% -p MemoryMax=8G -p MemorySwapMax=0 -- \
  env HTDIR=$F RELDIR=$F/bin HELPERS=2 FAKE_MANAGER_STALL=60:240 MANAGER_WAIT_LIMIT_S=60 nice -n 10 bash $S/scripts/reh17.sh limitdrain 16 '[200000,240000]' > $S/logs/reh17_limitdrain.txt 2>&1 &
b=$!
wait $a; echo "stall rc=$?"; wait $b; echo "limitdrain rc=$?"
