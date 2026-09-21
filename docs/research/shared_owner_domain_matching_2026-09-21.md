# Ordered saved-rule matching on five-loop successor ranges

## Outcome and scope

The three examined same-support successor boxes capped at rank 11 have applicable
saved rules. Their parent/source boxes also classify using saved rules once one
bounded inactive coordinate is split into its ten integer values. No IBPs,
owners, routes or terminals were generated, and no positive power was truncated.

This resolves **local ordered guard applicability for these input boxes**, not
RHS specialization/cancellation, descent, further successor coverage or complete
R10 closure. There are three examples here, not an exhaustive test of all 217
above-entry same-support regions, nor the other pinch/activation obligations in
the [complete inventory](shared_owner_domain_inventory_2026-09-21.md).
Independent certification remains deferred.

The new generic Rust service and `owner-domain-match` CLI follow existing batch
priority, fixed coordinates, source conditions, equality guards, excluded
conjunctions and original denominators. Native Symbolica-backed services handle
all polynomial algebra. The query retains its exact inactive-rank simplex;
the saved entry-rank cap never clips intermediate queries. See the
[interface and Python steering example](../shared_owner_domain_matching.md).

## Reproducible workload

Inputs are three unchanged representative programs from the saved five-loop
census, totaling **97,121,035 encoded bytes**, with no routing records because
these are literal local-owner queries. The source/child pairs are exact integer
coordinate translations of three recorded one-step prefilter examples. They
preserve unbounded positive axes, rank caps 10/11 and all original coordinate
bounds. The input boxes do not themselves encode predecessor guards or establish
that an individual RHS contribution survives coefficient cancellation.

All commands use the same frozen optimized CLI through
`examples/python/match_shared_owner_domains.py`, CPU affinity 40, native
inner pools of one, a 32 GiB address-space ceiling, and no elapsed deadline.
There is no concurrent RustRed build or solve. Each command performs a fresh
load; compilation is outside every timing below. These are shared-host
diagnostics, not controlled speedup or IBP-generation benchmarks.

| Query batch | Exact classified queries | Selected-rule / unresolved pieces | Preparation (s) | Matching (s) | Whole command (s) | CPU (s) | Peak RSS (KiB) | Exit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Three source/child boundary-ray pairs | 6/6 | 9 / 0 | 3.091 | 0.138 | 3.42 | 3.39 | 510,504 | 0 |
| Three full source/child prefilter pairs | 5/6 | 13 / 1 | 2.863 | 0.079 | 3.13 | 3.09 | 511,596 | 4 |
| Identical boundary-ray repeat | 6/6 | 9 / 0 | 62.917 | 0.135 | 63.21 | 63.16 | 511,580 | 0 |
| Ten exact slices of unresolved source piece | 10/10 | 27 / 0 | 2.976 | 1.209 | 4.80 | 4.46 | 510,416 | 0 |

The repeated ray result is semantically identical, including every piece and
native counter; only three timing fields differ. Its long preparation interval
is real and unexplained here: 36.82 user and 26.34 system CPU seconds accompany
the 63.21-second wall time. It must not be hidden, attributed to matching, or
described as a demonstrated idle/network delay. Matching itself remains near
0.135 seconds. No overall speedup or robust cold-load latency follows from this
small sample.

Every command reports zero exact gaps and zero invalid source conditions.
Exit 4 on the broad prefilter run correctly reports an unresolved guard, not a
resource failure or missing rule. Later independent queries still classify.

## Which existing rules apply?

Owner masks, rule IDs and coordinate positions below describe external input
data only; none is present as engine dispatch logic. Coordinates are zero-based.

| Owner mask | Recorded source rule | Existing rules on full child image |
| --- | ---: | --- |
| `010011111101011` | 232 | 1 |
| `011011111101001` | 289 | 41 for `n10=0`; 42 for `n10<0` |
| `111000100011101` | 342 | 3 |

The first source example is

```text
[0,1,0,0,1,t,1,1,1,1,-k,1,0,1,1], t>=4, 0<=k<=10.
```

Its recorded shift produces

```text
[0,1,0,-1,1,t-3,1,1,1,1,-k,1,0,1,2].
```

The child has rank `k+1`, and rule 1 applies over its entire queried box. On
the source, `k=0` already classifies under rule 232. With `1<=k<=10`, the first
excluded-conjunction predicate of rule 232 is coupled in the remaining free
indices. The deliberately narrow coordinate matcher returns `Unresolved`
instead of falling through to later rules or inventing an exact gap.

The input-only refinement fixes `k=1,2,...,10`, leaving **all** `t>=4`. These
ten disjoint slices exactly partition the unresolved piece; the rank simplex
and every other bound are unchanged. They are not ten sampled points. Native
guard matching then selects rule 232 on the generic pieces and existing rule
274 at `t=k+2` for `k=2,...,10`. At `k=1`, that exceptional value lies below the
requested `t>=4` domain. All ten slices classify with no unresolved remainder.
This refinement uses integer coordinate enumeration, not a new CAS kernel.

The other source examples demonstrate why priority matters too: the second
source selects rule 289, while the third uses rule 342 for `n1>=2` and later
rule 390 on `n1=1`. The records therefore cannot be treated as if the original
stored rule applied uniformly everywhere in its coarse prefilter.

## Validation and next implementation step

The release gates pass **2,588 core tests** (zero failures, 32 existing ignored)
and **296 application/integration tests** (zero failures). All **16 Python
steering tests** pass. Independent implementation, mathematical and runtime
reviews cover rule priority, original denominator poles, exact rank constraints,
partial results, compact monitoring, input partitioning and measurement scope.

The core remains conservative for coupled predicates; automatic bounded-axis
refinement is **not yet implemented**. The successful ten-slice experiment is
external steering of the existing API. A small next step is to make that
work-budgeted refinement part of the generic shared-domain worklist, choosing
bounded inactive coordinates without enumerating positive powers. Preserve
unresolved unbounded/coupled cases instead of sampling them away.

Then continue successor work through the same immutable owner library, resolving
native RHS zeros/coalescence and routing where needed, and nominate genuine
missing cases to the already available source-search/overlay service. Do not
regenerate owners merely because an intermediate rank exceeds its entry scope.
The complete R10 solve, durable shared worklist and 50-core/500-GB completion
measurement remain unfinished. Terminal minimization, numerical master mapping
and five-loop Vakint integration stay behind that gate.

Local evidence: `TMP/above-rank-owner-queries.RzVS5x/` and
`TMP/above-rank-owner-matches.pJURoX/`. Gate evidence:
`TMP/owner-domain-match-core.T6FY67/` and
`TMP/owner-domain-match-app.K46hnR/`. The frozen CLI SHA-256 is
`a31f9dfbb9a43e100c0929820f103d9c92447cc587d52606da201fb67ced5571`.
These diagnostic results are not shipped IBP artifacts or family certificates.
