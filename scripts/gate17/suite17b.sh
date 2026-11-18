#!/usr/bin/env bash
# gate17 item 6 (PART 2: the 15 binaries from procfix10 on + lib; part 1 was cut by my wrapper limit at 30 min): the lane's 32 process-layer binaries (suite_r23 list) + lib resources:: launch:: cpu_policy::
# supervisor::worksource supervisor::manager, from MY debug build of e07c7d88 (copies in S/bin/de07). Each binary is
# pid 1 of its own `unshare -Urpf --mount-proc --kill-child` (L10), private TMPDIR, --test-threads=4. The caller runs
# this script inside ONE user scope with CPUQuota (<= 800 % for all my work) and nice 10.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
cd "$WS/wt/ac-audit-gate14/crates/rustred-campaign" || exit 99
T=(
   procfix10 procfix11
   procfix11_seed procfix12 gate13_trigger gate13_uid procfix13_scope_settle procfix13_chain procfix14_handoff procfix7_tripwire
   procfix8_tripwire procfix9_tripwire gate9_pf3 gate10_sites int5_cli)
log=$S/logs/suite17_bins.part2.txt
echo "start $(date -u +%FT%TZ) $(git rev-parse --short HEAD) ${#T[@]} binaries" > "$log"
for t in "${T[@]}"; do
  b=$(ls "$S/bin/de07/" | grep -E "^${t}-[0-9a-f]{16}$")
  export TMPDIR=$S/rt/suite/$t; mkdir -p "$TMPDIR"
  echo "=== $t ($b) $(date -u +%T)" >> "$log"
  unshare -Urpf --mount-proc --kill-child -- "$S/bin/de07/$b" --test-threads=4 >> "$log" 2>&1
  echo "=== $t rc=$? $(date -u +%T)" >> "$log"
done
echo "end $(date -u +%FT%TZ)" >> "$log"
log=$S/logs/suite17_lib.txt
export TMPDIR=$S/rt/suite/lib; mkdir -p "$TMPDIR"
echo "start $(date -u +%FT%TZ)" > "$log"
unshare -Urpf --mount-proc --kill-child -- "$S/bin/de07/rustred_campaign-cc363e7c83da292e" resources:: launch:: cpu_policy:: supervisor::worksource supervisor::manager --test-threads=4 >> "$log" 2>&1
echo "end $(date -u +%FT%TZ) rc=$?" >> "$log"
