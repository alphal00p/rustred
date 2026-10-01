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
