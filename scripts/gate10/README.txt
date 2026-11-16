gate10 audit of ac/procfix7 3550b2ac (notes: WS/notes/implementer.gate10.md). Copies of the scratch harness
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/{scripts,src}; gate9's harness (scripts/gate9) adapted.
EVERY *_inner.sh runs ONLY inside a private user+pid+mount namespace (ns8r.sh root-mapped / ns8u.sh host-like) and refuses
otherwise; unit10.sh runs a batch as the auditor's own transient systemd user unit (refuses unless MemAvailable - MemoryMax >= 75 GB
and fewer than 2 gate10 units run).
  src/g10fault.c  = gate9's g9fault.c + G8_ERRNO=delay for op pidfd (pidfd_open delayed).
  src/g10chain.c  marked setsid'd FORK CHAIN (hop time switchable by a trigger file); src/g10hold.c holds its FIFO (EOF = no
                  incarnation left: the kernel's atomic liveness answer). g10_chain_inner.sh (+ batch_chain.sh, batch_chain_hogs.sh):
                  the KNOWN residual (crashed session's chain vs the resumed session); batch_rv.sh: M10-1 (recovery saw the chain
                  vanish, the final sweep misses it: exit 0 with the chain alive).
  cells_NS.txt    BLOCKER G10-B1: both /proc enumerations truncated (G8_OPS=readdir,getdents G8_ERRNO=trunc): exit 0 with the
                  crashed session's marked daemon alive (matrix10.sh cells_NS.txt OUT 3). Test: tests/gate10_listing.rs (ignored).
  cells_M.txt     the (a)(3) fault matrix (716 cells) for matrix10.sh + g10_inj_inner.sh / g10_live_inner.sh (record-hash prefix check
                  of session 1's ledger lines before / after via ledger_lines.py).
  g10_manifest_inner.sh / batch_gmf.sh: G9-B1 variants; g10_churn_inner.sh + src/g10churn.c / batch_gchurn.sh: G9-B2 variants (K threads).
  mutate10.py / mutrun10.sh: the mutation check in a scratch source copy (never the worktree).
  sites10.py: the independent site list (a)(1); clippytest/lib.rs: the clippy evasion probe (with the crate's clippy.toml).
  g10_ztake_inner.sh + src/g10zinit.c: lock take-over with N unreaped zombies (letter i).
