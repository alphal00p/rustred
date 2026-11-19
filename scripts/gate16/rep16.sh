#!/usr/bin/env bash
# gate16 item 7: N runs each (interleaved): proc_reaudit_regressions ALL (13 tests, incl. scan_of_an_unused_id_is_empty),
# proc_audit_regressions ALL (30, incl. a_m1_...), proc_reaudit_b rb_reuse; P at once. TSV S/logs/rep16.tsv.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate16
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
N=${1:-100}; P=${2:-16}
echo "start $(date -u +%FT%TZ) N=$N P=$P" > "$S/logs/rep16.meta"
for i in $(seq 1 "$N"); do
  echo "proc_reaudit_regressions ALL $i"
  echo "proc_audit_regressions ALL $i"
  echo "proc_reaudit_b rb_reuse $i"
done | xargs -P "$P" -L 1 bash "$S/scripts/rep16_one.sh" > "$S/logs/rep16.tsv"
echo "end $(date -u +%FT%TZ)" >> "$S/logs/rep16.meta"
