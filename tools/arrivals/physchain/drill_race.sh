#!/usr/bin/env bash
# lane arrivals: RACE a slow node X of the DRILL run4 (sample, not a fixpoint) with K SHARDS of the same walk (--shard I/K: full
# closure, hand-downs only of the record groups hashed to I; flags otherwise identical to chain_node_drill.sh's compute, so the
# same decisions), reusing X's saved rows and sampled incoming keys. NO MERGE (the T266 merge of 84 GB took ~1 h in Python):
# - the original walk finishes first: the shards are stopped, nothing else changes;
# - all shards finish first: X's children read the shard files directly (their incoming lists get the K shard files in place of
#   run4/X/out/X-<child>.arrivals.jsonl; drill_sample.py takes the union with least depth), run4/X/SHARDS lists the shard dirs
#   (drill_chains.py reads them for X's arrival lines), run4/X/done is touched, then the original unit is stopped and its partial
#   out/ kept as out.stopped-original.
# Usage: drill_race.sh <X> <K> <threads per shard>   env: MEMMAX (GB per shard, default 24), EXPECT (GB expected peak per shard for
# the start gate: host MemAvailable - EXPECT >= 75, default 12)
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run4
X=$1; K=$2; TH=$3; MEMMAX=${MEMMAX:-24}; EXPECT=${EXPECT:-12}
SPEC=$S/phys/drill3/$X/spec.json; EARGS=$R/$X.edges.txt
ARR=""; [ -s $R/$X/sampled.keys.bin ] && ARR="--arrivals $R/$X/sampled.keys.bin"
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
for ((i = 0; i < K; i++)); do
  D=$R/$X.s$i; mkdir -p $D/out $D/tmp
  [ -e $D/done ] && continue
  # core gate: the CPUQuota of my active drill units + this shard <= 96 cores
  while :; do used=$(for u in $(systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals-drill-*' | awk '{print $1}'); do systemctl --user show $u -p CPUQuotaPerSecUSec --value; done | awk '{v=$1; if (v ~ /ms$/) {sub("ms","",v); s+=v/1000} else if (v ~ /s$/) {sub("s","",v); s+=v}} END {print int(s+0.5)}')
    [ $((used + TH)) -le 96 ] && break
    echo "$(date -u +%FT%TZ) $X RACE shard $i waits for cores ($used of 96 in use)" >> $R/chain.log; sleep 60; done
  while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - EXPECT)) -ge 75 ] && break
    echo "$(date -u +%FT%TZ) $X RACE shard $i waits for memory ($av GB available, expected $EXPECT GB)" >> $R/chain.log; sleep 60; done
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax %sG (unit impl-arrivals-drill-%ss%s: RACE of DRILL %s, shard %s/%s; sample, not a fixpoint)\n' "$(date -u +%FT%TZ)" "$TH" "$MEMMAX" "$X" "$i" "$X" "$i" "$K" >> $WS/ops/CPU_LEDGER.md
  echo "$(date -u +%FT%TZ) $X RACE shard $i/$K starts (threads $TH, same flags + --shard; rows + sampled keys of $X reused)" >> $R/chain.log
  systemd-run --user --collect --unit=impl-arrivals-drill-${X}s$i -p CPUQuota=${TH}00% -p AllowedCPUs="$cpus" -p MemoryMax=${MEMMAX}G \
    -p MemorySwapMax=0 -p Nice=10 --setenv=TMPDIR=$D/tmp -- bash -c "source $WS/ops/env.sh; /run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a40 compute \
      --spec $SPEC --rows $R/$X/rows.jsonl --out-dir $D/out --parent $X --edge-producer 'drill walk (sample, not a fixpoint); edges of the chain run; shard $i/$K' \
      --exact none --depth --structural-min-keys 16 $ARR --shard $i/$K --threads $TH --cache-entries 4000000 --map-budget 20000000 --closure-chunk 2000000 \
      --local-flush 250000 --index-dir $R/$X/index \$(cat $EARGS) > $D/compute.out 2> $D/compute.err && touch $D/done" > $D/unit.txt 2>&1
  sleep 5
done
while :; do
  if [ -e $R/$X/done ] && [ ! -e $R/$X/SHARDS ]; then
    for ((i = 0; i < K; i++)); do systemctl --user stop impl-arrivals-drill-${X}s$i 2>/dev/null; done
    echo "$(date -u +%FT%TZ) $X RACE: the original walk finished first; shards stopped" >> $R/chain.log; exit 0
  fi
  n=0; for ((i = 0; i < K; i++)); do [ -e $R/$X.s$i/done ] && n=$((n + 1)); done
  if [ $n = $K ]; then
    python3 -I - "$R" "$X" "$K" <<'PY'
import json, os, sys
R, X, K = sys.argv[1], sys.argv[2], int(sys.argv[3])
shards = [f'{R}/{X}.s{i}' for i in range(K)]
plan = [json.loads(l) for l in open(R + '/plan.jsonl.full' if os.path.exists(R + '/plan.jsonl.full') else R + '/plan.jsonl')]
changed = []
for o in plan:
    p = o['incoming']
    ls = [l.strip() for l in open(p) if l.strip()]
    out = []
    hit = False
    for l in ls:
        if l.startswith(f'{R}/{X}/out/{X}-'):
            name = os.path.basename(l)
            out += [f'{d}/out/{name}' for d in shards if os.path.exists(f'{d}/out/{name}')]
            hit = True
        else:
            out.append(l)
    if hit:
        open(p + '.tmp', 'w').write(''.join(x + '\n' for x in out)); os.replace(p + '.tmp', p)
        changed.append(o['node'])
json.dump({'node': X, 'shards': shards, 'rule': 'shards of one walk (--shard I/K): disjoint record groups; union = the unsharded walk',
           'children_incoming_rewritten': changed}, open(f'{R}/{X}/SHARDS', 'w'), indent=1)
print(changed)
PY
    touch $R/$X/done
    systemctl --user stop impl-arrivals-drill-$X 2>/dev/null; sleep 5
    [ -d $R/$X/out ] && mv $R/$X/out $R/$X/out.stopped-original
    cp $R/$X.s0/compute.err $R/$X/compute.err.shard0
    echo "$(date -u +%FT%TZ) $X done by RACE: $K shards finished first (children read the shard files: $R/$X/SHARDS); original walk stopped (out.stopped-original)" >> $R/chain.log
    exit 0
  fi
  for ((i = 0; i < K; i++)); do
    if [ ! -e $R/$X.s$i/done ] && ! systemctl --user is-active -q impl-arrivals-drill-${X}s$i; then
      echo "$(date -u +%FT%TZ) $X RACE shard $i ended without done: race abandoned (original continues)" >> $R/chain.log
      for ((j = 0; j < K; j++)); do systemctl --user stop impl-arrivals-drill-${X}s$j 2>/dev/null; done; exit 2
    fi
  done
  sleep 15
done
