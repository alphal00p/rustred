# Finite-domain strategy: literature and measured-priority audit

2026-09-22. Research-only follow-up to the
[bounded-routing pilot](bounded_routing_pilot_2026-09-22.md) and
[finite starting-domain contract](../finite_starting_domains.md).
This report introduces no implementation, solver run, physical-envelope proof,
or closure claim. It incorporates the later semantic-admission replay and
native routing census supplied during the audit, rather than repeating the
earlier recommendation to prioritize full-permutation routing.

Post-audit validation: the two fixture counts mentioned below have now been
corrected with explicit semantic-equivalence assertions. All 361 release
application tests pass (one diagnostic ignored), including the complete parity
loops; the 82 separately run integration tests also pass. The gate caveat below
records the initial audit state, not the corrected validation result.

## Executive conclusion

The immediate problem is not an observed shortage of IBP equations. It is the
cost of propagating, comparing and conservatively enlarging sets of integrals
through already available rules. Target-restricted IBP literature supports
repairing actual missing targets, but does not turn a large unfinished routing
queue into evidence that new rules are needed.

The best next sequence is:

1. Measure the implemented exact semantic-containment service in the same
   recursive campaign; its completed-descriptor replay is already encouraging.
2. Tighten affine routing with the audited source-support degree budgets.
3. If admission remains dominant, index a few monotone summary features before
   running full containment checks.
4. Only if necessary, add selected shared-support pinch inequalities or a
   bounded refinement of particularly loose images.
5. Invoke target-directed IBP generation only for a concrete reachable gap.

Full-permutation routing is sound but presently low priority: the census found
no applicable map among the measured nonliteral routing calls. Neither more
cores nor a larger returned-result buffer addresses the observed serial
admission bottleneck by itself.

## What the latest evidence establishes

The constrained diagonal starts from 980 points. Its stopped recursive pilot
completed 49,686 domains, retained 145,431 queued domains and observed zero
missing-rule frontiers. Traversal took 106.956 s before a cooperative
optimization stop. The coordinator performed 4.972 billion containment
comparisons; late utilization averaged only about 2.08 cores with 50 configured
workers. This is incomplete work, not closure or a defensible fifteen-hour ETA.

Subsequent diagnostic results sharpen the next choice:

| Diagnostic | Observed result | What it does not establish |
|---|---:|---|
| Replay of 49,686 completed descriptors | Raw/semantic comparisons: 112,362,704 / 22,484,047 | Replay of every original admission request |
| Same replay, both execution orders, three repetitions each | Median paired admission-wall ratio 4.271×; admissions 49,686 / 25,207 | Recursive campaign speedup or completion |
| Verified routing census | 41 nonliteral full permutations among 8,179 nonliteral maps | Their use by the current workload |
| Actual nonliteral traffic | 32 maps, 27,807 calls; zero full permutations | Absence of eligible maps in future workloads |
| Support-aware affine root-image diagnostic | Strictly smaller projected root covers in 17,304 / 27,807 calls (62.23%) | Number of successor subsets eliminated or end-to-end speedup |
| Uniform-total-rank lower-bound control | Smaller projected root covers in 3,696 calls | Benefit equal to support-aware bounds |

The replay excludes parsing and verification equally. Its raw baseline is a
compact test-only reconstruction, not execution of a historical release
binary; the first index remains allocated while the second runs. Alternating
order addresses one obvious bias but is not a statistical campaign benchmark.

The affine diagnostic identifies 20,340 records with at least one newly
unpinchable axis. The 634,217 historical Route children attached to those
records are **not** a count of children that would disappear. The diagnostic
does not enumerate candidate successor subsets.

Immediate gate prerequisite for priority 1: the latest application run reports
359 passing tests and two failures at stale hard-coded historical domain-count
assertions, before their worker-comparison assertions execute. Replace those
baseline-count assumptions with the intended semantic coverage assertions and
rerun the gate before launching the comparison campaign. This is not an
observed worker-parity failure. The accompanying release core gate passes
2,758 tests with 32 existing ignored diagnostics; all 82 integration tests pass.

Local evidence is under `TMP/semantic-containment-gate.qn9MTE/` and
`TMP/route-permutation-census.mWBz2v/`, especially
`semantic-replay-audit.json`, `RESULTS_AUDIT.md`,
`AFFINE_SUPPORT_BOUND_PROOF.md` and `support-stdout.jsonl`. These are untracked
receipts, not distributable closing artifacts.

## 1. Avoid comparisons, not just expensive comparison arithmetic

Schulz's feature-vector indexing uses numerical features whose inequalities
are necessary for subsumption. A trie retrieves a conservative candidate set,
then the authoritative subsumption test decides. The same structure supports
forward and reverse queries. This is particularly relevant when individual
checks are tolerable but their number is enormous.
[Schulz, *Simple and Efficient Clause Subsumption with Feature Vector Indexing*,
Sections 4–5](https://wwwlehre.dhbw-stuttgart.de/~sschulz/PAPERS/Schulz2013-FVI.pdf).

ProVerif subsequently reports a similar new-object-versus-all-history
bottleneck and uses monotone feature indexing to reduce attempted checks.
Its clause problem differs from RustRed's geometry; the transferable lesson is
candidate filtering, not its unification or proof kernel.
[Blanchet, Cheval and Cortier, Section 4.1(a)](https://bblanche.gitlabpages.inria.fr/proverif/snp22/snp22.pdf).

**RustRed-specific proposal:** reuse `DomainPowerSummary`, not raw optional
labels. For a candidate domain C to lie in container B, every exact coordinate
and aggregate extremum must satisfy the corresponding inclusion inequality.
For example, `max_A(C) <= max_A(B)` and `min_D(C) >= min_D(B)` are necessary.
Use a small fixed subset of these extrema to retrieve candidate IDs, then call
the existing exact `contains` method. Reverse retirement reverses the query
direction. Infinity remains an ordered value, not a large finite sentinel.

Start with a measured selective coordinate or aggregate range index within
the existing phase/support bucket, not a high-dimensional general spatial
library. Stable IDs can preserve deterministic selection. Index deletion must
retire lookup entries only; raw keys, pending tasks and diagnostics remain.
The explicit finite-comparison-budget lane has a different existing contract
and should not silently inherit changed comparison accounting.

Relevant seams are
`crates/rustred-app/src/application/routed_campaign/walking/queue.rs::admit`
and
`crates/rustred-core/src/solver/candidate_reduction/power_domain/summary.rs`.
The exact summary service already exists; this recommendation is not to
reimplement it or run domain projection inside each pairwise comparison.

**Falsifiable experiment:** capture a bounded admission stream, including
rejected requests, and compare indexed retrieval against the exhaustive exact
summary predicate. Require no missed valid containers and identical retained
obligations. Report retrieval nodes, candidates returned, full comparisons,
reverse maintenance, summary-build time, publisher CPU, index RSS and raw-key
hits. Reject the index if maintenance costs erase lookup savings. The existing
completed-descriptor replay is a useful first filter, not that full experiment.

## 2. Preserve affine support before considering a routing portfolio

TIDE explicitly chooses among available sector shifts to reduce expansion of
the highest-power numerator, caches symmetry reductions, and separates wanted
integrals from auxiliary source integrals. Its delayed subsector execution and
numerical dependency tracing avoid carrying irrelevant equations into the
expensive reduction. These are practical target-directed techniques, not a
universal bound on the needed auxiliary region.
[Luthe, *Fully massive vacuum integrals at 5 loops*, Sections 8.1.1–8.1.4](https://noah.nrw/ubbihs/download/pdf/5131192).

Kira 3 also explains why numerator-bearing symmetry relations can be costly
and generates IBPs in mapped sectors instead of relying exclusively on such
relations. Its sector-dependent seeding and target-based equation selection
reinforce the distinction between requested outputs and the equations needed
to obtain them. Seed truncation is not permission to discard actual reduction
descendants.
[Kira 3, Sections 3.1–3.2](https://arxiv.org/html/2505.20197v1).

The immediate RustRed opportunity is narrower. Current preparation accepts
one selected manifest witness per source and rejects duplicate selected routes.
There is no already populated alternative-map portfolio to switch on. Moreover,
all measured nonliteral maps have genuinely affine rows. A full-permutation
shortcut would leave the measured fanout unchanged.

Write the inactive numerator rows, using the already verified native map, as

```
Q_i = c_i + sum_j M_ij T_j,       L_i <= t_i <= U_i,
numerator = product_i Q_i^t_i,    sum_i t_i <= R_max.
```

Here `R_max` is the source domain's implied bound, not a new entry restriction.
For target column j, let S_j contain the inactive source rows with nonzero
M_ij. Any generated monomial has exponent beta_j bounded by

```
B_j = min(sum_(i in S_j) U_i,
          R_max - sum_(i not in S_j) L_i).
```

Thus an active denominator mapped to T_j with source local lower l has target
local lower at least `max(0, l-B_j)` while it remains positive. Pinching it is
impossible if `B_j < l+1`. Empty sums and unbounded inputs require their exact
mathematical meanings; checked wide arithmetic must not clip bounds.

For example, `(T1+1)^3/(T0^2 T1)` can never pinch T0. A total-rank-three bound
alone admits that spurious possibility; source support gives B0=0. Conversely,
`(T0+1)^2/T0^3` really has a power-one T0 term, so retaining all original lower
bounds without subtracting a degree budget is unsound.

This inequality is a direct RustRed-specific derivation, independently audited
in the local proof receipt, not a theorem borrowed from the IBP papers. It
needs only existing verified linear-row support and domain extrema. Symbolica
remains the authority for coefficients and exact zero tests. It needs no
polynomial expansion, reconstruction, new graph matcher or CAS kernel.

**Falsifiable experiment:** first compare all small-domain exact native
transport endpoints against the tightened image, including constants,
cancellation and source conditions. Then repeat the constrained recursive
input with identical saved rules and resources. Count candidate masks rejected
before child construction, emitted children, child projection empties, admitted
regions, containment work and completed-prefix time. Tighten each child's
surviving axes from the source budgets; never reuse extra bounds inferred only
under the all-positive root's assumptions. The diagnostic 62.23% is opportunity,
not a forecast of runtime gain.

## 3. A compact next precision level: shared-support inequalities

The exponent geometry of a product of affine linear factors is naturally
related to sums of coordinate simplices: each factor allocates its degree
among the target variables it actually contains, with constants represented
by a zero-degree choice. Weighted Minkowski sums of coordinate simplices and
their inequality descriptions are studied by Postnikov. This is mathematical
context, not a recommendation to construct their full polyhedra here.
[Postnikov, Section 6](https://math.mit.edu/~apost/papers/permutohedron_full.pdf).

A direct necessary bound extends the single-column calculation. For target
subset J, let S(J) be the union of source rows touching at least one column in
J. Then

```
sum_(j in J) beta_j <= B(J)
B(J) = min(sum_(i in S(J)) U_i,
           R_max - sum_(i not in S(J)) L_i).
```

Consequently, a proposed pinch of every j in J is impossible if its required
positive powers sum to more than B(J). Existing total-rank pinch bounds remain
necessary too; independent column budgets must not replace them.

For a concrete distinction, consider
`(T0+T1)^2*(T2+1)^3/(T0^2*T1^2)`. Total rank is five, and separate column caps
allow degree two on T0 and T1. Yet both cannot be pinched together: only the
first degree-two factor can supply either, so B({0,1})=2 rather than four.
This is an overcover exclusion, not cancellation-dependent algebra.

**Deferred experiment:** only after the single-column implementation is
measured, sample remaining emitted pinch subsets and count violations of this
union bound. Precomputed support bitsets and checked sums suffice. Test only
small or already visited subsets first; do not enumerate all subsets solely
to prepare a general constraint catalogue. Coefficient cancellation can remove
endpoints but cannot create an endpoint violating these bounds. No claim of
an exact image, jointly attainable extrema or complete reachability follows.

## 4. Antichains and refinement: useful, with an obligation boundary

Antichain fixed-point algorithms require a simulation preorder compatible with
the transition system and the property being checked. Set inclusion is useful
only with those compatibility conditions. A pending larger region is not
already solved merely because it contains another request.
[Doyen and Raskin, Sections 2–3](https://lsv.ens-paris-saclay.fr/~doyen/papers/Antichains_Algorithms_Finite_Automata.pdf).

Fixed-template abstract domains explain why keeping a few relational bounds
can be much cheaper than full polyhedral manipulation. Trace partitioning
explains how retaining selected distinctions avoids precision losses caused
by merging paths. Neither paper proves that RustRed's conservative routing
worklist terminates or that its current physical envelope is sufficient.
[Template-polyhedra model checking](https://theory.stanford.edu/~srirams/papers/tacas08.html),
[Rival and Mauborgne, trace partitioning](https://software.imdea.org/~mauborgn/publi/toplas29.pdf).

**Application, not imported theorem:** retain the current lookup-only
retirement policy. A future cancellation of dominated pending work would need
explicit transfer of all coverage/source-validity obligations and a fairness
argument. Do not add it as an incidental index optimization.

If a conservative image reports an apparent gap, preserve its source witness
and test a concrete member through exact transport before generating rules.
Refine the responsible image only when it is genuinely too loose. A bounded
split budget must return incomplete work when exhausted, never success.
Keeping selected source-support distinctions may be cheaper than repeatedly
splitting all coordinates. This remains a proposal to test after the narrower
support-aware bounds, not a new abstract-interpretation framework to implement.

## 5. When an actual rule gap appears

Tube seeding constructs sparse source neighborhoods connecting selected
targets to simpler integrals and studies rank-ten target chunks over finite
fields. Its example needs extra diagonal paths for targets missed by the
initial tubes. This is strong evidence for adaptive, target-directed source
selection, but not proof that any fixed tube width or path family suffices for
five-loop exact parametric rules.
[Berman et al., Sections 4–5](https://arxiv.org/pdf/2606.10698).

Blade further warns that useful relations can involve supersectors rather
than only the target sector and its subsectors. A repair strategy must therefore
retain an explicit source-expansion fallback if a narrow search fails; that
failure cannot silently create a master or imply impossibility.
[Blade, Section II.5](https://arxiv.org/html/2405.14621v1).

Read-only inspection of `vendor/spired/src/solver.tpp` confirms that its
case-directed source search, modular pivot detection and exact rule recovery
address relation discovery. They do not supply the missing campaign-level
domain/subsumption data structure. RustRed should keep those responsibilities
separate: exact reachable target first, bounded source proposal second,
Symbolica-backed exact replay third, and immutable rule publication last.
Finite nonminimal declared terminals remain acceptable; unprocessed work,
unlucky samples and budget exhaustion do not become terminals automatically.

## Ranked next experiments and stop criteria

Before the first campaign rerun, restore the integrated validation gate. The
current application suite has 359 passing tests, two failing hard-coded
baseline node counts and one ignored diagnostic. Both failures precede their
worker-comparison loops; they do not establish a parallel mismatch. The
semantic-equivalence explanation must be retained in the revised assertions,
and all downstream comparisons must actually run. The release core passes
2,758 tests (32 ignored), and all 82 separately run integration tests pass.

| Priority | Experiment | Expected benefit / complexity | Reject or defer if |
|---|---|---|---|
| 1 | Same-input recursive rerun with cached semantic containment | Directly targets measured serial cost; implementation already present | Better replay does not improve equal-work recursive progress |
| 2 | Source-support degree bounds in existing affine route visitor | Reduces fabricated regions before admission; narrow, audited arithmetic | Native endpoint differential test fails or total admission savings are negligible |
| 3 | Small monotone-feature index on exact summaries | Avoids pair scans remaining after semantic deduplication; moderate bookkeeping | Index/update RSS or CPU outweighs saved comparisons |
| 4 | Selected union-support pinch bounds | Removes shared-budget combinations missed by independent caps; bounded extension | Few residual subsets violate the bound |
| 5 | Reachability-guided target repair | Finds equations only where a demonstrated gap requires them | No concrete reachable missing target exists |

Do not widen the full physical campaign until the first two experiments show
sustained progress with controlled queue growth. Keep serial preparation,
traversal, publisher time, CPU utilization and RSS separate. Different admitted
domain sets may prevent literal prefix equality; in that case use unchanged
starting inputs and independently checked coverage/work measures, not a ratio
of arbitrary stop times.

Finally, literature does not authenticate the physical input envelope. The
documented marginal profile at five loops is A24/R14/D>=10 under explicit
assumptions; GammaLoop's full UV jet requires the separately discussed
A24/R15/D>=9 envelope. The current R10 diagonal is neither. Source-derived
descendant bounds may exceed entry bounds legitimately. Preserve those facts,
accept finite nonminimal terminal sets, and defer numerical evaluation and
minimality work as requested.
