#!/usr/bin/env bash
# gate12 item 3: READ-ONLY measurement of pids_proven over the whole host /proc (no namespace: it must see the host; the
# test lists and reads /proc only, spawns and signals nothing).
G12_HOSTWALK_N=${1:-12} /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12/bin/gate12_walk_hostwalk --ignored --exact g12_hostwalk_measure --nocapture
