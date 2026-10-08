# Saved-campaign publication: optional deep verification

## Purpose and assurance

Ordinary publication should not repeat an independent campaign proof whenever
the starting rank is increased. The new default extracts a trusted saved-scope
inventory; `--deep-verification` retains the independent geometry and successor
reinspection. A modular numerical check is not substituted for an exact check.
Rather, the optional checks are explicitly *not performed* in the fast mode.

Both modes authenticate the captured checkpoint files and their exact
request/owner bindings. The fast mode checks record structure and completion
of the saved dependency graph, then enumerates the rules and terminal keys
actually classified in recorded Apply inspections. The graph's edges are
trusted in this mode. It never replaces encountered terminals by every residual
declared in an installed program. It reports `TRUSTED_SAVED_SCOPE`, not `PASS`,
and `independently_verified: false`.

The existing deep verifier proves coinductive dependency coverage: sealed cycles
can count as closed. Neither mode establishes strict descent, termination of
coefficient back-substitution, unrestricted family closure, master independence,
or numerical master values. The published starting scope remains explicit.

## What changed

- Deduplicate identical recorded Apply scopes using a digest index **and full
  structural equality**. Hash collisions never merge distinct scopes.
- Match successful Apply scopes without constructing RHS successor domains or
  replaying Route inspections. Partial and G2 records use their actual inspected
  residual, not the original larger domain. Abandoned uninspected records are
  excluded. Failed/interrupted records retain the full local prefix replay so
  later, never-visited matches cannot be invented.
- Accumulate identities in worker-local sets/maps and merge deterministically,
  retaining multiplicities and resource limits without a lock per classification.
- Hash checkpoint buffer fills, not each tiny decoded integer. Track consumed
  bytes separately from prefetched bytes; unread tails cannot pass validation.
- Read the native record projection directly, removing a serialize-to-JSON-text
  and parse-back round trip. Framing, record counts and EOF authentication remain.
- Decode compact domain images into checked stack buffers instead of allocating
  two temporary coordinate vectors per saved domain.
- Avoid building the reverse graph when every saved record is sealed. This is
  the same coinductive closure result, not a new mathematical assumption.
- In the optional exact union checker, cache integer interval sums, borrow
  owner masks, apply/undo individual constraints in place, and allocate only
  nonempty split pieces. Predicate answers, traversal order and budget outcomes
  are unchanged.
- Emit loading, structural-check and census progress. Inspection distinguishes
  inventory completion from independent verification.

The old CP6 format lacks a saved rule/terminal identity census, so publication
still reads its graph/records, prepares the owners, and performs exact matching.
This change is not an incremental checkpoint-inventory architecture and does
not promise constant-time publication as the campaign grows.

## Existing checkpoint widths

Testing found a pre-existing mismatch between authenticated on-disk arity and
the current executable's padded storage capacity. In particular, THE_ONE stores
15 slots while the new dispatch uses capacity 16. G2 residual bytes must be
decoded at their original width, then compared using checked zero padding.
The fix preserves the original-width record binding and rejects nonzero/open
omitted coordinates or owner bits on nonexistent wire axes. It does not change
the checkpoint or require regenerating rules. This covers cold publication;
the separate runtime `AnchorView` resume-width issue is outside this change.

Two more capacity-mode gaps were exposed by actual cold controls and repaired:
partial overlays now restore their physical owner/root/cases/order with the
existing checked padding machinery, retaining all source replay and digest
checks; native Route pinches no longer reopen nonexistent storage axes as
unbounded numerator directions. These are input/geometry fixes, not weakened
verifier comparisons. The old failed control receipts are preserved as failures.

The real Python lifecycle also found that a verification-only upgrade reset a
completed finite-search cursor by unconditionally calling `extend`. Exact
same-inventory/same-depth imports now retain that cursor unchanged. Genuine
changes of inventory or depth still use the original extension operation.

## Measurements and validation

THE_ONE's earlier full R≤4,D≤9 publication completed with 335/335 queried
domains verified, 938 raw terminals, 6,851 encountered rules and no violations.
Its saved graph contains 60,115,005 domains and 596,470,375 edges. Full publication
took 14,135 seconds: checkpoint loading 1,347 s, preparation 88 s, structural
checks 10,332 s, and native reinspection 2,286 s. A 15-second sampling profile
identified interval-geometry work and allocation/copying as the main serial cost.
These are measurements of that earlier run, not new-path projections.

Four alternating matched single-core pairs, using actual saved anchor regions:

| Exact union predicate workload | Before | After | Speedup |
|---|---:|---:|---:|
| 49 four-loop regions, 2,000 repetitions each | 1.03–1.05 s | 0.290–0.293 s | 3.54–3.58× |
| First 2,000 THE_ONE anchor regions, 100 repetitions each | 10.28–10.46 s | 2.413–2.417 s | 4.26–4.33× |

These are warm **predicate** timings, not whole-publication speedups or a random
five-loop sample. Eight optimized tests compare the old/new recursion including
remaining region budgets, exhaust small integer grids, and check cached-state
mutation/undo. Implementation and independent audit were separate.

Read-only evidence, binaries, exact commands and receipts are under
`TMP/fast-publication-20261008/`; they are not committed. The compatible frozen
baseline is `9e6c0e9f4c6aa9b51062c213a168fac5ae4c1d2a50454eeab2e06f4c337fb327`.
Its complete four-loop control takes 6.359 s (28 raw →20 normalized keys);
the five-loop scalar R=0,D≤9 control takes 82.730 s (196→196 keys), of which
72.784 s is owner preparation. Host contention is not controlled; comparisons
must report full timing boundaries and should not extrapolate the geometry
speedup to preparation-dominated cases.

The final release control uses the same executable for fast/deep modes
(`a28f294bf7cc88a9a0804a07cfc5681f4e146f424f968fc7144b464b8d908797`),
four workers on physical CPUs 32–35, and nested pools limited to one. Compilation
is excluded. On the saved four-loop control, fast publication takes **3.667 s**
versus **15.174 s** with deep replay. Both enumerate exactly 429 rule identities,
28 raw terminal identities, 8,804 rule classification events and 85 terminal
events. The normalized output is 20 keys; finite native payload (9,053 bytes)
and collection payload (5,964 bytes) are byte-identical between modes. These
are single completed controls, not a statistical performance guarantee.

The corrected five-loop scalar fast control also completes: 141.805 s,
196 raw/normalized keys. Owner preparation takes 127.240 s; inventory matching
4.444 s, and saved-graph checks 0.007 s. This is slower than the older physical-
arity baseline overall: the new capacity-dispatch build and changed preparation
cost must not be disguised as an across-the-board speedup. The useful comparison
for the optional verification switch is fast versus deep in the same build.
That same-build deep scalar control completes in 162.047 s, with preparation
131.356 s and reinspection 21.425 s. Its 1,180 rule identities, 196 terminal
identities, 17,592 rule events, 564 terminal events, finite native payload
(27,155 bytes) and collection payload (24,324 bytes) match fast mode exactly.

| Saved publication control, same new build | Fast wall / CPU | Deep wall / CPU | Fast / deep peak RSS |
|---|---:|---:|---:|
| Combined four-loop control | 3.667 / 9.09 s | 15.174 / 44.79 s | 144 / 186 MiB |
| Five-loop scalar R=0,D≤9 | 141.805 / 266.31 s | 162.047 / 339.66 s | 7.21 / 7.28 GiB |

CPU time is user plus system. Preparation dominates the small scalar case;
the four-loop input has 26,025 saved domains, versus 33,539 in the scalar case.
These are publication controls, not IBP-generation or master-refinement times.

The actual Python workflow passes fast publication, refinement, deep assurance
upgrade, inspection, repeated-refinement no-op, fast-after-deep no-downgrade,
and genuine rank-2/D≤5 extension. Assurance upgrades preserve the exact native
and collection bytes and completed finite-work cursor. All source checkpoints
remain unchanged. Evidence: `workflow-fixed-validation.json` and
`four-loop-fast-deep-comparison-fixed-fixed.json` in the evidence directory.

Focused native tests pass for 49 master/public-workflow cases, six cold-overlay
cases and three physical-capacity Route controls. Python passes 134 campaign/
dashboard tests and 15 saved-campaign command tests. The broad capacity-mode verifier filter has
67 passing tests, one ignored, and eight known failures in optional **CP5
result-file binding**: physical-width and padded JSON produce different raw
digests. The strict binding checks were not weakened. Normal publication does
not supply this optional result file, and the real deep publication control
passes. This separate representation issue remains deferred, not reported as
a passing full suite.

Formatting is checked for every changed Rust source. Workspace-wide formatting
also encounters pre-existing unrelated differences and a missing research
module (`tools/research/rule_optimizer/routed_cancellation/record.rs`); unrelated
code is left unchanged rather than claiming that broader check passes.

## Full-size diagnostic and context-cache repair

The first corrected THE_ONE fast run exposed a separate bottleneck after
checkpoint loading/preparation: only 64,783 of 13,105,672 unique Apply scopes
were processed in 305.864 s. It was deliberately interrupted with a clean scratch
checkpoint; **this is not a successful publication timing**. Sixteen workers
were fully busy. A ten-second, 99 Hz user-CPU profile collected 14,190 samples
without losses: 95.30% lay in `algebra::thread_owned::clone_thread_owned`.
Within that function, 36.15% was linear pointer lookup and 55.92% was full-cache
cleanup scanning. The earlier small controls did not expose this growth.

The old cache treated already-local variable maps as new sources, allowing
retained context chains. The repair indexes both source and local identities
with a thread-local hash map. Each key has a Weak reference to its exact
allocation to prevent pointer-reuse errors. Recognized local polynomials reuse
their own context; only eight queued entries are examined per cache miss.
Expired local aliases are cleaned independently of their original source.
All polynomial copying and zero construction still use Symbolica's existing
`clone_with_context_of` and `zero_with_new_context`/`zero_with_capacity` APIs.
No coefficients, variable ordering, algebraic identity or rule choice changes.

Seven optimized tests cover idempotent nested copies/zeros, source eviction,
2,048 live distinct maps, 10,000 expired-map churn steps, Weak ownership and
cross-thread isolation. Independent implementation and lifetime/code audit passed.
Three old/new microbenchmark pairs using the same optimized harness give:

| Context-cache workload | Original cache | Indexed/idempotent cache |
|---|---:|---:|
| 8,192 nested copies | 76.3–78.0 ms; 8,192 contexts | 0.380–0.383 ms; one context |
| 65,536 copies over 4,096 live source maps | 111–120 ms | 7.25–9.55 ms |

These are small cache microbenchmarks with old-first ordering, not whole-campaign
speedups. Evidence: `TMP/thread-owned-cache-20261008.KeyDLG/`. The same-build
publication controls above precede this additional cache fix; final real-run
measurements must be reported separately, without reusing those numbers as
measurements of the repaired cache.

The repaired optimized executable is SHA256
`0dd497fb2585a0fa7dfdd50f3c37f8457277bf443edfd2e41210024b970e8748`
(`target/release/rustred`; scratch copy `rustred-cache-delivery`). Build time
5m15s, excluded. The four-loop fast control completes in 2.429 s (inventory
0.506 s, previously 1.907 s); deep completes in 8.158 s. Scalar five-loop fast
completes in 137.128 s (inventory 0.925 s, previously 4.444 s), still dominated
by 127.514 s preparation. Both fast controls and the four-loop deep control
produce exactly the prior native/collection payloads and encountered/installed
inventories. This distinguishes a measured census improvement from unrelated
setup time. Scalar deep also passes in 137.481 s (preparation 125.330 s,
reinspection 3.003 s), so scalar total fast/deep time is essentially tied in
this measurement. All four control outputs match the preceding executable
exactly. The fresh full Python lifecycle passes again; initial, refined,
deep-upgraded and extended native/collection payloads match the preceding
lifecycle byte-for-byte. Evidence: `matched-controls-summary-cache.json`,
`workflow-cache-validation.json`, and the cache comparison receipts in the
publication evidence directory. The corrected full-size test completed; its
result is recorded below.

The corrected full-size census received one independent ten-second, 99 Hz
CPU profile on CPUs 96–111 (16 workers): 13,938 samples, no lost samples.
`clone_thread_owned` fell from 95.30% to 6.21% of sampled CPU; bounded cache
cleanup was 3.69%, and domain-geometry projection was the largest remaining
symbol at 19.56%. Early measured ten-second intervals processed 15,363 then
18,494 scopes/s. Sampling overhead was not measured separately. This confirms
the cache hotspot was removed, but these early intervals alone establish
neither full publication time nor final inventory equality. Evidence:
`the-one-cache-census.profile-summary.json` and its flat sampling report.

The full-size scratch command uses the existing completed generation-16
checkpoint read-only; publication output is separate from the campaign:

```bash
# Run inside the repository's Nix development shell, with the license inherited.
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1
export RUST_MIN_STACK=67108864
taskset -c 96-111 time -v \
  -o TMP/fast-publication-20261008/the-one-cache.time \
  TMP/fast-publication-20261008/rustred-cache-delivery walk-publish \
  --command campaigns/five-loop-rank-ladder-THE-ONE/runs/20261008T064842.577006Z/request.json \
  --checkpoint campaigns/five-loop-rank-ladder-THE-ONE/checkpoints/main \
  --directory TMP/fast-publication-20261008/the-one-fast \
  --resume --threads 16 \
  --events TMP/fast-publication-20261008/the-one-cache.events.jsonl
```

Here `--resume` continues only the scratch publication phase, not campaign
generation. Its previously interrupted census was not saved incrementally and
is recomputed. The older deep measurement used 32 workers; this new 16-worker
measurement is an operational comparison, not a controlled paired speedup.

## Completed THE_ONE publication

The final run exited successfully and published the full saved R≤4,D≤9 scope.
It took **25m08.13s** wall time, **8,685.22 s** CPU and **50.39 GiB** peak RSS.
No swapping was recorded for this process. Sixteen census workers were pinned
to CPUs 96–111. Compilation is excluded; files were already in the host page
cache (zero filesystem input blocks reported). The shared host's load average
was approximately 80, so this is not an isolated benchmark.

| Phase | Time |
|---|---:|
| Checkpoint loading/authentication | 795.710 s |
| Owner preparation | 138.679 s |
| Saved graph readiness checks | 26.155 s |
| Exact encountered inventory census | 451.528 s |
| Other setup, teardown and publication | 96.058 s |
| **Whole process** | **1,508.130 s** |

The census processed all 13,105,672 unique Apply scopes, skipping 33,504,137
Route records and 6,917,731 full-cover records. The old deep publication took
14,135 s with 32 workers, or about 9.37 times this observed wall time. This
combines the intentionally lighter assurance mode and implementation changes;
it is **not** an equal-work/equal-resource speedup or a claim that publication
is constant-time. Peak memory is higher than the earlier roughly 38 GiB run.
Loading remains the largest cost; no online incremental census was added.

Independent output comparison confirms:

- Exactly the same 938 raw terminal keys, normalization payload and seed set.
- The same 6,851 encountered-rule count, 46,447,607 rule classification events,
  170,914 terminal events and zero zero-sector events.
- Identical installed inventory and family/program/query/amendment bindings.
- All native `.rrbin` bytes outside the serialized seed vector are identical.
  That vector differs only in order: the original carried append-ordered seeds
  from preceding stages, whereas the new scratch publication starts sorted.

The old artifact does not retain individual encountered rule IDs, so the
full-size rule comparison is count-based, not an exact-ID equality claim.
The small four-loop controls above do compare full rule pages. The scratch
binary-prefix decoder used for terminal/seed comparison was independently
audited against the Rust codec and a native four-loop page; it is not a new
algebra authenticator. The publication's normal native writer and checkpoint
authentication still ran. Exact output comparison is documented in
`the-one-original-fast-comparison.json` beside `the-one-cache.time` and events.

Assurance correctly reports `TRUSTED_SAVED_SCOPE` and
`independently_verified: false`; the older deep artifact reports `PASS`.
Both retain their finite starting scope, and neither establishes master
independence or unrestricted family closure. The user's production artifact,
checkpoint and running refinement were not replaced or interrupted.

## Usage

```bash
# Default: publish the completed recorded scope without deep reinspection.
python -B examples/python/saved_campaign.py publish --campaign "$CAMPAIGN"

# Optional stronger audit, including upgrading an existing publication.
python -B examples/python/saved_campaign.py publish --campaign "$CAMPAIGN" \
  --deep-verification

# Explicitly return the remembered preference to fast publication.
python -B examples/python/saved_campaign.py extend --campaign "$CAMPAIGN" \
  --rank 5 --no-deep-verification

python -B examples/python/saved_campaign.py inspect --campaign "$CAMPAIGN"
```

The rank in the example is a user choice, not an automatically enlarged scope.
Existing publications remain inspectable throughout new work. Verification-mode
changes use distinct publication phases; native resume cannot silently change
its assurance mode. Deep upgrades preserve previous finite master substitutions
and collection state. Explicit refinement inherits source assurance and does
not trigger another graph replay.
