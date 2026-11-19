# inside a private ns: run the procfix3_proc sweep test, then list every live process still carrying RR_CAMPAIGN_JOB
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6
mount -t tmpfs -o size=32m tmpfs /dev/shm 2>/dev/null
o=$($S/bin/tests/procfix3_proc-1e9d7d9895fd2676 --exact sweep_kills_only_marked_members_and_never_a_decoy --nocapture --test-threads=1 2>&1)
r=$(echo "$o" | grep -o "test result: [a-zA-Z]*"); k=$(echo "$o" | grep -o "killed=[0-9]*")
left=""; for p in /proc/[0-9]*; do p=${p#/proc/}; [ "$p" = 1 ] || [ "$p" = $$ ] && continue; v=$(tr '\0' '\n' < /proc/$p/environ 2>/dev/null | grep '^RR_CAMPAIGN_JOB=' ); [ -n "$v" ] && left="$left $p:$(cat /proc/$p/comm):${v#RR_CAMPAIGN_JOB=}"; done
echo "PF3 $r $k marked_alive_after:[$left]"
