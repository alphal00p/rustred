#!/usr/bin/env bash
# lane arrivals: closure path of key K at chain node X (compute --closure-only --depth --trace-key; same inputs as the
# node's walk in run R). Usage: trace_run.sh <bin> <R> <X> '<key json>'   (run inside a transient unit)
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
BIN=$1; R=$2; X=$3; K=$4
eval "$(python3 -I -c 'import json,sys
for l in open(sys.argv[1]):
    o=json.loads(l)
    if o["node"]==sys.argv[2]: print("SPEC=%s EARGS=%s INC=%s" % (o["spec"], o["edge_args"], o["incoming"]))' "$R/plan.jsonl" "$X")"
D=$R/$X; T=$D/trace; mkdir -p "$T/tmp" "$T/out"
source $WS/ops/env.sh
export TMPDIR=$T/tmp
arr=(); while read -r f; do [ -n "$f" ] && arr+=(--arrivals "$f"); done < "$INC"
name=$(python3 -I -c 'import json,sys;print("_".join(str(x) for x in json.loads(sys.argv[1])))' "$K")
"$BIN" compute --spec "$SPEC" --rows "$D/rows.jsonl" --out-dir "$T/out" --parent "$X" --edge-producer "closure trace" \
  --exact none --depth "${arr[@]}" --closure-only --trace-key "$K" --threads 8 --cache-entries 2000000 --index-dir "$D/index" \
  $(cat "$EARGS") > "$T/$name.summary.json" 2> "$T/$name.err" || exit 2
python3 -I -c 'import json,sys
s=json.load(open(sys.argv[1])); json.dump(s["trace"][0], open(sys.argv[2],"w"), indent=1)' "$T/$name.summary.json" "$T/$name.json"
