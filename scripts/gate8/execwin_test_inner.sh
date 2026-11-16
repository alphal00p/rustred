S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null)"
t=$S/tmp/ew.$$; mkdir -p $t
GATE8_TRIALS=${1:-3000} TMPDIR=$t unshare -Urpf --mount-proc --kill-child -- $S/bin/tests/gate8_flake-7a3b39af3f8b7b16 --ignored --nocapture 2>&1 | grep -E 'gate8 exec|test result|panicked'
