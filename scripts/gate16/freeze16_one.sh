#!/usr/bin/env bash
# gate16 item 7(b): repeat the lane's freeze experiment for a_m1: run ONE a_m1 test (binary pid 1 of its own private
# namespace); when the job writes g.pid (the group's birth), SIGSTOP that test process (OUR OWN child of the unshare we
# started: exact pid, cmdline checked) for 2.5 s, then SIGCONT. args: <old|new> <i>
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate16
v=$1; i=$2
case "$v" in old) B=$S/bin/d041/proc_audit_regressions ;; new) B=$S/bin/d6c95/proc_audit_regressions ;; *) exit 2 ;; esac
export TMPDIR=$S/rt/frz/$v-$i; mkdir -p "$TMPDIR"
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14/crates/rustred-campaign || exit 99
nice -n 10 unshare -Urpf --mount-proc --kill-child -- "$B" --exact a_m1_stale_group_number_reused_by_decoy_group_survives --test-threads=1 > "$TMPDIR/out.txt" 2>&1 &
U=$!
froze=no
for _ in $(seq 600); do
  g=$(find "$TMPDIR" -name g.pid 2>/dev/null | head -1)
  [ -n "$g" ] && [ -s "$g" ] && break
  sleep 0.05
done
P=$(cat /proc/$U/task/$U/children 2>/dev/null | awk '{print $1}')
if [ -n "${g:-}" ] && [ -n "$P" ] && tr '\0' ' ' < /proc/$P/cmdline | grep -q -- "$B --exact a_m1_"; then
  kill -STOP "$P"; sleep 2.5; kill -CONT "$P"; froze=yes
fi
wait "$U"; rc=$?
res=$(grep -m1 -E '^test a_m1' "$TMPDIR/out.txt" | cut -c1-60)
why=$(grep -m1 -E 'timed out|panicked' "$TMPDIR/out.txt" | cut -c1-160)
printf '%s\t%s\tfroze=%s\trc=%s\t%s\t%s\n' "$v" "$i" "$froze" "$rc" "$res" "$why"
