# K6 exact boundary walk: audited transient results

## Authority and claim boundary

This note is the authoritative record of the audited K6 boundary-walk
measurements captured on 2026-09-01. It records exact progress by the current
proposal-only discovery and owner-admission machinery. It is **not** evidence
of a closing K6 artifact, a completed probe program, or practical scaling to
K21. The measurement driver was a transient harness; its results are retained
here as research evidence rather than as a durable acceptance test.

No oracle support, coefficient, or rule was supplied. Candidate rows were
regenerated from ordinary sources, lifted and replayed exactly, and admitted
only through the existing guard, descent, executable-owner, and exact
`BoxCover` authority boundaries. The exact compiler remained non-closed and
no K6 artifact was published.

## Audited walk

For one immutable owner-ledger snapshot, the walk orders effective residual
dimension `r` from high to low. Within an `r` class it orders parent free
dimension `d` from high to low and uses boundary codimension `c = d - r`.
Pinned-axis subsets, finite-axis assignments, and simplex offsets are all
canonical. Positive-dimensional classes use the degree-zero simplex profile
with positive margin two; `r = 0` uses the typed vertex profile. Any admitted
owner changes the ledger revision, invalidates every sibling task from the old
snapshot, and causes an exact replan.

At the authenticated starting snapshot, revision 9 had 9 owners and 28 exact
uncovered boxes. Its free-dimension histogram for `d = 0..6` was
`[0, 0, 0, 0, 25, 3, 0]`. The nine starting owners were themselves regenerated,
replayed, and admitted exactly without oracle rules, supports, or
coefficients; they were not supplied as trusted input to the boundary walk. A
complete degree-zero schedule at that *unchanged* snapshot would have been:

| effective `r` | parent classes `(d,c)` | boundary faces | tasks |
| ---: | --- | ---: | ---: |
| 4 | `(5,1)`, `(4,0)` | 40 | 103 |
| 3 | `(5,2)`, `(4,1)` | 130 | 382 |
| 2 | `(5,3)`, `(4,2)` | 180 | 558 |
| 1 | `(5,4)`, `(4,3)` | 115 | 367 |
| 0 | `(5,5)`, `(4,4)` | 28 | 91 |
| **total** |  | **493** | **1,501** |

The previously revalidated maximal-interior `(5,0)` bulk class contributes
three additional tasks. The table is a fixed-revision planning census, not a
claim that either transient run completed it: every successful mutation
replaced the snapshot and restarted canonical planning on new geometry.

## Captured measurements

Both measurements used `RAYON_NUM_THREADS=1` and began from the revision-9
snapshot above. The stop in both cases was the report cap.

| report cap | stop | runtime | plans | final revision | owners | boxes | stale siblings | final free-dimension histogram `d=0..6` |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 512 | `ReportCap` | 87.84 s | 86 | 40 | 40 | 84 | 31 | `[0, 0, 0, 65, 17, 2, 0]` |
| 2,048 | `ReportCap` | 379.11 s | 217 | 74 | 74 | 109 | 63 | `[0, 0, 17, 76, 14, 2, 0]` |

| report cap | no nomination | duplicate | strict geometric shrink | changed without geometric shrink | no rebased circuit | incomplete | closed |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 512 | 274 | 207 | 17 | 14 | 0 | 0 | 0 |
| 2,048 | 1,029 | 954 | 28 | 37 | 0 | 0 | 0 |

The owner and revision increases are exact admissions: 31 mutations in the
512-report run and 65 in the 2,048-report run. Increasing box counts do not
contradict exact cover shrinkage; subtracting an admitted owner may fragment a
canonical complement into more boxes. The migration from dimensions four and
five toward dimensions two and three is useful geometric progress, but it is
not a monotone volume measure and does not establish closure.

Neither run reached one unchanged snapshot on which the declared probe
program was exhausted. `ReportCap` is therefore neither exhaustion nor
closure, and stale-sibling counts are expected consequences of transactional
revision invalidation. Only `CompilerClosed` can establish closure. The
transient harness dropped each full task report after extracting its target,
scalar fields, and compact census, but still retained those compact records in
`O(reports)` memory rather than using a bounded window. Its retention model is
therefore not the intended production design; the measurements do not
establish a causal explanation for the observed RSS trend. The plan counts are
restart telemetry across changing ledger revisions, not counts of classes
exhausted at one stable snapshot.

## Durable deterministic checkpoint

Commit `ac68736` preserves a bounded, oracle-disabled regression that rebuilds
the authenticated revision-nine ledger by executing ordinary-source probes,
then performs two independent 80-report boundary walks. Both walks reproduce
the same canonical plan/task transcript, exact owner content keys, and exact
uncovered partition. Each reaches revision 18 with 18 owners and 39 boxes,
with free-dimension histogram `[0, 0, 0, 17, 20, 2, 0]`. The outcome census is
33 no-nomination, 38 duplicate, 6 changed-without-geometric-shrink, and 3
strict-shrink reports, with no incomplete proposal, replay failure, or exact
obstruction. Nine mutations are followed by exact replanning and an
independently checked stale-sibling rejection.

The regression was independently audited and reproduced. Its typed stop is
deliberately `ReportCap`, and its compiler status is deliberately
`Incomplete(NonFinite)`: it proves deterministic exact progress, not closure
or schedule exhaustion. In particular, the test-only stable-sweep telemetry
must not be promoted into a production exhaustion certificate until the stop
policy retains canonical replay-attempt/query-rejection evidence and exact
obstruction/refinement state.

Commit `e12f4a5` independently reproduces the same checkpoint through the
compact production coordinator without retaining the diagnostic transcript.
Its cumulative census has 10 immutable epochs, 20 materialized plans, 80 task
reports, and 135 invalidated plan-suffix tickets. It stops at the typed request
for report 81 against the declared limit 80, at revision 18 and canonical
location `(class=0,r=5,d=5,c=0,task=1)`. Of the 80 declared D37 probes, 47
replayed canonically and 33 completed with modular support that did not lift;
all stalls, rejections, exact-lift errors, canonical query rejections,
incomplete proposals, and exact obstructions are zero. A support-lift miss is
therefore retained as an exact finite-program outcome, not misclassified as
an operational/refinement failure and never promoted to closure evidence.

Commit `34b6243` adds an ignored 256-report checkpoint whose single test
performs two fresh authenticated revision-nine reconstructions. Both agree
exactly and reach revision 29 with 29 owners, 56 uncovered boxes, and
free-dimension histogram `[0, 0, 0, 37, 17, 2, 0]`. Its outcomes are 138
no-proposal, 98 duplicate, 8 changed-without-geometric-shrink, and 12
strict-shrink reports. All incomplete-proposal, scheduler-failure,
canonical-rejection, and exact-obstruction counters remain zero. The typed
stop requests report 257 against limit 256 at
`(revision=29,class=2,r=4,d=4,c=0,task=3)`. This is another deterministic
report-cap prefix, not schedule exhaustion; in particular, two dimension-five
uncovered boxes still remain.

## Audited compact production coordinator

Commit `e873de8` installs the independently audited, topology-neutral
window-one coordinator required by the next measurements. Its public stop
taxonomy separates exact compiler closure, owner-set mutation, actionable
refinement, operational bounds, hard failure, and clean exhaustion of one
declared finite configuration. In particular, `ExhaustedAtConfig` is not a
closure result and cannot be produced from a report cap.

The production seam is compact and transactional:

1. A pure class schedule emits only `(r,d,c,profile)` descriptors and builds a
   plan on demand against the current exact complement.
2. A campaign epoch is keyed by opaque owner-ledger snapshot identity,
   immutable plan identity, a fresh nonce, and its boundary/resource
   configuration. The current coordinator also retains a caller-declared
   campaign label and exact probe cardinality, but does not yet structurally
   own the adapter, source incidence, or probe contents. That remaining gap
   must be closed before `ExhaustedAtConfig` is treated as a production
   certificate. Worker count is telemetry, not semantic identity.
3. Workers may evaluate immutable tickets concurrently, but results commit in
   canonical class/task/probe order. Any owner-set mutation discards all later
   results and replans from the new exact revision.
4. The current serial coordinator consumes and drops each task report after
   retaining checked scalar counters. Its live memory is bounded by the exact
   uncovered partition, the class schedule, the largest materialized class
   plan, and one evaluated task. It retains no completed task history. A later
   parallel executor may replace the single-task window with a bounded
   out-of-order window without changing serial commit semantics.
5. Proposal evaluation and serial owner application remain separate. Typed
   stops distinguish `CompilerClosed`, `OwnerSetChanged`, `NeedsRefinement`,
   `Failed`, and `ExhaustedAtConfig`; exhaustion has no closure authority.

Every task currently declares a nonzero exact probe cardinality. The coordinator checks
that all seven scheduler outcome buckets sum to that cardinality and that
canonical replay attempts agree with both scheduler replay and replay-engine
telemetry. Any owner mutation ends the immutable epoch immediately; stale
siblings are discarded and the next call replans from the new opaque ledger
identity. Before reporting clean exhaustion it revalidates that identity and
requires an exact `Incomplete(NonFinite)` compiler snapshot with a nonfinite
uncovered region, zero missing terminals, and zero guard-incomplete owners.
The copied snapshot remains telemetry rather than publication authority:
artifact sealing must still consume the live exact ledger.

Commit `b377b10` now preflights every fallible compact-census join and every
possible compiled-owner counter state before serial owner application. The
exact ledger itself stages and validates compilation, partition comparison,
and revision advance before replacing live state; after application the
coordinator only selects one prevalidated scalar state. Adversarial tests
force distinct possible-action counter overflows and prove that both the
exact snapshot and opaque ledger identity remain unchanged.

The next production-hardening gate is to replace the caller label, per-call
adapter, and arbitrary probe callback with an owned, topology-neutral,
task-relative modular-probe program and a coordinator-bound ordinary-source
incidence. This needs no new digest or repeated artifact authentication. The
next measured gate is the independent 512-report checkpoint, followed by an
honest unchanged complete zero-offset-seed/degree-zero sweep. Richer source
neighborhoods are justified only after the corresponding smaller program has
been exhausted honestly; they must not be inferred from a report-cap stop.
