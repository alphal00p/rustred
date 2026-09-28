#!/usr/bin/env bash
# W1 ops/N3 control session (Ordered, own CPUs, interleaved arms).
# usage: ab_session.sh TAG REPEATS "arm=BIN ..." [families] [cpus] [workers]
#   runs  KNOB_OUT_ROOT=/common/dev/rustred/TMP/w1-ops/runs knob_run.py for each repeat, family, arm
#   label <TAG>-<arm>-r<repeat>; then strict compare vs TMP/w0/oracle/runs/c4l-ordered/<fam>
#   (or c5f-ordered/five-finite) with compare_walk_records --mode strict.
set -u
TAG=$1; REPEATS=$2; ARMS=$3; FAMS=${4:-"fg bmw h x"}; CPUS=${5:-80-85}; WORKERS=${6:-}
ROOT=/common/dev/rustred
WT=$ROOT/.claude/worktrees/agent-ade877816b107b1cf
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
export KNOB_OUT_ROOT=$ROOT/TMP/w1-ops/runs
export TMPDIR=$ROOT/TMP
for rep in $(seq 1 $REPEATS); do
  for fam in $FAMS; do
    for pair in $ARMS; do
      arm=${pair%%=*}; bin=${pair#*=}
      label=$TAG-$arm-r$rep
      extra=()
      [ -n "$WORKERS" ] && extra=(--workers $WORKERS)
      $PY $ROOT/TMP/w1-ops/tools/knob_run.py --binary $bin --family $fam --label $label --cpus $CPUS \
          --policy ordered "${extra[@]}" --keep-result
      ref=$ROOT/TMP/w0/oracle/runs/c4l-ordered/$fam
      [ $fam = five-finite ] && ref=$ROOT/TMP/w0/oracle/runs/c5f-ordered/five-finite
      out=$KNOB_OUT_ROOT/$label/$fam
      nice -n 19 taskset -c 86-87,342-343 $PY $WT/examples/python/compare_walk_records.py --mode strict \
          $ref/result.json $out/result.json --output $out/strict-vs-4a17f9c7.json > /dev/null 2>&1
      echo "STRICT $label/$fam: $(grep -o '"verdict": *"[A-Z]*"' $out/strict-vs-4a17f9c7.json) $(grep -o '"differing_records": *[0-9]*' $out/strict-vs-4a17f9c7.json)"
    done
  done
done
