#!/usr/bin/env bash
# lane arrivals: T230 of the DRILL run4 (sample, not a fixpoint): its K=4 shard 3 is slow (46 % of items after ~2.5 h); the record
# groups of K=4 shard 3 (hash % 4 == 3) are exactly those of the K=32 shards j = 3, 7, .., 31 (hash % 32 == j, j % 4 == 3). This
# manager races shard 3 against those 8 pieces and replaces drill_race.sh's T230 manager (stopped first). Covering set when done:
# T230.s0, s1, s2 + (T230.s3 or all 8 pieces T230.k32s<j>), whichever completes first; then (as drill_race.sh): the children's
# incoming lists get the cover's files, run4/T230/SHARDS lists the cover, run4/T230/done, the original walk is stopped.
# If the original walk finishes first, every shard / piece is stopped.
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run4; X=T230
TH=${TH:-6}; SPEC=$S/phys/drill3/$X/spec.json; EARGS=$R/$X.edges.txt; ARR="--arrivals $R/$X/sampled.keys.bin"
PIECES="3 7 11 15 19 23 27 31"
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
for j in $PIECES; do
  D=$R/$X.k32s$j; mkdir -p $D/out $D/tmp
  [ -e $D/done ] && continue
  while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - 12)) -ge 75 ] && break
    echo "$(date -u +%FT%TZ) $X piece k32s$j waits for memory ($av GB available)" >> $R/chain.log; sleep 60; done
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax 24G (unit impl-arrivals-drill-T230k32s%s: DRILL T230 piece %s/32 racing K=4 shard 3; sample, not a fixpoint)\n' "$(date -u +%FT%TZ)" "$TH" "$j" "$j" >> $WS/ops/CPU_LEDGER.md
  echo "$(date -u +%FT%TZ) $X piece $j/32 starts (threads $TH; races K=4 shard 3)" >> $R/chain.log
  systemd-run --user --collect --unit=impl-arrivals-drill-T230k32s$j -p CPUQuota=${TH}00% -p AllowedCPUs="$cpus" -p MemoryMax=24G \
    -p MemorySwapMax=0 -p Nice=10 --setenv=TMPDIR=$D/tmp -- bash -c "source $WS/ops/env.sh; /run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a40 compute \
      --spec $SPEC --rows $R/$X/rows.jsonl --out-dir $D/out --parent $X --edge-producer 'drill walk (sample, not a fixpoint); edges of the chain run; shard $j/32' \
      --exact none --depth --structural-min-keys 16 $ARR --shard $j/32 --threads $TH --cache-entries 4000000 --map-budget 20000000 --closure-chunk 2000000 \
      --local-flush 250000 --index-dir $R/$X/index \$(cat $EARGS) > $D/compute.out 2> $D/compute.err && touch $D/done" > $D/unit.txt 2>&1
  sleep 5
done
stop_pieces() { for j in $PIECES; do systemctl --user stop impl-arrivals-drill-T230k32s$j 2>/dev/null; done; }
while :; do
  if [ -e $R/$X/done ] && [ ! -e $R/$X/SHARDS ]; then
    stop_pieces; for i in 0 1 2 3; do systemctl --user stop impl-arrivals-drill-T230s$i 2>/dev/null; done
    echo "$(date -u +%FT%TZ) $X: the original walk finished first; shards and pieces stopped" >> $R/chain.log; exit 0
  fi
  cover=""
  if [ -e $R/$X.s0/done ] && [ -e $R/$X.s1/done ] && [ -e $R/$X.s2/done ]; then
    if [ -e $R/$X.s3/done ]; then
      cover="$R/$X.s0 $R/$X.s1 $R/$X.s2 $R/$X.s3"; stop_pieces; how="K=4 shard 3 finished first; pieces stopped"
    else
      np=0; for j in $PIECES; do [ -e $R/$X.k32s$j/done ] && np=$((np + 1)); done
      if [ $np = 8 ]; then
        cover="$R/$X.s0 $R/$X.s1 $R/$X.s2"; for j in $PIECES; do cover="$cover $R/$X.k32s$j"; done
        systemctl --user stop impl-arrivals-drill-T230s3 2>/dev/null; how="the 8 pieces finished before K=4 shard 3 (stopped)"
      fi
    fi
  fi
  if [ -n "$cover" ]; then
    python3 -I - "$R" "$X" $cover <<'PY'
import json, os, sys
R, X, shards = sys.argv[1], sys.argv[2], sys.argv[3:]
plan = [json.loads(l) for l in open(R + '/plan.jsonl.full' if os.path.exists(R + '/plan.jsonl.full') else R + '/plan.jsonl')]
changed = []
for o in plan:
    p = o['incoming']
    ls = [l.strip() for l in open(p) if l.strip()]
    out, hit = [], False
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
json.dump({'node': X, 'shards': shards, 'rule': 'shards of one walk: K=4 shards 0-2 + (K=4 shard 3 or the K=32 shards j = 3 mod 4); disjoint record groups; union = the unsharded walk',
           'children_incoming_rewritten': changed}, open(f'{R}/{X}/SHARDS', 'w'), indent=1)
print(changed)
PY
    touch $R/$X/done
    systemctl --user stop impl-arrivals-drill-$X 2>/dev/null; sleep 5
    [ -d $R/$X/out ] && mv $R/$X/out $R/$X/out.stopped-original
    cp $R/$X.s0/compute.err $R/$X/compute.err.shard0
    echo "$(date -u +%FT%TZ) $X done by RACE ($how): children read the cover's files ($R/$X/SHARDS); original walk stopped (out.stopped-original)" >> $R/chain.log
    exit 0
  fi
  sleep 15
done
