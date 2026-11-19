#!/usr/bin/env bash
# gate13 item 7: the FROZEN host-step script (WS/bin/frozen/20261117T1058Z-host-step, read-only, unchanged) in
# --rehearse-ns, as pid 1 of a private user+pid+mount namespace (rule L10), fake children only.
# Usage: reh13.sh <tag> <items> <case_ms json>
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
F=$WS/bin/frozen/20261117T1058Z-host-step
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate13
TAG=$1; ITEMS_=$2; CASE_=$3
# shellcheck disable=SC1091
source "$WS/ops/env.sh"                      # first (L22)
export TMPDIR="$S/reh/$TAG.tmp"; mkdir -p "$TMPDIR"   # after env.sh (L22)
unset RR_CAMPAIGN_JOB RR_CAMPAIGN_FAILPOINT
export REL="$F/bin" OUT="$S/reh/$TAG" ITEMS="$ITEMS_" CASE_MS="$CASE_"
echo "reh13 $TAG start $(date -u +%FT%TZ) items $ITEMS case_ms $CASE_MS TMPDIR $TMPDIR"
taskset -c 0-167,176-191,216-257,261,263-270,272-287,320-334,336-383 \
  unshare -Urpf --mount-proc -- bash "$F/host_step_fake1.sh" --rehearse-ns
rc=$?
echo "reh13 $TAG end $(date -u +%FT%TZ) rc=$rc"
exit $rc
