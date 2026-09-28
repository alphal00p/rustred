#!/usr/bin/env bash
# i2 lane: 30-min plan-v3 five-loop probes (legacy engine 4a17f9c7, W24 each, concurrently), original vs i2b witnesses.
# CPUs: orig on 88-107,344-347 and i2b on 108-127,364-367 (20 cores + 4 SMT siblings each); cooperative stop at 1,800 s.
set -u
cd /common/dev/rustred
export TMPDIR=/common/dev/rustred/TMP
WT=/common/dev/rustred/.claude/worktrees/fable51-inputs
T=$WT/tools/research/inputs
H=$WT/TMP/bin/cp5hop
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
Q=$WT/examples/input/five_loop_qcd_feynman_d9d10/queries.json
O=TMP/w1/i2/probe
mkdir -p $O
OC=${OC:-88-107,344-347}; IC=${IC:-108-127,364-367}
nice -n 19 taskset -c 126 $PY $T/loadmon.py --cpus $OC --match $O/orig/ --out $O/orig.load.jsonl &
nice -n 19 taskset -c 126 $PY $T/loadmon.py --cpus $IC --match $O/i2b/ --out $O/i2b.load.jsonl &
nice -n 5 taskset -c 88-127,344-383 $PY $T/probe.py --queries $Q --out $O/orig --cpus $OC --workers 24 --stop-after 1800 \
  --cp5hop $H > $O/orig.log 2>&1 &
P1=$!
nice -n 5 taskset -c 88-127,344-383 $PY $T/probe.py --queries $Q --out $O/i2b --cpus $IC --workers 24 --stop-after 1800 \
  --selection /common/dev/rustred/TMP/w1/i2/selection-i2b.json --cp5hop $H > $O/i2b.log 2>&1 &
P2=$!
wait $P1; echo "orig exit $? $(date -u +%FT%TZ)" >> $O/run.log
wait $P2; echo "i2b exit $? $(date -u +%FT%TZ)" >> $O/run.log
wait
echo "done $(date -u +%FT%TZ)" >> $O/run.log
