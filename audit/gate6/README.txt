gate6 audit repro scripts (copies of /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6/scripts, src).
Every *_inner.sh runs ONLY inside a private user+pid namespace (inns.sh: unshare -Urpf --mount-proc; innsu.sh: host-like map, uid 1000, no capabilities); in_ns_guard6 refuses otherwise.
Key repros: nd_inner.sh (non-dumpable ownership -> exit 8), supnd_inner.sh (run book A12), cp_inner.sh (shared work_dir copy), m1/m2/m3/m4_inner.sh, crashsig_inner.sh, e_inner.sh. Notes: WS/notes/implementer.gate6.md.
