#!/usr/bin/env bash
# gate12 rehearse12.sh TAG [ITEMS CASE_MS] : host-tests/host_step_fake1.sh --rehearse-ns as pid 1's child of a private
# user+pid+mount namespace (private /dev/shm, private TMPDIR after env.sh); run INSIDE a gate12 unit (unit12.sh: AllowedCPUs
# = the L21 base set, held list empty). Release binaries S/bin/release. OUT = S/rehearse/TAG.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12
TAG=$1; ITEMS=${2:-240}; CASE_MS=${3:-[60000,120000]}
source $WS/ops/env.sh >/dev/null 2>&1
export TMPDIR=$S/tmp/rehearse-$TAG-$$; mkdir -p $TMPDIR $S/rehearse
t0=$(date +%s)
REL=$S/bin/release OUT=$S/rehearse/$TAG ITEMS=$ITEMS CASE_MS=$CASE_MS \
  unshare -Urpf --mount-proc --kill-child -- \
  /home/vhirschi/.nix-profile/bin/bash -c 'mount -t tmpfs -o size=32m,mode=1777 tmpfs /dev/shm && exec /home/vhirschi/.nix-profile/bin/bash '"$WS"'/plan/auto-campaign/host-tests/host_step_fake1.sh --rehearse-ns'
rc=$?
echo "rehearsal $TAG rc=$rc wall=$(( $(date +%s) - t0 )) s"
exit $rc
