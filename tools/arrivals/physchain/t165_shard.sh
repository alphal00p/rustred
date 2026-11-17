#!/usr/bin/env bash
# lane arrivals: one SHARD of T165 top-up part B (rows of T165.B reused; --shard I/K): compute only, phase gate before the walk
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; R=$S/physchain/run2
I=$1; K=$2; TH=$3; X=T165; D=$R/$X.B$I
SPEC=$(python3 -I -c 'import json,sys
for l in open(sys.argv[1]):
    o=json.loads(l)
    if o["node"]=="T165": print(o["spec"])' $R/plan.jsonl.with-T165)
[ -e $D/done ] && exit 0
mkdir -p $D/tmp $D/out
source $WS/ops/env.sh
export TMPDIR=$D/tmp
arr=(); while read -r f; do [ -n "$f" ] && arr+=(--arrivals "$f"); done < $R/T165.B/incoming.txt
while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - 28)) -ge 60 ] && break
  echo "$(date -u +%FT%TZ) $X.B$I waits at the walk boundary: $av GB available, walk peak 28 GB" >> $R/chain.log; sleep 60; done
echo "$(date -u +%FT%TZ) $X.B$I walk phase starts (shard $I/$K, threads $TH): $(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo) GB available" >> $R/chain.log
/run/current-system/sw/bin/time -v $S/bin/rr_arrivals.a38 compute --spec "$SPEC" --rows $R/T165.B/rows.jsonl --out-dir $D/out --parent $X \
  --edge-producer "handdown-routes chain probe: frozen WS/bin/frozen/20261114T1718Z-handdown-chain/p_handdown_routes (4ad761d9, sha256 a9ebd498c99f); edge files of the route lane or of lane arrivals (same probe + the route lane's gen_edge.py); map chosen" \
  --exact none --depth --structural-min-keys 16 "${arr[@]}" --shard $I/$K --threads $TH --cache-entries 8000000 --map-budget 30000000 --closure-chunk 2000000 --local-flush 250000 --index-dir $R/T165.B/index \
  $(cat $R/$X.edges.txt) > $D/compute.out 2> $D/compute.err || { echo "$(date -u +%FT%TZ) $X.B$I compute FAILED" >> $R/chain.log; exit 2; }
echo "$(date -u +%FT%TZ) $X.B$I done $(grep -E 'Elapsed|Maximum resident' $D/compute.err | tr '\n' ' ')" >> $R/chain.log
touch $D/done
