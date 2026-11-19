#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
exec bash $S/scripts/batch_e10.sh "eB1 2001 syn 1" "eB2 2002 syn 4" "eB3 2003 unp 1 20" "eB4 2004 syn 2" "eB5 2005 unp 3 20"
