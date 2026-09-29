#!/usr/bin/env bash
# epoch-s2 gate 4 addendum (protocol §18.2): CP5 resume across binaries on FG, both directions
# (4a17f9c7 -> branch and branch -> 4a17f9c7), Ordered, strict identity of the resumed run vs the
# frozen 4a17f9c7 Ordered W6 reference (int-ref-g3), ignoring only the paused leg's receipt key.
# usage: cp5_resume.sh NEWBIN TAG   (CPUs 8-13)
set -u
NEW=${1:?}; TAG=${2:?}
R=/common/dev/rustred; C=$R/TMP/fable51-controls; P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
REF=$C/bin/rustred-4a17f9c7
REFRES=$C/int-ref-g3/fg/result.json
OUT=$R/TMP/epoch-s2/runs/cp5-resume-$TAG; mkdir -p $OUT
cd $R; export TMPDIR=$R/TMP
echo "$(date -u +%FT%TZ) NEW=$NEW sha256=$(sha256sum $NEW | cut -c1-64) REF=$REF REFRES=$REFRES" >> $OUT/log
for dir in ref2new new2ref; do
  first=$REF; second=$NEW; [ $dir = new2ref ] && { first=$NEW; second=$REF; }
  label=epoch-s2-resume-$dir-$TAG
  $P $C/resume_control.py --first $first --second $second --family fg --label $label --cpus 8-13 --policy ordered \
    --workers 6 --stop-at-committed 40000 > $OUT/$dir.txt 2>&1
  fx=$(grep -o '"first_exit": [0-9-]*' $C/$label/fg/report.json | grep -o '[0-9-]*$')
  rx=$(grep -o '"resume_exit": [0-9-]*' $C/$label/fg/report.json | grep -o '[0-9-]*$')
  nix develop --command python $R/examples/python/compare_walk_records.py --mode strict --ignore-top uncommitted_inspections \
    $REFRES $C/$label/fg/run2/result.json > $OUT/$dir.strict.json 2>&1
  v=$(grep -o '"verdict": "[A-Z]*"' $OUT/$dir.strict.json | head -1 | grep -o '[A-Z]*"$' | tr -d '"')
  gate=FAIL; [ "$fx" = 4 ] && [ "$rx" = 0 ] && [ "$v" = PASS ] && gate=PASS
  echo "$(date -u +%FT%TZ) GATE cp5-resume-$dir $gate first_exit=$fx resume_exit=$rx strict=$v $(grep -o '"differing_records": [0-9]*' $OUT/$dir.strict.json | head -1)" >> $OUT/log
done
echo "$(date -u +%FT%TZ) done" >> $OUT/log
