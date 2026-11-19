#!/usr/bin/env bash
# gate15 item 6: the unproven-end flow (procfix7 mj1) and the 0-us live-session fork churner (procfix9), each binary pid 1
# of its own private namespace, private TMPDIR (after env.sh), --nocapture.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate15
# shellcheck disable=SC1091
source "$WS/ops/env.sh" >/dev/null 2>&1
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14/crates/rustred-campaign || exit 99
for t in "procfix7 mj1_an_unproven_end_is_named_its_lease_released_and_the_run_ends_8" "procfix9 g9_b2_fork_churner_live_session"; do
  set -- $t
  T=$S/rt/item6.$1; mkdir -p "$T"
  echo "== $1 $2 start $(date -u +%FT%TZ) load $(cut -d' ' -f1 /proc/loadavg)"
  env TMPDIR="$T" unshare -Urpf --mount-proc --kill-child -- "$S/bin/a422/$1" "$2" --exact --nocapture --test-threads=1 2>&1 | tail -30
  echo "== $1 rc=${PIPESTATUS[0]} end $(date -u +%FT%TZ)"
done
