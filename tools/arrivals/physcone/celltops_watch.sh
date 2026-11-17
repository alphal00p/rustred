#!/usr/bin/env bash
# lane arrivals: recompute the CELL_TOPS cones when CELL_TOPS.tsv changes (helper VH 2 replaces the n4 hint); log in physcone/celltops.log;
# flag file physcone/ALERT_T162 when S(T162) > 39 on the main or margin line
PC=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone
TSV=/common/build-cache/vhirschi/p_integral_dev/helper-share/tadic/inputs/CELL_TOPS.tsv
last=$(sha256sum $TSV | cut -c1-64)
while :; do
  sleep 300
  cur=$(sha256sum $TSV 2>/dev/null | cut -c1-64)
  [ -z "$cur" ] || [ "$cur" = "$last" ] && continue
  sleep 60  # let the writer finish
  cur=$(sha256sum $TSV | cut -c1-64)
  cd $PC
  m=$(python3 -I cones_variant.py celltops_n4); u=$(python3 -I cones_variant.py celltops_upper)
  echo "$(date -u +%FT%TZ) TSV changed $last -> $cur; main $m; margin $u" >> $PC/celltops.log
  python3 -I -c 'import json,sys
for f in sys.argv[1:]:
    s = json.load(open(f))["nodes"]["T162"]["S"]
    if s is not None and s > 39: open("'$PC'/ALERT_T162", "a").write(f + " S(T162)=" + str(s) + "\n")' cones.celltops_n4.json cones.celltops_upper.json
  last=$cur
done
