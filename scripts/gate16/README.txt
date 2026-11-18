gate16 review scripts (audit of ac/procfix7 rounds 12-14 at 041e7fe8; test-only 6c9501da). Scratch root:
/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate16 (logs/, bin/, rt/, host-rehearsal/, replay/).
Every script that spawns or signals runs each binary as pid 1 of its own `unshare -Urpf --mount-proc --kill-child`.
  suite16.sh      item 10: the lane's 32 process-layer binaries + lib, LIGHT_PIDNS=1 (light_build gate16)
  rep16.sh / rep16_one.sh   item 7: 100 runs each (proc_reaudit_regressions ALL, proc_audit_regressions ALL, rb_reuse)
  rep16b_one.sh   item 7: the same runner on the 6c9501da proc_audit_regressions binary
  freeze16_one.sh item 7(b): the a_m1 freeze experiment (SIGSTOP of OUR OWN test process, the namespace's pid 1, 2.5 s)
  reh16.sh        item 8: copy of procfix7 reh_r9b.sh (paths only): host_step_fake1.sh --rehearse-ns, fake manager stall
  trig16.sh       item 3: gate13_trigger N times (clean and the M-a mutant)
