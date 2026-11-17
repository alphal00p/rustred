#!/usr/bin/env bash
# lane arrivals: one node X of the DRILL run4 (sample, not a fixpoint) as K SHARDS (generalises t266_drill_shards.sh / t266_race.sh):
# reuses X's saved rows (run4/X/rows.jsonl) and its sampled incoming keys (run4/X/sampled.keys.bin); every shard computes the full
# closure and decides / hands down only the record groups hashed to I (--shard I/K); decisions PER KEY (--structural-min-keys
# 1000000000: same keys, senders and first senders as the structural walk; the T266 race showed the structural tail can stall on
# factorisations). Merge by merge_topup.py into run4/X/out (exact for shards), compute.err := shard 0's, run4/X/done, then
# plan.jsonl restored from plan.jsonl.full (the caller takes X out of plan.jsonl while the shards run).
# Usage: drill_shards.sh <X> <K> <threads per shard>
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run4
X=$1; K=$2; TH=$3
SPEC=$S/phys/drill3/$X/spec.json; EARGS=$R/$X.edges.txt
ARR=""; [ -s $R/$X/sampled.keys.bin ] && ARR="--arrivals $R/$X/sampled.keys.bin"
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
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax 24G (unit impl-arrivals-drill-%ss%s: DRILL %s shard %s/%s, per-key decisions; sample, not a fixpoint)\n' "$(date -u +%FT%TZ)" "$TH" "$X" "$i" "$X" "$i" "$K" >> $WS/ops/CPU_LEDGER.md
  echo "$(date -u +%FT%TZ) $X shard $i/$K starts (threads $TH, per-key decisions, rows + sampled keys of $X reused)" >> $R/chain.log
  systemd-run --user --wait --collect --unit=impl-arrivals-drill-${X}s$i -p CPUQuota=${TH}00% -p AllowedCPUs="$cpus" -p MemoryMax=24G \
    -p MemorySwapMax=0 -p Nice=10 --setenv=TMPDIR=$D/tmp -- bash -c "source $WS/ops/env.sh; /run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a40 compute \
      --spec $SPEC --rows $R/$X/rows.jsonl --out-dir $D/out --parent $X --edge-producer 'drill walk (sample, not a fixpoint); edges of the chain run; shard $i/$K (per-key decisions)' \
      --exact none --depth --structural-min-keys 1000000000 $ARR --shard $i/$K --threads $TH --cache-entries 4000000 --map-budget 20000000 --closure-chunk 2000000 \
      --local-flush 250000 --index-dir $R/$X/index \$(cat $EARGS) > $D/compute.out 2> $D/compute.err && touch $D/done" > $D/unit.txt 2>&1 &
  pids+=($!)
  sleep 5
done
for p in "${pids[@]}"; do wait $p; done
for ((i = 0; i < K; i++)); do
  [ -e $R/$X.s$i/done ] || { echo "$(date -u +%FT%TZ) $X shard $i/$K NOT done: no merge (see $R/$X.s$i/compute.err)" >> $R/chain.log; exit 2; }
done
ins=(); for ((i = 0; i < K; i++)); do ins+=($R/$X.s$i/out); done
python3 -I $P/merge_topup.py "${ins[0]}" "${ins[1]}" $R/$X/out $X "${ins[@]:2}" > $R/$X/merge.json 2> $R/$X/merge.err || { echo "$(date -u +%FT%TZ) $X shard merge FAILED" >> $R/chain.log; exit 2; }
cp $R/$X.s0/compute.err $R/$X/compute.err
echo "$(date -u +%FT%TZ) $X done ($K shards merged by merge_topup.py: $(head -c 300 $R/$X/merge.json)) $(grep -E 'Elapsed|Maximum resident' $R/$X.s0/compute.err | tr '\n' ' ')" >> $R/chain.log
touch $R/$X/done
python3 -I - "$R" "$X" <<'PY'
import json, os, sys
R, X = sys.argv[1:3]
full = [l for l in open(R + '/plan.jsonl.full')]
cur = {json.loads(l)['node'] for l in open(R + '/plan.jsonl')}
keep = [l for l in full if json.loads(l)['node'] in cur or json.loads(l)['node'] == X]
open(R + '/plan.jsonl.tmp', 'w').write(''.join(keep)); os.replace(R + '/plan.jsonl.tmp', R + '/plan.jsonl')
PY
echo "$(date -u +%FT%TZ) plan.jsonl: $X back in" >> $R/chain.log
