#!/usr/bin/env bash
# restart the DRILL run4 scheduler once T230 is done (its cover manager t230_cover.sh touches run4/T230/done); the scheduler
# stopped at 11:48Z because its only walk (T230's original) was stopped and T230's children wait on T230.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run4
until [ -e $R/T230/done ]; do sleep 20; done
sleep 10
echo "$(date -u +%FT%TZ) scheduler restarted (T230 done)" >> $R/chain.log
cd $R && NODE_MEM=24 EXPECT_DEFAULT=6 CORES=64 exec bash $P/run_dag_drill.sh $S/bin/rr_arrivals.a40 $R >> $R/dag.out 2>&1 < /dev/null
