#!/usr/bin/env bash
# epoch-s2: run epoch controls at one width, then the oracle on each.
# usage: controls.sh BIN LABEL WORKERS CPUS "FAMILIES" [oracle|oracle32|oracle16]
# (oracle/oracle32: CPUs 0-15,256-271 at 32 threads; oracle16: 0-7,256-263 at 16 threads)
set -u
BIN=${1:?}; LABEL=${2:?}; W=${3:?}; CPUS=${4:?}; FAMS=${5:?}; ORC=${6:-}
R=/common/dev/rustred; P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
LOG=$R/TMP/epoch-s2/runs/controls.log
cd $R
for fam in $FAMS; do
  echo "$(date -u +%FT%TZ) start $LABEL/$fam W$W cpus=$CPUS bin=$(basename $BIN)" >> $LOG
  if [ $fam = four-all-p5 ]; then
    $P examples/input/four_loop_combined/tools/run_c4l.py --binary $BIN --command $R/TMP/c4l-s2/commands/command-four-all-p5.json \
      --family four-all-p5 --label $LABEL --cpus $CPUS --policy epoch --workers $W --timeout-seconds 3000 --out-root $R/TMP/epoch-s2/runs \
      > $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics 2>&1
  else
    timeout 3500 $P TMP/epoch-s2/run_control.py --binary $BIN --family $fam --label $LABEL --cpus $CPUS --policy epoch --workers $W --timeout-seconds 3000 \
      > $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics 2>&1
  fi
  echo "$(date -u +%FT%TZ) end $LABEL/$fam rc=$? $(grep -o '"exit_code": [0-9]*' $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics) $(grep -o '"whole_command_seconds": [0-9.]*' $R/TMP/epoch-s2/runs/$LABEL-$fam.metrics)" >> $LOG
  case "$ORC" in
    oracle16) $R/TMP/epoch-s2/oracle.sh $BIN $R/TMP/epoch-s2/runs/$LABEL/$fam 0-7,256-263 16 >> $LOG ;;
    oracle|oracle32) $R/TMP/epoch-s2/oracle.sh $BIN $R/TMP/epoch-s2/runs/$LABEL/$fam 0-15,256-271 32 >> $LOG ;;
  esac
done
echo "$(date -u +%FT%TZ) done $LABEL" >> $LOG
