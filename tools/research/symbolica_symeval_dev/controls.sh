#!/usr/bin/env bash
# controls.sh <label> <new-binary> : strict Ordered identity of <new-binary> vs rustred-4a17f9c7
#  C-4L FG/BMW/H/X (W6, ref and new interleaved per family, concurrent families) on the lane CPUs,
#  then C-5F (W50, new only; compared against the W0 audit's 4a17f9c7 ref-r1/ref-r2 of the same command).
set -uo pipefail
label=$1; NEW=$2; MODE=${3:-all}
D=/common/dev/rustred/TMP/symbolica-main/symeval-dev
WT=/common/dev/rustred/.claude/worktrees/fable51-symeval
REF=/common/dev/rustred/TMP/fable51-controls/bin/rustred-4a17f9c7
PY="nix develop /common/dev/rustred --command python"
C=$WT/examples/python/compare_walk_records.py
OUT=$D/runs/controls-$label
mkdir -p $OUT
cd /common/dev/rustred
export TMPDIR=/common/dev/rustred/TMP
if [ "$MODE" != c5f ]; then
echo "== c4l start $(date -u +%FT%TZ)"
$PY $WT/tools/research/symbolica_lane/ab_controls.py --out-root $OUT/c4l --bin ref=$REF --bin $label=$NEW \
  --family fg:40-45 --family bmw:46-51 --family h:52-55,296-297 --family x:298-303 --repeats 1 --policy ordered --workers 6 > $OUT/c4l.log 2>&1
echo "== c4l rc=$? $(date -u +%FT%TZ)"
for fam in fg bmw h x; do
  a=$OUT/c4l/$fam/ref-r1/result.json; b=$OUT/c4l/$fam/$label-r1/result.json
  nice -n 5 taskset -c 40-55,296-311 $PY $C --mode strict $a $b --output $OUT/c4l/$fam/strict-ref-vs-$label.json > /dev/null 2> $OUT/c4l/$fam/strict.stderr
  echo "c4l $fam ref-r1 vs $label-r1: exit $? $(grep -o '"differing_records": [0-9]*' $OUT/c4l/$fam/strict-ref-vs-$label.json) $(grep -o '"identical": [a-z]*' $OUT/c4l/$fam/strict-ref-vs-$label.json | head -1)"
done
fi
if [ "$MODE" != c4l ]; then
echo "== c5f start $(date -u +%FT%TZ)"
$PY $WT/tools/research/symbolica_lane/ab_controls.py --out-root $OUT/c5f --bin $label=$NEW \
  --family five-finite:40-55,296-311 --repeats 1 --policy ordered --workers ${C5F_WORKERS:-32} > $OUT/c5f.log 2>&1
echo "== c5f rc=$? $(date -u +%FT%TZ)"
for r in ref-r1 ref-r2; do
  a=/common/dev/rustred/TMP/w0/symbolica/runs/c5f/five-finite/$r/result.json; b=$OUT/c5f/five-finite/$label-r1/result.json
  nice -n 5 taskset -c 40-55,296-311 $PY $C --mode strict $a $b --output $OUT/c5f/strict-w0$r-vs-$label.json > /dev/null 2> $OUT/c5f/strict-$r.stderr
  echo "c5f w0 $r vs $label-r1: exit $? $(grep -o '"differing_records": [0-9]*' $OUT/c5f/strict-w0$r-vs-$label.json) $(grep -o '"identical": [a-z]*' $OUT/c5f/strict-w0$r-vs-$label.json | head -1)"
done
fi
echo "== done $(date -u +%FT%TZ)"
