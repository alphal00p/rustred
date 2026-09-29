#!/usr/bin/env bash
# epoch-s2 socket-1 session under socket1.lock (<= 60 min): C-5F epoch W50 (128-177), four-all and
# four-all-p5 epoch W96 (128-223). Foreign busy CPUs recorded at lock acquisition.
R=/common/dev/rustred; P=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
LOG=$R/TMP/epoch-s2/runs/controls.log
B=$R/TMP/epoch-s2/bin/rustred-142a3ae6
echo "$(date -u +%FT%TZ) waiting socket1.lock for the epoch-s2 socket-1 session" >> $LOG
exec 9>$R/TMP/locks/socket1.lock
flock -w 14400 9 || { echo "$(date -u +%FT%TZ) socket1.lock timeout" >> $LOG; exit 1; }
busy=$(mpstat -P 128-255 1 1 2>/dev/null | awk '/Average/ && $2 ~ /^[0-9]+$/ {b+=100-$NF} END {printf "%.1f", b/100}')
echo "$(date -u +%FT%TZ) socket1.lock acquired; foreign busy CPUs 128-255: $busy" >> $LOG
cd $R
timeout 3000 $P TMP/epoch-s2/run_control.py --binary $B --family five-finite --label s2b-c5f-w50 --cpus 128-177 --policy epoch --workers 50 --timeout-seconds 2400 > TMP/epoch-s2/runs/s2b-c5f-w50-five-finite.metrics 2>&1
echo "$(date -u +%FT%TZ) end s2b-c5f-w50/five-finite $(grep -o '"exit_code": [0-9]*' TMP/epoch-s2/runs/s2b-c5f-w50-five-finite.metrics) $(grep -o '"whole_command_seconds": [0-9.]*' TMP/epoch-s2/runs/s2b-c5f-w50-five-finite.metrics)" >> $LOG
timeout 600 $P TMP/epoch-s2/run_control.py --binary $B --family four-all --label s2b-w96 --cpus 128-223 --policy epoch --workers 96 --timeout-seconds 400 > TMP/epoch-s2/runs/s2b-w96-four-all.metrics 2>&1
echo "$(date -u +%FT%TZ) end s2b-w96/four-all $(grep -o '"exit_code": [0-9]*' TMP/epoch-s2/runs/s2b-w96-four-all.metrics)" >> $LOG
timeout 600 $P examples/input/four_loop_combined/tools/run_c4l.py --binary $B --command $R/TMP/c4l-s2/commands/command-four-all-p5.json --family four-all-p5 --label s2b-w96 --cpus 128-223 --policy epoch --workers 96 --timeout-seconds 400 --out-root $R/TMP/epoch-s2/runs > TMP/epoch-s2/runs/s2b-w96-four-all-p5.metrics 2>&1
echo "$(date -u +%FT%TZ) end s2b-w96/four-all-p5 $(grep -o '"exit_code": [0-9]*' TMP/epoch-s2/runs/s2b-w96-four-all-p5.metrics)" >> $LOG
busy=$(mpstat -P 128-255 1 1 2>/dev/null | awk '/Average/ && $2 ~ /^[0-9]+$/ {b+=100-$NF} END {printf "%.1f", b/100}')
echo "$(date -u +%FT%TZ) socket-1 session done; foreign+own busy CPUs 128-255: $busy" >> $LOG
