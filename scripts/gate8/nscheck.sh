. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8/scripts/lib.sh
in_ns_guard6; shm_guard
echo "uid=$(id -u) pid1=$(cat /proc/1/comm) self=$$ shm=$(stat -c '%U %a' /dev/shm) caps=$(grep CapEff /proc/self/status)"
$S/bin/g7hostile ndself $S/tmp/ndself.$$ & sleep 0.5; P=$(cat $S/tmp/ndself.$$)
echo "nd pid $P environ readable=$(head -c1 /proc/$P/environ >/dev/null 2>&1 && echo yes || echo NO) stat readable=$(head -c1 /proc/$P/stat >/dev/null 2>&1 && echo yes || echo NO)"
kill -KILL $P; rm -- $S/tmp/ndself.$$
