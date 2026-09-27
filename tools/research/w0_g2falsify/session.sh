#!/usr/bin/env bash
# W0 G2' falsifier session (throwaway). Flag off vs on, same binary, same CPUs,
# same session, interleaved repeats. usage: session.sh BIN PHASE...
#   phases: smoke c4l c5f hotsub post-c4l post-c5f post-hotsub
# CPU halves are disjoint sets of physical cores (n and n+256 are SMT siblings).
set -u
BIN=$1; shift
ROOT=/common/dev/rustred
WT=$ROOT/.claude/worktrees/fable51-g2f
T=$WT/tools/research/w0_g2falsify
OUT=$ROOT/TMP/w0/g2falsify/runs
A=16-23,272-279
B=24-31,280-287
ALL=16-31,272-287
HOTQ=$ROOT/TMP/w0/falsify/inputs/hotsub-r1a12.json
PY="nix develop $ROOT --command python"
STATS=$T/target/release/g2stats
SHA=$(basename "$BIN" | sed 's/rustred-//')
log() { echo "$(date -u +%FT%TZ) $*"; }

arm() { # label family cpus policy workers on|off [extra run_arm args]
  local label=$1 fam=$2 cpus=$3 policy=$4 workers=$5 flag=$6; shift 6
  local env=()
  [ "$flag" = on ] && env=(--env RUSTRED_WALK_G2_DONLY=1)
  log "start $label/$fam cpus=$cpus policy=$policy W$workers g2=$flag"
  $PY $T/run_arm.py --binary "$BIN" --family "$fam" --label "$label" --cpus "$cpus" \
      --policy "$policy" --workers "$workers" --time-limit 3000 --grace 300 "${env[@]}" "$@" \
      > "$OUT/.$label-$fam.runlog" 2>&1
  log "end $label/$fam exit=$(grep -o '"exit_code": [0-9-]*' $OUT/$label/$fam/metrics.json 2>/dev/null)"
}

post() { # label family on|off : audit + g2stats + pending, pinned to the given cpus
  local label=$1 fam=$2 flag=$3 cpus=$4 union=${5:-}
  local d=$OUT/$label/$fam
  local extra=()
  [ "$flag" = on ] && extra=(--g2-residual-anchors)
  taskset -c "$cpus" nice -n 19 $PY $WT/examples/python/audit_owner_domain_walk.py "$d" --require-closure \
      "${extra[@]}" --no-output > "$d/audit.json" 2> "$d/audit.err"
  log "audit $label/$fam: $(grep -o '"audit": "[A-Z]*"' $d/audit.json | head -1)"
  taskset -c "$cpus" nice -n 19 $STATS "$d/result.json" $union > "$d/g2stats.json" 2> "$d/g2stats.err"
  taskset -c "$cpus" nice -n 19 $PY $T/pending.py "$d" > /dev/null 2> "$d/pending.err"
}

strict() { # ref new outfile
  $PY $WT/examples/python/compare_walk_records.py --mode strict "$1" "$2" > "$3" 2>&1
  log "strict $(basename $(dirname $2)): $(grep -o '"verdict": "[A-Z]*"\|PASS\|FAIL' $3 | head -1)"
}

for phase in "$@"; do
  case $phase in
    smoke)
      arm smoke-off-$SHA fg $A ordered 6 off &
      arm smoke-on-$SHA fg $B ordered 6 on &
      wait
      post smoke-off-$SHA fg off $A & post smoke-on-$SHA fg on $B & wait
      strict $ROOT/TMP/w0/oracle/runs/c4l-ordered/fg/result.json $OUT/smoke-off-$SHA/fg/result.json $OUT/smoke-off-$SHA/fg/strict-vs-4a17f9c7.txt
      ;;
    c4l)
      for fam in fg bmw h x; do
        arm c4l-off-r1-$SHA $fam $A ordered 6 off &
        arm c4l-on-r1-$SHA $fam $B ordered 6 on &
        wait
      done
      for fam in fg bmw h x; do
        arm c4l-on-r2-$SHA $fam $A ordered 6 on &
        arm c4l-off-r2-$SHA $fam $B ordered 6 off &
        wait
      done
      ;;
    post-c4l)
      for fam in fg bmw h x; do
        post c4l-off-r1-$SHA $fam off 16-19,272-275 & post c4l-on-r1-$SHA $fam on 20-23,276-279 &
        post c4l-off-r2-$SHA $fam off 24-27,280-283 & post c4l-on-r2-$SHA $fam on 28-31,284-287 &
        wait
        for r in r1 r2; do
          strict $ROOT/TMP/w0/oracle/runs/c4l-ordered/$fam/result.json $OUT/c4l-off-$r-$SHA/$fam/result.json \
                 $OUT/c4l-off-$r-$SHA/$fam/strict-vs-4a17f9c7.txt
        done
      done
      ;;
    c5f)
      arm c5f-ord-off-r1-$SHA five-finite $ALL ordered 24 off
      arm c5f-ord-on-r1-$SHA five-finite $ALL ordered 24 on
      arm c5f-rdy-off-r1-$SHA five-finite $ALL ready 24 off
      arm c5f-rdy-on-r1-$SHA five-finite $ALL ready 24 on
      arm c5f-ord-on-r2-$SHA five-finite $ALL ordered 24 on
      arm c5f-ord-off-r2-$SHA five-finite $ALL ordered 24 off
      arm c5f-rdy-on-r2-$SHA five-finite $ALL ready 24 on
      arm c5f-rdy-off-r2-$SHA five-finite $ALL ready 24 off
      ;;
    post-c5f)
      i=0
      for l in ord-off-r1 ord-on-r1 rdy-off-r1 rdy-on-r1 ord-on-r2 ord-off-r2 rdy-on-r2 rdy-off-r2; do
        flag=off; [[ $l == *-on-* ]] && flag=on
        c=$((16 + 2 * i)); post c5f-$l-$SHA five-finite $flag $c,$((c + 1)),$((c + 256)),$((c + 257)) --union &
        i=$((i + 1))
      done
      wait
      for r in r1 r2; do
        strict $ROOT/TMP/w0/oracle/runs/c5f-ordered/five-finite/result.json $OUT/c5f-ord-off-$r-$SHA/five-finite/result.json \
               $OUT/c5f-ord-off-$r-$SHA/five-finite/strict-vs-4a17f9c7.txt
      done
      ;;
    hotsub)
      arm hotsub-r1a12-off-r1-$SHA hot $A ready 12 off --queries $HOTQ &
      arm hotsub-r1a12-on-r1-$SHA hot $B ready 12 on --queries $HOTQ &
      wait
      arm hotsub-r1a12-on-r2-$SHA hot $A ready 12 on --queries $HOTQ &
      arm hotsub-r1a12-off-r2-$SHA hot $B ready 12 off --queries $HOTQ &
      wait
      ;;
    post-hotsub)
      post hotsub-r1a12-off-r1-$SHA hot off 16-19,272-275 --union & post hotsub-r1a12-on-r1-$SHA hot on 20-23,276-279 --union &
      post hotsub-r1a12-off-r2-$SHA hot off 24-27,280-283 --union & post hotsub-r1a12-on-r2-$SHA hot on 28-31,284-287 --union &
      wait
      ;;
    *) log "unknown phase $phase" ;;
  esac
done
log "session done: $*"
