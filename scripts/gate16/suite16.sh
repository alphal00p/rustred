#!/usr/bin/env bash
# gate16 item 10: the process-layer suite on audit/gate16 = 041e7fe8 (the lane's 32 binaries of suite_r22.sh incl.
# procfix15_manager) + lib resources:: launch:: cpu_policy:: supervisor::worksource supervisor::manager. LIGHT_PIDNS=1.
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14 || exit 99
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate16
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
T=(proc_launch proc_signals proc_pdeathsig proc_memory proc_audit_regressions proc_reaudit_regressions proc_reaudit_b
   procfix3_proc procfix9 sup_identity procfix10_dirs sup_faults procfix15_manager gate12_walk gate12_site gate11_walk gate11_site procfix10 procfix11
   procfix11_seed procfix12 gate13_trigger gate13_uid procfix13_scope_settle procfix13_chain procfix14_handoff procfix7_tripwire
   procfix8_tripwire procfix9_tripwire gate9_pf3 gate10_sites int5_cli)
args=(); for t in "${T[@]}"; do args+=(--test "$t"); done
log=$S/logs/suite16_bins.txt
echo "start $(date -u +%FT%TZ) $(git rev-parse --short HEAD) ${#T[@]} binaries; product diff vs 041e7fe8: $(git diff --stat 041e7fe8 -- crates/rustred-campaign/src | tail -1)" > "$log"
LIGHT_PIDNS=1 nice -n 10 "$WS/plan/auto-campaign/implementer_ops/light_build.sh" gate16 test -p rustred-campaign --no-fail-fast "${args[@]}" -- --test-threads=4 >> "$log" 2>&1
rc=$?; echo "end $(date -u +%FT%TZ) rc=$rc" >> "$log"
sha256sum target/debug/rr_campaign >> "$log"
log=$S/logs/suite16_lib.txt
echo "start $(date -u +%FT%TZ) $(git rev-parse --short HEAD)" > "$log"
LIGHT_PIDNS=1 nice -n 10 "$WS/plan/auto-campaign/implementer_ops/light_build.sh" gate16 test -p rustred-campaign --lib -- resources:: launch:: cpu_policy:: supervisor::worksource supervisor::manager --test-threads=4 >> "$log" 2>&1
rc=$?; echo "end $(date -u +%FT%TZ) rc=$rc" >> "$log"
