#!/usr/bin/env bash
# lane arrivals: RACE for the T266 drill shards stuck in their tail (structural decisions = Symbolica factorizations of heavy
# records, single-threaded for > 1 h): per stuck shard I, a second walk of the same shard with --structural-min-keys 1000000000
# (every record decided PER KEY by exact evaluation; same keys, senders and first senders, only the decision provenance differs:
# the a29 note of run2), into run4/T266.sIr. Per shard the first to finish wins: the other unit is stopped; a winning race dir
# replaces T266.sI (the structural one kept as T266.sI.structural-stopped). When all K shards are done: merge (merge_topup.py) into
# run4/T266/out, compute.err := shard 0's, run4/T266/done, plan.jsonl restored.
# Usage: t266_race.sh <K> <threads> <stuck shard indices...>
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run4; X=T266
K=$1; TH=$2; shift 2; STUCK="$*"
SPEC=$S/phys/drill3/T266/spec.json; EARGS=$R/T266.edges.txt
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
for i in $STUCK; do
  D=$R/$X.s${i}r; mkdir -p $D/out $D/tmp
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax 24G (unit impl-arrivals-drill-T266s%sr: RACE of DRILL T266 shard %s/%s, per-key decisions; sample, not a fixpoint)\n' "$(date -u +%FT%TZ)" "$TH" "$i" "$i" "$K" >> $WS/ops/CPU_LEDGER.md
  echo "$(date -u +%FT%TZ) T266 shard $i/$K RACE starts (per-key decisions, --structural-min-keys 1000000000, threads $TH)" >> $R/chain.log
  systemd-run --user --collect --unit=impl-arrivals-drill-T266s${i}r -p CPUQuota=${TH}00% -p AllowedCPUs="$cpus" -p MemoryMax=24G \
    -p MemorySwapMax=0 -p Nice=10 --setenv=TMPDIR=$D/tmp -- bash -c "source $WS/ops/env.sh; /run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a40 compute \
      --spec $SPEC --rows $R/T266/rows.jsonl --out-dir $D/out --parent $X --edge-producer 'drill walk (sample, not a fixpoint); edges of the chain run; shard $i/$K (race: per-key decisions)' \
      --exact none --depth --structural-min-keys 1000000000 --shard $i/$K --threads $TH --cache-entries 4000000 --map-budget 20000000 --closure-chunk 2000000 \
      --local-flush 250000 --index-dir $R/T266/index \$(cat $EARGS) > $D/compute.out 2> $D/compute.err && touch $D/done" > $D/unit.txt 2>&1
  sleep 5
done
# per stuck shard: the first to finish wins
left="$STUCK"
while [ -n "$left" ]; do
  nl=""
  for i in $left; do
    if [ -e $R/$X.s$i/done ]; then
      systemctl --user stop impl-arrivals-drill-T266s${i}r 2>/dev/null
      echo "$(date -u +%FT%TZ) T266 shard $i: structural walk finished first; race stopped" >> $R/chain.log
    elif [ -e $R/$X.s${i}r/done ]; then
      systemctl --user stop impl-arrivals-drill-T266s$i 2>/dev/null; sleep 5
      mv $R/$X.s$i $R/$X.s$i.structural-stopped && mv $R/$X.s${i}r $R/$X.s$i
      echo "$(date -u +%FT%TZ) T266 shard $i: RACE (per-key) finished first; structural walk stopped (kept as T266.s$i.structural-stopped)" >> $R/chain.log
    elif ! systemctl --user is-active -q impl-arrivals-drill-T266s$i && ! systemctl --user is-active -q impl-arrivals-drill-T266s${i}r; then
      echo "$(date -u +%FT%TZ) T266 shard $i: both walks ended without done: stop" >> $R/chain.log; exit 2
    else
      nl="$nl $i"
    fi
  done
  left=$(echo $nl)
  [ -n "$left" ] && sleep 30
done
for ((i = 0; i < K; i++)); do
  [ -e $R/$X.s$i/done ] || { echo "$(date -u +%FT%TZ) T266 shard $i/$K NOT done: no merge" >> $R/chain.log; exit 2; }
done
ins=(); for ((i = 0; i < K; i++)); do ins+=($R/$X.s$i/out); done
python3 -I $P/merge_topup.py "${ins[0]}" "${ins[1]}" $R/$X/out $X "${ins[@]:2}" > $R/$X/merge.json 2> $R/$X/merge.err || { echo "$(date -u +%FT%TZ) T266 shard merge FAILED" >> $R/chain.log; exit 2; }
cp $R/$X.s0/compute.err $R/$X/compute.err
echo "$(date -u +%FT%TZ) T266 done ($K shards merged by merge_topup.py: $(head -c 300 $R/$X/merge.json)) $(grep -E 'Elapsed|Maximum resident' $R/$X.s0/compute.err | tr '\n' ' ')" >> $R/chain.log
touch $R/$X/done
cp $R/plan.jsonl.full $R/plan.jsonl.tmp && mv $R/plan.jsonl.tmp $R/plan.jsonl
echo "$(date -u +%FT%TZ) plan.jsonl restored (T266 back in)" >> $R/chain.log
