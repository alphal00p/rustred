#!/usr/bin/env bash
# Round-2 extended Python audit (--require-closure) paired with the round-2 verifier report
# (--verify-report) over every W0.2 calibration output. usage: audit_all.sh [AUDIT.py]
set -u
A=${1:-/common/dev/rustred/.claude/worktrees/fable51-oracle/examples/python/audit_owner_domain_walk.py}
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
O=/common/dev/rustred/TMP/w0/oracle
R=$O/runs
V=$O/r2/verify
D=$O/r2/audits
mkdir -p $D
run() {  # cpu name rundir [extra]
  local cpu=$1 name=$2 dir=$3; shift 3
  local pair=()
  [ -f "$V/$name.json" ] && pair=(--verify-report "$V/$name.json")
  taskset -c $cpu nice -n 5 $PY $A "$dir" --command "$dir/command.json" --require-closure "${pair[@]}" "$@" \
    --output $D/$name.json > $D/$name.stdout 2> $D/$name.stderr
  echo "$name exit=$?"
}
cpu=0
for label in c4l-ordered c4l-ready; do
  for f in fg bmw h x; do run $cpu $label-$f $R/$label/$f & cpu=$((cpu+1)); done
done
run $cpu c5f-ordered $R/c5f-ordered/five-finite & cpu=$((cpu+1))
run $cpu c5f-ready $R/c5f-ready/five-finite & cpu=$((cpu+1))
run $cpu frontier-fixture-fg $R/frontier-fixture/fg & cpu=$((cpu+1))
# C-HOT: CP3 state, no edge-based verifier report exists; audit only (engine closure consistency).
CH=/common/dev/rustred/TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run
taskset -c $cpu nice -n 5 $PY $A "$CH" --require-closure --output $D/chot.json > $D/chot.stdout 2> $D/chot.stderr && echo "chot exit=0" || echo "chot exit=$?" &
wait
