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
transient harness also retained full task payloads, which contributes to its
RSS trend; production memory behavior cannot be inferred from it. The plan
counts are restart telemetry across changing ledger revisions, not counts of
classes exhausted at one stable snapshot.

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

## Minimal production coordinator

The audited production seam should remain topology-neutral and compact:

1. A pure class schedule emits only `(r,d,c,profile)` descriptors and builds a
   plan on demand against the current exact complement.
2. A campaign epoch is keyed by opaque owner-ledger snapshot identity,
   immutable plan identity, a fresh nonce, and the full typed search
   configuration. Configuration includes family/context/source and
   predecessor authority, sector and ordering, boundary profile, source
   radius, sampling policy, and resource limits. Worker count is telemetry,
   not semantic identity.
3. Workers may evaluate immutable tickets concurrently, but results commit in
   canonical class/task/probe order. Any owner-set mutation discards all later
   results and replans from the new exact revision.
4. The coordinator retains compact counters plus a bounded out-of-order
   window, consuming task reports rather than retaining their replay payloads.
   Its target retained state is `O(number of classes + worker window)`, not
   `O(number of tasks)`.
5. Proposal evaluation and serial owner application remain separate. Typed
   stops distinguish `CompilerClosed`, `OwnerSetChanged`, `NeedsRefinement`,
   `Failed`, and `ExhaustedAtConfig`; exhaustion has no closure authority.

An honest `ExhaustedAtConfig` requires every task and probe in the declared
program to finish at one unchanged opaque snapshot, with no budget,
rejection, or exact-lift error. The next durable gate is therefore a bounded
coordinator with compact retention and stable-snapshot exhaustion accounting,
followed by a reproduced K6 regression. Radius-one or richer source
neighborhoods are justified only after the corresponding smaller program has
been exhausted honestly; they must not be inferred from a report-cap stop.
