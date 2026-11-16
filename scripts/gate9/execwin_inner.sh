. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null)"
$S/bin/g8execwin ${1:-500} /run/current-system/sw/bin/sleep
