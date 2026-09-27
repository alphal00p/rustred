#!/usr/bin/env bash
# Extended Python audit (--require-closure) over every W0.2 calibration output.
set -u
A=/common/dev/rustred/.claude/worktrees/fable51-oracle/examples/python/audit_owner_domain_walk.py
O=/common/dev/rustred/TMP/w0/oracle
R=$O/runs
mkdir -p $O/audits
run() {  # name rundir [extra args]
  local name=$1 dir=$2; shift 2
  local cmd=()
  [ -f "$dir/command.json" ] && cmd=(--command "$dir/command.json")
  nix develop /common/dev/rustred --command python $A "$dir" "${cmd[@]}" --require-closure "$@" \
    --output $O/audits/$name.json > $O/audits/$name.stdout 2> $O/audits/$name.stderr
  echo "$name exit=$?"
}
cpu=46
for label in c4l-ordered c4l-ready; do
  for f in fg bmw h x; do
    taskset -c $cpu nice -n 5 bash -c "$(declare -f run); A=$A O=$O; run $label-$f $R/$label/$f" &
    cpu=$((cpu+1))
  done
done
for label in c5f-ordered c5f-ready; do
  taskset -c $cpu nice -n 5 bash -c "$(declare -f run); A=$A O=$O; run $label $R/$label/five-finite" &
  cpu=$((cpu+1))
done
taskset -c $cpu nice -n 5 bash -c "$(declare -f run); A=$A O=$O; run frontier-fixture-fg $R/frontier-fixture/fg" &
cpu=$((cpu+1))
taskset -c $cpu nice -n 5 bash -c "$(declare -f run); A=$A O=$O; run chot /common/dev/rustred/TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run" &
wait
