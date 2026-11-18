#!/usr/bin/env bash
# gate17 item 3: N runs each, interleaved, P at once (the caller: ONE user scope with a CPUQuota, nice 10): rb_reuse before
# (041e7fe8) / after (e07c7d88); proc_reaudit_regressions ALL after; proc_audit_regressions ALL after. TSV S/logs/rep17.tsv.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
N=${1:-100}; P=${2:-16}
echo "start $(date -u +%FT%TZ) N=$N P=$P" > "$S/logs/rep17.meta"
for i in $(seq 1 "$N"); do
  echo "before proc_reaudit_b rb_reuse $i"
  echo "after proc_reaudit_b rb_reuse $i"
  echo "after proc_reaudit_regressions ALL $i"
  echo "after proc_audit_regressions ALL $i"
done | xargs -P "$P" -L 1 bash "$S/scripts/rep17_one.sh" > "$S/logs/rep17.tsv"
echo "end $(date -u +%FT%TZ)" >> "$S/logs/rep17.meta"
