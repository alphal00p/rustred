# A1 five-loop stop: an isolated saved-rule coverage gap

## Observed production state

The user-launched campaign `five-loop-a1-epoch-repaired-20260930` stopped on
October1 after49,917,808 native inspections with one committed `NativeFrontier`,
zero native errors and13,808,105 pending obligations. CP6 generation8 saved
successfully with zero abandoned obligations. No production files or process
lifecycle were changed by this investigation.

This is not the earlier P1 `UnionUndecided` failure. The repaired optional
planner handled138 undecided-union and19 lender-budget fallbacks during this
run. Those counters do not identify the new frontier's cause.

## Exact point and meaning

Frontier node64,952,600 is an Apply obligation for owner mask
`000011001001011`. Its local lower and upper bounds coincide:

```text
x = [0,10,0,0,0,1,0,0,0,0,1,0,0,0,0]
```

On active axes the physical propagator index is `a_i = 1 + x_i`; on inactive
axes it is `a_i = -x_i`. Thus the exact physical integral has indices

```text
a = [0,-10,0,0,1,2,0,0,1,0,-1,1,0,1,1].
```

Its negative-index degree (the campaign's numerator-rank convention) is11,
the positive-power sum is7, and their difference is−4. The local-coordinate
sum12 is not numerator rank: one unit is positive propagator excess.

The recorded disposition is `ExactGap`: the saved owner exhausts its rules
and explicit terminals without matching this point. It is not an undecided
predicate or an exhausted classification budget. The same native record
separately contains an optional guard-work refusal at another piece; that
refusal is not the frontier disposition.

The owner came from parent30699, native ordinal1, published sector1611,
representative28686. Its unchanged86,017,923-byte bundle is
`inputs/owners/0000-000011001001011.rrbin`. Generation used
`max_numerator_rank=10` and finite-case policy `search`.

The rank mismatch is a supported explanation to investigate, not yet a proved
cause. Generation explicitly guarantees exceptional traversal only within its
requested rank, not recursive closure above that rank. Intermediate reductions
may increase numerator degree. Conversely, conservative successor boxes can
contain points whose exact coefficients vanish. Required-versus-helper ancestry
has not been established. Neither deleting this point by reimposing the entry
rank nor asserting that a required physical integral lacks an IBP is justified.

## Cheap independent reproduction

Used the existing optimized `2cd97ff…` CLI with a temporary selection containing
only this unchanged owner, one exact singleton query, no routes and no
`--follow-successors`. Original source loading and local match allowances were
preserved; there was no production graph restore, rule generation or RHS walk.

| Observation | Result |
| --- | --- |
| Inclusive guarded time | 3.021074 s |
| Native preparation | 1.621187 s |
| Local matching | 0.000608 s |
| Sampled process-tree peak RSS | 472,670,208 bytes |
| Native exit | 0: classification completed |
| Exact gaps / selected rules / terminals | 1 / 0 / 0 |
| Unresolved predicates / errors | 0 / none |
| All queries locally applicable | false |

The same point reproduces with the new speed build. Relevant local-dispatch,
applicability and rescue code is unchanged between production and that build.
Resuming is not a rule repair: it retains the committed frontier, and the
existing rescue policy deliberately does not pretend that a bounded `ExactGap`
is a recognized unbounded-helper failure.

Independent review checked result interpretation, scope, bindings and process
drainage. All probe groups are absent and locks released. The local command,
input and result receipts are under
`TMP/postlaunch-20260930/production-frontier-20261001/`:
`SINGLETON_PLAN.json`, `SINGLETON_BOUND.json`, `singleton-result.json`,
`SINGLETON_RECEIPT.json` and `TRIAGE.md`.

## Implications for the optimization plan

A subsequent isolated neighborhood screen varied only the two inactive
coordinates: `x[1]=a` for0..12 and `x[10]=b` for0..2, retaining every other
coordinate of the frontier. Each of these39 singleton queries used its actual
rank `a+b`, positive-power sum7 and exact difference `7-a-b`. It completed in
3.033671s inclusive with26 selected rules,10 explicit terminals, three exact
gaps and no unresolved predicate or error. No RHS was traversed.

| Tested slice | Observed saved-rule classification |
| --- | --- |
| b=1, a=0 | Selected rule480 |
| b=1, a=1..9 | Explicit terminals, ranks2..10 |
| b=1, a=10..12 | Exact gaps, ranks11..13 |
| b=0 or b=2 | Rules except the a=b=0 terminal; includes a covered rank14 point |

This strengthens the rank-truncated exceptional-slice hypothesis. It does not
prove an infinite residual ray, a missing IBP, required-query ancestry, or that
extending the terminal table is the correct repair. In particular, rank>10 is
not a global dispatch cutoff: neighboring points beyond it have valid saved
rules. The run and inputs are in the same local evidence directory under
`rank-neighborhood/`; independent review accepted its inputs, interpretation and
process drainage.

### The neighboring recurrence's exact exceptional condition

A separate existing-CLI `owner-guarded-apply` diagnostic tested the three
neighboring rules184,261 and480 at the original frontier point. It completed
in3.004859s inclusive, with all three inspections complete and no errors.
Rule261 and rule480 returned `EmptyFixedFace`. Rule184 reported the excluded
conjunction consisting of `F=0` and `n0=0`, where

```text
F = 1+n12+n10-n1-n1*n12-n1*n10
    +4*n0+2*n0*n12+2*n0*n10+2*n0*n1.
```

Here `n0`, `n1`, etc. are zero-based **physical integral indices**, not the
nonnegative local coordinates `x`. On the tested slice `n0=n12=0`, this becomes
`F=(1+n10)(1-n1)`. At `x[10]=1`, hence `n10=-1`, both members of the excluded
conjunction vanish for every `n1`. The displayed denominator of the rule is
`-F+2*d*n0`, which also vanishes identically on this slice, not just at a special
dimension. Applying this generic recurrence by dividing by that denominator is
therefore not a legitimate repair.

This gives a concrete guard-level explanation for one adjacent rule's failure,
alongside the complete saved-rule classification above. It does not show that
no different IBP can reduce this case, that the slice is reached by a required
physical input, or that every point of an infinite ray is uncovered. Retained
diagnostic residuals alone are not asserted to be nonempty or disjoint; the
point classification and explicit substitution establish the statement here.

The unchanged owner was loaded once, with no production graph restore, source
generation or successor walk. Inputs, exact guard output and guarded process
receipt are under `guarded-neighbors/` beside the earlier probes. Independent
review accepted the interpretation and confirmed both additional native process
groups drained. No terminal or owner was added.

### A source-derived local relation is now found

The generic `probe_owner_case` research example loads this same owner and
searches the nominated physical case against its original source system. The
search uses the saved mathematical ordering, prospective default source
visitation, and the actual descendant rank11, not the entry-rank10 bound.
No saved rules or terminal table are used as equations in this search.

| Source depth | Search time | Inclusive diagnostic time | Result |
| --- | ---: | ---: | --- |
| 1 | 0.114708 s | 2.746143 s | No rule; target remains unresolved |
| 2 | 0.344215 s | 2.940293 s | One direct descending rule; no residuals |

The depth2 rule has112 RHS terms,156 retained source-trace rows and no index
exception branches. Both runs completed without errors or resource stops, and
all owned processes drained. These are local diagnostic timings, not timings
for the full campaign or its closure. The release research build (core release,
app opt-level1) took773.420s separately; neither table entry includes compilation.
Evidence is in `nominated-point/` and `nominated-point-depth2/` under the same
local frontier evidence directory; the frozen executable digest begins
`b7f4b9f7fea6f527`.

The fixed relation subsequently passed independent original-source replay and
installed lookup (3.757804s inclusive, terminal count unchanged). At that point
it was **not yet a delivered repair**: RHS obligations, durable transport and
continuation still needed validation. Depth3 was not run because depth2 already found the
target relation. The depth1 miss does not establish that the integral is a
master, and it was not installed as one.

For anticipation, leaving n12 unfixed gives
`F=(1-n1)(1+n10+n12)` at n0=0. Since these three indices are inactive,
`1-n1` cannot vanish and the two integer possibilities for the other factor
are `(n10,n12)=(-1,0)` and `(0,-1)`. The latter companion point at n1=-10
is already covered by saved rule261 (3.104368s inclusive classification;
zero gaps or unresolved predicates). One recurrence's exceptional condition
therefore does not itself imply a gap in the full installed rule collection.

The next source-driven probe left n1 genuinely parametric, with no rank cutoff,
keeping the other fourteen physical coordinates fixed. **It succeeded:** two
rules, zero residuals, and independent original-source/guard replay passed.
The symbolic rule has83 RHS terms and80 retained trace rows; the exceptional
fixed boundary has601 RHS terms and956 trace rows. Existing lookup still uses
the old boundary rule and old terminals where they already apply. The new rule
covers x1=10 and the entire interval x1>=11. A complete12-piece ray census has
zero gaps or unresolved predicates, and the83 existing terminals are unchanged.

| Phase | Seconds |
| --- | ---: |
| Owner preparation | 1.714432 |
| Symbolic/numerical source search | 10.260562 |
| Independent source replay, including source preparation | 18.262132 |
| Installed whole-ray inspection | 0.000983 |
| Inclusive guarded run | 30.865645 |

Evidence: `nominated-ray-replayed/` in the same local evidence directory;
sampled tree peak472,768,512bytes and all owned processes drained. These are
local repair measurements, not full-campaign timings. The result proves local
applicability on this exceptional slice, **not recursive closure of every RHS**,
coverage of other faces, or full five-loop closure. New replay methods accept
the real partial-domain result type; residual-free installation rejects any
returned unresolved integrals. Durable native transport, cold source/descent
validation and downstream checks were the next delivery gates at that point.
No required query or master declaration is changed.

### Persisted repair and downstream diagnostics

The fully optimized binary subsequently exported the two replayed rules as a
220,735-byte native `DomainRules` payload (SHA256
`77941434f7d2bd6d161edce32946d5e70608b80fec518c543eee9020a6787e7a`).
Export took24.883s inclusive, including7.953s source search and14.085s replay.
A separate process cold-loaded it in16.736s, regenerated the source identities,
checked guards and strict descent, and reproduced the complete12-piece local
ray census with83 old terminals unchanged. It performed no new generation.
This is now a durable partial repair, not just an in-memory candidate.

Using all67 owners and8246 verified routes, three neighboring exact roots
at n1=-10,-11,-12 all have83 immediate successors, no conditional problems
and no new terminals (939 global saved terminals unchanged). Their deeper
finite trace has **not completed**:

| Diagnostic | Inclusive time | Result |
| --- | ---: | --- |
| 20,000 operational nodes |126.229s| Node allowance exhausted;348 completed |
| 1,000,000 operational nodes |132.453s| Routed expansion budget exhausted;4956 completed |
| Existing CLI, detailed cause |134.659s| Same finite expansion-budget failure |

The last trace reports4059436 projected aggregate routed endpoints against a
4000000 allowance. Its failed Route target is
`[0,-11,0,0,1,0,0,0,1,0,-1,1,0,2,1]`. There are819606 scheduled operational
nodes,814649 pending and one failed node, with zero recorded missing-rule or
missing-owner frontiers. Owner/route preparation takes roughly109s and cold
replay14s; the trace itself takes3.4s. Peak sampled process-tree memory is about
5.4GB. These are diagnostic costs, not a controlled production speed comparison.

The transport error wrapper includes resource-limit failures; it does **not**
indicate a bad momentum map or invalid repair here. The CLI's native exit4
correctly denotes incomplete tracing. The first diagnostic wrapper incorrectly
expected exit2; its original receipt is retained alongside an interpretation,
not relabeled successful closure. All owned process groups drained. This finite
test neither establishes recursive closure of the ray nor justifies changing
the production scope. Evidence is under `nominated-ray-export/`,
`nominated-ray-cold/`, `nominated-ray-full-follow/`,
`nominated-ray-full-follow-1m/` and the CLI diagnostic directory beside them.

The final real-CLI symbolic campaign check used the bounded band x1=10..12,
the same fourteen fixed coordinates and full saved routing. Cold installation
passed; initial application produced166 successor domains and zero problems.
The walk completed2067 inspections with zero frontiers before its requested
20000-domain diagnostic allowance (17932 pending, one budget-failed record).
Inclusive135.957s, walk3.966s, peak5.113GB; all processes drained. Evidence:
`nominated-band-cli-walk/`. This exercises the actual campaign path and is not
recursive closure of that band. Combined with exact cold replay and the public
checkpoint tests, it supports shipping this partial repair in a fresh walk.

### Follow-up, separate from repair delivery

1. Investigate actual missing point/face support using existing directed
   `SectorSolver::solve_domains_with_observer` or isolated-case search. Do not
   regenerate every owner merely to diagnose one point. Partial-domain output
   is not a complete-sector artifact; publication and checkpoint amendments
   remain separate, source-bound operations.
2. Make rank excursions and exceptional-domain fragmentation visible when
   comparing candidate recurrences. The first algebraically valid recurrence
   need not give the best downstream domain traversal. This is motivation for
   an opt-in bounded candidate portfolio, not proof that such a portfolio
   eliminates this particular gap or wins on total runtime.
3. Separately test whether routing overcoverage introduced the point. Any
   exclusion must be justified by actual support/guard evidence, never the
   expectation that rank should stay below an entry bound.

The existing unlinked saved-case comparison harness is not yet a frontier
repair tool. Its saved-rank10 exceptional replay cannot establish coverage of
this rank11 point; source-rule equality is also not search-workload equality.
Its independent audit requires baseline-gate fail-fast behavior before any
accepted alternative trial and separate timing controls before speed claims.

No mathematical scoped-closure claim, unrestricted five-loop claim, new master
declaration, production resume recommendation or production mutation follows
from this diagnostic.
