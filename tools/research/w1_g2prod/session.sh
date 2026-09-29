#!/usr/bin/env bash
# Lane g2prod gate session: production G2' residual anchors (--g2-residual-anchors union).
# usage: session.sh BIN PHASE...   (run a snapshot copy; never edit while a session runs it)
# Phases:
#   identity     flag off: FG/BMW/H/X/four-all Ordered W6 vs a fresh 4a17f9c7 run of the same session (strict
#                compare + equal containment_checks), four-all-p5 W6 (run_c4l.py), C-5F Ordered W16 vs the
#                4a17f9c7 reference TMP/fable51-controls/int-ref-g3/five-finite
#   c4l          flag on: FG/BMW/H/X/four-all (+ four-all-p5) Ordered W6
#   post-c4l     oracle (walk-verify-closure --require-closure --reinspect all, assert_oracle_pass, paired audit)
#   main         C-5F Ordered/Ready W24 off/union x2 interleaved (all lane CPUs, perf stat), then C-HOT-sub r1a12
#                Ready W12 off/union x2 (two runs at a time on the CCD halves)
#   post-main    stats, pending series, audits for every main run; oracle on every union run and on r1 off runs
#   drills       C-5F G2' pause/resume: Ready multi-prefix diagnostic pause + resume (ready_resume_control.py);
#                3 pause points Ordered (strict vs the uninterrupted main run) and Ready; oracle on every final
# CPU sets: lane 16-31,272-287; halves A=16-23,272-279 B=24-31,280-287; W6 on physical cores 16-21 / 24-29.
set -u
BIN=${1:?usage: session.sh BIN PHASE...}; shift
R=/common/dev/rustred
WT=$R/.claude/worktrees/fable51-g2f
T=$WT/tools/research/w1_g2prod
OUT=$R/TMP/w1/g2prod/runs
LOG=$R/TMP/w1/g2prod/logs
REF=$R/TMP/fable51-controls/bin/rustred-4a17f9c7
REFRUNS=$R/TMP/fable51-controls/int-ref-g3
PY="nix develop $R --command python"
ALL=16-31,272-287; A=16-23,272-279; B=24-31,280-287; A6=16-21; B6=24-29
HOTQ=$R/TMP/w0/falsify/inputs/hotsub-r1a12.json
SHA=$(basename "$BIN" | sed 's/rustred-//')
export TMPDIR=$R/TMP
mkdir -p $OUT $LOG
log() { echo "$(date -u +%FT%TZ) $*" | tee -a $LOG/session-$SHA.log; }
arm() { # label family cpus policy workers g2 [run_arm args]
  local label=$1 fam=$2 cpus=$3 policy=$4 workers=$5 g2=$6; shift 6
  log "start $label/$fam cpus=$cpus policy=$policy W$workers g2=$g2 $*"
  $PY $T/run_arm.py --binary "$BIN" --family "$fam" --label "$label" --cpus "$cpus" --policy "$policy" \
      --workers "$workers" --g2 "$g2" --time-limit 3300 --grace 240 "$@" > "$OUT/.$label-$fam.runlog" 2>&1
  log "end $label/$fam $(grep -o '"exit_code": [0-9-]*' $OUT/$label/$fam/metrics.json 2>/dev/null) $(grep -o '"whole_command_seconds": [0-9.]*' $OUT/$label/$fam/metrics.json 2>/dev/null)"
}
refarm() { # label family cpus policy workers  (4a17f9c7 reference)
  local label=$1 fam=$2 cpus=$3 policy=$4 workers=$5
  log "start $label/$fam cpus=$cpus policy=$policy W$workers bin=4a17f9c7"
  $PY $T/run_arm.py --binary "$REF" --family "$fam" --label "$label" --cpus "$cpus" --policy "$policy" \
      --workers "$workers" --time-limit 3300 --grace 240 > "$OUT/.$label-$fam.runlog" 2>&1
  log "end $label/$fam $(grep -o '"exit_code": [0-9-]*' $OUT/$label/$fam/metrics.json 2>/dev/null)"
}
metric() { grep -o "\"$2\": \"*[0-9-]*" $1 2>/dev/null | head -1 | grep -o '[0-9-]*$'; }
strict() { # reference-dir new-dir tag [compare args]
  local ref=$1 new=$2 tag=$3; shift 3
  $PY $WT/examples/python/compare_walk_records.py --mode strict "$@" $ref/result.json $new/result.json > $LOG/strict-$tag.json 2>&1
  local rc=$?
  local verdict=$(grep -o '"verdict": "[A-Z]*"' $LOG/strict-$tag.json | head -1 | grep -o '[A-Z]*"$' | tr -d '"')
  local cr=$(metric $ref/metrics.json containment_checks) cn=$(metric $new/metrics.json containment_checks)
  local gate=FAIL; [ "$verdict" = PASS ] && [ -n "$cr" ] && [ "$cr" = "$cn" ] && gate=PASS
  log "STRICT $tag $gate verdict=$verdict rc=$rc checks ref=$cr new=$cn $(grep -o '"records": [0-9]*' $LOG/strict-$tag.json | head -1)"
  printf '%s\t%s\t%s\t%s\t%s\n' "$tag" "$gate" "$verdict" "$cr" "$cn" >> $LOG/strict-$SHA.tsv
}
oracle() { # run-dir tag cpus threads   (NO_ORACLE=1: skip; run oracle_parallel.sh later)
  local dir=$1 tag=$2 cpus=$3 threads=$4
  if [ -n "${NO_ORACLE:-}" ]; then log "ORACLE $tag deferred (NO_ORACLE)"; return; fi
  local start=$(date +%s)
  env RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 \
    BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1 taskset -c $cpus nice -n 5 $BIN walk-verify-closure \
    --command "$dir/command.json" --threads $threads --require-closure --reinspect all \
    --output $dir/verify.json --force > $dir/verify.stdout 2> $dir/verify.stderr
  local vx=$?
  $PY $WT/examples/python/assert_oracle_pass.py $dir/verify.json > $dir/gate.txt 2>&1
  local gx=$?
  taskset -c $cpus $PY $WT/examples/python/audit_owner_domain_walk.py "$dir" --command "$dir/command.json" \
    --require-closure --verify-report $dir/verify.json --output $dir/audit.json > $dir/audit.stdout 2> $dir/audit.stderr
  local ax=$?
  local av=$(grep -o '"audit": "[A-Z]*"' $dir/audit.json 2>/dev/null | head -1 | grep -o '[A-Z]*"$' | tr -d '"')
  local paired=$(grep -o '"paired": [a-z]*' $dir/audit.json 2>/dev/null | head -1 | awk '{print $2}')
  local roots=$(grep -h 'roots independently verified' $dir/gate.txt | grep -o '[0-9]*/[0-9]*')
  local gate=FAIL; [ $vx = 0 ] && [ $gx = 0 ] && [ $ax = 0 ] && [ "$av" = PASS ] && [ "$paired" = true ] && gate=PASS
  log "ORACLE $tag $gate verify_exit=$vx gate_exit=$gx audit_exit=$ax audit=$av paired=$paired roots=$roots wall=$(( $(date +%s) - start ))s"
  printf '%s\t%s\t%s\n' "$tag" "$gate" "$roots" >> $LOG/oracle-$SHA.tsv
}
stats() { # run-dir cpus
  taskset -c $2 nice -n 10 $PY $T/g2prod_stats.py $1 > /dev/null 2> $1/g2stats.err
  taskset -c $2 nice -n 10 $PY $WT/tools/research/w0_g2falsify/pending.py $1 > /dev/null 2> $1/pending.err
}
p5() { # binary label cpus g2
  local bin=$1 label=$2 cpus=$3 g2=$4 extra=()
  [ "$g2" != off ] && extra=(--extra --g2-residual-anchors $g2)
  log "start $label/four-all-p5 bin=$(basename $bin) cpus=$cpus g2=$g2"
  $PY $R/examples/input/four_loop_combined/tools/run_c4l.py --binary $bin \
    --command $R/TMP/c4l-s2/commands/command-four-all-p5.json --family four-all-p5 --label $label --cpus $cpus \
    --policy ordered --workers 6 --timeout-seconds 900 --stop-natives 60000 "${extra[@]}" > $OUT/.$label-four-all-p5.runlog 2>&1
  log "end $label/four-all-p5 rc=$?"
}
log "session BIN=$BIN sha256=$(sha256sum $BIN | cut -c1-64) head=$(git -C $WT rev-parse HEAD) phases=$*"
for phase in "$@"; do
  case $phase in
    identity)
      for fam in fg bmw h x four-all; do
        refarm id-ref-$SHA $fam $B6 ordered 6 &
        arm id-off-$SHA $fam $A6 ordered 6 off &
        wait
        strict $OUT/id-ref-$SHA/$fam $OUT/id-off-$SHA/$fam $fam-vs-fresh-4a17f9c7
        strict $REFRUNS/$fam $OUT/id-off-$SHA/$fam $fam-vs-int-ref-g3
      done
      arm id-off-$SHA five-finite $A ordered 16 off
      strict $REFRUNS/five-finite $OUT/id-off-$SHA/five-finite five-finite-W16-vs-int-ref-g3
      ;;
    identity-lite)
      # Flag off vs the 4a17f9c7 reference outputs of TMP/fable51-controls/int-ref-g3 (no fresh reference run).
      for fam in fg bmw h x four-all; do
        arm id-off-$SHA $fam $A6 ordered 6 off
        strict $REFRUNS/$fam $OUT/id-off-$SHA/$fam $fam-vs-int-ref-g3
      done
      arm id-off-$SHA five-finite $A ordered 16 off
      strict $REFRUNS/five-finite $OUT/id-off-$SHA/five-finite five-finite-W16-vs-int-ref-g3
      ;;
    union-smoke)
      # One flag-on C-5F Ready W24 run with the oracle (the named control on this binary).
      arm c5f-rdy-union-r1-$SHA five-finite $ALL ready 24 union
      oracle $OUT/c5f-rdy-union-r1-$SHA/five-finite c5f-rdy-union-r1 $ALL 32
      ;;
    identity-p5)
      p5 $REF id-ref-p5-$SHA $B6 off &
      p5 $BIN id-off-p5-$SHA $A6 off &
      wait
      strict $R/TMP/fable51-controls/id-ref-p5-$SHA/four-all-p5 $R/TMP/fable51-controls/id-off-p5-$SHA/four-all-p5 four-all-p5-vs-fresh-4a17f9c7
      ;;
    c4l)
      for fam in fg bmw h x four-all; do
        arm c4l-union-r1-$SHA $fam $A6 ordered 6 union &
        arm c4l-union-r2-$SHA $fam $B6 ordered 6 union &
        wait
        strict $OUT/c4l-union-r1-$SHA/$fam $OUT/c4l-union-r2-$SHA/$fam $fam-union-r1-vs-r2
      done
      ;;
    post-c4l)
      for fam in fg bmw h x four-all; do
        oracle $OUT/c4l-union-r1-$SHA/$fam c4l-union-$fam $ALL 32
        stats $OUT/c4l-union-r1-$SHA/$fam $ALL
        stats $OUT/id-off-$SHA/$fam $ALL
      done
      ;;
    matrix)
      # Oracle mutation matrix: drained FG (flag off), the FG frontier fixture, and the G2' FG run.
      mkdir -p $R/TMP/w1/g2prod/matrix
      log "matrix start"
      taskset -c $ALL $PY $WT/examples/python/oracle_mutation_matrix.py \
          --run $OUT/id-off-$SHA/fg --frontier-run $R/TMP/w0/oracle/runs/frontier-fixture/fg \
          --frontier-expect-closed 60/124 --g2-run $OUT/c4l-union-r1-$SHA/fg --rustred $BIN --threads 4 --jobs 8 \
          --output $R/TMP/w1/g2prod/matrix/matrix-$SHA.json > $LOG/matrix-$SHA.log 2>&1
      log "matrix rc=$? $(grep -c '^ok ' $LOG/matrix-$SHA.log) ok, $(grep -c '^BAD' $LOG/matrix-$SHA.log) bad"
      ;;
    c4l-widths)
      # Ordered G2' records identical across widths (four-all W6 vs W24).
      arm c4l-union-w24-$SHA four-all $ALL ordered 24 union
      strict $OUT/c4l-union-r1-$SHA/four-all $OUT/c4l-union-w24-$SHA/four-all four-all-union-W6-vs-W24
      ;;
    main)
      for spec in ord:off:r1 ord:union:r1 rdy:off:r1 rdy:union:r1 rdy:union:r2 rdy:off:r2 ord:union:r2 ord:off:r2; do
        IFS=: read pol g2 rep <<< "$spec"
        policy=ordered; [ $pol = rdy ] && policy=ready
        arm c5f-$pol-$g2-$rep-$SHA five-finite $ALL $policy 24 $g2 --perf
      done
      arm hotsub-off-r1-$SHA hot $A ready 12 off --queries $HOTQ --perf &
      arm hotsub-union-r1-$SHA hot $B ready 12 union --queries $HOTQ --perf &
      wait
      arm hotsub-union-r2-$SHA hot $A ready 12 union --queries $HOTQ --perf &
      arm hotsub-off-r2-$SHA hot $B ready 12 off --queries $HOTQ --perf &
      wait
      ;;
    post-main)
      for label in c5f-ord-off-r1 c5f-ord-union-r1 c5f-rdy-off-r1 c5f-rdy-union-r1 c5f-rdy-union-r2 c5f-rdy-off-r2 \
                   c5f-ord-union-r2 c5f-ord-off-r2; do
        stats $OUT/$label-$SHA/five-finite $ALL
      done
      for label in hotsub-off-r1 hotsub-union-r1 hotsub-union-r2 hotsub-off-r2; do
        stats $OUT/$label-$SHA/hot $ALL
      done
      for label in c5f-ord-union-r1 c5f-rdy-union-r1 c5f-ord-union-r2 c5f-rdy-union-r2 c5f-ord-off-r1 c5f-rdy-off-r1; do
        oracle $OUT/$label-$SHA/five-finite $label $ALL 32
      done
      for label in hotsub-union-r1 hotsub-union-r2 hotsub-off-r1; do
        oracle $OUT/$label-$SHA/hot $label $ALL 32
      done
      strict $OUT/c5f-ord-union-r1-$SHA/five-finite $OUT/c5f-ord-union-r2-$SHA/five-finite c5f-ord-union-r1-vs-r2
      ;;
    drills)
      log "drill ready-multi-prefix start"
      $PY $WT/examples/python/ready_resume_control.py --command $OUT/c5f-rdy-union-r1-${REFSHA:-$SHA}/five-finite/command.json \
          --binary $BIN --output $OUT/drill-ready-mp-$SHA --cpus $ALL > $LOG/drill-ready-mp-$SHA.log 2>&1
      log "drill ready-multi-prefix rc=$? $(grep -o '"verdict": "[A-Z]*"' $OUT/drill-ready-mp-$SHA/report.json 2>/dev/null | head -1)"
      oracle $OUT/drill-ready-mp-$SHA/resumed drill-ready-mp-resumed $ALL 32
      for pol in ord rdy; do
        policy=ordered; [ $pol = rdy ] && policy=ready
        log "drill 3-pause $pol start"
        $PY $T/pause_drill.py --command $OUT/c5f-$pol-union-r1-${REFSHA:-$SHA}/five-finite/command.json --binary $BIN \
            --out $OUT/drill-$pol-3p-$SHA --cpus $ALL --stops 250000,500000,750000 > $LOG/drill-$pol-3p-$SHA.log 2>&1
        log "drill 3-pause $pol rc=$? $(tail -1 $LOG/drill-$pol-3p-$SHA.log)"
        oracle $OUT/drill-$pol-3p-$SHA/phase-3 drill-$pol-3p-final $ALL 32
      done
      strict $OUT/c5f-ord-union-r1-${REFSHA:-$SHA}/five-finite $OUT/drill-ord-3p-$SHA/phase-3 c5f-ord-union-uninterrupted-vs-3-pauses \
          --ignore-top g2_residual_anchors --ignore-top uncommitted_inspections
      ;;
    activation)
      # G2' activation on a checkpoint written without G2' (C-5F Ready W24): flag off until 400k committed
      # domains, then resume with --g2-residual-anchors union --g2-activate-on-resume to exhaustion.
      log "drill activation start"
      $PY $T/pause_drill.py --command $OUT/c5f-rdy-off-r1-${REFSHA:-$SHA}/five-finite/command.json --binary $BIN \
          --out $OUT/drill-rdy-activation-$SHA --cpus $ALL --stops 400000 \
          --resume-extra=--g2-residual-anchors --resume-extra=union --resume-extra=--g2-activate-on-resume \
          > $LOG/drill-rdy-activation-$SHA.log 2>&1
      log "drill activation rc=$? $(tail -1 $LOG/drill-rdy-activation-$SHA.log)"
      oracle $OUT/drill-rdy-activation-$SHA/phase-1 drill-rdy-activation-final $ALL 32
      ;;
    *) log "unknown phase $phase" ;;
  esac
done
log "session done"
