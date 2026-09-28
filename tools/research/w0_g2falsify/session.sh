#!/usr/bin/env bash
# W0 G2' falsifier session (throwaway). Flag off vs on, same binary, same CPUs,
# same session, interleaved repeats. usage: session.sh BIN PHASE...
#   phases: smoke c4l c5f hotsub post-c4l post-c5f post-hotsub   (binary B1: arms off/on)
#           c4l3 post-c4l3 hotsub3 post-hotsub3 c5f3 post-c5f3     (binary B2: arms off/m1/m2)
#           c4lx post-c4lx hotsubx post-hotsubx c5fx post-c5fx     (binary B3: arms off/m1/m2/u)
#           c4ln post-c4ln hotsubn post-hotsubn c5fn post-c5fn     (binary B4: arms off/u/n, perf stat, n=3)
# Arm flags: off (unset), on|m1 (RUSTRED_WALK_G2_DONLY=1), m2 (=2), u (=u, union),
#            n (=n, union with full-native anchors only).
# Never edit this file while a session runs it: run a snapshot copy.
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
VERIFY=$T/target/release/g2verify
SHA=$(basename "$BIN" | sed 's/rustred-//')
log() { echo "$(date -u +%FT%TZ) $*"; }

arm() { # label family cpus policy workers on|off [extra run_arm args]
  local label=$1 fam=$2 cpus=$3 policy=$4 workers=$5 flag=$6; shift 6
  local env=()
  case $flag in on|m1) env=(--env RUSTRED_WALK_G2_DONLY=1) ;; m2) env=(--env RUSTRED_WALK_G2_DONLY=2) ;;
    u) env=(--env RUSTRED_WALK_G2_DONLY=u) ;; n) env=(--env RUSTRED_WALK_G2_DONLY=n) ;; esac
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
  [ "$flag" != off ] && extra=(--g2-residual-anchors)
  taskset -c "$cpus" nice -n 19 $PY $WT/examples/python/audit_owner_domain_walk.py "$d" --require-closure \
      "${extra[@]}" --no-output > "$d/audit.json" 2> "$d/audit.err"
  log "audit $label/$fam: $(grep -o '"audit": "[A-Z]*"' $d/audit.json | head -1)"
  taskset -c "$cpus" nice -n 19 $STATS "$d/result.json" $union > "$d/g2stats.json" 2> "$d/g2stats.err"
  if [ "$flag" != off ] && [ -x $VERIFY ]; then
    taskset -c "$cpus" nice -n 19 $VERIFY "$d/result.json" > "$d/g2verify.json" 2> "$d/g2verify.err"
    log "g2verify $label/$fam: $(grep -o '"verdict": "[A-Z]*"' $d/g2verify.json | head -1)"
  fi
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
    c4l3)
      for fam in fg bmw h x; do
        arm c4l-off-r1-$SHA $fam $A ordered 6 off & arm c4l-m2-r1-$SHA $fam $B ordered 6 m2 & wait
      done
      for fam in fg bmw h x; do
        arm c4l-m2-r2-$SHA $fam $A ordered 6 m2 & arm c4l-off-r2-$SHA $fam $B ordered 6 off & wait
      done
      for fam in fg bmw h x; do
        arm c4l-m1-r1-$SHA $fam $A ordered 6 m1 & arm c4l-m1-r2-$SHA $fam $B ordered 6 m1 & wait
      done
      ;;
    post-c4l3)
      for fam in fg bmw h x; do
        post c4l-off-r1-$SHA $fam off 16-17,272-273 & post c4l-m2-r1-$SHA $fam m2 18-19,274-275 &
        post c4l-off-r2-$SHA $fam off 20-21,276-277 & post c4l-m2-r2-$SHA $fam m2 22-23,278-279 &
        post c4l-m1-r1-$SHA $fam m1 24-25,280-281 & post c4l-m1-r2-$SHA $fam m1 26-27,282-283 &
        wait
        for r in r1 r2; do
          strict $ROOT/TMP/w0/oracle/runs/c4l-ordered/$fam/result.json $OUT/c4l-off-$r-$SHA/$fam/result.json \
                 $OUT/c4l-off-$r-$SHA/$fam/strict-vs-4a17f9c7.txt
        done
      done
      ;;
    hotsub3)
      arm hotsub-r1a12-off-r1-$SHA hot $A ready 12 off --queries $HOTQ &
      arm hotsub-r1a12-m1-r1-$SHA hot $B ready 12 m1 --queries $HOTQ &
      wait
      arm hotsub-r1a12-m2-r1-$SHA hot $A ready 12 m2 --queries $HOTQ &
      arm hotsub-r1a12-off-r2-$SHA hot $B ready 12 off --queries $HOTQ &
      wait
      arm hotsub-r1a12-m1-r2-$SHA hot $A ready 12 m1 --queries $HOTQ &
      arm hotsub-r1a12-m2-r2-$SHA hot $B ready 12 m2 --queries $HOTQ &
      wait
      ;;
    post-hotsub3)
      i=0
      for l in off-r1 m1-r1 m2-r1 off-r2 m1-r2 m2-r2; do
        flag=${l%-r*}; c=$((16 + 2 * i))
        post hotsub-r1a12-$l-$SHA hot $flag $c,$((c + 1)),$((c + 256)),$((c + 257)) --union &
        i=$((i + 1))
      done
      wait
      ;;
    c5f3)
      for l in ord-off-r1 ord-m1-r1 ord-m2-r1 rdy-off-r1 rdy-m1-r1 rdy-m2-r1 \
               ord-m2-r2 ord-m1-r2 ord-off-r2 rdy-m2-r2 rdy-m1-r2 rdy-off-r2; do
        pol=${l%%-*}; rest=${l#*-}; flag=${rest%-r*}
        policy=ordered; [ "$pol" = rdy ] && policy=ready
        arm c5f-$l-$SHA five-finite $ALL $policy 24 $flag
      done
      ;;
    post-c5f3)
      i=0
      for l in ord-off-r1 ord-m1-r1 ord-m2-r1 rdy-off-r1 rdy-m1-r1 rdy-m2-r1 \
               ord-m2-r2 ord-m1-r2 ord-off-r2 rdy-m2-r2 rdy-m1-r2 rdy-off-r2; do
        rest=${l#*-}; flag=${rest%-r*}; c=$((16 + i))
        post c5f-$l-$SHA five-finite $flag $c,$((c + 256)) --union &
        i=$((i + 1))
      done
      wait
      for r in r1 r2; do
        strict $ROOT/TMP/w0/oracle/runs/c5f-ordered/five-finite/result.json $OUT/c5f-ord-off-$r-$SHA/five-finite/result.json \
               $OUT/c5f-ord-off-$r-$SHA/five-finite/strict-vs-4a17f9c7.txt
      done
      ;;
    c4lx)
      for fam in fg bmw h x; do
        arm c4l-off-r1-$SHA $fam $A ordered 6 off & arm c4l-u-r1-$SHA $fam $B ordered 6 u & wait
      done
      for fam in fg bmw h x; do
        arm c4l-u-r2-$SHA $fam $A ordered 6 u & arm c4l-off-r2-$SHA $fam $B ordered 6 off & wait
      done
      for fam in fg bmw h x; do
        arm c4l-m2-r1-$SHA $fam $A ordered 6 m2 & arm c4l-m2-r2-$SHA $fam $B ordered 6 m2 & wait
      done
      ;;
    post-c4lx)
      for fam in fg bmw h x; do
        post c4l-off-r1-$SHA $fam off 16-17,272-273 & post c4l-u-r1-$SHA $fam u 18-19,274-275 &
        post c4l-off-r2-$SHA $fam off 20-21,276-277 & post c4l-u-r2-$SHA $fam u 22-23,278-279 &
        post c4l-m2-r1-$SHA $fam m2 24-25,280-281 & post c4l-m2-r2-$SHA $fam m2 26-27,282-283 &
        wait
        for r in r1 r2; do
          strict $ROOT/TMP/w0/oracle/runs/c4l-ordered/$fam/result.json $OUT/c4l-off-$r-$SHA/$fam/result.json \
                 $OUT/c4l-off-$r-$SHA/$fam/strict-vs-4a17f9c7.txt
        done
      done
      ;;
    hotsubx)
      for pair in off-r1:u-r1 m2-r1:off-r2 u-r2:m1-r1 m1-r2:m2-r2; do
        la=${pair%%:*}; lb=${pair##*:}
        arm hotsub-r1a12-$la-$SHA hot $A ready 12 ${la%-r*} --queries $HOTQ &
        arm hotsub-r1a12-$lb-$SHA hot $B ready 12 ${lb%-r*} --queries $HOTQ &
        wait
      done
      ;;
    post-hotsubx)
      i=0
      for l in off-r1 u-r1 m2-r1 off-r2 u-r2 m1-r1 m1-r2 m2-r2; do
        flag=${l%-r*}; c=$((16 + 2 * i))
        post hotsub-r1a12-$l-$SHA hot $flag $c,$((c + 1)),$((c + 256)),$((c + 257)) --union &
        i=$((i + 1))
      done
      wait
      ;;
    c5fx)
      for l in ord-off-r1 ord-u-r1 ord-m2-r1 rdy-off-r1 rdy-u-r1 rdy-m2-r1 \
               ord-m2-r2 ord-u-r2 ord-off-r2 rdy-m2-r2 rdy-u-r2 rdy-off-r2; do
        pol=${l%%-*}; rest=${l#*-}; flag=${rest%-r*}
        policy=ordered; [ "$pol" = rdy ] && policy=ready
        arm c5f-$l-$SHA five-finite $ALL $policy 24 $flag
      done
      ;;
    post-c5fx)
      i=0
      for l in ord-off-r1 ord-u-r1 ord-m2-r1 rdy-off-r1 rdy-u-r1 rdy-m2-r1 \
               ord-m2-r2 ord-u-r2 ord-off-r2 rdy-m2-r2 rdy-u-r2 rdy-off-r2; do
        rest=${l#*-}; flag=${rest%-r*}; c=$((16 + i))
        post c5f-$l-$SHA five-finite $flag $c,$((c + 256)) --union &
        i=$((i + 1))
      done
      wait
      for r in r1 r2; do
        strict $ROOT/TMP/w0/oracle/runs/c5f-ordered/five-finite/result.json $OUT/c5f-ord-off-$r-$SHA/five-finite/result.json \
               $OUT/c5f-ord-off-$r-$SHA/five-finite/strict-vs-4a17f9c7.txt
      done
      ;;
    c4ln)
      for fam in fg bmw h x; do
        arm c4l-off-r1-$SHA $fam $A ordered 6 off --perf & arm c4l-n-r1-$SHA $fam $B ordered 6 n --perf & wait
      done
      for fam in fg bmw h x; do
        arm c4l-n-r2-$SHA $fam $A ordered 6 n --perf & arm c4l-off-r2-$SHA $fam $B ordered 6 off --perf & wait
      done
      ;;
    post-c4ln)
      for fam in fg bmw h x; do
        post c4l-off-r1-$SHA $fam off 16-19,272-275 & post c4l-n-r1-$SHA $fam n 20-23,276-279 &
        post c4l-off-r2-$SHA $fam off 24-27,280-283 & post c4l-n-r2-$SHA $fam n 28-31,284-287 &
        wait
        for r in r1 r2; do
          strict $ROOT/TMP/w0/oracle/runs/c4l-ordered/$fam/result.json $OUT/c4l-off-$r-$SHA/$fam/result.json \
                 $OUT/c4l-off-$r-$SHA/$fam/strict-vs-4a17f9c7.txt
        done
      done
      ;;
    hotsubn)
      # Halves A/B are separate CCDs; each arm runs on both halves.
      for pair in off-r1:u-r1 n-r1:off-r2 u-r2:n-r2 off-r3:n-r3 u-r3:off-r4; do
        la=${pair%%:*}; lb=${pair##*:}
        arm hotsub-r1a12-$la-$SHA hot $A ready 12 ${la%-r*} --queries $HOTQ --perf &
        arm hotsub-r1a12-$lb-$SHA hot $B ready 12 ${lb%-r*} --queries $HOTQ --perf &
        wait
      done
      ;;
    post-hotsubn)
      i=0
      for l in off-r1 u-r1 n-r1 off-r2 u-r2 n-r2 off-r3 n-r3 u-r3 off-r4; do
        flag=${l%-r*}; c=$((16 + i))
        post hotsub-r1a12-$l-$SHA hot $flag $c,$((c + 256)) --union &
        i=$((i + 1))
      done
      wait
      ;;
    c5fn)
      for l in ord-off-r1 ord-u-r1 ord-n-r1 rdy-off-r1 rdy-u-r1 rdy-n-r1 \
               ord-n-r2 ord-off-r2 ord-u-r2 rdy-n-r2 rdy-off-r2 rdy-u-r2 \
               ord-u-r3 ord-n-r3 ord-off-r3 rdy-u-r3 rdy-n-r3 rdy-off-r3; do
        pol=${l%%-*}; rest=${l#*-}; flag=${rest%-r*}
        policy=ordered; [ "$pol" = rdy ] && policy=ready
        arm c5f-$l-$SHA five-finite $ALL $policy 24 $flag --perf
      done
      ;;
    post-c5fn)
      i=0
      for l in ord-off-r1 ord-u-r1 ord-n-r1 rdy-off-r1 rdy-u-r1 rdy-n-r1 \
               ord-n-r2 ord-off-r2 ord-u-r2 rdy-n-r2 rdy-off-r2 rdy-u-r2 \
               ord-u-r3 ord-n-r3 ord-off-r3 rdy-u-r3 rdy-n-r3 rdy-off-r3; do
        rest=${l#*-}; flag=${rest%-r*}; c=$((16 + i % 16)); c2=$((c + 256))
        post c5f-$l-$SHA five-finite $flag $c,$c2 --union &
        i=$((i + 1))
        [ $((i % 16)) -eq 0 ] && wait
      done
      wait
      for r in r1 r2 r3; do
        strict $ROOT/TMP/w0/oracle/runs/c5f-ordered/five-finite/result.json $OUT/c5f-ord-off-$r-$SHA/five-finite/result.json \
               $OUT/c5f-ord-off-$r-$SHA/five-finite/strict-vs-4a17f9c7.txt
      done
      ;;
    *) log "unknown phase $phase" ;;
  esac
done
log "session done: $*"
