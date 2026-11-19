. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/scripts/lib.sh
in_ns_guard6; shm_guard
R=$S/runs/churnmark; mkdir -p $R
env -i RR_CAMPAIGN_JOB=g9test.0123456789abcdef:J:a1 PATH=/bin $S/bin/g9churn $R/p 0; sleep 0.5; CP=$(cat $R/p)
ok=0; tries=0; while [ $tries -lt 2000 ] && [ $ok = 0 ]; do tries=$((tries+1)); for t in $(ls /proc/$CP/task 2>/dev/null); do v=$(tr '\0' '\n' < /proc/$CP/task/$t/environ 2>/dev/null | grep RR_CAMPAIGN_JOB) && { ok=1; echo "thread $t environ: $v (after $tries listings)"; break; }; done; done
echo "leader state: $(awk '{print $3}' /proc/$CP/stat) Threads: $(grep Threads /proc/$CP/status)  cpu ticks: $(awk '{print $14+$15}' /proc/$CP/stat)"
sleep 2; echo "2 s later: state $(awk '{print $3}' /proc/$CP/stat) $(grep Threads /proc/$CP/status) cpu ticks $(awk '{print $14+$15}' /proc/$CP/stat)"
kill -KILL $CP; sleep 0.2; echo "after SIGKILL: $(ls /proc/$CP 2>/dev/null | wc -l) entries"
