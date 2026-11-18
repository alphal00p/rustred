#!/usr/bin/env bash
# gate15 item 8: the process-layer suite (the lane's 31 binaries of suite_r20.sh) + lib resources:: launch:: cpu_policy::
# supervisor::worksource on audit/gate15 = a422bfa9. LIGHT_PIDNS=1 (every test binary pid 1 of its own private namespace).
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14 || exit 99
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate15
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
T=(proc_launch proc_signals proc_pdeathsig proc_memory proc_audit_regressions proc_reaudit_regressions proc_reaudit_b
   procfix3_proc procfix9 sup_identity procfix10_dirs sup_faults gate12_walk gate12_site gate11_walk gate11_site procfix10 procfix11
   procfix11_seed procfix12 gate13_trigger gate13_uid procfix13_scope_settle procfix13_chain procfix14_handoff procfix7_tripwire
   procfix8_tripwire procfix9_tripwire gate9_pf3 gate10_sites int5_cli)
args=(); for t in "${T[@]}"; do args+=(--test "$t"); done
log=$S/logs/suite15_bins.txt
echo "start $(date -u +%FT%TZ) $(git rev-parse --short HEAD) ${#T[@]} binaries; product diff vs a422bfa9: $(git diff --stat a422bfa9 -- crates/rustred-campaign/src | tail -1)" > "$log"
LIGHT_PIDNS=1 nice -n 10 "$WS/plan/auto-campaign/implementer_ops/light_build.sh" gate15 test -p rustred-campaign --no-fail-fast "${args[@]}" -- --test-threads=4 >> "$log" 2>&1
rc=$?; echo "end $(date -u +%FT%TZ) rc=$rc" >> "$log"
sha256sum target/debug/rr_campaign >> "$log"
log=$S/logs/suite15_lib.txt
echo "start $(date -u +%FT%TZ) $(git rev-parse --short HEAD)" > "$log"
LIGHT_PIDNS=1 nice -n 10 "$WS/plan/auto-campaign/implementer_ops/light_build.sh" gate15 test -p rustred-campaign --lib -- resources:: launch:: cpu_policy:: supervisor::worksource --test-threads=4 >> "$log" 2>&1
rc=$?; echo "end $(date -u +%FT%TZ) rc=$rc" >> "$log"
