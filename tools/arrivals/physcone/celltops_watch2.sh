#!/usr/bin/env bash
# lane arrivals: on a CELL_TOPS.tsv change (helper VH 2: exact n4 per unit): recompute the main / margin cones into files bound to
# the NEW sha (cones.<rule>.<sha16>.json; earlier ones kept), compare the main-line tops with the previous file (flag DRILL_RESTART
# when they differ), flag ALERT_T162 when S(T162) > 39 or a main-line cell lies outside BOXDEF v3; log in physcone/celltops.log
PC=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone
TSV=/common/build-cache/vhirschi/p_integral_dev/helper-share/tadic/inputs/CELL_TOPS.tsv
last=$(sha256sum $TSV | cut -c1-64)
while :; do
  sleep 120
  cur=$(sha256sum $TSV 2>/dev/null | cut -c1-64)
  [ -z "$cur" ] || [ "$cur" = "$last" ] && continue
  sleep 60; cur=$(sha256sum $TSV | cut -c1-64)
  cd $PC
  # a failing recompute must not leave a stale copy under the new sha (2026-11-17 00:54Z: the 'superseded' hint crash)
  if ! m=$(python3 -I cones_variant.py celltops_n4 2>>$PC/celltops.err) || ! u=$(python3 -I cones_variant.py celltops_upper 2>>$PC/celltops.err); then
    echo "$(date -u +%FT%TZ) TSV changed ${last:0:16} -> ${cur:0:16}: RECOMPUTE FAILED (see celltops.err); nothing written" >> $PC/celltops.log
    touch $PC/RECOMPUTE_FAILED; last=$cur; continue
  fi
  cp cones.celltops_n4.json cones.celltops_n4.${cur:0:16}.json; cp cones.celltops_upper.json cones.celltops_upper.${cur:0:16}.json
  echo "$(date -u +%FT%TZ) TSV changed ${last:0:16} -> ${cur:0:16}; main $m; margin $u" >> $PC/celltops.log
  python3 -I - "${last:0:16}" "${cur:0:16}" <<'PY'
import json, sys
old, new = sys.argv[1:3]
PC = '/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone'
a = json.load(open(f'{PC}/cones.celltops_n4.{old}.json')); b = json.load(open(f'{PC}/cones.celltops_n4.{new}.json'))
out = b['cells_outside_boxdef_v3_reached'] or {}
s162 = b['nodes']['T162']['S']; s178 = b['nodes']['T178']['S']
rec = {'old': old, 'new': new, 'S_T162': [a['nodes']['T162']['S'], s162], 'S_T178': [a['nodes']['T178']['S'], s178],
       'outside_main': out, 'tops_changed': a['tops'] != b['tops']}
open(f'{PC}/celltops_change.{new}.json', 'w').write(json.dumps(rec, indent=1))
if a['tops'] != b['tops']:
    open(f'{PC}/DRILL_RESTART', 'w').write(new + '\n')
if (s162 is not None and s162 > 39) or out:
    open(f'{PC}/ALERT_T162', 'a').write(json.dumps({'new': new, 'S_T162': s162, 'outside': list(out)}) + '\n')
print(json.dumps(rec)[:400])
PY
  last=$cur
done
