#!/usr/bin/env bash
# lane arrivals: physical chain WITH CHAIN DEPTH (run2). Wave 1 ran with a26; waves 2-5 with a28 (= a26 walk + cost-ordered record
# groups, commit db21d3ca: identical outputs). Coordinator 2026-11-16T06:14Z: walks while host MemAvailable - expected >= 75 GB,
# <= 3 walks until the package gate's T266 check ends (then PAR2=5 by a driver restart between nodes).
R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2
WAVES=1 PAR=1 GATE=75 FORCE_NODES=T266 NODE_MEM=40 EXPECT_DEFAULT=8 bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run_chain10.sh /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/bin/rr_arrivals.a26 $R
WAVES="${WAVES2:-2 3 4 5}" PAR=${PAR2:-3} GATE=75 NODE_MEM=40 EXPECT_DEFAULT=8 bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run_chain10.sh /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/bin/rr_arrivals.a28 $R
