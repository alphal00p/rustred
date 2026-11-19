#!/usr/bin/env bash
# gate17: one run of proc_audit_regressions b_c4_orphans_daemons_and_escapees (my e07c7d88 debug binary), pid 1 of its own
# private namespace, private TMPDIR; full output kept for a failing run. args: <i>
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
i=$1; export TMPDIR=$S/rt/bc4/$i; mkdir -p "$TMPDIR"; export RUST_BACKTRACE=1
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14/crates/rustred-campaign || exit 99
b=$S/bin/de07/$(ls "$S/bin/de07" | grep -E '^proc_audit_regressions-[0-9a-f]{16}$')
out=$(unshare -Urpf --mount-proc --kill-child -- "$b" --exact b_c4_orphans_daemons_and_escapees --test-threads=1 2>&1); rc=$?
if [ "$rc" != 0 ]; then printf '%s\n' "$out" > "$S/logs/bc4_fail_$i.txt"; fi
printf '%s\t%s\n' "$i" "$rc"
[ "$rc" = 0 ] && case "$TMPDIR" in "$S"/rt/bc4/?*) rm -rf -- "$TMPDIR";; esac
exit 0
