#!/usr/bin/env bash
# lane arrivals: physical chain WITH CHAIN DEPTH (run2, binary a26), coordinator 2026-11-16T06:14Z: T266 starts now below the gate;
# further walks while host MemAvailable - expected peak >= 75 GB; at most 3 walks until the package gate's T266 check ends, then 5.
B=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/bin/rr_arrivals.a26; R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2
FORCE_NODES=T266 WAVES=1 PAR=1 GATE=75 NODE_MEM=40 EXPECT_DEFAULT=8 bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run_chain10.sh $B $R
WAVES="2 3 4 5" PAR=${PAR2:-3} GATE=75 NODE_MEM=40 EXPECT_DEFAULT=8 bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run_chain10.sh $B $R
