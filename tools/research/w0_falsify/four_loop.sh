#!/usr/bin/env bash
# Run the four four-loop controls (FG/BMW/H/X, W6 Ordered) in parallel on
# disjoint 6-CPU slices of 264-287. usage: four_loop.sh LABEL BINARY [KEY=VALUE env...]
set -u
LABEL=$1; BIN=$2; shift 2
R=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf/tools/research/w0_falsify/run_arm.py
declare -A CPUS=([fg]=264-269 [bmw]=270-275 [h]=276-281 [x]=282-287)
cd /common/dev/rustred
for fam in fg bmw h x; do
  nix develop /common/dev/rustred --command python $R --binary "$BIN" --family $fam --label "$LABEL" \
    --cpus ${CPUS[$fam]} --workers 6 --policy ordered --time-limit 1500 --grace 300 ${1:+--env "$@"} \
    > /common/dev/rustred/TMP/w0/falsify/$LABEL-$fam.log 2>&1 &
done
wait
echo "four-loop $LABEL done"
