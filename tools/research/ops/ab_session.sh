#!/usr/bin/env bash
# W1.4 ops / N3 control session: Ordered walks with the arms interleaved per
# family, each run gated on
#   - strict record identity against the frozen 4a17f9c7 Ordered reference
#     (compare_walk_records --mode strict: verdict PASS, 0 differing records);
#   - the oracle (handoff 0.1 item 6, tools/research/ops/oracle_check.py):
#     walk-verify-closure --require-closure --reinspect all with
#     verdict == PASS and roots_independently_verified == roots_total, plus the
#     paired audit PASS.
# usage: ab_session.sh TAG REPEATS "arm=BIN ..." [families] [cpus] [workers]
#   runs tools/research/ops/knob_run.py (perf counters and the recorder on by
#   default) as <TAG>-<arm>-r<repeat>/<family> under $KNOB_OUT_ROOT
#   (default TMP/w1-ops/runs).
# env: ORACLE=all|first|none (default all; first = repeat 1 only),
#      ORACLE_CPUS (default: the run CPUs), ORACLE_THREADS (default 16),
#      ORACLE_ASYNC=1 (verify run k on ORACLE_CPUS while run k+1 walks; one
#      verifier at a time; results collected at the end),
#      KNOB_EXTRA (extra knob_run.py arguments, e.g. "--no-perf").
# References: fg/bmw/h/x TMP/w0/oracle/runs/c4l-ordered/<family>,
#   five-finite TMP/w0/oracle/runs/c5f-ordered/five-finite,
#   four-all TMP/fable51-controls/c4l-4a17f9c7-ordered-w6-rep1/four-all.
# Exit status 1 if any run fails its exit code, strict compare or oracle gate.
set -u
TAG=$1; REPEATS=$2; ARMS=$3; FAMS=${4:-"fg bmw h x"}; CPUS=${5:-80-85}; WORKERS=${6:-}
ORACLE=${ORACLE:-all}; ORACLE_CPUS=${ORACLE_CPUS:-$CPUS}; ORACLE_THREADS=${ORACLE_THREADS:-16}
ROOT=/common/dev/rustred
WT=$ROOT/.claude/worktrees/agent-ade877816b107b1cf
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
export KNOB_OUT_ROOT=${KNOB_OUT_ROOT:-$ROOT/TMP/w1-ops/runs}
export TMPDIR=$ROOT/TMP
failures=0
pending=""
oracle_line() {  # run dir -> "PASS 248/248 PASS audit PASS 10s"
  $PY -c "import json,sys; r=json.load(open(sys.argv[1])); print(r['gate'], f\"{r['roots_independently_verified']}/{r['roots_total']}\", r['verdict'], 'audit', r['audit'], f\"{r['verify_seconds'] or 0:.0f}s\")" "$1/oracle/oracle-gate.json" 2>/dev/null || echo "FAIL error"
}
for rep in $(seq 1 $REPEATS); do
  for fam in $FAMS; do
    case $fam in
      five-finite) ref=$ROOT/TMP/w0/oracle/runs/c5f-ordered/five-finite ;;
      four-all) ref=$ROOT/TMP/fable51-controls/c4l-4a17f9c7-ordered-w6-rep1/four-all ;;
      *) ref=$ROOT/TMP/w0/oracle/runs/c4l-ordered/$fam ;;
    esac
    for pair in $ARMS; do
      arm=${pair%%=*}; bin=${pair#*=}
      label=$TAG-$arm-r$rep
      extra=()
      [ -n "$WORKERS" ] && extra=(--workers $WORKERS)
      # shellcheck disable=SC2086
      $PY $WT/tools/research/ops/knob_run.py --binary $bin --family $fam --label $label --cpus $CPUS \
          --policy ordered "${extra[@]}" --keep-result ${KNOB_EXTRA:-}
      out=$KNOB_OUT_ROOT/$label/$fam
      code=$($PY -c "import json,sys; print(json.load(open(sys.argv[1]))['exit_code'])" $out/metrics.json 2>/dev/null)
      nice -n 19 taskset -c $CPUS $PY $WT/examples/python/compare_walk_records.py --mode strict \
          $ref/result.json $out/result.json --output $out/strict-vs-4a17f9c7.json > /dev/null 2>&1
      strict=$($PY -c "import json,sys; r=json.load(open(sys.argv[1])); print('PASS' if r.get('verdict')=='PASS' and r.get('differing_records')==0 else 'FAIL', r.get('differing_records'))" $out/strict-vs-4a17f9c7.json 2>/dev/null || echo "FAIL unreadable")
      oracle="skipped"
      if [ "$ORACLE" = all ] || { [ "$ORACLE" = first ] && [ $rep = 1 ]; }; then
        if [ "${ORACLE_ASYNC:-0}" = 1 ]; then
          wait  # one verifier at a time
          $PY $WT/tools/research/ops/oracle_check.py $out --cpus $ORACLE_CPUS --threads $ORACLE_THREADS > /dev/null 2>&1 &
          pending="$pending $label/$fam"
          oracle="async"
        else
          $PY $WT/tools/research/ops/oracle_check.py $out --cpus $ORACLE_CPUS --threads $ORACLE_THREADS > /dev/null 2>&1
          oracle=$(oracle_line $out)
        fi
      fi
      echo "RUN $label/$fam exit=$code strict=$strict oracle=$oracle"
      [ "$code" = 0 ] || failures=$((failures+1))
      case $strict in PASS*) ;; *) failures=$((failures+1)) ;; esac
      case $oracle in PASS*|skipped|async) ;; *) failures=$((failures+1)) ;; esac
    done
  done
done
wait
for run in $pending; do
  oracle=$(oracle_line $KNOB_OUT_ROOT/$run)
  echo "ORACLE $run $oracle"
  case $oracle in PASS*) ;; *) failures=$((failures+1)) ;; esac
done
echo "SESSION $TAG failures=$failures"
[ $failures = 0 ]
