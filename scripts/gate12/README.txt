gate12 audit of ac/procfix7 4435f6db (notes: WS/notes/implementer.gate12.md). Scratch harness copies; gate11's harness
(scripts/gate11, its lib.sh / ns8r.sh / ns8u.sh / matrix11.sh / g11filter.c) is reused with paths gate12 (the copies live in
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12/scripts).
  src/g12thr.c          G11-M1 variants: depth N (nested non-leader-thread forks), texit / texitz (parent thread exits right
                        after the fork), churn MS (threads churn, the marked child's parent thread lives), hop MS (the marked
                        child re-parented from thread to thread for ever).
  src/g12stream.c       a new thread forks a child and exits every MS ms (tests/gate12_walk.rs: G12-M1).
  g12_drop_inner.sh / cells12.txt (40 crashed-session cells) and g12_droplive_inner.sh / cells12live.txt (8 live cells):
                        item 1 under gate11's listing filter (both enumerations drop exactly the marked pid).
  rehearse12.sh         host_step_fake1.sh --rehearse-ns in a private namespace (item 2 / 3).
  prereq12_inner.sh     the script's refusals, in a namespace (item 3 (c)).
  mutate12.py / mutrun12.sh  gate11's M4 M6 M8 M9 applied uncommitted in this worktree, the named tests, restored (item 4).
  suite12.sh / suite12_targets.txt  the process-layer part of the suite (item 6).
  hostwalk12.sh         READ-ONLY measurement of pids_proven over the whole host /proc (tests/gate12_walk.rs, ignored test).
