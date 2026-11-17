#!/usr/bin/env bash
# lane arrivals: ONE node X of an arrival CHAIN (coordinator 2026-11-15: arrivals through intermediate nodes count, taken
# to closure), inside a private pid namespace. Resumable: X is skipped when $R/<X>/done exists (its arrival files are bound
# to the edge files' sha256 in their headers and are never recomputed).
#   1. rows   = keys-only closure of X from its box ∪ every incoming arrival set   (rr_arrivals closure-rows)
#   2. walk   = exact closure from those starts, hand-downs on X's outgoing edges   (rr_arrivals compute --exact none)
# Usage: chain_node2.sh <bin> <chain run dir R> <X> <spec> <edge-args file> <incoming-arrivals file>
#   edge-args file: one line of `--edge E --child-table L=T ...`; incoming file: one arrival-set path per line (may be empty)
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
BIN=$1; R=$2; X=$3; SPEC=$4; EARGS=$5; INC=$6
D=$R/$X
[ -e "$D/done" ] && exit 0
[ -e "$WS/plan/auto-campaign/implementer_ops/PAUSE" ] && { echo "$(date -u +%FT%TZ) PAUSE: $X not started" >> "$R/chain.log"; exit 75; }
# every incoming arrival file must come from a FINISHED parent (its done marker): else the node is not started
while read -r f; do
  [ -z "$f" ] && continue
  pd=$(dirname "$(dirname "$f")")
  if [ ! -e "$pd/done" ] || [ ! -s "$f" ]; then
    echo "$(date -u +%FT%TZ) $X NOT STARTED: input $f not finished" >> "$R/chain.log"; exit 3
  fi
done < "$INC"
mkdir -p "$D/tmp" "$D/index" "$D/out"
source $WS/ops/env.sh
export TMPDIR=$D/tmp          # AFTER env.sh (L22)
arr=(); while read -r f; do [ -n "$f" ] && arr+=(--arrivals "$f"); done < "$INC"
# the rows depend only on the spec and the incoming arrival files: reused while that input list is unchanged
INSIG=$(cat "$SPEC" "$INC" | sha256sum | cut -c1-64)
if [ -e "$D/rows.done" ] && [ "$(cat "$D/rows.done")" = "$INSIG" ] && [ -s "$D/rows.jsonl" ]; then
  echo "$(date -u +%FT%TZ) $X rows reused" >> "$R/chain.log"
else
  echo "$(date -u +%FT%TZ) $X start rows (incoming files: $(grep -c . "$INC" || true)) bin=$(sha256sum $BIN | cut -c1-16)" >> "$R/chain.log"
  /run/current-system/sw/bin/time -v "$BIN" closure-rows --spec "$SPEC" "${arr[@]}" --out "$D/rows.jsonl" --summary "$D/rows.summary.json" \
    --threads 8 --index-dir "$D/index" > "$D/rows.out" 2> "$D/rows.err" || { echo "$(date -u +%FT%TZ) $X rows FAILED" >> "$R/chain.log"; exit 2; }
  echo "$INSIG" > "$D/rows.done"
fi
[ "${ROWS_ONLY:-0}" = 1 ] && { echo "$(date -u +%FT%TZ) $X rows only (ROWS_ONLY)" >> "$R/chain.log"; exit 0; }
if [ ! -s "$EARGS" ]; then
  echo "$(date -u +%FT%TZ) $X has no outgoing edge file: rows only" >> "$R/chain.log"
  touch "$D/done"; exit 0
fi
# shellcheck disable=SC2046
/run/current-system/sw/bin/time -v "$BIN" compute --spec "$SPEC" --rows "$D/rows.jsonl" --out-dir "$D/out" --parent "$X" \
  --edge-producer "handdown-routes chain probe: frozen WS/bin/frozen/20261114T1718Z-handdown-chain/p_handdown_routes (4ad761d9, sha256 a9ebd498c99f); edge files of the route lane or of lane arrivals (same probe + the route lane's gen_edge.py); map chosen" \
  --exact none --threads 8 --cache-entries 2000000 --index-dir "$D/index" $(cat "$EARGS") > "$D/compute.out" 2> "$D/compute.err" \
  || { echo "$(date -u +%FT%TZ) $X compute FAILED" >> "$R/chain.log"; exit 2; }
# the rows file is large and regenerable: keep its summary, remove it (literal-prefix guard, L18)
X_ROWS=$D/rows.jsonl
case "$X_ROWS" in /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/chain*/T*/rows.jsonl) rm -- "$X_ROWS";; esac
echo "$(date -u +%FT%TZ) $X done $(grep -E 'Elapsed|Maximum resident' "$D/compute.err" | tr '\n' ' ')" >> "$R/chain.log"
touch "$D/done"
