#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
exec bash $S/scripts/batch_e.sh "eA1 1001 syn 4" "eA2 1002 syn 1" "eA3 1003 syn 2" "eA4 1004 unp 2 20" "eA5 1005 syn 3"
