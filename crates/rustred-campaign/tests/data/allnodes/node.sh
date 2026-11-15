#!/usr/bin/env bash
# Lane allnodes: ONE node of the integer-index sweep (runs INSIDE its private pid namespace, started by launch_node.sh).
# Steps (epsframe's design on the integer path): Tier K keys-only (mode k) -> strict Tier C rounds: the check's own fact
# requests answered by rr_proof instantiate (frozen engine-line7b) as real child jobs, up to $MAXR rounds and $BUDGET requested
# keys -> decided: decisive zeros' second route (rr_coef_facts facts) -> mode z2. Resumable: final.json = done, never redone.
# Usage: node.sh <node>   env: W SPECS BIN PROOF COEF BUDGET (number | auto) BUDGET_MARGIN BUDGET_CAP MAXR THREADS KONLY (comma list of Tier-K-only nodes)
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
status=start; total=0; r=0; facts=""; second=no; budget=$BUDGET
case ",$KONLY," in
  *",$node,"*) status=tier_k_only_by_brief ;;
  *)
    if ! grep -q '"mode":"k"' "$nd/results.jsonl" 2>/dev/null || grep -q '"error"' "$nd/results.jsonl"; then
      status=load_or_tier_k_failed
    else
      # the request budget: a parameter; BUDGET=auto -> what the node needs: BUDGET_MARGIN x the Tier K (superset) closure
      # keys, capped at BUDGET_CAP (every key is requested at most once, and the Tier C closure lies inside Tier K's)
      if [ "$BUDGET" = auto ]; then
        budget=$(python3 -I -c '
import json,math,sys
k=[json.loads(l) for l in open(sys.argv[1]) if json.loads(l)["mode"]=="k"][-1]
print(min(int(sys.argv[3]), math.ceil(k["keys"]*float(sys.argv[2]))))' "$nd/results.jsonl" "$BUDGET_MARGIN" "$BUDGET_CAP")
      else budget=$BUDGET; fi
      run env AN_MODE=c
      while :; do
        k=$(python3 -I -c 'import json,sys;print(sum(len(json.loads(l)["keys"]) for l in open(sys.argv[1])))' "$nd/requests.jsonl" 2>/dev/null || echo -1)
        [ "$k" = "-1" ] && { status=no_request_file; break; }
        [ "$k" = "0" ] && { status=decided; break; }
        # PREFETCH (round 1 only, PREFETCH=1): the Tier K superset's outside-box winners join the round's child jobs
        extra=/dev/null
        if [ $r -eq 0 ] && [ "${PREFETCH:-0}" = 1 ] && [ -s "$nd/k_requests.jsonl" ]; then
          extra=$nd/k_requests.jsonl
          k=$((k + $(python3 -I -c 'import json,sys;print(sum(len(json.loads(l)["keys"]) for l in open(sys.argv[1])))' "$extra")))
        fi
        [ $r -ge $MAXR ] && { status=max_rounds; break; }
        total=$((total + k)); [ $total -gt $budget ] && { status=over_budget; break; }
        r=$((r + 1))
        # temporary-facts gate (implementer 2026-11-15T09:5xZ: all nodes in flight under FACTS_LIMIT bytes): reserve
        # k x FACTS_BYTES_PER_KEY in $W/inflight/<node> under a lock; a node alone always proceeds
        need=$(( k * ${FACTS_BYTES_PER_KEY:-7000} )); mkdir -p "$W/inflight"
        while :; do
          exec 7>"$W/inflight.lock"; flock 7
          used=$(cat "$W"/inflight/* 2>/dev/null | awk -v me="$(cat "$W/inflight/$node" 2>/dev/null || echo 0)" '{s+=$1} END{printf "%d", s-me}')
          if [ "$used" -eq 0 ] || [ $((used + need)) -le ${FACTS_LIMIT:-50000000000} ]; then
            echo $(( need + $(cat "$W/inflight/$node" 2>/dev/null || echo 0) )) > "$W/inflight/$node"; flock -u 7; exec 7>&-; break
          fi
          flock -u 7; exec 7>&-
          echo "$(date -u +%FT%TZ) facts gate: waiting (in flight ${used} B, need ${need} B)" >> "$nd/last.log"; sleep 30
        done
        # the round's requests as child jobs of at most CHUNK_KEYS keys each (rr_proof holds a job's answers in memory)
        nch=$(python3 -I -c '
import json,sys
lim=int(sys.argv[3]); j=0; n=0; o=None
def nxt():
    global j,n,o
    if o: o.close()
    j+=1; n=0; o=open(f"{sys.argv[2]}.{j}","w")
nxt()
import itertools
for l in itertools.chain(open(sys.argv[1]), open(sys.argv[4])):
    q=json.loads(l); keys=q["keys"]
    while keys:
        if n>=lim: nxt()
        take=keys[:lim-n]; keys=keys[lim-n:]
        o.write(json.dumps({"table":q["table"],"offset":q["offset"],"rule":q["rule"],"keys":take})+"\n"); n+=len(take)
o.close(); print(j)' "$nd/requests.jsonl" "$nd/work/rr.$r" "${CHUNK_KEYS:-500000}" "$extra")
        for j in $(seq 1 "$nch"); do
          "$PROOF" instantiate --requests "$nd/work/rr.$r.$j" --out "$nd/work/f.$r.$j" > "$nd/proof.$r.$j.log" 2>&1 \
            || { status=proof_failed_round_${r}_job_$j; break 2; }
          facts="$facts:$nd/work/f.$r.$j"
        done
        AN_FACTS=$facts run env AN_MODE=c
      done
      if [ "$status" = decided ] && [ -s "$nd/zreq.jsonl" ]; then
        "$COEF" facts --requests "$nd/zreq.jsonl" --out "$nd/work/f2" --threads "$THREADS" > "$nd/coef.log" 2>&1 \
          && { AN_FACTS=$facts AN_FACTS2=$nd/work/f2 run env AN_MODE=z2; second=yes; } || second=coef_failed
      fi
    fi ;;
esac
pf=off; [ "${PREFETCH:-0}" = 1 ] && pf=on
printf '{"node": "%s", "rounds": %d, "requested_keys": %d, "budget": "%s", "budget_rule": "%s", "prefetch": "%s", "chunk_keys": "%s", "stop": "%s", "second_route": "%s", "wall_s": %d, "utc": "%s"}\n' \
  "$node" "$r" "$total" "$budget" "$BUDGET margin=${BUDGET_MARGIN:-} cap=${BUDGET_CAP:-}" "$pf" "${CHUNK_KEYS:-}" "$status" "$second" "$(( $(date +%s) - t0 ))" "$(date -u +%FT%TZ)" > "$nd/final.json"
# keep finished nodes small (L18: literal-prefix guards; the facts' digests stay in results.jsonl `fact_source`):
# the temporary index, TMPDIR, the work dir (requests to and answers of the child jobs) and the row / request files
P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/allnodes
for X in "$W/index/$node" "$W/tmp/$node" "$nd/work"; do
  case "$X" in $P/run*/index/?*|$P/run*/tmp/?*|$P/run*/nodes/?*/work) [ -d "$X" ] && rm -r -- "$X";; esac
done
for X in "$nd/rows_k.jsonl" "$nd/rows_c.jsonl" "$nd/requests.jsonl" "$nd/proof_requests.jsonl" "$nd/k_requests.jsonl"; do
  case "$X" in $P/run*/nodes/?*/*.jsonl) [ -f "$X" ] && rm -- "$X";; esac
done
X=$W/inflight/$node
case "$X" in $P/run*/inflight/?*) [ -f "$X" ] && rm -- "$X";; esac
