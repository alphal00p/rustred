#!/usr/bin/env bash
# Strict Ordered record comparison: each new-binary run vs ref-r1, and ref-r2 vs ref-r1.
# usage: compare_all.sh RUNS_DIR NEWLABEL family...
set -u
RUNS=$1; NEW=$2; shift 2
C=/common/dev/rustred/.claude/worktrees/fable51-symbolica/examples/python/compare_walk_records.py
cd /common/dev/rustred
for fam in "$@"; do
  for other in ref-r2 $NEW-r1 $NEW-r2; do
    a=$RUNS/$fam/ref-r1/result.json; b=$RUNS/$fam/$other/result.json
    [ -f "$b" ] || { echo "$fam $other MISSING"; continue; }
    out=$RUNS/$fam/strict-ref-r1-vs-$other.json
    nice -n 5 taskset -c 288-319 nix develop --command python $C --mode strict $a $b --output $out > /dev/null 2>$out.stderr
    rc=$?
    v=$(grep -o '"verdict": "[A-Z]*"' $out 2>/dev/null)
    echo "$fam ref-r1 vs $other: exit $rc $v"
  done
done
