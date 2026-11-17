#!/usr/bin/env bash
# lane arrivals: one node X of the DRILL walk (sample, not a fixpoint): start = X's top-cell drill keys (its spec) + a seeded sample
# of K = 300 keys arriving from its parents' drill walks (drill_sample.py); then the same closure + hand-downs as the chain run.
# Usage: chain_node_drill.sh <bin> <R> <X> <spec> <edge-args file> <incoming file>
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals
BIN=$1; R=$2; X=$3; SPEC=$4; EARGS=$5; INC=$6
D=$R/$X
[ -e "$D/done" ] && exit 0
while read -r f; do
  [ -z "$f" ] && continue
  pd=$(dirname "$(dirname "$f")")
  if [ ! -e "$pd/done" ] || [ ! -s "$f" ]; then echo "$(date -u +%FT%TZ) $X NOT STARTED: input $f not finished" >> "$R/chain.log"; exit 3; fi
done < "$INC"
mkdir -p "$D/tmp" "$D/index" "$D/out"
source $WS/ops/env.sh
export TMPDIR=$D/tmp
arr=()
if grep -q . "$INC"; then
  MASK=$(python3 -I -c 'import json,sys; m=json.load(open(sys.argv[1]))[sys.argv[2]]; print(json.loads(open(m["table"]).readline())["mask"])' $S/phys/specs/manifest.json $X)
  python3 -I $S/physchain/drill_sample.py $X "$INC" 300 "$MASK" "$D/sampled.keys.bin" > "$D/sample.json" || { echo "$(date -u +%FT%TZ) $X drill sample FAILED" >> "$R/chain.log"; exit 2; }
  arr=(--arrivals "$D/sampled.keys.bin")
fi
echo "$(date -u +%FT%TZ) $X drill start rows ($(cat $D/sample.json 2>/dev/null))" >> "$R/chain.log"
/run/current-system/sw/bin/time -v "$BIN" closure-rows --spec "$SPEC" "${arr[@]}" --out "$D/rows.jsonl" --summary "$D/rows.summary.json" \
  --threads "${THREADS:-8}" --index-dir "$D/index" > "$D/rows.out" 2> "$D/rows.err" || { echo "$(date -u +%FT%TZ) $X rows FAILED" >> "$R/chain.log"; exit 2; }
if [ ! -s "$EARGS" ]; then touch "$D/done"; exit 0; fi
# shellcheck disable=SC2046
/run/current-system/sw/bin/time -v "$BIN" compute --spec "$SPEC" --rows "$D/rows.jsonl" --out-dir "$D/out" --parent "$X" \
  --edge-producer "drill walk (sample, not a fixpoint); edges of the chain run" \
  --exact none --depth --structural-min-keys 16 "${arr[@]}" --threads "${THREADS:-8}" --cache-entries 4000000 --map-budget 20000000 \
  --closure-chunk 2000000 --local-flush 250000 --index-dir "$D/index" $(cat "$EARGS") > "$D/compute.out" 2> "$D/compute.err" \
  || { echo "$(date -u +%FT%TZ) $X compute FAILED" >> "$R/chain.log"; exit 2; }
# run4 keeps its rows (closure traces for the drill chains, 2026-11-17)
case "$D/rows.jsonl" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run3/T*/rows.jsonl) rm -- "$D/rows.jsonl";; esac
echo "$(date -u +%FT%TZ) $X done $(grep -E 'Elapsed|Maximum resident' "$D/compute.err" | tr '\n' ' ')" >> "$R/chain.log"
touch "$D/done"
