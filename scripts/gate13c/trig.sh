#!/usr/bin/env bash
# gate13c item 2: gate13_trigger runs (pid 1 of a private user+pid+mount namespace each, private TMPDIR). Args: TAG BIN RUNS
set -u
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate13c
TAG=$1; BIN=$2; RUNS=$3
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
exec >> "$S/logs/trig_$TAG.txt" 2>&1
for i in $(seq "$RUNS"); do
  T=$S/rt/trig.$TAG.$i; mkdir -p "$T"
  echo "== $TAG run $i $(date -u +%FT%TZ) load $(cut -d' ' -f1 /proc/loadavg)"
  env TMPDIR="$T" unshare -Urpf --mount-proc --kill-child -- "$BIN" --nocapture --test-threads=1 2>&1
  echo "rc_${TAG}_$i=$?"
done
