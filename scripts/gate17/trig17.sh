#!/usr/bin/env bash
# gate17 item 4: gate13_trigger (my e07c7d88 test binary; it execs WT/target/debug/rr_campaign) N times, each pid 1 of its
# own private namespace. The caller runs it inside ONE user scope with a CPUQuota, nice 10. args: TAG N
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
TAG=$1; N=$2
cd "$WS/wt/ac-audit-gate14/crates/rustred-campaign" || exit 99
B=$S/bin/de07/$(ls "$S/bin/de07" | grep -E '^gate13_trigger-[0-9a-f]{16}$')
sha256sum ../../target/debug/rr_campaign > "$S/logs/trig_$TAG.txt"
for i in $(seq "$N"); do
  T=$S/rt/trig.$TAG.$i; mkdir -p "$T"
  echo "== run $i load $(cut -d' ' -f1 /proc/loadavg)" >> "$S/logs/trig_$TAG.txt"
  env TMPDIR="$T" unshare -Urpf --mount-proc --kill-child -- "$B" --nocapture --test-threads=1 >> "$S/logs/trig_$TAG.txt" 2>&1
  echo "== run $i rc=$?" >> "$S/logs/trig_$TAG.txt"
done
