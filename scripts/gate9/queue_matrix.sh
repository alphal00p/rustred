#!/usr/bin/env bash
# gate9: wait for >= 100 GB MemAvailable (checked every 30 s), then run the three matrix units (unit8.sh refuses below 100 GB anyway)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
for c in A B C; do
  while :; do a=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo); [ "$a" -ge 100 ] && break; sleep 30; done
  P=12; [ $c = B ] && P=10
  nohup scripts/unit8.sh m$c 1200 $S/scripts/matrix.sh $S/cells_$c.txt $S/logs/matrix_$c.txt $P > $S/logs/unit_m$c.txt 2>&1 &
  sleep 60
done
wait
