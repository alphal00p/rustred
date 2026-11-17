#!/usr/bin/env bash
# lane arrivals: one part of the T165 TOP-UP walk (coordinator item 10.2): rows + walk of T165 on its box + the given incoming files
# (part A = the 44 FINAL inputs, part B = T180's file once T180 is done); merged by merge_topup.py (proved = from scratch on T203/T208).
# Usage (inside a unit): t165_part.sh <bin> <part> <incoming list> <threads>
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; R=$S/physchain/run2
BIN=$1; PART=$2; INC=$3; TH=$4; X=T165; D=$R/$X.$PART
SPEC=$(python3 -I -c 'import json,sys
for l in open(sys.argv[1]):
    o=json.loads(l)
    if o["node"]=="T165": print(o["spec"])' $R/plan.jsonl.with-T165)
EARGS=$R/$X.edges.txt
[ -e $D/done ] && exit 0
mkdir -p $D/tmp $D/out $D/index
source $WS/ops/env.sh
export TMPDIR=$D/tmp
cp "$INC" $D/incoming.txt
arr=(); while read -r f; do [ -n "$f" ] && arr+=(--arrivals "$f"); done < $D/incoming.txt
if [ ! -s $D/rows.done ]; then
  echo "$(date -u +%FT%TZ) $X.$PART start rows (incoming files: $(grep -c . $D/incoming.txt)) threads=$TH bin=$(sha256sum $BIN | cut -c1-16)" >> $R/chain.log
  /run/current-system/sw/bin/time -v "$BIN" closure-rows --spec "$SPEC" "${arr[@]}" --out $D/rows.jsonl --summary $D/rows.summary.json \
    --threads $TH --index-dir $D/index > $D/rows.out 2> $D/rows.err || { echo "$(date -u +%FT%TZ) $X.$PART rows FAILED" >> $R/chain.log; exit 2; }
  date -u +%FT%TZ > $D/rows.done
fi
# PHASE GATE before the walk (implementer 12:3xZ): host MemAvailable - walk peak (30 GB) >= 75, else wait here with the rows saved
while :; do
  av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  [ $((av - 30)) -ge 75 ] && break
  echo "$(date -u +%FT%TZ) $X.$PART waits at the rows/walk boundary (rows saved): $av GB available, walk peak 30 GB" >> $R/chain.log; sleep 60
done
echo "$(date -u +%FT%TZ) $X.$PART walk phase starts: $(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo) GB available, threads $TH" >> $R/chain.log
/run/current-system/sw/bin/time -v "$BIN" compute --spec "$SPEC" --rows $D/rows.jsonl --out-dir $D/out --parent $X \
  --edge-producer "handdown-routes chain probe: frozen WS/bin/frozen/20261114T1718Z-handdown-chain/p_handdown_routes (4ad761d9, sha256 a9ebd498c99f); edge files of the route lane or of lane arrivals (same probe + the route lane's gen_edge.py); map chosen" \
  --exact none --depth --structural-min-keys 16 "${arr[@]}" --threads $TH --cache-entries 8000000 --map-budget 30000000 --index-dir $D/index \
  $(cat $EARGS) > $D/compute.out 2> $D/compute.err || { echo "$(date -u +%FT%TZ) $X.$PART compute FAILED" >> $R/chain.log; exit 2; }
echo "$(date -u +%FT%TZ) $X.$PART done $(grep -E 'Elapsed|Maximum resident' $D/compute.err | tr '\n' ' ')" >> $R/chain.log
touch $D/done
