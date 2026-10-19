# Generation: resource-changing resume and final dashboard

This applies to the six-/seven-loop study generation launcher and its generic
shared/isolated generation schedulers. It does not change the separate five-loop
finite-demand or recursive campaign engines, algebra, rules, or coverage claims.

## Resume without discarding completed sectors

Completed native sector checkpoint identity binds the family/source, selected
sectors, root, permutation, ordering and mathematical generation policies. Worker
counts and resource grants are deliberately not part of that identity.

On `--resume`, callers may now change `--workers`, `--cpus`, `--jobs`,
`--max-active-sectors`, `--max-memory-bytes`, `--ram-margin-percent`, and the
read-only `--dashboard-executable`. Shared execution needs a matching worker/CPU
count, but does not divide that budget into fixed parent pools. Isolated execution
retains its equal-slot divisibility requirement. Sector concurrency overrides
apply only to shared execution.

The saved schedule is authenticated structurally before checking the replacement
schedule against the current process affinity. This permits moving to a different
CPU allocation. Changed resources are atomically saved under the campaign writer
lock when `--start` actually admits the run. A metadata-only preview does not
change files, and its printed resume command retains the requested overrides.
Subsequent resumes without overrides reuse the last admitted choices.

The frozen solver executable, input hashes, mathematical policies, and native
payload export permissions remain unchanged. RAM is a guard allowance, not a host
reservation; changing its operational ceiling does not silently rewrite native
codec permissions. Completed sectors are reused after native validation; a sector
interrupted before its checkpoint must restart. Changing the execution mode or
mathematical generation settings still requires a fresh prepared campaign.

## Dashboard independent of the frozen solver

`rustred-preparation-monitor` is a small, read-only native entry point built from
the same Ratatui presentation module as the full CLI. It needs no Symbolica
initialization. It can be updated independently of the campaign's frozen solver.

The launcher prefers `target/release/rustred-preparation-monitor`, then the local
full CLI, then the campaign solver; `--dashboard-executable` selects explicitly.
A missing/failed viewer falls back to plain progress without stopping generation.

On stop, failure, or completion, the interactive viewer leaves the same coloured
table in primary-terminal scrollback after restoring the alternate screen. It
does not replace that final frame with the old one-line/plain summary or issue
cursor-position queries. `NO_COLOR`, piped output and JSON retain their existing
noninteractive behavior. Stopped jobs show zero active workers and preserve last
native counts as historical observations, not live CPU activity or closure proof.

## October 19 failure and validation

The failed `scalar-r0-shared-112w` attempt recorded a native SIGKILL (`exit_-9`),
not an orderly process-RSS guard stop. Sampled peak process-tree RSS was about
506.7 GiB, below its 530.9 GiB soft threshold; host available memory at exit was
about 11.8 GiB. This is consistent with host memory exhaustion or an unsampled
burst. Kernel logs were inaccessible, so an OOM-killer attribution is not proved.
The three partial parent checkpoint directories contain 37 + 17 + 2 = 56 sector
shards; none of the fourteen complete parent bundles was published. No production
restart or checkpoint modification was performed for this change.

Validation:

- 68 steering, scheduler, pipeline, real-native and PTY tests pass.
- Real native controls resume the same checkpoints with 2 → 1 → 4 workers,
  different CPU sets and parent jobs, without solving their completed sectors again.
- A copied 21-coordinate six-tadpole checkpoint resumed with four workers in
  39.922 s, reused one sector, solved zero new sectors and wrote byte-identical
  output. This is a plumbing control, not a coupled six-loop closure result.
- Read-only PTY replay of the actual failed snapshot retains the sector counts,
  the full `41976r×203032c` frame size, the final table and stop reason. Its source
  snapshot hash is unchanged. The rendered terminal image was visually reviewed.

Local evidence: `TMP/generation-resume-ui-20261019/`. The optimized viewer is
frozen at `TMP/releases/20261019-generation-resume-dashboard/rustred-preparation-monitor`,
SHA256 `573f4d453cf383905d4d3a49d55cc31ef109ca3b121141d0a34bdd2d15d77793`.
Evidence, binaries and production outputs are not committed.

## Example: lower-concurrency resume of the existing six-loop run

```bash
cd /shared/codex-2/rustred
nix develop --command python -B six_loop_study/generate.py \
  --directory "$PWD/six_loop_study/runs/scalar-r0-shared-112w" \
  --resume --start \
  --workers 32 --cpus 192-223 --jobs 14 --max-active-sectors 8 \
  --max-memory-bytes 600000000000 \
  --dashboard-executable "$PWD/TMP/releases/20261019-generation-resume-dashboard/rustred-preparation-monitor"
```

Omit `--start` for a nonmutating preview. This keeps all fourteen parents eligible
but admits at most eight sectors concurrently into the shared 32-worker pool.
The cap, rather than merely reducing parent jobs, directly limits concurrently
resident sector solves. These are conservative example settings, not a measured
optimal choice or a guarantee that any individual large frame fits. No arbitrary
upper RAM ceiling, host reserve, runtime deadline or new algebraic restriction is
introduced. The user decides whether and when to launch.
