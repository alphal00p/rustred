#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
for v in dangling dir zero torn gone foreign; do $S/scripts/ns8r.sh $S/scripts/g10_manifest_inner.sh mf-r-$v r $v 2>&1 | grep RESULT; done
for v in eacces unreadable dangling torn gone; do $S/scripts/ns8u.sh $S/scripts/g10_manifest_inner.sh mf-u-$v u $v 2>&1 | grep RESULT; done
