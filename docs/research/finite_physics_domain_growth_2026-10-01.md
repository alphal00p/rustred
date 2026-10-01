# Finite physics inputs and symbolic dependency growth, 2026-10-01

## Status and question

This is a source/saved-evidence investigation, not a new termination proof or
permission to clip intermediate integrals. The original production campaign,
its 116 required physics queries and 67 auxiliary queries remain unchanged.
The separate helper-free rank-zero controls use the original 67 owners, 8,246
routes and repaired overlay: first 67 unit-index points, then, only under a
separate grant, the 116 required queries intersected with R=0. The latter keeps
all original positive-power and A/D constraints, including 35 expected empty
rows; the input-only census predicts 81 nonempty rows across 56 owners.

### Rank-zero input cardinality is not uniformly small

The frozen `queries-physics.json` has SHA256
`fc9bb5a196b84600f1ca3f88364a4b342c53c608617cb5fd2bdaa2663c84e58e`.
Its R0 scalar-input arithmetic is:

| Required-row class | Rows | Distinct owners | Nonempty rows / owners | Raw sum of input tuples |
|---|---:|---:|---:|---:|
| Exact D=9 or D=10 | 98 | 49 | 63 / 38 | 1,185 |
| Nested D>=9, finite A | 18 | 18 | 18 / 18 | 7,173,673 |
| All required R0 | 116 | 67 | 81 / 56 | 7,174,858 |

These are counts of the specified input geometry, not native admitted/covered
integrals, independent master integrals, or a new enumeration run. For support
size t, R=0 implies A=D. All local lower bounds are zero, and every active
coordinate upper is nonbinding at the effective A maximum. Hence a row has
`sum_A binomial(A-1,t-1)` tuples, where A runs from `max(t,Dmin)` through
`min(Amax,Dmax)` (omitting an absent Dmax). This is elementary positive
composition counting; no reducer, checkpoint decoder or new geometry kernel
was used. The table reports the raw row sum without a deduplication algorithm.
As an additional check, repeated rows for the same owner have disjoint A
intervals, and the two classes use disjoint owner sets.

The nested original A caps are **20 through 23**, not only 20 through 22. At
R0 their D>=9 lower bound does not collapse them to A=9/10. Each ten-line owner
with A<=23 contributes 1,144,066 tuples. Thus the hypothesis that the entire
helper-free R0 input is only a modest collection of one-/two-dot points is
false: the 18 required nested rows dominate. An exact-D-only experiment would
omit all 18 of those required rows and could not replace the present scope.
Any later concrete-path comparison should use a small explicitly matched
panel from a representative problematic nested query, not silently substitute
the easier exact-D class. No seven-million-point enumeration is proposed.

Three effects must not be conflated:

1. Actual IBP successors can raise numerator rank while reducing another
   complexity component. Physical input power counting is not a descendant cap.
2. A symbolic successor region can overapproximate the nonzero image of a
   recurrence or momentum map. Repeated overapproximation can create obligations
   not needed by the original finite inputs.
3. Even faithful finite regions can generate many overlapping partitions,
   repeated native inspections and canonical routing obligations. A large
   queue is not by itself an infinite concrete reduction or an IBP cycle.

No actual expanding concrete cycle has been established here. In particular,
the previously checked five-loop owner 17941 rule-217 R-raising point is not
such a witness: it cannot immediately re-enter that case, and its two opposite
R-raising terms cancel under the separately verified symmetry at that point.
That cancellation is not a generic statement about the whole parametric case.

## A concrete four-loop conditional-image loss: ten points versus six

The saved native evidence is in
`TMP/postlaunch-20261001/rule-quality-portfolio/owner-481-ordered-geometry-r3/a1-r1/`:

- `ordered.json`, SHA256
  `bf2915a2c514bf60fc4007f696205c720e6f006a03388fd4ea3659ca29f241cf`;
- `guarded.json`, SHA256
  `8dab2f9c59666a868ed0395cf63ccece735b2c2276362369f62ec2d1728abad8`.

`ordered.queries[0].pieces[393]` selects batch 0, rule 40 of owner
`0111100001` (native owner 481). The corresponding guarded query has ID
`ordered-piece-393`. It finishes successfully with 37 successors, zero problems,
no native/report error and no optional coefficient refusal. This is an actual
priority-selected case, not merely an original RHS term from an unselected rule.

Local coordinates are x=n-1 on active axes and x=-n on inactive axes. The source
box has lower `[1,0,0,0,0,0,0,1,0,0]`, upper
`[1,0,0,0,0,0,0,null,null,0]`, rank cap 5, and no A/D constraints. Thus only
x7 and x8 vary, with x7>=1, x8>=0 and x7+x8<=4. The displayed case exclusions
are n7!=0 and n0-n5!=0. Both hold throughout this source box: n7<=-1 and
n0-n5=-1. All original denominator checks are either n0-n5 or 1; there are no
additional source nonzero conditions in this report.

Event index 26 of this query is a `guarded_successor` with:

- restricted coefficient `n8`, classified **conditional**;
- argument shift `[0,0,0,0,0,0,0,-1,0,0]`, under the explicitly reported
  convention `source_n = child_m + argument_shift`;
- the same installed target owner, target lower
  `[1,0,0,0,0,0,0,0,0,0]`, upper
  `[1,0,0,0,0,0,0,null,null,0]`, and rank cap 4.

Consequently the target local coordinates obey y7=x7-1, y8=x8. The reported
envelope is y7>=0, y8>=0, y7+y8<=3: **10 integer points**. The coefficient is
nonzero only when x8=y8>=1, so the nonzero image of this particular event has
**6 integer points**. Its other four envelope points have y8=0 and y7=0..3.
For example, source physical powers `[-1,1,1,1,1,0,0,-1,0,1]` map to
`[-1,1,1,1,1,0,0,0,0,1]`, but the coefficient is zero.

This arithmetic uses the saved native selected case, guards, coefficient and
mapping; it is not a new algebraic-equivalence engine. The four points are
spurious **for this edge**, not necessarily absent from the complete RHS or
reachable graph. Other terms could reach them. Neither a four-node saving nor
an expanding feedback cycle is demonstrated by the example.

The current queue conversion retains the target box, rank and A/D constraints
but only a Boolean `conditional` classification, not the coefficient's nonzero
predicate. See
[walking/inspection.rs:538](/common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/inspection.rs:538).
The existing optional application refinement only accepts a single varying
finite axis and therefore does not cover this two-varying-axis case; see
[applied/refinement.rs:22](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/owners/domains/applied/refinement.rs:22).

### Measured selective-splitting falsifier

The separately granted native diagnostic partitioned exactly this source domain
into x8=0 and x8>=1, retaining the rank cap, all other bounds, ordered-case
guards and all 44 original denominator checks. Evidence is
`TMP/postlaunch-20261001/coefficient-face-split-v1/execution-r1/`.
It completed in **4.534894 seconds inclusive** (3.780531 seconds in the native
phase), with zero problems/refusals and its owned group drained. The complete
unsplit query output exactly matches the pinned historical baseline.

The witness disappears on the zero face. On the positive face its coefficient
is uniformly nonzero and the target lower bound is y8=1, giving the precise
six-point image above. This validates the local precision mechanism without
requiring arbitrary-polynomial guarded domains. However, partitioning increases
local native operations **173 to 271**, term visits 58 to 88, predicate checks
46 to 92, and emitted successors 37 to 59 (22 plus 37). Fourteen other baseline
events absent on the zero face were already restricted to source x8>=1; they
are not fourteen additional false-edge removals caused by this split.

No native target-union comparison, recursive walk or global work improvement is
claimed. Other RHS paths can still reach the four excluded witness points.
The extra partition cost makes a broad splitting rollout unjustified by this
result alone; another pilot needs a concrete net-work mechanism and a separate
grant, not merely the smaller image of this one event.

## What the present geometry already preserves, and what it does not

The current walker does **not** generally replace every routed successor by a
full orthant. It sends the actual successor bounds to routing; the source
comment at `walking/inspection.rs:552` explicitly forbids the old widening.
Sign-fixed rule shifts transport boxes and rank/A/D bounds, and native
`DomainPowerSummary` provides exact integer-set inclusion for that vocabulary.
It does not represent arbitrary coefficient predicates or arbitrary linear
correlations; see
[power_domain/summary.rs:70](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/power_domain/summary.rs:70).

Affine routing still needs an overcover of polynomial numerator expansion.
Current routing preserves a positive-power upper bound and D minimum, while
discarding the supplied D maximum because affine constant terms can increase
D. It uses A' as a safe D' upper bound; see
[domain_overcover/power.rs:7](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/routed/domain_overcover/power.rs:7).
Special homogeneous maps may admit tighter bounds, but homogeneity must be
checked on the actual relevant map rows; vacuum kinematics or equal masses do
not justify assuming it. Lost numerator-incidence correlations and conditional
coefficient faces are separate precision issues.

## Total excess E: useful relational precision, not yet a global cap

Let t be the active support size and E=A-t+R, the sum of positive propagator
excess and numerator powers. The legacy A1 comparator uses support count and
sector lexicographic order before A+R, and then numerator/coordinate ties; see
[solver/index.rs:357](/common/dev/rustred/crates/rustred-core/src/solver/index.rs:357).
Therefore same-support descent implies E does not increase, but a changed
support of equal cardinality can precede a larger E. The applied walker admits
some support swaps/reactivations inside a wider saved root that the concrete
routed evaluator rejects; this is explicit in
[support_transitions.rs:76](/common/dev/rustred/crates/rustred-core/src/solver/candidate_reduction/owners/domains/applied/tests/support_transitions.rs:76).

For an admitted affine momentum map with a unit active-denominator bijection,
the exact polynomial endpoints themselves obey E'<=E. To see the limitation and
the useful invariant, let B be mapped denominator powers, e an expansion
monomial with total degree T<=R, c=sum(min(Bj,ej)) on active axes, and k the
number of pinched active axes. Then A'=A-c, R'=T-c and t'=t-k, so
E'=A+T-2c-t+k<=E-2c+k<=E-k. Affine constants do not invalidate this argument.
This describes exact endpoints, not every point of a separately widened cover.

The simple proposed bound E<=E0+t0*maximum_pinch_jump is consequently **not
established** for the full saved-owner domain walk. Same-cardinality support
changes can increase E, and canonical routing can reset the earlier sector-lex
component. A finite-owner transition argument would need actual selected,
guard-valid support transitions and compatible potentials, not just permissive
loading or a census of raw shifts. Unresolved guard feasibility cannot be
treated as an absent edge.

An existing service already proves sector-dependent total-excess envelopes for
a complete single-root census under a compatible E-primary source order:
[source_port/total_excess.rs:96](/common/dev/rustred/crates/rustred-core/src/foundry/artifact/source_port/total_excess.rs:96).
It checks all sectors in comparator order, rechecks coefficient zeros for
apparently nonlower transitions, and propagates explicit successor degrees.
This is not currently a global routed, mixed-owner certificate, nor can a
caller-written report substitute for its checked evidence.

A per-domain E bound could still improve precision without claiming global
termination. Each actual rule image would need a sound updated E bound; routing
would need the endpoint proof above reflected in its cover; intersections,
containment, residual differences, checkpoints and cold replay would all have
to preserve the new relation. Existing A/R/D summaries cannot encode A+R by
renaming D=A-R or by separately capping A and R. Merely tightening rectangular
upper bounds loses the correlation again. This is a scoped geometry extension,
not a descriptor change or a safe entry-R cutoff. First find a real saved image
where the E relation removes substantial false obligations; the guard-loss
example above already concerns a predicate, not specifically a lost E bound.

## Helpers, physics scope and alternatives

An initial full orthant can subsume many translated boxes and consolidate their
scheduling responsibility. The index is built only from actual initial
admissions; it is explicitly not completed-coverage authority. See
[initial_orthants.rs:1](/common/dev/rustred/crates/rustred-app/src/application/routed_campaign/walking/initial_orthants.rs:1).
The 13 original auxiliary starts with unbounded positive power can also request
genuinely more work than finite physical starts: dot-to-numerator recurrences
may require unbounded ranks over an unbounded dot input family. This is not all
an abstraction artifact. The 54 finite-A auxiliary starts do not qualify for
that full-orthant shortcut but may still help ordinary containment/reuse.

The helper-free controls isolate this tradeoff; no helper should be silently
reintroduced under a different name. A demand-driven alternative would group
already requested or actually reached regions, retain their exact union or
checked residual coverage, and prioritize reuse without asserting an unprocessed
cover closed. Larger speculative anchors add obligations and need their own
cost/coverage accounting.

Gauge-renormalizable constraints already motivate the physical entry A/R/D
profiles documented in
[finite_starting_domains.md](/common/dev/rustred/docs/finite_starting_domains.md).
Graph/forest-specific derivative incidence and joint profiles may tighten an
entry union further, subject to the stated gauge, resummation and UV-jet
assumptions. They do not show that an individual intermediate scalar integral
outside the entry profile vanishes. Ward/Slavnov-Taylor cancellations concern
the assembled amplitude and cannot be applied to unrelated scalar obligations
without retaining that amplitude information.

Two longer-range directions have distinct authority requirements:

- Target-directed exact Laporta elimination could replace a demonstrably costly
  finite transition by a replayed relation to cheaper reached targets. A finite
  solve is not a closure certificate; retain source provenance, denominator
  guards, native admission and all unresolved successors. It must not create a
  circular claim that its chosen target set is already closed. A relation that
  violates the currently admitted descent cannot be silently installed as an
  ordinary rule. The separate Laporta/helper-alternatives investigation owns
  the detailed existing API and literature assessment. In particular, the
  convenience bridge dispatches arities only through 12; a five-loop arity-15
  diagnostic needs the existing const-generic solver and the saved owner/order
  binding. Existing feedback nomination handles missing rules, and appended
  batches do not override an earlier applicable rule. A shortcut for an already
  selected harmful rule therefore needs explicit composed-program precedence,
  not simply another repair appended at the end. Overlay source replay alone
  does not prove descent, case coverage, RHS closure or the terminal basis.
- Designing IBP identities to avoid doubled propagators is a genuine
  mechanism, not post-hoc descendant clipping. Gluza, Kajda and Kosower impose
  propagator-divisibility conditions on IBP-generating vectors in their
  [primary paper](https://arxiv.org/abs/1009.0472). That construction does not
  establish completeness or favorable cost for this five-loop workload.

The finite-master theorem of Smirnov and Petukhov establishes finiteness of the
master space for a fixed graph, not termination or a complexity bound for this
particular recurrence/abstraction worklist; see the
[primary paper](https://arxiv.org/abs/1004.4199).

## Next discriminators, without widening authority

The unit control completed in 182.420889 seconds inclusive. Cold-All verified
all 67 native inspections and 67 roots with zero errors, frontiers, uncovered
obligations, edges, cycles, successors or routed domains. Native preparation
took 77.890736 seconds; traversal took 0.056523 seconds. All four owned phases
drained, and the wrapper and all recorded native/supervisor PIDs were absent
on independent inspection. Evidence is
`TMP/postlaunch-20261001/five-loop-rank0-v1/receipts/unit/RESULT.json`.

These are 67 immediate leaf obligations, not a nontrivial recurrence or routing
test. The aggregate receipts do not expose terminal-versus-exact-zero
classification; the application maps both to the same count effect. An older
67-owner corner experiment found explicit terminals, but that historical result
does not independently classify the current payloads. The live walk's final
engine closure sample remained stale at 16/67; cold-All independently recovered
67/67, so the stale sample is not an unresolved coverage failure.

The inherited outer phase controller had a 600 GB hard/570 GB soft sampled RSS
envelope, whereas the inner native walk supervisor correctly enforced 150 GB
hard/142.5 GB soft and a 150 GB host reserve. Actual sampled peak was 5.265 GB.
This wider outer allowance was not clear in the original protocol; the physics
arm was held for a pilot-local correction before release. These are sampled
process-tree RSS guards, not a kernel-enforced address-space limit. This
resource discrepancy does not turn the observed leaf coverage into a recurrence
test, nor does it license silently relabeling the original unit receipts.

### Physical R0 result: full scoped coverage, with substantial symbolic work

The separately granted physical arm also passed, in **587.901650 seconds
inclusive**, using the corrected outer memory envelope. Its evidence is
`TMP/postlaunch-20261001/five-loop-rank0-v1/receipts/physics/RESULT.json`.

| Measured quantity | Physical R0 arm |
|---|---:|
| Required queries admitted and independently certified | 116 / 116 |
| Required input roots independently verified | 92 / 92 |
| Queries recorded as absorbed by existing initial roots | 24 |
| Scheduled symbolic domains | 1,870,662 |
| Completed native inspections, all cold rechecked | 1,685,449 |
| Saved dependency edges | 11,101,902 |
| Native events | 21,530,046 |
| Native preparation / traversal | 76.524542 s / 221.211886 s |
| Sampled process-tree peak RSS | 6.839017 GB |
| Abstract graph nodes lying on cycles | 551,273 |

All four phases have sampled hard/soft ceilings of 150/142.5 GB. All owned groups
drained; the wrapper and all five recorded native/supervisor child PIDs were
independently absent. Workers joined, pending and abandoned obligations were
zero, the final checkpoint is generation 4, and cold-All found no violations,
errors, frontiers, uncovered obligations or count mismatches. Saved owner and
request bindings match; pre-cut and joint-support pruning remained off.

The 24 absorbed queries are **not** the independently predicted 35 empty input
rows: they are different notions. The final aggregate receipt does not expose
the native per-row empty/nonempty split or maximum reached rank, so neither is
invented from other counters. The original entry R0 constraint was never imposed
on descendants. All 116 original required IDs and positive-power/A/D geometry
were retained under the declared R0 transformation, with no auxiliary starts.

This establishes native scoped successor coverage for this finite R0 query
union, not the original R>0 production campaign or all positive-power orthants.
In particular, **551,273 is a count of nodes on abstract graph cycles, not a
count of SCCs or witnessed concrete IBP cycles**. Cold certification accepts
sealed finite obligation graphs, including cycles. It is neither a new proof
of concrete termination nor evidence of physical nontermination. The stale
live closure sample was again smaller than the cold result; all 1,870,662
recorded domains were closed by the cold oracle.

Final work counters include 903,632 Route inspections and 15,918,098 successors,
of which 71,198 were conditional. This does not make the rule-40 zero-face
mechanism wrong, but a low conditional-event fraction alone neither proves
that mechanism dominant nor rules out a few expensive conditional cones.
Counts of symbolic domains and scalar input tuples are different units, and
the totals do not attribute the work to particular owners or required rows.

An independently granted comparison of a small, matched concrete input panel
through the existing memoized exact `--targets` evaluator can distinguish symbolic
overcover/partitioning from concrete transport work. A concrete refusal of a
reactivated axis is an interface/authority difference, not evidence of a real
IBP cycle. The already trivial 67-unit control does not justify running that
comparison now: a later comparison must use concrete points from the actually
problematic finite input, with the same input points on both paths. No finite
enumeration of the entire dotted physics input is implied.

The completed rule-40 zero-face split confirms precision but increases local work.
A per-domain E experiment needs a separate real E-inflation witness and native
geometry design. Target-directed Laporta is a larger synthesis change and
should be gated by a specific harmful transition plus exact source replay.
None authorizes changing production, dropping descendants, adding helpers,
claiming full-family termination, or launching another pilot automatically.
