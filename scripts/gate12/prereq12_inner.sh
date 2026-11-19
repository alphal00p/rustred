#!/usr/bin/env bash
# gate12 item 3(c): host_step_fake1.sh refusals, run INSIDE a private user+pid namespace only.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12
H=$WS/plan/auto-campaign/host-tests/host_step_fake1.sh; B=/home/vhirschi/.nix-profile/bin/bash
[ "$(cat /proc/1/comm)" != systemd ] || { echo "not in a namespace"; exit 99; }
mkdir -p $S/prereq/exists
c() { local name=$1; shift; out=$("$@" 2>&1); rc=$?; echo "CASE $name rc=$rc :: $(echo "$out" | grep -v '^{\|^ "\|^---' | head -3 | tr '\n' ' ' | cut -c1-260)"; }
c dryrun_release env REL=$S/bin/release $B $H --dry-run
c dryrun_debug_failpoint env REL=$S/bin/debug $B $H --dry-run
c run_in_namespace env REL=$S/bin/release $B $H --run
c rr_job_in_env env REL=$S/bin/release RR_CAMPAIGN_JOB=x:y $B $H --dry-run
c out_exists env REL=$S/bin/release OUT=$S/prereq/exists ITEMS=1 $B $H --rehearse-ns
c rel_missing env REL=$S/prereq/nothing $B $H --dry-run
c no_args env REL=$S/bin/release $B $H
c two_args env REL=$S/bin/release $B $H --run --dry-run
c rel_unset env -u REL $B $H --dry-run
echo "--- full dry-run output (release):"; env REL=$S/bin/release $B $H --dry-run
ls -la $S/prereq
