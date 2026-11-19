#!/usr/bin/env bash
# gate15: gate13_trigger (execs WT/target/debug/rr_campaign) N times, each pid 1 of its own private namespace.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate15
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
TAG=$1; N=$2
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14/crates/rustred-campaign || exit 99
sha256sum ../../target/debug/rr_campaign > "$S/logs/trig_$TAG.txt"
for i in $(seq "$N"); do
  T=$S/rt/trig.$TAG.$i; mkdir -p "$T"
  echo "== run $i load $(cut -d' ' -f1 /proc/loadavg)" >> "$S/logs/trig_$TAG.txt"
  env TMPDIR="$T" unshare -Urpf --mount-proc --kill-child -- "$S/bin/a422/gate13_trigger" --nocapture --test-threads=1 >> "$S/logs/trig_$TAG.txt" 2>&1
  echo "== run $i rc=$?" >> "$S/logs/trig_$TAG.txt"
done
