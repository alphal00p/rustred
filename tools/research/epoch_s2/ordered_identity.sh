#!/usr/bin/env bash
# epoch-s2 gate 4: legacy Ordered strict identity of the branch binary vs the frozen 4a17f9c7 references
# (TMP/fable51-controls/int-ref-g3, integrator W6 on 0-5 and C-5F W16 on 8-15,264-271; Ordered is deterministic).
# usage: ordered_identity.sh NEWBIN LABEL
set -u
NEW=${1:?}; LABEL=${2:?}
R=/common/dev/rustred; P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
REF=$R/TMP/fable51-controls/int-ref-g3
OUT=$R/TMP/epoch-s2/runs/$LABEL; mkdir -p $OUT
LOG=$OUT/identity.log
cd $R
echo "$(date -u +%FT%TZ) NEW=$NEW sha256=$(sha256sum $NEW | cut -c1-64) REF=$REF (4a17f9c7)" >> $LOG
for spec in "fg 6 8-13" "bmw 6 8-13" "h 6 8-13" "x 6 8-13" "five-finite 16 8-15,264-271"; do
  set -- $spec; fam=$1; W=$2; C=$3
  timeout 3500 $P TMP/epoch-s2/run_control.py --binary $NEW --family $fam --label $LABEL --cpus $C --policy ordered --workers $W > $OUT/$fam.metrics 2>&1
  nix develop --command python $R/examples/python/compare_walk_records.py --mode strict $REF/$fam/result.json $OUT/$fam/result.json > $OUT/cmp-$fam.json 2>&1
  verdict=$(grep -o '"verdict": "[A-Z]*"' $OUT/cmp-$fam.json | head -1 | grep -o '[A-Z]*"$' | tr -d '"')
  diff=$(grep -o '"differing_records": [0-9]*' $OUT/cmp-$fam.json | head -1)
  cr=$(grep -o '"containment_checks": "*[0-9]*' $REF/$fam/metrics.json | grep -o '[0-9]*$'); cn=$(grep -o '"containment_checks": "*[0-9]*' $OUT/$fam/metrics.json | grep -o '[0-9]*$')
  ex=$(grep -o '"exit_code": [0-9]*' $OUT/$fam/metrics.json | grep -o '[0-9]*$')
  gate=FAIL; [ "$verdict" = PASS ] && [ "$ex" = 0 ] && [ -n "$cr" ] && [ "$cr" = "$cn" ] && gate=PASS
  echo "$(date -u +%FT%TZ) GATE4 $fam $gate verdict=$verdict $diff checks ref=$cr new=$cn exit=$ex W$W" >> $LOG
done
# The combined four-loop controls (C-4L-comb-O rows): four-all through run_control, four-all-p5 through run_c4l.
for fam in four-all four-all-p5; do
  if [ $fam = four-all-p5 ]; then
    $P examples/input/four_loop_combined/tools/run_c4l.py --binary $NEW --command $R/TMP/c4l-s2/commands/command-four-all-p5.json \
      --family four-all-p5 --label $LABEL --cpus 8-13 --policy ordered --workers 6 --timeout-seconds 1800 --out-root $R/TMP/epoch-s2/runs \
      > $OUT/$fam.metrics 2>&1
  else
    timeout 3500 $P TMP/epoch-s2/run_control.py --binary $NEW --family $fam --label $LABEL --cpus 8-13 --policy ordered --workers 6 > $OUT/$fam.metrics 2>&1
  fi
  nix develop --command python $R/examples/python/compare_walk_records.py --mode strict $REF/$fam/result.json $OUT/$fam/result.json > $OUT/cmp-$fam.json 2>&1
  verdict=$(grep -o '"verdict": "[A-Z]*"' $OUT/cmp-$fam.json | head -1 | grep -o '[A-Z]*"$' | tr -d '"')
  diff=$(grep -o '"differing_records": [0-9]*' $OUT/cmp-$fam.json | head -1)
  cr=$(grep -o '"containment_checks": "*[0-9]*' $REF/$fam/metrics.json | grep -o '[0-9]*$'); cn=$(grep -o '"containment_checks": "*[0-9]*' $OUT/$fam/metrics.json | grep -o '[0-9]*$')
  gate=FAIL; [ "$verdict" = PASS ] && [ -n "$cr" ] && [ "$cr" = "$cn" ] && gate=PASS
  echo "$(date -u +%FT%TZ) GATE4 $fam $gate verdict=$verdict $diff checks ref=$cr new=$cn W6" >> $LOG
done
echo "$(date -u +%FT%TZ) done" >> $LOG
