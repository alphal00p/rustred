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

### Smallest selective-splitting falsifier

Partition exactly this source domain into x8=0 and x8>=1, retaining the full
rank cap, all other bounds, ordered-case guards and all original successors.
Use the existing native guarded application on each piece. The first check is
that the zero face loses this event while the complementary nonzero image is
retained. Then, only if warranted and separately granted, walk the original
panel versus this exact partition with identical owner/routing authority.

The useful criterion is reduced total native/queue work including extra splits,
not merely a smaller image of one event. A failure to reduce unique obligations,
or a splitting cost larger than the saved inspections, falsifies this particular
optimization. Full arbitrary-polynomial guarded domains are unnecessary for
this test. No split experiment is launched by this document.

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

Review the separately granted physical R0 result next. If symbolic work is large,
an independently granted comparison of a small, matched concrete input panel
through the existing memoized exact `--targets` evaluator can distinguish symbolic
overcover/partitioning from concrete transport work. A concrete refusal of a
reactivated axis is an interface/authority difference, not evidence of a real
IBP cycle. The already trivial 67-unit control does not justify running that
comparison now: a later comparison must use concrete points from the actually
problematic finite input, with the same input points on both paths. No finite
enumeration of the entire dotted physics input is implied.

The saved rule-40 zero-face split is the smallest concrete precision falsifier.
A per-domain E experiment needs a separate real E-inflation witness and native
geometry design. Target-directed Laporta is a larger synthesis change and
should be gated by a specific harmful transition plus exact source replay.
None authorizes changing production, dropping descendants, adding helpers,
claiming full-family termination, or launching another pilot automatically.
