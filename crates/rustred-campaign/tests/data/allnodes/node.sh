#!/usr/bin/env bash
# Lane allnodes: ONE node of the integer-index sweep (runs INSIDE its private pid namespace, started by launch_node.sh).
# Steps (epsframe's design on the integer path): Tier K keys-only (mode k) -> strict Tier C rounds: the check's own fact
# requests answered by rr_proof instantiate (frozen engine-line7b) as real child jobs, up to $MAXR rounds and $BUDGET requested
# keys -> decided: decisive zeros' second route (rr_coef_facts facts) -> mode z2. Resumable: final.json = done, never redone.
# Usage: node.sh <node>   env: W SPECS BIN PROOF COEF BUDGET MAXR THREADS KONLY (comma list of Tier-K-only nodes)
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
node=$1
nd=$W/nodes/$node
[ -e "$nd/final.json" ] && exit 0
mkdir -p "$nd/work" "$W/tmp/$node" "$W/index/$node"
source $WS/ops/env.sh
export TMPDIR=$W/tmp/$node          # AFTER env.sh (L22)
rec=$(grep -F "\"node\": \"$node\"" "$SPECS/nodes.jsonl")
spec=$(echo "$rec" | python3 -I -c 'import json,sys;print(json.loads(sys.stdin.read())["spec"])')
family=$(echo "$rec" | python3 -I -c 'import json,sys;print(json.loads(sys.stdin.read())["family"])')
echo "$(date -u +%FT%TZ) start node=$node pid=$$ TMPDIR=$TMPDIR" >> "$nd/last.log"
run() { AN_NODE=$node AN_SPEC=$spec AN_DIR=$nd AN_FAMILY=$family AN_INDEX=$W/index/$node AN_OUT=$nd/results.jsonl \
        AN_THREADS=$THREADS "$@" "$BIN" --ignored --exact allnodes_node --test-threads 1 >> "$nd/last.log" 2>&1; }
t0=$(date +%s)
run env AN_MODE=k
status=start; total=0; r=0; facts=""; second=no
case ",$KONLY," in
  *",$node,"*) status=tier_k_only_by_brief ;;
  *)
    if ! grep -q '"mode":"k"' "$nd/results.jsonl" 2>/dev/null || grep -q '"error"' "$nd/results.jsonl"; then
      status=load_or_tier_k_failed
    else
      run env AN_MODE=c
      while :; do
        k=$(python3 -I -c 'import json,sys;print(sum(len(json.loads(l)["keys"]) for l in open(sys.argv[1])))' "$nd/requests.jsonl" 2>/dev/null || echo -1)
        [ "$k" = "-1" ] && { status=no_request_file; break; }
        [ "$k" = "0" ] && { status=decided; break; }
        [ $r -ge $MAXR ] && { status=max_rounds; break; }
        total=$((total + k)); [ $total -gt $BUDGET ] && { status=over_budget; break; }
        r=$((r + 1))
        python3 -I -c '
import json,sys
with open(sys.argv[2],"w") as o:
    for l in open(sys.argv[1]):
        q=json.loads(l); o.write(json.dumps({"table":q["table"],"offset":q["offset"],"rule":q["rule"],"keys":q["keys"]})+"\n")' \
          "$nd/requests.jsonl" "$nd/work/rr.$r"
        "$PROOF" instantiate --requests "$nd/work/rr.$r" --out "$nd/work/f.$r" > "$nd/proof.$r.log" 2>&1 || { status=proof_failed_round_$r; break; }
        facts="$facts:$nd/work/f.$r"
        AN_FACTS=$facts run env AN_MODE=c
      done
      if [ "$status" = decided ] && [ -s "$nd/zreq.jsonl" ]; then
        "$COEF" facts --requests "$nd/zreq.jsonl" --out "$nd/work/f2" --threads "$THREADS" > "$nd/coef.log" 2>&1 \
          && { AN_FACTS=$facts AN_FACTS2=$nd/work/f2 run env AN_MODE=z2; second=yes; } || second=coef_failed
      fi
    fi ;;
esac
printf '{"node": "%s", "rounds": %d, "requested_keys": %d, "stop": "%s", "second_route": "%s", "wall_s": %d, "utc": "%s"}\n' \
  "$node" "$r" "$total" "$status" "$second" "$(( $(date +%s) - t0 ))" "$(date -u +%FT%TZ)" > "$nd/final.json"
# keep finished nodes small (L18: literal-prefix guards; the facts' digests stay in results.jsonl `fact_source`):
# the temporary index, TMPDIR, the work dir (requests to and answers of the child jobs) and the row / request files
P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/allnodes
for X in "$W/index/$node" "$W/tmp/$node" "$nd/work"; do
  case "$X" in $P/run*/index/?*|$P/run*/tmp/?*|$P/run*/nodes/?*/work) [ -d "$X" ] && rm -r -- "$X";; esac
done
for X in "$nd/rows_k.jsonl" "$nd/rows_c.jsonl" "$nd/requests.jsonl" "$nd/proof_requests.jsonl"; do
  case "$X" in $P/run*/nodes/?*/*.jsonl) [ -f "$X" ] && rm -- "$X";; esac
done
