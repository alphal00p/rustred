# Exact finite demand versus symbolic SCC obligations

Research and bounded diagnostic record, 2026-10-04. The diagnostics below did
not change scheduler, algebra, source-program, terminal or checkpoint semantics.
A subsequent authorized **default-off finite replay implementation is under
acceptance**, described separately below; it is not a production or performance
claim.
The canceled local observer test is a negative result, not an unfinished
optimization. The first discriminator completed with the existing concrete
tracer on the **original four required singleton inputs**, not a newly selected
cohort. A separately authorized H55 diagnostic tests the same mechanism on the
original larger training demand; neither changes reusable rule output.

## Recorded witness: real broadening, circular local demand

The completed original four-point baseline has 19,074 symbolic nodes and
101,338 edges. Its unchanged context contains 67 owners, 8,246 ordered routes
and two overlays. All four initial roots are explicitly required. This is not
the separate 116-query physics campaign, and final reachability does not
reconstruct the original required/auxiliary ancestry of a derived emission.

The retained inventory is
`TMP/rule-optimizer-20261003/profiles/demand-slice-q1707-a2738-v1/inventory.json`.
It contains complete native records, scopes, merge/dispatch stamps, anchor
records, all fourteen incoming-parent geometries and sealed-input hashes.
Its read-only graph analysis took 1.025 seconds, without native replay.

Both Q1707 and A2738 are Apply domains for owner2 `101010000110001`.
Q is the physical point
`(1,-2,1,0,1,0,0,0,0,1,1,-1,0,0,1)`, with A6/R3/D3.
A fixes six positive powers to one and permits nine nonpositive coordinates
with total numerator rank at most three: exactly 220 integer points, A≤6,
D∈[3,6]. The alias Q→A is an uninspected T3 record at merge46. A's later
merge161 G2 record inspects only its D=3 residual (165 points), retaining the
complement's dependency on anchor417/stamp26. Thus A was not already locally
inspected at alias time, and its G2 record is not a whole-A native application.

| Immediate parents of A2738 | Recorded type | Relationship |
| --- | --- | --- |
| 1665,1668,1672,1698,1700,1703,1707,1710,1713 | uninspected T3 aliases | synthetic reuse, not native RHS images |
| 741,745,897,4830 | whole native Apply | native inspections, post-resolution graph edges |
| 2738 | G2 residual self-edge | retain residual scope and anchor obligations |

**All fourteen are internal to the same 334-node SCC.** That SCC has 2,296
external incoming edges from 1,628 parents: 1,331 native Route, 212 whole
native Apply, 52 G2 Apply and 33 aliases. Replaying four local Apply parents
would therefore measure a circular internal image, not the independently
justified incoming demand union. The existing Apply observer cannot observe
the 1,331 Route parents. No such narrow observer was launched; no replacement
union or scheduler request was produced.

A's final cone has 2,331 nodes, 1,912 native records and 1.061 summed inspection
seconds, but deleting only Q→A loses zero nodes reachable from the four roots.
The recorded cycle 417→1707→2738→417 is an abstract obligation cycle, **not a
cycle of concrete integrals**. Neither its cone nor its cardinality ratio is
an exclusive-work saving. The archived graph's cold All PASS certifies its
scoped coverage, not independent termination or coefficient back-substitution.

Evidence pins: records file SHA256 `d5342b7b…002dc6`, dependency-part
`5303a87e…bb0cf`, initial-input part `c9c1c225…baede`, all under
`candidates/lower-owner-0-3-fourpoint-baseline-v1/baseline/checkpoints/main/`.
The inventory has full digests; the original graph analysis is
`profiles/lower-owner-0-3-pointgraph-result-v1.json` under the same TMP root.

## What the literature suggests—and does not prove

**Refine the demanded relation, not merely its node label.** Clarke et al.'s
CEGAR procedure tests whether an abstract path has a compatible sequence of
concrete transitions, then refines a spurious prefix; §§4.2–4.3 also discuss
loop counterexamples. The relevant lesson is that individually plausible
edges need not compose on the same states. An alias here is not a physical
transition at all, so an SCC cannot simply be fed to an algebraic cycle solver.
A demand-specific native image is the required witness. This analogy supplies
neither an IBP identity nor authority to delete side exits.
[Counterexample-Guided Abstraction Refinement, §§4.2–4.3](https://www.cs.cmu.edu/~emc/papers/Conference%20Papers/Counterexample-guided%20Abstraction%20Refinement.pdf).

Stein, Chang and Sridharan distinguish a requested context from a reusable
procedure summary and retain dependencies needed for consistent reuse. Their
§5 termination argument explicitly uses convergence of widening at recursive
calls. It cannot prove concrete IBP termination or justify replacing a wider
unfinished obligation by a clipped graph. This was already discussed in the
shared-policy note; it is not a new demand-slice invention.
[Interactive Abstract Interpretation with Demanded Summarization, §§4–5](https://www.bennostein.org/toplas24.pdf).

**Accelerate a proved relation, not a graph SCC.** Bozga, Iosif and Konečný
derive exact iteration for ultimately periodic classes, including
difference-bound, octagonal and finite-monoid affine relations. Their §4.3
formula requires every intermediate guard, not just endpoint membership.
A fixed index translation has identity linear part and is a promising special
case; an ordered union of guarded rules with polynomial coefficient zeros and
all RHS branches is not automatically in that class. A support summary also
does not sum products of rational IBP weights.
[Fast Acceleration of Ultimately Periodic Relations, §§3,4.3](https://www-verimag.imag.fr/TR/TR-2010-3.pdf).

Leroux and Sutre's Theorem3.4 relates termination of their accelerated
reachability semi-algorithms to flatness: reachability represented by finitely
many bounded path schemes. Their proof does not assert that an arbitrary
branching SCC is flat, and their algorithm is not a ready-made finite-time
solver for our guard language. Before proposing acceleration, find a repeated
native translation motif whose complete side exits and intermediate predicates
are demonstrably compact.
[Flat Counter Automata Almost Everywhere!, §3](https://drops.dagstuhl.de/storage/16dagstuhl-seminar-proceedings/dsp-vol06081/DagSemProc.06081.4/DagSemProc.06081.4.pdf).

## Existing services already cover much of the proposed mechanism

The public `RoutedCandidateReducer::trace_targets*` follows exact physical
`IntegralKey`s with shared deduplication across all input roots. The parallel
version retains campaign-aggregate limits, cancellation and joined workers.
Apply specializes native coefficients and coalesces locally before publishing
keys. Route's private `transport_support_with_usage` retains the exact
Symbolica polynomial and visits its nonzero, coalesced support; it is **not**
the symbolic Route overcover. The public `Prepared::transport` additionally
returns native coefficient-weighted endpoints. No coefficient display parsing
or new CAS is needed for either operation.

Relevant source is `solver/candidate_reduction/routed/{trace,campaign/worker}.rs`,
`sector/symmetry/integral_transport/transport.rs`, and
`family/numerator_expansion/visit.rs` in rustred-core. The existing CLI
`routed-campaign` performs the same full-selection preparation, accepts exact
CSV roots and `--entry-domains`, and runs the shared concrete trace. Entry caps
are checked only at admission; descendants are not clipped to them. It returns
a finite diagnostic summary, not a retained graph, portable source proof,
global cancellation result, CP6 cold certificate or reduction to masters.

For successful concrete steps, a useful conditional finiteness argument is
already available: same-support Apply strictly decreases an admitted
well-founded owner order; leaving the owner requires a support-cardinality
drop; each verified route preserves that cardinality or pinches further.
An infinite path would eventually have no support drops and contradict the
local order. Each step branches finitely, so finite roots give a finite tree
by König's lemma. This assumes those exact native checks succeed throughout;
an uncovered rule, pole or resource refusal remains an incomplete result. It
is not an ETA, small cardinality bound, or proof about symbolic containment
edges and unbounded auxiliary regions.

Nor is an inductive finite envelope new: the existing
`foundry/artifact/source_port/total_excess.rs` proves sector-dependent simplex
bounds with unchanged source/guard/descent checks. For its admitted
support-primary, total-excess-primary order, E does not increase at fixed
support; a pinch shift s satisfies
`Echild ≤ Eparent + ||s||₁ + kparent − kchild`.
The required order and complete sector census must be checked, not assumed for
every saved program. `walking/verify_closure/graph.rs` already accepts sealed
SCCs coinductively. Neither a new SCC algorithm nor another loose finite box
addresses the precision question.

**API boundary.** Pinned Symbolica3.0.0 provides exact polynomial grouping,
restriction and rational arithmetic; RustRed's authenticated
`IndexedCoefficientContext::specialize_fixed_indices` also retains the raw
denominator witness. Current public Symbolica3.0.1 documentation exposes
polynomial/exact-algebra services, but this review found no ready public
Presburger transitive-closure service matching our guarded integral relation.
This is a scoped API finding, not a claim that no such software exists.
[Symbolica Rust API](https://docs.rs/symbolica/3.0.1/symbolica/),
[polynomial services](https://symbolica.io/docs/polynomials.html).
For comparison, isl's documented `isl_map_transitive_closure` may return an
overapproximation and exposes an exactness flag. Adding it would introduce a
separate dependency/proof boundary, not make polynomial guards Presburger or
make an overapproximation safe for dropping obligations.
[isl manual, transitive closure](https://libisl.sourceforge.io/manual.pdf).

## Completed falsifier: the same four exact roots jointly

The original query file contains four distinct singleton boxes; all lower
coordinates equal upper coordinates. Conversion is exact:
active physical power `1+local`, inactive physical power `−local`.
Their actual A/R/D values are respectively `(7,3,4)`, `(7,3,4)`, `(9,1,8)`
and `(9,2,7)`. Preserve the original, sometimes looser, caps and all four
required IDs through `--entry-domains`; do not replace them by a fitted scope.

The sole existing-CLI diagnostic was preregistered in
`profiles/fourpoint-concrete-trace-v1/plan.json`, with literal `targets.csv`,
command, original-query pin and unchanged full67/8246/2 baseline manifest.
It uses the tested subset binary `a3c9e542…2570ec`, W16/CPUs32–47,
150GB owned memory plus150GB host reserve, 540s cooperative/570s hard and600s
inclusive guard. Limits are fixed at one million operational nodes and rule
applications,16million coalescing additions,64million transport-operation
allowance and4million pre-coalescing transport endpoints; native per-call
limits remain unchanged. No retry, policy search, source search or resizing.
It completed cleanly, with all owned processes drained. No new observer
adapter was required. `native-result.json`, `events.jsonl` and
`execution-result.json` in that directory retain the raw observations.

| Exact four-key result | Observed value |
| --- | ---: |
| distinct physical keys / operational nodes | 44,139 / 44,143 |
| rule applications / exact transport calls | 19,201 / 14,587 |
| declared terminal keys / proved-zero keys | 112 / 10,239 |
| duplicate scheduling hits | 393,143 |
| maximum numerator rank / dot excess | 3 / 6 |
| native trace / native preparation-plus-trace | 0.312s / 88.654s |
| whole guarded attempt | 93.058s |

There are no failed, queued or active nodes, missing rules/owners, frontier,
native error, cancellation or cap hit. The reported 1,947,422 transport
operations and 368,592 endpoints are respectively conservative operation and
projected pre-coalescing endpoint **budget usage**, not a count of exact
coefficient monomials or final emitted endpoints. Preparation and small native
handoff/report overhead account for about88.342s by subtraction; the retained
heartbeat does not provide a finer preparation-phase timing decomposition.
Independent audit checked the hashes, complete-state fields and owned drain.

The same-build symbolic primary baseline has19,112 domains and17,479 native
inspections,1.006s traversal and87.361s preparation, with cold All PASS. These
are **different graph objects** from exact physical/operational keys, not a
like-for-like backend timing race. The exact trace is affordable but does not
produce fewer counted objects: it supplies no demonstrated symbolic-overcover
count reduction or exclusive-work attribution. Its0.312s versus1.006s traversal
is a possible **finite-workload representation** signal despite more objects,
not a matched repeated performance result. Whole cost is preparation-dominated;
the symbolic total also includes independent cold verification absent from the
concrete diagnostic. No speedup, closure or back-substitution claim follows.

The result does not identify whether Route expansion, Apply guard precision,
containment reuse, publication or other bookkeeping causes that local timing
difference. Accordingly it does **not** activate the proposed singleton-Route
exact-image bridge. That separate test would change only Route images in an
otherwise identical symbolic walk and require cold-verifiable retained native
images. Preseeding narrow nodes, clipping A's edges, disabling G2 or toggling
inspector lookup does not prevent coordinator re-aliasing. The representative
ten-point D2/D12 slice also remains deferred: it is neither external-demand
completeness nor a substitute four-root test.

## Next discriminator: bounded exact discharge, not another Route heuristic

The distinct hypothesis is that a small, exactly admitted physical demand can
be discharged by a fresh bounded exact traversal of an immutable prepared
program bank, instead of publishing all abstract descendants. It may be useful
even when it visits more cheap concrete objects. The existing
`RoutedCandidateReducer` already owns shared immutable programs and verified
route transports; each `trace_targets*` call creates fresh request worklists
and deduplication state. `RoutedFeedbackSession::routed_reducer()` exposes it
after ordinary full-context preparation. No source-feedback round, policy
mutation, large expression clone or shared discovered-domain state is needed
to measure this hypothesis.

The immediate no-rebuild discriminator is the unchanged original H55 point
`(0,3,1,1,0,1,1,1,0,2,1,1,0,0,0)` (A12/R0/D12), preserving its original
A≤16/R≤4/D∈[12,14] entry caps. The frozen proposal is
`profiles/h55-concrete-trace-v1/{plan.json,command.json,targets.csv}`.
It keeps the same67/8246/2 manifest, tested `a3c9e542…2570ec` executable,
one-million-node and other finite allowances, W16/CPUs32–47 and600s inclusive
guard. The single authorized execution completed without retry or resizing.
Raw `native-result.json`, `events.jsonl` and `execution-result.json` are retained
beside the plan; all owned processes drained and CPU32–47 was released.

| Exact H55 result | Observed value |
| --- | ---: |
| distinct physical keys / operational nodes | 355,584 / 356,202 |
| rule applications / exact transport calls | 164,074 / 145,366 |
| declared terminal keys / proved-zero keys | 248 / 45,896 |
| duplicate scheduling hits | 3,265,937 |
| maximum numerator rank / dot excess | 4 / 11 |
| native trace / native preparation-plus-trace | 2.759s / 91.286s |
| whole guarded attempt | 95.087s |

The original entry geometry and caps remain in the native receipt. There is
no frontier, missing rule/owner, failed/queued/active work, native error,
cancellation or cap hit. Projected endpoint allowance2,763,255 remains below
4million; transport-operation allowance15,507,763 remains below64million.
Neither is an exact emitted-coefficient count. Preparation and minor native
handoff/report overhead are88.527s by subtraction. Sampled peak owned RSS is
5.215GB; it is neither total host memory nor an algebraic storage bound.

The historical symbolic reference has385,477 domains,314,824 native
inspections and27.708s traversal, cold All PASS. It used executable
`8ef80b52…cf0a7`, so it is **not a same-build performance control**. The2.759s
exact trace is a strong affordability signal despite a broadly similar number
of differently defined objects. It warrants a separately authorized
matched-build symbolic comparison before designing a summary feature. It is
not a tenfold speedup claim: symbolic publication and cold verification retain
stronger evidence, scheduling differs, and the runs are not paired repeats.
The old whole-arm224.904s includes cold verification and must not be compared
to95.087s as equal work. No source generation, source replay, new terminal,
coefficient back-substitution or finite-summary installation occurred.

### Same-build symbolic control: the local contrast persists

A subsequent authorized baseline-only run used the **same frozen a3c9
executable**, original H55 query bytes, complete67/8246/2 context and original
symbolic walk settings. It did not alter the scheduler or finite-work policy.
All phases drained within251.594s inclusive; no retry was needed. Evidence is
`candidates/h55-samebuild-symbolic-baseline-v1/`, especially
`single-arm-observation.json`, `baseline/{measurement,cold-all}.json` and
`baseline/run/result.json`.

| Same-build measurement | Exact trace | Symbolic baseline |
| --- | ---: | ---: |
| physical keys / symbolic domains (different objects) | 355,584 keys | 385,815 domains |
| rule applications / native domain inspections (different units) | 164,074 | 314,985 |
| preparation or preparation-plus-minor-overhead | 88.527s, by subtraction | 88.323s native field |
| traversal wall | 2.759s | 34.760s |
| sampled peak owned RSS, traversal process including preparation | 5.215GB | 5.538GB |
| waited user+system CPU, traversal process including preparation | 892.370s | 1,020.562s |
| independent symbolic cold verification | absent | PASS, 114.923s native |

The symbolic graph has2,655,808 edges and5,344,012 native events. Cold All/Off
re-inspected every314,985 native record with no error/frontier/uncovered image
or count mismatch, independently verified its one required root and found
all385,815 records closed. Its70,840 abstract-cycle nodes are not concrete
cycles. The live walk intentionally retained checkpoint-only `status=incomplete`,
`finalization=not_evaluated` and stale closure counters; the complete scoped
coverage conclusion comes from cold verification, not rewriting those flags.
The evaluator reports `completed_cold_verified`, no issues and zero debt.

Cold component timers are7.152s load,87.598s preparation,13.010s reinspection
and6.893s checks; roughly0.271s of other overhead remains in the114.923s total.
Guarded cold wall is118.624s, sampled peak RSS5.357GB and waited CPU1,081.357s.
The full symbolic arm is249.131s (251.594s including outer setup/postprocessing),
versus95.087s for the exact-only attempt. These totals remain **unequal work**:
the exact lane has no independent cold certificate or persisted symbolic graph.
CPU numbers are waited child/descendant accounting, not sampled utilization;
symbolic coordinator/subphase timers overlap and must not be summed as savings.

The build confound is now removed for this observation, but one-shot timing,
different schedulers, granularity and retained outputs remain. The roughly
12.6-fold traversal contrast justifies investigating a bounded exact-discharge
kernel; it is not an achieved12.6-fold program optimization. A worker-budget
check is still essential before nesting a trace inside existing inspectors:
sixteen inner workers per inspector would be oversubscription, not a fair
implementation of this measurement. No finite-summary feature was implemented
at this measurement stage.

The existing CLI couples route preparation and tracing to the same worker
count (`routed_campaign/prepare.rs` passes `request.workers` to route
verification). Retained W16 heartbeat observations show owner import around
3–15s, overlay replay around16–33s and map verification around34–88s. These
one-second samples locate work, not exact nonoverlapping timers; they do not
justify blindly projecting a serial preparation ETA. A600s W1 proposal was
therefore superseded **before any execution**, not rescued after a cap failure.
The independently reviewed single W1 diagnostic is recorded under
`profiles/h55-concrete-trace-w1-v1/`: affinity still reserves32–47, requested
preparation/trace workers are both1 and inner pools remain1. Physics, entry
caps, binary and all finite work allowances are unchanged; only the fixed
wall allowance is prospectively1800s inclusive (1680s cooperative/1740s hard).
It completed in139.877s inclusive, with no retry, error, frontier, cap hit or
undrained process. Native preparation-plus-trace was134.819s; the **serial
trace took13.627s**, leaving121.192s preparation and minor overhead by
subtraction. Sampled peak RSS was5.223GB and waited CPU137.455s for the whole
prepared traversal. The conservative1800s prospective allowance was not
needed, but it was not changed in flight. No600s W1 attempt was executed.

`worker-count-comparison.json` records complete final-snapshot equality with
W16 after deleting exactly `workers` and `elapsed_seconds`, plus exact
entry-admission equality. All physical/operational/application/transport,
terminal/zero, duplicate-work, degree and debt counters agree. This is a useful
aggregate worker-count check, not retained key-set/coefficient equality: the
CLI does not publish those complete objects. Nor does the whole-process CPU
difference isolate tracing from route preparation or identify its cause.

The serial13.627s versus symbolic W16 traversal34.760s retains a meaningful
safe-inner-kernel feasibility signal (about2.55-fold local wall contrast,
not the12.6-fold W16 contrast). It does not measure a nested implementation,
its overlapping-summary duplication, cold replay or equal-output cost.
Specifically, W1 still uses the campaign FIFO API, which spawns one worker
and supports cooperative cancellation between native operations. This is
**not** a timing of the separate inline DFS `trace_targets` API, which exposes
no cancellation hook. Substituting that API inside inspectors would require
separate cancellation/budget design; this receipt does not authorize an
uninterruptible inline kernel or promise zero thread-launch overhead.
The next gate is a small separately reviewed finite-discharge/cold-recipe
design with cumulative budgets and unchanged original-source claims, not
another worker sweep or automatic feature installation. No further native run
or finite-summary implementation was performed during those diagnostics.

**The certification seam identified by those diagnostics.** Core
`candidate_reduction/model.rs` explicitly says finite reachability is not a
certificate, even for its entries: source provenance is not replayed by that
reducer. The concrete summary retains no replayable per-node graph or exact
used-rule ledger. Existing `SourcePortAudit::replay_sector_rule_batch` is the
identity half: it replays selected ordinary-source circuits and their guards,
but separately requires coverage of every actually used rule and declared
endpoint. The CP5/CP6 cold oracle re-inspects symbolic domain records and exact
alias/anchor coverage; the diagnostic build could not accept a concrete-trace summary as
a sealed replacement obligation. No current public service inspected here
turns this CLI summary directly into a `ClosedArtifact`. This does not mean
storing every internal edge is logically necessary: an audited **cold replay
recipe** can instead bind the exact finite input set/enumeration policy,
complete immutable programs/routes/terminal convention, algorithm version and
limits, then require a fresh full exact trace with no error or frontier.
The verifier must recompute success, not trust the saved counters. Such an
explicit finite-closed disposition and request/checkpoint binding were missing.
The bounded implementation below addresses that seam. Replay establishes only
scoped reachability under the admitted candidate
identities; it must preserve, not silently strengthen, the current separate
original-source audit claims.

A later finite-discharge record would need immutable family/owner/route/policy
identity, exact admitted demand membership, all native Apply/Route guards and
tails, a cold reinspection contract (complete retained images **or** bounded
fresh replay), and original-source authority wherever claimed. It must not
declare the input a new terminal. Per-summary and cumulative walk budgets,
cancellation and fail-closed algebra/descent handling remain mandatory; only
explicitly safe unsupported/frontier/budget outcomes could defer to ordinary
symbolic work, never turn a hard error into successful discharge.
Such a record could cover only an explicit finite cell; it cannot generalize
its enumerated tuples into a parametric recurrence. Reusable symbolic output
would still require source-proved parametric rules and explicit coverage of
their remaining domain. Thus this is a possible finite leaf in a reusable
program's proof/workload graph, not a hidden switch to per-integral tables.

One fast whole-root trace is also insufficient evidence for many cheap
summaries: independently retracing each overlapping demand could repeatedly
visit the same large downstream subgraph. The current trace shares exact-key
deduplication **within** one request, but fresh calls intentionally share no
work state. Any future within-walk proved-key reuse must bind the identical
program/route/source-condition context and retain certification and cumulative
accounting; no such cache may leak between policy trials. Measure aggregate
duplicate work, cold replay, retained-state cost and whole-walk cost against a
contemporaneous control before claiming benefit from prepared-bank reuse.

## Authorized implementation: one fresh singleton replay, acceptance pending

The narrow first slice adds `OwnerDomainWalkRequest::finite_replay: Option<
OwnerDomainWalkFiniteReplayLimits>` and CLI `--finite-replay-initial-singleton`.
Omission is the legacy path: no new request-binding marker, record trailer or
finite-work report. Enabling it requires a **fresh CP6 epoch checkpoint**;
runtime resume, amendments, CP5 and memory-only walks are explicitly refused.
Cold checkpoint loading and verification are intentionally still supported.

Only the original first query, admitted as initial ID0, can be attempted. Its
actual dispatched image must be whole, coordinate-singleton and exactly equal
to that original query, including rank and A/D caps. An empty cap intersection
declines rather than tracing the excluded point. A source-validity frontier,
later ID0 substituted after an inadmissible first query, partial D-band or G2
scope cannot become a summary. Existing G2, containment and D-band policies
remain enabled for all other ordinary work; they are not globally disabled to
make the experiment succeed. A fresh per-walk atomic reservation prevents a
second attempt after cancellation, decline or redispatch.

The new core caller-thread API uses the **same checked FIFO campaign kernel**
as the measured cancellable tracer, not the separate DFS API. It starts fresh
key/worklist state, shares immutable admitted algebra and route payloads, and
spawns no worker. Existing native Apply coalescing, original poles/source
conditions, exact transport, terminal/zero convention and descent checks remain
authoritative. Cancellation is cooperative between native operations, not an
interruption of Symbolica. There is no new algebra primitive, source audit,
terminal, coefficient back-substitution, cross-attempt cache or parametric
generalization.

The six CLI allowance suffixes below are prefixed `--finite-replay-`:

| Allowance | Default | Charged quantity |
| --- | ---: | --- |
| `max-nodes` | 1,000,000 | entry, unique operational-state and pending-work admission |
| `max-rule-applications` | 1,000,000 | native attempts/reservations, not only successful formulas |
| `max-transport-calls` | 1,000,000 | native routing calls |
| `max-transport-operations` | 64,000,000 | conservative native expansion-operation bounds |
| `max-transport-endpoints` | 4,000,000 | projected pre-coalescing endpoint bounds |
| `max-coalescing-additions` | 16,000,000 | native coalescing reservations and work |

Zero is an explicit zero-work allowance. At app preparation the node/transport
fields configure the concrete-trace aggregate slots, replacing unrelated
`routed-campaign` defaults which symbolic walking previously did not consume.
This matters because its default100,000 transport calls would censor the
already observed H55 workload. The core attempt then min-intersects its budget
with the **actually admitted** trace and matching-reduction limits. Original
per-formula expansion, scratch, algebra and matching limits are not enlarged.
Online preparation and cold replay use the same policy mapping.

Online decline is deliberately narrow: ineligible geometry, native frontier,
or explicitly typed aggregate/transport-expansion resource exhaustion may
fall through to ordinary symbolic inspection, once. Cancel, panic, source,
context, algebra, descent and unsupported-support-transition errors remain
errors; a name containing “unsupported” is not a fallback classifier. Separate
finite-work diagnostics retain applications, attempts, exact transport work,
conservative reservations and failed/declined prefixes. These do not masquerade
as ordinary symbolic successor counts, and total inspector elapsed includes
the attempted trace plus any subsequent fallback.

A successful result has explicit native kind `FiniteReplay` and typed
`Scope::FiniteReplay(Recipe)`, with version and six limits. Its enclosing typed
record preserves the full original domain; existing request/payload digests
bind the immutable bank. It has a completion marker, not fabricated ordinary
RHS edges or an invented terminal. P1 and record validation reject partial,
reused, malformed, error/frontier or non-ID0 versions. Diagnostic JSON and
saved success counters are **not proof**. Cold `reinspect=All` with
`reference-levers=Off` must reconstruct the same original membership and
perform a fresh complete exact trace. Any replay frontier, budget failure,
cancellation or hard error fails certification; cold never falls back to
ordinary symbolic coverage.

Implementation lives in core `routed/campaign/inline.rs`, app
`walking/finite_replay.rs`, the small epoch inspector/merge adapters, typed
record/codec and `verify_closure/finite_replay.rs`. Core focused tests passed
(119 routed tests including seven new inline cases, plus219 owner tests),
including caller-thread identity, mid-trace cancellation, failed reservations,
original pole/source conditions and late descent failure. A preserved first
test-fixture failure used an out-of-range synthetic index; its replacement
uses400 legal lower children without reducing the256-operation callback gate.
Independent app source review passed. The first semantic-test compile found
four legacy test literals missing the new optional field; the continuation
adds `None`, preserving their semantics. The v3 semantic-test binary
`dee1c247…a74e9` passed the22-test finite-replay filter, including typed
mutation/cold negatives and late resource-plus-cancellation classification.
The five other focused filters passed3 record,134 restore,6 input,44 CLI and46
cold-verifier tests. These overlapping filters must not be summed; the cold
filter also retained one previously ignored exploratory sunset fixture, not a
new finite-replay skip. Semantic tests used the declared32MiB libtest stack;
the native acceptance controls will not inherit that override.

Final optimized app and CLI linking passed in539.472s and18.888s respectively,
with clean owned-process drain; the tested CLI is `c862ba6e…a121be`.
Default-off native compatibility and the same-build H55 off/on native control
remain acceptance gates. The prepared-only
commands are in
`TMP/rule-optimizer-20261003/candidates/finite-replay-acceptance-v1/plan.json`:
original immutable staged inputs, fresh outputs, unchanged G2/D-band/rolling
controls, direct existing guard and mandatory cold All/Off. The thin runner
binds the final executable hash to its successful link receipt at launch;
independent command/runner review passed. No finite-replay native acceptance
or performance result exists yet.

## Conditional geometry lead: exact support as an integer-flow relation

This is a distinct, **unimplemented** possibility, not a claim that another
Route optimization is worthwhile. The previous five-group H55 deletion screen
removed232,654 edges but disconnected only8,675 nodes (2.25% of385,477) and
1.60% of summed inspection time. That fixed-graph optimistic screen is not a
bound on a changed program, but remains negative evidence against prioritizing
a large routing feature for those groups.

An authenticated affine numerator map has factors
`L_i(x) = sum_j a_ij x_j`, including a dummy constant column `x_0=1`.
For fixed nonnegative integer source powers m, a monomial contribution to
`product_i L_i(x)^m_i` is an integer allocation z with

```
z_ij >= 0;  z_ij = 0 whenever a_ij = 0;
sum_j z_ij = m_i;  sum_i z_ij = e_j.
```

Suppose the nonzero rational coefficient signs factor as
`sign(a_ij)=r_i c_j`, with row and column signs in `{−1,+1}`. Every contribution
to the same exponent vector then has the same sign
`product_i r_i^m_i product_j c_j^e_j`; positive multinomial factors cannot
cancel. Thus this integer-flow relation describes **exact nonzero support of
one product**, not merely its convex hull. Eliminating the constant column is
safe because `e_0=sum_i m_i−sum_{j>0} e_j` is fixed. Zero factors and zero
powers need their ordinary native cases. A failed sign test is inconclusive,
not proof of cancellation. Nor does the result prevent cancellation across a
sum of different transported terms or establish guard/descent/closure facts.

The sign criterion is the balanced signed-bipartite-graph condition: switching
row/column signs makes every supported edge positive. Zaslavsky gives the
equivalence and spanning-tree recognition in §§2.1–2.2; the application to
multinomial support above is our elementary inference, not an IBP theorem in
that paper. Only genuinely rational-constant signs are admitted here, not
assumed signs of symbolic functions.
[Signed Graphs and Geometry, Theorem2.1 and §2.2.2](https://people.math.binghamton.edu/zaslav/Tpapers/sggm.pdf).

**Native fit and missing representation.**
`sector/symmetry/integral_transport/compile.rs` already admits rational-constant
inactive affine rows, unit active-row bijections and retained family/Jacobian
conditions. Public `Prepared::verified_map().denominators()` exposes the typed
coefficients; `Prepared::transport` supplies the exact Symbolica endpoint
oracle. No coefficient-text parser or new algebra engine is needed. Current
`domain_overcover/support.rs::JointSourceSupport::can_pinch` checks the total
cost of a removed-column set against its union of supplying rows. It is not
the full transportation/Hall system and does not retain joint exponent
correlations. However, the current box/rank/A-D payload cannot represent a
general existential flow relation without losing correlations again. A new
typed relational boundary would therefore be a real architectural obligation,
not a one-line tighter bound.

Barvinok–Woods Theorem1.7 shows that integer projection can have a compact
rational-generating-function representation **in fixed dimension**; its
dimension-dependent complexity is substantial. It neither solves coefficient
cancellation nor supplies an existing RustRed/Symbolica service. Current
Symbolica polynomial services are suitable for native exact expansion and
comparison, not a justification for adding a general Presburger engine.
[Short Rational Generating Functions, Theorem1.7](https://arxiv.org/pdf/math/0211146),
[Symbolica polynomial services](https://symbolica.io/docs/polynomials.html).

**Smallest prospective falsifier, no run authorized.** Reuse only the admitted
four-loop map in `domain_overcover/joint_support_tests.rs::fixture4`, not a
campaign-wide search. First inspect its native typed row signs. If certified,
compare the flow-predicted support against `Prepared::transport` for the fixed
sixteen points with source numerator powers on axes0 and2 each in0..3, other
inactive powers zero and the four active powers one. Require complete exact
native support equality and at least one unreachable tuple admitted by the
current joint-pruned cover. A hand-derived nominee is source
`(-1,1,-1,1,1,0,0,0,0,1)` and target
`(0,1,1,1,-1,0,0,0,0,0)`: it would consume two units from one numerator row
whose supply is one. This is **not yet a native-observed counterexample**.
Stop on failed sign admission, no extra precision, any oracle mismatch or
budget refusal; no automatic map/degree search. Even a pass establishes only
a capability witness, not worthwhile full-context cost or permission to
replace current routing.

This also bounds the finite-proof-lifting analogy. Translating a finite DAG
usually translates its terminal keys away from the unchanged finite terminal
boundary; matching several neighboring traces is not a reusable parametric
proof. Path-focused abstract interpretation suggests proposing an invariant
then checking every escaping path, but its termination proof relies on
widening and its SMT transition language does not automatically cover our
polynomial pole/zero conditions. Here any candidate relation still needs
complete native image, original guards, side exits and checked terminal
coverage. Tube/standard-pair discovery and fixed-translation corridor
acceleration already appear in prior research; they are not new names for
this experiment or for the current singleton replay implementation.
[Using Bounded Model Checking to Focus Fixpoint Iterations, §§3.2–3.5](https://arxiv.org/pdf/1106.2637).

## Bolder alternative: eliminate an independent loop, not an SCC

Transverse-integration identities factor sectors with disjoint denominator
blocks despite polynomial cross-block numerators. The paper performs tensor
decomposition one block at a time, retaining the remaining loop momenta as
numerator variables; lower-loop reductions remain separate obligations.
This suggests eliminating an entire repeated numerator recurrence, not
reordering the same graph.
[Reduction to Master Integrals and Transverse Integration Identities, §§3.2,4.2](https://arxiv.org/html/2409.04783v2).

The archived family `root-0001.toml` confirms that owner2's active
`D1=k1²−1` is independent of the other active denominators
`D3,D5,D10,D11,D15`, which form a four-loop five-line banana. However Q1707's
numerator is only `D2² D12`, independent of k1. All nine incoming aliases also
use only D2,D4,D9,D12. Angular projection is tautological there; they are not
mixed-numerator savings witnesses. The broader220-point A includes D6,D7,D8,D14
couplings, whose true externally demanded occurrence/cost remains unmeasured.
Owner0 is a connected five-loop six-line banana, not this product sector.

There is already generic code at
`foundry/artifact/factorized_product_moments/{compile,angular,partial_angular,runtime}.rs`.
It implements bounded native scalar angular recurrences and retains traversed
`d,d+2,…` guards. But this is private execution of an authenticated factorization
recipe with `ClosedArtifact` dependencies, not a candidate-source oracle.
The current correlated-block compiler requires a complete active scalar-product
basis: ten active denominators for a four-loop block, whereas this remainder
has five. It cannot simply be plugged into this owner, and an old K6 prototype
description is not evidence of current candidate-path admission.

A narrower prospective nomination is the scalar angular relation
`⟨(k1·v)²⟩ = k1² v²/d`, for k1-independent v, with arbitrary positive powers
on the original active denominators and suitable spectator numerators. The
brackets denote angular averaging (or the corresponding rotationally invariant
integrated identity), **not pointwise equality of integrands**.
Here the isolated vacuum block has no external Gram denominator; angular
projection introduces dimension factors, not a new loop-dependent mass pole.
Expressing the scalar products in the original fifteen denominators keeps the
family, but pinched terms, radial shifts, d≠0 and every original condition
remain. This is a nomination, not a proved saved rule: require exact original
ordinary-IBP source reconstruction, full RHS, unchanged native descent and
guard/export checks. The existing tangent-source C19 experiment already
targeted the isolated loop and was negative; do not rebrand it as new evidence.

Cross-family factorization of Q itself could still reduce a scalar tadpole
times a four-loop block, but that is not an angular shortcut: it needs admitted
lower artifacts and a terminal/master-basis mapping. Alternatively, lifting a
four-loop recurrence into the same fifteen-axis family needs authenticated
original-row/coordinate embedding and full ordinary-source replay. Those
rest-loop ordinary rows already exist. The prospective novelty is economical
program structure and reuse, not new identities supplied for free by the
closed-product module.

The cheapest reopening condition is one **recorded, actually demanded mixed
numerator** whose complete native workload contains appreciable work exclusive
to that mechanism. Q1707 does not qualify, and the broad cone's shared cost
does not supply it. Until then, neither a product-artifact plugin nor a general
Presburger/acceleration framework is justified. The four-root result and fixed
H55 diagnostic are workload evidence only; these larger alternative
architectures remain deferred.
