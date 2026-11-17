#!/usr/bin/env bash
# lane arrivals: closure paths of keys K1..Kn at node X of the DRILL walk (sample, not a fixpoint): same start as the node's drill
# walk (its spec = own top-cell keys, + R/X/sampled.keys.bin), compute --closure-only --depth --trace-key ...; rows re-made by
# closure-rows when the walk removed them. Per key one file R/X/trace/<key>.json (the trace entry: key, depth, in_rows, path).
# Usage: trace_run_drill.sh <bin> <R> <X> <keys file: one key json per line>   (run inside a transient unit)
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
BIN=$1; R=$2; X=$3; KF=$4
eval "$(python3 -I -c 'import json,sys
for l in open(sys.argv[1]):
    o=json.loads(l)
    if o["node"]==sys.argv[2]: print("SPEC=%s EARGS=%s" % (o["spec"], o["edge_args"]))' "$R/plan.jsonl" "$X")"
D=$R/$X; T=$D/trace; mkdir -p "$T/tmp" "$T/out" "$T/index"
source $WS/ops/env.sh
export TMPDIR=$T/tmp
arr=(); [ -s "$D/sampled.keys.bin" ] && arr=(--arrivals "$D/sampled.keys.bin")
ROWS=$D/rows.jsonl
if [ ! -s "$ROWS" ]; then
  ROWS=$T/rows.jsonl
  [ -s "$T/rows.done" ] || { "$BIN" closure-rows --spec "$SPEC" "${arr[@]}" --out "$ROWS" --summary "$T/rows.summary.json" \
      --threads 8 --index-dir "$D/index" > "$T/rows.out" 2> "$T/rows.err" && date -u +%FT%TZ > "$T/rows.done"; } || exit 2
fi
tk=(); while read -r k; do [ -n "$k" ] && tk+=(--trace-key "$k"); done < "$KF"
b=$(basename "$KF" .txt)
"$BIN" compute --spec "$SPEC" --rows "$ROWS" --out-dir "$T/out" --parent "$X" --edge-producer "drill closure trace" \
  --exact none --depth "${arr[@]}" --closure-only "${tk[@]}" --threads 8 --cache-entries 2000000 --index-dir "$D/index" \
  $(cat "$EARGS") > "$T/$b.summary.json" 2> "$T/$b.err" || exit 2
python3 -I -c 'import json,sys
s=json.load(open(sys.argv[1]))
for t in s["trace"]:
    json.dump(t, open(sys.argv[2] + "/" + "_".join(str(x) for x in t["key"]) + ".json", "w"), indent=1)' "$T/$b.summary.json" "$T"
