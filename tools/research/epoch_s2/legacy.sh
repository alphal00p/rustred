#!/usr/bin/env bash
# epoch-s2: legacy reference runs. usage: legacy.sh BIN LABEL POLICY WORKERS CPUS "FAMILIES"
set -u
BIN=${1:?}; LABEL=${2:?}; POL=${3:?}; W=${4:?}; CPUS=${5:?}; FAMS=${6:?}
R=/common/dev/rustred; P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
LOG=$R/TMP/epoch-s2/runs/legacy.log
cd $R
for fam in $FAMS; do
  echo "$(date -u +%FT%TZ) start $LABEL/$fam $POL W$W cpus=$CPUS bin=$(basename $BIN)" >> $LOG
  if [ $fam = four-all-p5 ]; then
    $P examples/input/four_loop_combined/tools/run_c4l.py --binary $BIN --command $R/TMP/c4l-s2/commands/command-four-all-p5.json \
      --family four-all-p5 --label $LABEL --cpus $CPUS --policy $POL --workers $W --timeout-seconds 1800 --stop-natives 60000 --out-root $R/TMP/epoch-s2/runs \
      > $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics 2>&1
  else
    timeout 3500 $P TMP/epoch-s2/run_control.py --binary $BIN --family $fam --label $LABEL --cpus $CPUS --policy $POL --workers $W --timeout-seconds 1800 \
      > $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics 2>&1
  fi
  echo "$(date -u +%FT%TZ) end $LABEL/$fam rc=$? $(grep -o '"exit_code": [0-9]*' $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics) $(grep -o '"whole_command_seconds": [0-9.]*' $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics)" >> $LOG
done
echo "$(date -u +%FT%TZ) done $LABEL" >> $LOG
