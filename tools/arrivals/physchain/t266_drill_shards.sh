#!/usr/bin/env bash
# lane arrivals: T266 of the DRILL run4 (sample, not a fixpoint) as K SHARDS (--shard I/K: closure in full, hand-downs only of the
# record groups hashed to I; disjoint, merged exactly by merge_topup.py: keys union, depth min, least first sender, senders summed
# = exact for shards). Reuses T266's saved rows. When all shards are done: merge into run4/T266/out, compute.err := shard 0's,
# run4/T266/done, plan.jsonl restored (T266 was taken out of it so the scheduler would not relaunch the 8-thread walk).
# Usage: t266_drill_shards.sh <K> <threads per shard>
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run4; X=T266
K=$1; TH=$2
SPEC=$S/phys/drill3/T266/spec.json; EARGS=$R/T266.edges.txt
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
pids=()
for ((i = 0; i < K; i++)); do
  D=$R/$X.s$i; mkdir -p $D/out $D/tmp
  [ -e $D/done ] && continue
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax 24G (unit impl-arrivals-drill-T266s%s: DRILL T266 shard %s/%s, sample, not a fixpoint)\n' "$(date -u +%FT%TZ)" "$TH" "$i" "$i" "$K" >> $WS/ops/CPU_LEDGER.md
  echo "$(date -u +%FT%TZ) T266 shard $i/$K starts (threads $TH, rows of T266 reused)" >> $R/chain.log
  systemd-run --user --wait --collect --unit=impl-arrivals-drill-T266s$i -p CPUQuota=${TH}00% -p AllowedCPUs="$cpus" -p MemoryMax=24G \
    -p MemorySwapMax=0 -p Nice=10 --setenv=TMPDIR=$D/tmp -- bash -c "source $WS/ops/env.sh; /run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a40 compute \
      --spec $SPEC --rows $R/T266/rows.jsonl --out-dir $D/out --parent $X --edge-producer 'drill walk (sample, not a fixpoint); edges of the chain run; shard $i/$K' \
      --exact none --depth --structural-min-keys 16 --shard $i/$K --threads $TH --cache-entries 4000000 --map-budget 20000000 --closure-chunk 2000000 \
      --local-flush 250000 --index-dir $R/T266/index \$(cat $EARGS) > $D/compute.out 2> $D/compute.err && touch $D/done" > $D/unit.txt 2>&1 &
  pids+=($!)
  sleep 5
done
for p in "${pids[@]}"; do wait $p; done
for ((i = 0; i < K; i++)); do
  [ -e $R/$X.s$i/done ] || { echo "$(date -u +%FT%TZ) T266 shard $i/$K NOT done: no merge (see $R/$X.s$i/compute.err)" >> $R/chain.log; exit 2; }
done
ins=(); for ((i = 0; i < K; i++)); do ins+=($R/$X.s$i/out); done
python3 -I $P/merge_topup.py "${ins[0]}" "${ins[1]}" $R/$X/out $X "${ins[@]:2}" > $R/$X/merge.json 2> $R/$X/merge.err || { echo "$(date -u +%FT%TZ) T266 shard merge FAILED" >> $R/chain.log; exit 2; }
cp $R/$X.s0/compute.err $R/$X/compute.err
echo "$(date -u +%FT%TZ) T266 done ($K shards merged by merge_topup.py: $(head -c 300 $R/$X/merge.json)) $(grep -E 'Elapsed|Maximum resident' $R/$X.s0/compute.err | tr '\n' ' ')" >> $R/chain.log
touch $R/$X/done
cp $R/plan.jsonl.full $R/plan.jsonl.tmp && mv $R/plan.jsonl.tmp $R/plan.jsonl
echo "$(date -u +%FT%TZ) plan.jsonl restored (T266 back in)" >> $R/chain.log
