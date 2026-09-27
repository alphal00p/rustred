#!/usr/bin/env bash
# W0.6 close-out (session 2026-09-27 evening): same-CPU plan-v3 control (probe-v3b, CPUs of probe-I1b)
# and the I1 closure probe (probe-I1), concurrently, each W24, cooperative stop at 3,240 s.
set -u
cd /common/dev/rustred
export TMPDIR=/common/dev/rustred/TMP
T=/common/dev/rustred/.claude/worktrees/fable51-inputs/tools/research/inputs
H=/common/dev/rustred/.claude/worktrees/fable51-inputs/TMP/bin/cp5hop
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
V3CPUS=100-117,362-367
I1CPUS=88-95,120-127,344-351
nice -n 19 taskset -c 96 $PY $T/loadmon.py --cpus $V3CPUS --match TMP/w0/inputs/probe-v3b/ --out TMP/w0/inputs/probe-v3b.load.jsonl &
nice -n 19 taskset -c 96 $PY $T/loadmon.py --cpus $I1CPUS --match TMP/w0/inputs/probe-I1/ --out TMP/w0/inputs/probe-I1.load.jsonl &
nice -n 5 taskset -c 88-127,344-383 nix develop --command python $T/probe.py \
  --queries TMP/w0/inputs/candidates/plan-v3/queries.json --out TMP/w0/inputs/probe-v3b \
  --cpus $V3CPUS --workers 24 --stop-after 3240 \
  --cp5hop $H --bounds TMP/w0/inputs/candidates/bounds-v3.tsv > TMP/w0/inputs/probe-v3b.log 2>&1 &
P1=$!
nice -n 5 taskset -c 88-127,344-383 nix develop --command python $T/probe.py \
  --queries TMP/w0/inputs/candidates/plan-I1/queries.json --out TMP/w0/inputs/probe-I1 \
  --cpus $I1CPUS --workers 24 --stop-after 3240 \
  --cp5hop $H --bounds TMP/w0/inputs/candidates/bounds-I1.tsv > TMP/w0/inputs/probe-I1.log 2>&1 &
P2=$!
wait $P1; echo "probe-v3b exit $?" >> TMP/w0/inputs/run_probes_b.log
wait $P2; echo "probe-I1 exit $?" >> TMP/w0/inputs/run_probes_b.log
wait
echo done >> TMP/w0/inputs/run_probes_b.log
