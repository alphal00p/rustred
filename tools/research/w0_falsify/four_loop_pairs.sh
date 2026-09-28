#!/usr/bin/env bash
# Four-loop controls (W6 Ordered) two at a time on two 6-CPU slices.
# usage: four_loop_pairs.sh LABEL BINARY SLICE_A SLICE_B [KEY=VALUE env...]
set -u
LABEL=$1; BIN=$2; A=$3; B=$4; shift 4
R=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf/tools/research/w0_falsify/run_arm.py
cd /common/dev/rustred
run() { nix develop /common/dev/rustred --command python $R --binary "$BIN" --family $1 --label "$LABEL" \
    --cpus $2 --workers 6 --policy ordered --time-limit 1500 --grace 300 ${ENVS:+--env $ENVS} \
    > /common/dev/rustred/TMP/w0/falsify/$LABEL-$1.log 2>&1; }
ENVS="$*"
run fg $A & run bmw $B & wait
run h $A & run x $B & wait
echo "four-loop pairs $LABEL done"
