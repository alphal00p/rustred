#!/usr/bin/env bash
# epoch-s2 fix round: the S2 gates on one binary, within CPUs 0-15,256-271 (orchestrator rule 19:
# no socket-1 runs; W50/W96 need more CPUs than the lane owns and the binary refuses W > the mask).
# usage: fixround_gates.sh BIN TAG
#   track A (0-7,256-263): W6 six controls + oracle (16 threads), then W12 six controls, then C-5F W12
#   track B (8-15,264-271): gate 4 Ordered identity (FG/BMW/H/X W6, C-5F W16, four-all, four-all-p5),
#                           CP5 resume across binaries (FG, both directions), FG B=8 identity W6/W12
#   then (all 32): W24 six controls, C-5F W24 + oracle (32 threads); identity receipts and gate 3 tables.
set -u
BIN=${1:?}; TAG=${2:?}
R=/common/dev/rustred; S=$R/TMP/epoch-s2; RUNS=$S/runs
P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
LOG=$RUNS/fixround-$TAG.log
cd $R
say() { echo "$(date -u +%FT%TZ) $*" >> $LOG; }
say "start BIN=$BIN sha256=$(sha256sum $BIN | cut -c1-64)"
four="fg bmw h x four-all four-all-p5"
trackA() {
  $S/controls.sh $BIN $TAG-w6 6 2-7 "$four" oracle16
  say "trackA W6+oracle done"
  $S/controls.sh $BIN $TAG-w12 12 0-7,256-259 "$four"
  say "trackA W12 done"
  $S/controls.sh $BIN $TAG-c5f-w12 12 0-7,256-259 five-finite
  say "trackA C-5F W12 done"
}
trackB() {
  $S/ordered_identity.sh $BIN gate4-$TAG
  say "trackB gate4 done"
  $S/cp5_resume.sh $BIN $TAG
  say "trackB CP5 resume done"
  for W in 6 12; do
    RUSTRED_EPOCH_LOCKSTEP_B=8 $S/controls.sh $BIN $TAG-b8-w$W $W 8-13,264-269 fg
  done
  say "trackB B=8 FG W6/W12 done"
}
trackA & a=$!
trackB & b=$!
wait $a $b
$S/controls.sh $BIN $TAG-w24 24 0-15,256-271 "$four"
say "W24 done"
$S/controls.sh $BIN $TAG-c5f-w24 24 0-15,256-271 five-finite oracle32
say "C-5F W24 + oracle done"
for fam in $four; do
  for W in 12 24; do
    $P $S/identity.py $RUNS/$TAG-w6/$fam $RUNS/$TAG-w$W/$fam --out $RUNS/identity-$TAG-$fam-w6-w$W.json > /dev/null 2>&1
    say "IDENTITY $fam W6 vs W$W rc=$? $(grep -o '"identical": [a-z]*' $RUNS/identity-$TAG-$fam-w6-w$W.json)"
  done
done
$P $S/identity.py $RUNS/$TAG-c5f-w24/five-finite $RUNS/$TAG-c5f-w12/five-finite --out $RUNS/identity-$TAG-five-finite-w24-w12.json > /dev/null 2>&1
say "IDENTITY five-finite W24 vs W12 rc=$? $(grep -o '"identical": [a-z]*' $RUNS/identity-$TAG-five-finite-w24-w12.json)"
$P $S/identity.py $RUNS/$TAG-b8-w6/fg $RUNS/$TAG-b8-w12/fg --out $RUNS/identity-$TAG-fg-b8-w6-w12.json > /dev/null 2>&1
say "IDENTITY fg B=8 W6 vs W12 rc=$? $(grep -o '"identical": [a-z]*' $RUNS/identity-$TAG-fg-b8-w6-w12.json)"
$P $S/compare.py $TAG-w6 ref-ready-w6 --families fg,bmw,h,x,four-all,four-all-p5 --out $S/gate3-$TAG-w6.json > /dev/null 2>&1
$P $S/compare.py $TAG-c5f-w24 ref-ready-w16 --families five-finite --out $S/gate3-$TAG-c5f.json > /dev/null 2>&1
say "gate3 tables written rc=$?"
say "done"
