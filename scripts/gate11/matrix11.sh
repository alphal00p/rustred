#!/usr/bin/env bash
# gate11 matrix11.sh CELLS OUT P : each line "<ns8r.sh|ns8u.sh> <inner script name> TAG MODE args..." -> one namespaced run
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; cd $S
cells=$1; out=$2; P=${3:-8}
one() { set -- $1; ns=$1; inner=$2; shift 2
  r=$(timeout -s KILL ${CELL_TIMEOUT:-900} scripts/$ns scripts/$inner "$@" 2>&1 | tee -a $out.detail | grep -E '^RESULT' | tail -1)
  [ -n "$r" ] || r="RESULT tag=$1 mode=$2 rc=HARNESS"
  echo "$r cell=[$*]" >> $out; }
export -f one; export S out
grep -v '^#' $cells | grep . | xargs -d '\n' -P $P -I{} bash -c 'one "$@"' _ {}
echo "=== matrix done $(date -u +%FT%TZ) $(wc -l < $out) results" >> $out
