#!/usr/bin/env bash
# W0.11 D1(b) falsifier batch: run every (family, variant) of TMP/w0/wv/pieces/queries
# at W6 Ordered (the historical command lines) on four disjoint lane CPU slots, then
# audit each run with both W0.2 oracles (Python audit, walk-verify-closure with full
# F10 re-inspection) and the piece-cone analysis. Nothing touches campaigns/.
# Usage: [JOBS="fam:variant ..."] [POLICY=ordered] [WORKERS=6] pieces_batch.sh LABEL [TIMEOUT_S]
# (four-all's historical command is Ready W24: pass POLICY=ordered WORKERS=6 for W6 Ordered)
set -u
LABEL=$1; TIMEOUT=${2:-900}
WT=/common/dev/rustred/.claude/worktrees/fable51-wv
T=$WT/tools/research/wv
Q=/common/dev/rustred/TMP/w0/wv/pieces/queries
R=/common/dev/rustred/TMP/w0/wv/pieces/runs/$LABEL
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
AUDIT=/common/dev/rustred/.claude/worktrees/fable51-oracle/examples/python/audit_owner_domain_walk.py
VERIFY=/common/dev/rustred/TMP/w0/oracle/bin/rustred-f4d1870f
mkdir -p $R
LOG=$R/batch.log
SLOTS=("72-75,328-331" "76-79,332-335" "80-83,336-339" "84-87,340-343")
JOBS=${JOBS:-"fg:base fg:unbounded fg:shells fg:aslabs fg:r12aslabs bmw:base bmw:unbounded bmw:shells bmw:shellsk bmw:aslabs bmw:r12aslabs h:base h:unbounded h:shells h:aslabs h:r12aslabs x:base x:unbounded x:shells x:aslabs x:r12aslabs fg:rank14 fg:rank16 bmw:rank14k bmw:rank16k h:rank14 h:rank16 x:rank14 x:rank16 fg:rank20 bmw:rank20k h:rank20 x:rank20"}
one() {  # fam variant cpus
  local fam=$1 v=$2 cpus=$3 out=$R/$1-$2
  echo "$(date -u +%FT%TZ) start $fam-$v cpus=$cpus" >> $LOG
  $PY $T/run_variant.py --family $fam --queries $Q/$fam-$v.json --out $out --cpus $cpus \
      --timeout-seconds $TIMEOUT ${POLICY:+--policy $POLICY} ${WORKERS:+--workers $WORKERS} \
      > $out.run.stdout 2> $out.run.stderr
  echo "$(date -u +%FT%TZ) run $fam-$v exit=$?" >> $LOG
  local req=()
  grep -q '"frontiers": 0,' $out/metrics.json && grep -q '"recursive_worklist_exhausted": true' $out/metrics.json && req=(--require-closure)
  taskset -c $cpus nice -n 5 nix develop /common/dev/rustred --command python $AUDIT $out --command $out/command.json \
      "${req[@]}" --output $out.audit.json > $out.audit.stdout 2> $out.audit.stderr
  echo "$(date -u +%FT%TZ) audit $fam-$v exit=$? ${req[*]}" >> $LOG
  env RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1 taskset -c $cpus nice -n 5 nix develop /common/dev/rustred --command \
      $VERIFY walk-verify-closure --command $out/command.json --threads 8 --output $out.verify.json --force "${req[@]}" \
      > $out.verify.stdout 2> $out.verify.stderr
  echo "$(date -u +%FT%TZ) verify $fam-$v exit=$? ${req[*]}" >> $LOG
  $PY $T/piece_cones.py $out > $out.cones.json 2> $out.cones.stderr
  echo "$(date -u +%FT%TZ) cones $fam-$v exit=$?" >> $LOG
}
slot() {  # index
  local i=$1 k=0
  for job in $JOBS; do
    if [ $((k % ${#SLOTS[@]})) -eq $i ]; then
      one ${job%%:*} ${job##*:} ${SLOTS[$i]}
    fi
    k=$((k + 1))
  done
}
echo "$(date -u +%FT%TZ) batch $LABEL begin timeout=$TIMEOUT" >> $LOG
for i in "${!SLOTS[@]}"; do slot $i & done
wait
echo "$(date -u +%FT%TZ) batch $LABEL end" >> $LOG
