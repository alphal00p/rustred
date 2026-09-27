#!/usr/bin/env bash
# Strict Ordered comparisons (compare_walk_records.py --mode strict) of
# run dirs against a reference run dir, STRICT_JOBS (default 4) in parallel,
# pinned to STRICT_CPUS (default: the knobs lane CPUs 44-51,300-307).
# Usage: strict_pairs.sh <reference-run-dir> <run-dir>...
# Writes <run-dir>/strict-vs-<reference label>.json and prints verdicts.
set -uo pipefail
WT=/common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75
CPUS=${STRICT_CPUS:-44-51,300-307}
JOBS=${STRICT_JOBS:-4}
REF=${1:?reference}; shift
tag=$(basename "$(dirname "$REF")")
for run in "$@"; do
  (
    out="$run/strict-vs-$tag.json"
    nice -n 5 taskset -c "$CPUS" nix develop /common/dev/rustred --command python \
      "$WT/examples/python/compare_walk_records.py" --mode strict "$REF/result.json" "$run/result.json" \
      --output "$out" > /dev/null 2>"$run/strict-vs-$tag.stderr"
    echo "$run vs $tag: $(grep -o '"verdict": *"[A-Z]*"' "$out" 2>/dev/null) $(grep -o '"differing_records": *[0-9]*' "$out" 2>/dev/null)"
  ) &
  while (( $(jobs -rp | wc -l) >= JOBS )); do sleep 1; done
done
wait
