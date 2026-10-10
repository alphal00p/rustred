# Rank-6 checkpoint restore: diagnosis and repair

## What was slow

THE_ONE's rank-6 extension was spending hours restoring generation 33, before
doing any new rank-6 domain work. The saved state contains **139,772,626 domains,
1,405,075,888 dependency edges, and 22,659,469 anchor records**. Its authenticated
payload is about 98 GB, including about 73 GB of sealed record segments.

The campaign retained its original frozen solver (`54302272…`). Updating the
checkout or publication executable had not changed that solver. A read-only
10-second, 99 Hz CPU profile of PID 2011971 collected 901 samples without loss:
36.85% in lattice `Region::nonempty`, 12.10% in `Region::with`, 13.10% in memory
copying, and substantial allocation/free overhead. A separate stack profile
confirmed exact union-cover validation. The process was busy, principally on
one core, not deadlocked. Its roughly 51 GiB resident set was not approaching
the configured memory ceiling.

Runtime checkpoint restore is **different from optional publication deep
verification**. Turning the latter off does not bypass restore checks.

## Delivered changes

1. Independent exact anchor checks borrow the saved immutable geometry and
   run in a bounded, invocation-local worker group. With W configured workers,
   at most W−1 validation threads plus the coordinator are used; W=1 validates
   inline. Small dynamically assigned chunks balance different cover costs.
   The lowest failing anchor is selected deterministically. No full checkpoint
   or Symbolica expression tree is cloned per worker.
2. The existing faster exact lattice kernel is included. No sampled proof,
   omitted coverage check, new CAS primitive, or relaxation of closure is used.
3. Native restore events expose stages, completed/total counts and worker cap
   to the existing dashboard/JSON stream. These are **checkpoint work counts,
   not newly generated domains**. Some decoding/indexing stages still report
   only stage boundaries and can remain serial.
4. Cooperative cancellation is checked between exact anchor checks and at
   restore boundaries. A private typed cancellation marker distinguishes a
   proven pre-session-adoption stop from an unrelated interrupted I/O error.
   Operational stops do not silently trigger a previous-generation fallback.
   Cancellation before adoption leaves the checkpoint and writer session
   unchanged; it does not pretend to have saved a new generation.
5. Old physical-width CP6 checkpoints can restore into the current dispatch
   capacity. G2 pieces and exact-record comparisons accept only authenticated
   compatible widths and fixed-zero padding. Original record bytes/digests
   remain intact. Saved dominant-orthant history is validated in its original
   wire width; the runtime shortcut is rebuilt using its unchanged predicate.
   Ordinary exact containment remains the fallback.
6. The existing guarded executable-upgrade path now recognizes CP6 as well as
   CP5. CP6 uses its nested native compatibility probe. Liveness checks,
   locking, frozen inputs, atomic publication and rollback remain in place;
   upgrade history distinguishes checkpoint formats.
7. Lookup reconstruction uses a restore-only insertion path that does not scan
   existing entries against an empty retirement set. This removes redundant,
   potentially quadratic bucket traversal. The fresh-index/no-deletion restore
   invariant is tested against the old path, including full layout, membership,
   lookups and orthant state. Normal index retirement and its empty-row cleanup
   remain unchanged. Existing test-only counters establish zero retirement
   scans; no timing threshold or new runtime instrumentation is needed.

No starting rank/D scope, rule choice, numerical master, or production campaign
was modified. Production was observed read-only throughout.

## Validation and measurement boundaries

Release builds use `--release --locked --features capacity-dispatch`.
Compilation is separate from runtime. Evidence and private checkpoint copies
are under `TMP/rank6-restore-diagnosis-20261010/` and are not committed.

Validation includes exact corruption rejection, serial/parallel equivalence,
worker bounds, deterministic failures, cancellation without fallback/session
adoption, physical-width restore/resave, mixed-width record history, dashboard
transport and the guarded CP6 upgrade path. Independent source audit was
performed by `rank6_restore_audit`, separate from `restore_parallel_fix` and
`restore_capacity_fix` implementation.

Real controls use copies of a four-loop checkpoint (26,025 domains, 49 anchors)
and a scalar five-loop checkpoint (33,539 domains, 1,673 anchors), with 1, 4 and
32 workers. Their acceptance criterion is successful restore/resave, identical
scheduled/native counts and record/edge digests, zero pending/frontier counts,
and unchanged source snapshots. CLI exit 4 / `status: incomplete` is expected
for these checkpoint-only reports without independent family certification;
it must not be reported as either a restore failure or a new closure proof.

The measured boundary is the **whole child process**, including owner
preparation, restore, report/save and teardown. Five-loop owner preparation
dominates these small controls. They are correctness controls, not a reliable
benchmark of 22.7 million anchor checks. CPU affinity was 32–63, away from
production's 64–95; build activity and shared-host contention were present.

An initial prototype (`fe376f07…`) exposed the four-loop orthant-width issue;
all three worker configurations failed closed. That failure and its fix are
retained in the evidence rather than hidden. The intermediate binary
`596b01aeb6aa48fd55b0f843dfda9cc935db5d1e7abd1f681a753cb6a567b5eb`
then passed all six copied-checkpoint controls. Original versus resaved metadata
matched structurally in every field. Resaved manifests matched across worker
budgets; old sealed records remained byte-identical. Whole checkpoint bytes are
not promised identical because physical-width sections are resaved at the
current dispatch width.

One pre-existing escrow telemetry timing assertion failed on the first native
test pass, then passed five isolated repeats and the full 148-test rerun with
no assertion changes. Independent audit found a plausible stale-diagnostics
race in that test's cancellation timing; the negative receipt is retained.

The first full-size prototype probe stopped at its 900-second diagnostic limit
and required killing only its owned private child after the 60-second grace
period (967.6 s total). It was still in lookup reconstruction. No anchor stage
was reached, no result was published, and both private authority files and the
production source remained unchanged. This establishes neither anchor
throughput nor whole-restore completion. It motivated the narrowly scoped
lookup improvement above.

The second private pre-index-fix probe confirmed the lookup diagnosis with a
10-second, 99 Hz profile: 978 samples, zero loss. Inclusive samples were 96.46%
in lookup rebuild, 76.05% in `index_survivor`, 69.09% in `index_prepared`, and
30.72% in `Block::reverse`; `Envelope::may_be_contained` accounted for 13.86%
self samples. These percentages overlap. They identify the redundant reverse
retirement scan, not a new rule-generation bottleneck.

The final six-run preservation matrix passed, independently audited. Times are
single observations on the shared host, not repeated speedup measurements:

| Checkpoint | Workers | Whole process, old → final (s) | Native restore phase, old → final (s) |
|---|---:|---:|---:|
| Four-loop | 1 | 2.015 → 2.015 | 0.167 → 0.165 |
| Four-loop | 4 | 1.662 → 2.014 | 0.135 → 0.271 |
| Four-loop | 32 | 1.362 → 1.662 | 0.135 → 0.264 |
| Scalar five-loop | 1 | 104.587 → 167.009 | 0.310 → 0.230 |
| Scalar five-loop | 4 | 104.499 → 185.902 | 0.310 → 0.338 |
| Scalar five-loop | 32 | 86.525 → 148.386 | 0.303 → 0.325 |

Five-loop owner preparation takes 161.291 / 179.331 / 142.363 s in the final
1/4/32-worker runs. These numbers **do not demonstrate whole-process speedup**;
parallel setup can also dominate tiny restores. Native phase accounting is
not a pure file-reader microbenchmark. The old frozen binary differs from the
current capacity-dispatch baseline as well as from this repair. All final
resaved manifests are byte-identical to the preceding `596b01…` controls, which
isolates the restore-only insertion change as preserving serialized authority.

Final receipts: `native-resume-benchmarks/prepared-2gvif66t/measurements.json`
and per-run `result.json`, `snapshot.json`, and `executed-command.json` under
the evidence root above. No source checkpoint was changed.

The delivery executable includes both parallel validation and the restore-only
index path:

```text
SHA256 814745116dd52749a6eb2272774503afc762d806823924cf4c35b8e732745ce7
```

### Full-size final probe

On the private generation-33 snapshot, lookup reconstruction completed in
approximately **104 seconds** (start about 560.565 s from launch, finish between
664.073 and 665.072 s). The pre-index-fix comparison was still in lookup after
more than 15 minutes. These are concurrent shared-host observations, not a
repeated paired benchmark or a whole-restore speedup ratio.

Exact anchor validation began about 697.694 s from launch. An independently
observed 8.02-second interval used 247.21 CPU-seconds, or **30.82 active cores**.
Completed anchors rose from 3,830,528 to 4,533,437 over an 8.007-second progress
interval: approximately **87,784 anchors/s**. Both endpoints were still in
`anchor_coverage`, without cancellation. RSS was stable at 62.20 GB. This
demonstrates actual use of the 31 configured proof workers, not a lifetime
`ps` percentage. Anchor difficulty varies; no full-restore ETA follows from
this sample, and later record validation/loading was not timed to completion.

Final focused validation: **150 optimized restore tests**, **5 cold G2
capacity tests**, **8 lattice tests**, **1 generic empty-row retirement
regression**, and **182 Python monitoring/upgrade/steering tests** pass.
The six actual checkpoint resume/resave controls above also pass. The final
CP6 upgrade preview matches generation 33 and leaves `applied: false`,
`launch_requested: false`; it correctly reports the existing live invocation.

## Restart procedure

The release executable is frozen at:

```text
/common/dev/rustred/TMP/rank6-restore-diagnosis-20261010/rustred-restore-fast-index-20261010
```

After stopping the existing invocation **and confirming it has exited**, resume
its pending rank-6 request using the current steering code:

```bash
cd /common/dev/rustred
nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign /common/dev/rustred/campaigns/five-loop-rank-ladder-THE-ONE \
  --resume \
  --upgrade-executable /common/dev/rustred/TMP/rank6-restore-diagnosis-20261010/rustred-restore-fast-index-20261010
```

Do not issue another `extend --rank 6`: that amendment already exists. The
launcher refuses to upgrade a live invocation. The old frozen restore has
coarse cancellation, so Ctrl+C may not stop it promptly; do not run a second
writer while it remains alive. This work did not stop/restart it automatically.
The new solver must still read and validate the saved state: “resume” is not
an instant transition, and no full-size restore ETA is asserted here.

## Remaining limits

- Record decoding, lookup reconstruction and some other restore stages remain
  serial. Parallel anchor checks do not imply an end-to-end 32× speedup.
- Cancellation cannot interrupt the middle of an individual exact cover or a
  decoder that only reports stage boundaries.
- Independent audit identified a separate pre-existing cold-verifier mismatch
  for **unresolved input frontiers** when physical arity differs from dispatch
  capacity (`verify_closure/epoch_checkpoint.rs`, `source_frontier_matches`).
  THE_ONE and the supplied resume controls have no such frontiers. It does not
  block this repair; cold verification of that other case needs its own fix.
