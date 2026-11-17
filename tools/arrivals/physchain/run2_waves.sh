#!/usr/bin/env bash
# lane arrivals: physical chain WITH CHAIN DEPTH (run2, binary a26): wave 1 at PAR 4 (the a25 cross-check unit T266 still runs:
# 5 walks at most), then waves 2-5 at PAR 5. Coordinator RAM reservation 200 GB (5 x 40 GB) until 2026-11-18T06:00Z.
B=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/bin/rr_arrivals.a26; R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2
WAVES=1 PAR=4 NODE_MEM=40 EXPECT_DEFAULT=8 bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run_chain9.sh $B $R
WAVES="2 3 4 5" PAR=5 NODE_MEM=40 EXPECT_DEFAULT=8 bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run_chain9.sh $B $R
