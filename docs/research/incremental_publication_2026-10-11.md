# Repeated publication: ordered decoding and raw-census reuse

## Why rank extensions were still expensive

The optional independent verifier was already disabled. Ordinary publication
still decoded and authenticated every recorded inspection, checked the saved
graph and query scope, and enumerated encountered Apply classifications. Reusing
the preceding artifact happened only after that work. THE_ONE's rank-6 result
has 140,465,585 domains, compared with 139,772,626 in its predecessor, yet both
paid for nearly the same complete history.

The unmodified running rank-6 publisher completed successfully in 2,916.752 s
(48m37s). It used `--no-deep-verification`, 32 threads, frozen executable
`814745116dd52749a6eb2272774503afc762d806823924cf4c35b8e732745ce7`.
All 469 required queries were resolved before publication began. This is scoped
saved-graph completion, not an unrestricted family closure or minimal-basis claim.
Its production directory and processes were not modified by this development.
Recorded rank-6 stages were: load 2,005.189 s, owner preparation 136.995 s,
saved-graph checks 82.237 s, census 486.885 s. Thus a cached census alone cannot
remove most of that run's cost; record decoding must improve as well.

## Implementation

1. CP6's original authenticated binary reader feeds bounded ordered batches to
   a private projection/decoding pool. Results are applied in record order;
   the coordinator and helpers stay inside the requested worker budget. CP5
   remains serial. The original record projection and digest semantics remain
   unchanged. Record/domain comparisons use stack arrays and compact-domain
   accessors rather than allocating an expanded domain per record.
2. Completed eligible fast inventories can write `inventory-census.bin`, an
   optional binary raw-census aggregate. It stores rule identities with event
   multiplicities, raw terminal keys, event totals and the precise immutable
   CP6 segment-prefix and classifier bindings. It is not a normalized master
   list, a second execution checkpoint, or a closure certificate.
3. Later publication still authenticates and decodes the current files and
   checks record/domain/G2 parity, graph completion and the current request.
   With a compatible cache, it classifies only records beyond the cached
   prefix and merges their exact raw inventory. Failed/partial publication,
   unsupported rescue histories or changed classifiers cannot authorize reuse.
   Missing, corrupt or incompatible cache state selects the full census.
4. Refinement copies the valid optional raw census independently of its master
   basis transformations. A completed census receipt is durable before native
   master preparation so a later pause need not discard that census. Cache
   write/copy failures produce a warning, not a false publication or a new
   mandatory authority.

Deep verification neither skips proof checks using this cache nor emits a
fast-only census cache. A cache hit does not report a global number of distinct
Apply scopes: prefix and suffix distinct counts can overlap. The report instead
gives reused records, reused Apply records and newly classified distinct scopes.

The immutable record prefix need not be sorted by node ID. Cache emission
requires that its fully checked records publish every node ID exactly once;
the old prefix therefore contains exactly the old ID set. Full current parity
checks bind those immutable records to the current coordinate images and G2
ledger. This is why no additional per-domain JSON hash or large cache is needed.

No computer-algebra operation or coefficient algorithm was added or replaced.

## Preliminary release measurements

First candidate executable: `2bf1f0f7362b0ceee2afb03216cfaa270fabda38c6f6939df6a0f82ae4d699cf`.
Optimized build took 3m13s, excluded from timings. Four-loop controls used the
same checkpoint, request, four workers and CPUs 40–43. Other work continued on
disjoint CPUs; these are shared-host measurements, not isolated benchmarks.

| Four-loop publication | Old full | New bootstrap | New cache hit |
|---|---:|---:|---:|
| Whole process | 2.543 s | 2.278 s | 1.879 s |
| Load | 0.239 s | 0.145 s | 0.148 s |
| Census | 0.406 s | 0.413 s | 0.110 s |

The cache reused all 26,025 records (1,517 Apply records), required no matcher
jobs, and occupied 10,243 bytes. Native finite-session and collection payloads
were byte-identical in all three outputs, and all encountered counts and
assurance fields agreed. The source checkpoint was unchanged. This is a
same-scope reuse control, not a rank-extension timing or a large-run speedup.

| Scalar five-loop publication | Old full | New bootstrap | New cache hit |
|---|---:|---:|---:|
| Whole process | 138.437 s | 133.776 s | 132.364 s |
| Load | 0.384 s | 0.232 s | 0.230 s |
| Census | 0.912 s | 0.972 s | 0.161 s |

This is the same four-worker protocol on 33,539 saved domains. Native payloads,
counts, assurance and the source checkpoint are unchanged. The 35,874-byte cache
reuses 4,519 Apply records with no new matcher jobs. Owner preparation dominates
(122–128 s), so the difference in whole-process timing is largely noise in an
unmodified stage; no substantial whole-campaign gain is inferred from this case.

### Real-record projection probes

A read-only private segment from THE_ONE contains 628,236 records / 246,655,332
bytes. Forward/reverse probes use the actual binary reader and original
projection/decoding implementation, but exclude domain/edge loading, owner
preparation, graph checks and the census. They are not complete publications.

| Workers | Mean seconds, batch 256 | Mean seconds, batch 4,096 |
|---|---:|---:|
| 4 | 3.597 | 3.506 |
| 16 | 2.335 | 2.209 |
| 32 | 2.982 | 2.570 |

Serial runs ranged from 4.623 to 5.018 s. The 32-worker ABBA pair and both
16-worker pairs favor 4,096 records; four-worker differences are noisy. The
final default is 4,096 records with the existing 16 MiB logical-frame byte bound.
An individual already-allowed oversized frame can cross that bound; this does
not make the batch an unbounded stream or change the input frame limit.
More workers are not automatically faster for this phase.

At the user's request to wind down, the additional full-size accelerated
publication experiment was deliberately **not started**. No full-size completion
or timing is claimed.

## Frozen delivery and final controls

Final executable:
`TMP/publication-incremental-20261011/rustred-publication-fast`, SHA256
`15a4d18124ce94166337585fba1f4a4e426ef3550d2e75e6770c36dc47dd43ad`.
The final optimized build completed in 3m12s, excluded from measurements.

The final four-loop repeat measured old/cold/warm wall times of
2.673 / 2.180 / 1.878 s; census times were 0.508 / 0.413 / 0.111 s.
Complete native and collection payload bytes, encountered counts and assurance
agree; the source pointer is unchanged. Both four-loop repetitions favor the
new implementation, without establishing a full-size campaign speed ratio.

The public Python workflow passes initial publication, inspection, refinement,
optional deep-verification upgrade, no-op repeat refinement and rank extension
from R=1,D=4 to R=2,D=5. Exact refined native/collection bytes survive the deep
upgrade; the extension retains the previously proved finite-feedback rows and
publishes the requested new scope. This is a fresh scratch four-loop campaign,
not a mutation or restart of THE_ONE. Evidence:
`TMP/fast-publication-20261008/workflow-incremental-final-validation.json`.

Final native release tests: **85 passed** (20 inventory, 40 master-reduction,
19 CP6 checkpoint, six record-input tests). The optional real-segment performance
probe is ignored by ordinary test execution and was explicitly run separately.
Tests cover malformed caches, changed bindings, chained extensions, partial/G2
residual scopes, unchanged deep verification, ordered first-error handling,
multiple segments/batches, cancellation and captured-stream authentication.
The new lifecycle regression verifies durable census persistence before native
preparation, cooperative pause/error handling and same-directory resume with
zero repeated matcher jobs and byte-identical final output. It is not an
abrupt-process-kill durability test.

Python steering/phase tests: **74 passed** (15 saved-campaign, 59 phase tests).
Independent implementation and measurement review found no blocking issue.
No production process, artifact, pointer or checkpoint was changed. Unrelated
worktree changes remain excluded from this delivery.

## Evidence and reproduction

Evidence root: `TMP/publication-incremental-20261011/`. Source fixtures and exact
commands are retained in each receipt. `tools/research/publication_incremental_bench.py`
accepts arbitrary `--command`, `--checkpoint`, `--baseline`, `--candidate`,
`--directory`, `--threads` and `--cpus` arguments. It requires fresh output,
compares complete publications, and never changes the source checkpoint.

The normal user commands are unchanged. Use the new optimized publisher for
the next extension. Existing artifacts do not retroactively acquire caches:
the first eligible new publication bootstraps one; later compatible publications
reuse it. Current record loading, graph checks and owner preparation still cost
time even on a cache hit.

For an explicit publisher selection, add
`--executable /common/dev/rustred/TMP/publication-incremental-20261011/rustred-publication-fast`
to the normal `examples/python/saved_campaign.py` command. The main
`target/release/rustred` currently contains the same executable. Existing
completed production artifacts remain valid; no campaign restart or rule
regeneration is required for this publication-only update.
