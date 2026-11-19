#!/usr/bin/env bash
# gate17 item 3: ONE run of a test binary (or one exact test), the binary pid 1 of a private user+pid+mount namespace
# (--kill-child), private TMPDIR, --test-threads=4. args: <before|after> <binary name> <exact test or ALL> <i>
# before = gate16's debug binaries of 041e7fe8 (S16/bin/d041, read only); after = mine of e07c7d88 (S/bin/de07).
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate17
v=$1; b=$2; t=$3; i=$4
case "$v" in
  before) bin=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate16/bin/d041/$b ;;
  after) bin=$S/bin/de07/$(ls "$S/bin/de07" | grep -E "^${b}-[0-9a-f]{16}$") ;;
esac
export TMPDIR=$S/rt/rep/$v-$b-$t-$i; mkdir -p "$TMPDIR"
cd /shared/vhirschi/t3code/vakint_dev/p_integral_dev/wt/ac-audit-gate14/crates/rustred-campaign || exit 99
sel=(); [ "$t" != ALL ] && sel=(--exact "$t")
t0=$(date +%s.%N)
out=$(unshare -Urpf --mount-proc --kill-child -- "$bin" "${sel[@]}" --test-threads=4 2>&1)
rc=$?
t1=$(date +%s.%N)
res=$(printf '%s\n' "$out" | grep -m1 '^test result' | sed 's/; 0 measured.*//')
failed=$(printf '%s\n' "$out" | grep -E '^test .* FAILED$' | awk '{print $2}' | tr '\n' ',')
why=$(printf '%s\n' "$out" | grep -m1 -E "panicked at|incomplete|timed out" -A2 | tr '\t\n' '  ' | cut -c1-300)
printf '%s\t%s\t%s\t%d\t%d\t%s\t%s\t%s\t%s\n' "$v" "$b" "$t" "$i" "$rc" "$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.1f", b-a}')" "$res" "$failed" "$why"
if [ "$rc" = 0 ]; then case "$TMPDIR" in "$S"/rt/rep/?*) rm -rf -- "$TMPDIR";; esac; fi
exit 0
