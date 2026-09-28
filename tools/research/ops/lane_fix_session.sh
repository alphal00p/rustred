#!/usr/bin/env bash
# Fix-round checks of the W1.4 ops lane on the lane CPUs (80-87,336-343; no
# run here uses >= 24 threads). Steps (each logs a verdict line to $LOG):
#   drills   FG frontier fixture: stop / record / binding drills for the three
#            binaries (stops pinned to the fixture's frontier records, oracle
#            gate), and the Ready stop drill through the production launcher
#            and the supervisor for campaign+mimalloc and release;
#   fourall  C-4L-comb-O: four-all Ordered W6, 3 arms x 2 repeats, strict vs
#            the 4a17f9c7 Ordered reference and the oracle gate;
#   c4l      C-4L FG/BMW/H/X Ordered W6 on CPUs 80-85, 3 arms x 3 interleaved
#            repeats, perf counters + recorder, strict + oracle gate on every
#            run (verifier asynchronously on 86-87,342-343);
#   resume   FG mid-walk cross-binary resumes 4a17f9c7 -> campaign and
#            4a17f9c7 -> campaign+mimalloc (strict + oracle);
#   contend  600 s equal-priority contention, 12 processes: core lib test
#            executable baseline (pre-hardening catalog test) and r2, whole
#            suite and persistence::catalog; app lib r2 walking::parallel.
# usage: lane_fix_session.sh STEP RELEASE CAMPAIGN MIMALLOC [CORE_R2 APP_R2]
set -u
STEP=$1; REL=$2; CAM=$3; MI=$4; CORE_R2=${5:-}; APP_R2=${6:-}
ROOT=/common/dev/rustred
WT=$ROOT/.claude/worktrees/agent-ade877816b107b1cf
OPS=$WT/tools/research/ops
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
LOG=$ROOT/TMP/w1-ops/runs/lane-fix-$STEP.log
LEGACY=$ROOT/TMP/fable51-controls/bin/rustred-4a17f9c7
export TMPDIR=$ROOT/TMP
exec >> $LOG 2>&1
echo "$(date -u +%FT%TZ) $STEP start"
case $STEP in
  drills)
    for pair in release=$REL campaign=$CAM campaignmi=$MI; do
      arm=${pair%%=*}; bin=${pair#*=}
      for mode in stop record binding; do
        $PY $OPS/frontier_drill.py --binary $bin --mode $mode --label drill5-$mode-$arm --cpus 80-85 > /dev/null 2>&1
        echo "DRILL $mode $arm exit=$? $($PY -c "import json,sys; s=json.load(open(sys.argv[1])); print({k: s.get(k) for k in ('stops','stops_exactly_at_kth_frontier_record','final_strict_vs_fixture','strict_vs_fixture','record_resume_accepted','stop_resume_refused')}, 'oracle', (s.get('final_oracle') or s.get('oracle') or s.get('record_resume_oracle') or {}).get('gate'))" $ROOT/TMP/w1-ops/runs/drill5-$mode-$arm/summary.json 2>&1 | cut -c1-600)"
      done
    done
    for pair in campaignmi=$MI release=$REL; do
      arm=${pair%%=*}; bin=${pair#*=}
      $PY $OPS/ready_launcher_drill.py --binary $bin --label ready-launcher-$arm --cpus 80-85 > $ROOT/TMP/w1-ops/runs/ready-launcher-$arm.log 2>&1
      echo "READY-LAUNCHER $arm exit=$? $(tail -40 $ROOT/TMP/w1-ops/runs/ready-launcher-$arm.log | tr -d '\n' | cut -c1-900)"
    done ;;
  fourall)
    ORACLE=all ORACLE_CPUS=80-87,336-343 ORACLE_THREADS=16 $OPS/ab_session.sh n3r2fa 2 \
      "release=$REL campaign=$CAM campaignmi=$MI" four-all 80-85 6
    echo "fourall exit $?" ;;
  c4l)
    ORACLE=all ORACLE_ASYNC=1 ORACLE_CPUS=86-87,342-343 ORACLE_THREADS=4 $OPS/ab_session.sh n3r2 3 \
      "release=$REL campaign=$CAM campaignmi=$MI" "fg bmw h x" 80-85
    echo "c4l exit $?"
    $PY $OPS/ab_table.py n3r2 release campaign campaignmi ;;
  resume)
    for pair in campaign=$CAM campaignmi=$MI; do
      arm=${pair%%=*}; bin=${pair#*=}
      $PY $OPS/pause_resume.py --first $LEGACY --second $bin --family fg --label fg-resume-4a17f9c7-to-$arm \
        --cpus 80-85 --workers 6 --stop-at-committed 50000 \
        --reference $ROOT/TMP/w0/oracle/runs/c4l-ordered/fg/result.json \
        --oracle --oracle-cpus 80-87,336-343 --oracle-threads 16 | cut -c1-900
    done ;;
  contend)
    base=$WT/TMP/ops/baseline/rustred-core-lib-census-c24c7417
    CWD=$WT/crates/rustred-core $OPS/contend.sh $base 600 $WT/TMP/ops/contend-r2-core-baseline 2 "" persistence::catalog &
    CWD=$WT/crates/rustred-core $OPS/contend.sh $CORE_R2 600 $WT/TMP/ops/contend-r2-core-hardened 2 "" persistence::catalog &
    CWD=$WT/crates/rustred-app $OPS/contend.sh $APP_R2 600 $WT/TMP/ops/contend-r2-app-parallel 4 walking::parallel &
    wait ;;
esac
echo "$(date -u +%FT%TZ) $STEP end"
