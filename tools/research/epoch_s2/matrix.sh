#!/usr/bin/env bash
R=/common/dev/rustred; B=$R/TMP/epoch-s2/bin/rustred-142a3ae6
F="fg bmw h x four-all four-all-p5"
$R/TMP/epoch-s2/controls.sh $B s2b-w6 6 2-7 "$F" oracle
$R/TMP/epoch-s2/controls.sh $B s2b-w12 12 2-13 "$F"
$R/TMP/epoch-s2/controls.sh $B s2b-w24 24 0-15,256-271 "$F"
echo "$(date -u +%FT%TZ) matrix done" >> $R/TMP/epoch-s2/runs/controls.log
