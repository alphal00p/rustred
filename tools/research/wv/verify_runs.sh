#!/usr/bin/env bash
# Re-run walk-verify-closure (rustred-f4d1870f, full F10 re-inspection) over batch run dirs.
# Usage: verify_runs.sh BATCH_DIR CPUS [names...]
set -u
D=$1; CPUS=$2; shift 2
VERIFY=/common/dev/rustred/TMP/w0/oracle/bin/rustred-f4d1870f
names=("$@"); [ ${#names[@]} -eq 0 ] && names=($(cd $D && ls -d */ | tr -d /))
for name in "${names[@]}"; do
  out=$D/$name
  req=()
  grep -q '"frontiers": 0,' $out/metrics.json && grep -q '"recursive_worklist_exhausted": true' $out/metrics.json && req=(--require-closure)
  s=$(date +%s)
  env RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 \
      BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1 TMPDIR=/common/dev/rustred/TMP taskset -c $CPUS nice -n 5 \
      nix develop /common/dev/rustred --command $VERIFY walk-verify-closure --command $out/command.json \
      --threads 8 --output $out.verify.json --force "${req[@]}" > $out.verify.stdout 2> $out.verify.stderr
  echo "$(date -u +%FT%TZ) verify $name exit=$? wall=$(( $(date +%s) - s ))s ${req[*]}"
done
