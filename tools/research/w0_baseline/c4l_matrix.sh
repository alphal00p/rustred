#!/usr/bin/env bash
# C-4L (four-loop FG/BMW/H/X, saved-cover envelope A<=19 R<=12 D>=7) for the W0.5
# baseline: frame-pointer build vs the canonical 4a17f9c7, W6, sequential,
# interleaved (round 1: fp then ref; round 2: ref then fp), Ordered x2 and Ready x1.
# Uses TMP/fable51-controls/run_control.py unchanged (label is a relative path so
# outputs land under TMP/w0/baseline/c4l/).  Usage: c4l_matrix.sh FP_BIN REF_BIN CPUS
set -u
FP=$1
REF=$2
CPUS=${3:-82-87}
RC=/common/dev/rustred/TMP/fable51-controls/run_control.py
OUT=/common/dev/rustred/TMP/w0/baseline/c4l
mkdir -p "$OUT"
run() {  # tag binary family policy
  local tag=$1 bin=$2 fam=$3 pol=$4
  echo "$(date -u +%FT%TZ) start $tag $fam $pol" >> "$OUT/matrix.log"
  python "$RC" --binary "$bin" --family "$fam" --label "../w0/baseline/c4l/$tag" \
    --cpus "$CPUS" --policy "$pol" --workers 6 > "$OUT/$tag-$fam.log" 2>&1
  echo "$(date -u +%FT%TZ) end $tag $fam $pol exit $?" >> "$OUT/matrix.log"
}
for fam in fg bmw h x; do
  run fp-ord-r1 "$FP" $fam ordered
  run ref-ord-r1 "$REF" $fam ordered
done
for fam in fg bmw h x; do
  run ref-ord-r2 "$REF" $fam ordered
  run fp-ord-r2 "$FP" $fam ordered
done
for fam in fg bmw h x; do
  run fp-rdy "$FP" $fam ready
  run ref-rdy "$REF" $fam ready
done
echo "$(date -u +%FT%TZ) matrix done" >> "$OUT/matrix.log"
