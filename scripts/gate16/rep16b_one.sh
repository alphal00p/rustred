#!/usr/bin/env bash
# gate16 item 7: ONE run of a test binary (or one exact test), the binary pid 1 of a private user+pid+mount namespace
# (--kill-child), private TMPDIR, --test-threads=4 (as in the suite). args: <bin in S/bin/d041> <exact test or ALL> <i>
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate16
b=$1; t=$2; i=$3
export TMPDIR=$S/rt/rep6c/$b-$t-$i; mkdir -p "$TMPDIR"
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14/crates/rustred-campaign || exit 99
sel=(); [ "$t" != ALL ] && sel=(--exact "$t")
t0=$(date +%s.%N)
out=$(nice -n 10 unshare -Urpf --mount-proc --kill-child -- "$S/bin/d6c95/$b" "${sel[@]}" --test-threads=4 2>&1)
rc=$?
t1=$(date +%s.%N)
res=$(printf '%s\n' "$out" | grep -m1 '^test result' | sed 's/; 0 measured.*//')
failed=$(printf '%s\n' "$out" | grep -E '^test .* FAILED$' | awk '{print $2}' | tr '\n' ',')
why=$(printf '%s\n' "$out" | grep -m1 -E "panicked at|incomplete|timed out" -A2 | tr '\t\n' '  ' | cut -c1-300)
printf '%s\t%s\t%d\t%d\t%s\t%s\t%s\t%s\n' "$b" "$t" "$i" "$rc" "$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.1f", b-a}')" "$res" "$failed" "$why"
if [ "$rc" = 0 ]; then case "$TMPDIR" in "$S"/rt/rep6c/?*) rm -rf -- "$TMPDIR";; esac; fi
exit 0
