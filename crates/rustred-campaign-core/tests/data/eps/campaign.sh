#!/usr/bin/env bash
# EPS CAMPAIGN SWEEP driver (lane epsframe): per node (smallest table first, resumable): Tier K keys-only superset,
# then strict Tier C rounds with rr_proof instantiate facts for the check's own requests (byte offsets + chart,
# engine-line7b, one job per chart), up to $MAXR rounds and $BUDGET requested keys per node.
# Usage: campaign.sh <nodes.jsonl> <work dir>   (env: LAYER=3 BUDGET=20000 MAXR=4 BIN=<eps_frame test binary>)
set -u
NODES=$1; W=$2
LAYER=${LAYER:-3}; BUDGET=${BUDGET:-20000}; MAXR=${MAXR:-4}
PROOF=${PROOF:-/shared/vhirschi/t3code/vakint_dev/p_integral_dev/bin/frozen/20261115T0652Z-engine-line7d/rr_proof}
SPLIT=$W/split_req.py
OUT=$W/results.jsonl; FIN=$W/final.jsonl
mkdir -p $W/tmp
export TMPDIR=$W/tmp
while read -r nl; do
  node=$(echo "$nl" | python3 -I -c 'import json,sys;print(json.loads(sys.stdin.read())["node"])')
  table=$(echo "$nl" | python3 -I -c 'import json,sys;print(json.loads(sys.stdin.read())["table"])')
  grep -q "\"node\": \"$node\"" $FIN 2>/dev/null && continue
  nd=$W/nodes/$node; mkdir -p $nd
  run() { EPSC_NODE=$node EPSC_TABLE=$table EPSC_DIR=$nd EPSC_LAYER=$LAYER EPSC_OUT=$OUT "$@" $BIN --ignored --exact eps_campaign_node --test-threads 1 > $nd/last.log 2>&1; }
  run env EPSC_MODE=k
  : > $nd/facts.jsonl
  run env EPSC_MODE=c
  status=start; total=0; r=0
  while [ $r -lt $MAXR ]; do
    k=$(python3 -I -c 'import json,sys;print(sum(len(json.loads(l)["keys"]) for l in open(sys.argv[1])))' $nd/requests.jsonl 2>/dev/null || echo -1)
    [ "$k" = "0" ] && { status=no_requests; break; }
    [ "$k" = "-1" ] && { status=no_request_file; break; }
    total=$((total + k)); [ $total -gt $BUDGET ] && { status=over_budget; break; }
    charts=$(python3 -I $SPLIT $nd/requests.jsonl $nd/rr) || { status=split_failed; break; }
    for c in $charts; do
      # one retry: a child that exits non-zero is rerun once (its exit code is recorded either way)
      $PROOF instantiate --requests $nd/rr.$c --out $nd/f.$c > $nd/proof.$c.log 2>&1
      rc=$?
      if [ $rc -ne 0 ]; then
        echo "{\"node\": \"$node\", \"job\": \"$c\", \"exit\": $rc, \"retry\": 1}" >> $W/proof_retries.jsonl
        $PROOF instantiate --requests $nd/rr.$c --out $nd/f.$c > $nd/proof.$c.retry.log 2>&1 || { status="proof_failed_exit_$?"; break 2; }
      fi
      cat $nd/f.$c >> $nd/facts.jsonl
    done
    r=$((r + 1))
    EPSC_FACTS=$nd/facts.jsonl run env EPSC_MODE=c
    status=round_$r
  done
  printf '{"node": "%s", "rounds": %d, "requested_keys": %d, "stop": "%s"}\n' "$node" "$r" "$total" "$status" >> $FIN
done < $NODES
echo CAMPAIGN_DONE >> $FIN
