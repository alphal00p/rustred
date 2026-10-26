# Reduction scheduling study: source audit and held pilot design

Date: 2026-10-26. Status: **SOURCE DESIGN; ALL SCIENTIFIC SCHEDULING DATA HELD**.

This note defines a bounded experiment in the visitation order of already
admitted reduction work. It records source contracts and hypotheses, not a
mathematical proof, a measured speedup, a production recommendation, or an
authorization to run actual five-loop work. The source audit uses native
baseline `b209adb1e774242bcbe0417fed223317566561bd` in
`workspace/5l-push/native-selected-cache-export`. References below use that
baseline's repository-relative paths and approximate line spans, before the
scheduling patch. The opt-in API was also inspected in the isolated
`workspace/5l-push/native-reduction-scheduling` worktree.

No actual five-loop reduction was run to prepare this note. Pure fixtures may
establish dispatch, cancellation, determinism, budget and cache behavior; their
timings cannot establish five-loop performance.

## The experiment and its limits

The implemented experimental surface is
`CandidateRoutedScheduling { roots, dependencies }`, with each dimension chosen
independently from `CandidateRoutedWorkOrder`:

| Order | Descending priority, before the original exact-key tie break |
| --- | --- |
| `Canonical` | Existing ascending integral key; dependencies use the existing full operational node order. |
| `NumeratorRankDescending` | Numerator rank, dot excess, support count, positive-power sum. |
| `PositivePowerSumDescending` | Positive-power sum, numerator rank, support count, dot excess. |
| `PropagatorCountDescending` | Support count, dot excess, numerator rank, positive-power sum. |
| `CornerDistanceDescending` | Numerator rank plus dot excess, support count, dot excess, numerator rank. |
| `CoefficientTermsDescending` | Existing sparse numerator plus denominator monomial count of the incoming coefficient. |

The default is `Canonical` on both dimensions. Every noncanonical priority has
a deterministic tie break using the original integral key or full operational
node, including Route/Apply phase and Apply owner. Equal scores do not permit
an implementation-dependent order. The coefficient score reads existing sparse
sizes; it does not expand algebra, estimate a child's completed coefficient, or
predict the full cost of its dependency graph. Priority vectors are heuristics,
not the mathematical ordering used to check formula descent. The API resides in
`crates/rustred-core/src/solver/candidate_reduction/routed/application/scheduling.rs`
in the scheduling worktree.

Roots still run one at a time to a completed terminal expansion, then contribute
to the output. Dependencies still use depth-first expansion and back-substitution.
Choosing a more difficult root or sibling first can change cache reuse and
intermediate output sizes. It does **not** merge weighted nonterminal descendants
from different roots before solving them. It therefore cannot, by itself, test
the stronger hypothesis that opposing contributions from different roots cancel
before those descendants need reduction. The initial duplicate-key coalescing
and cancellation already occurs before the priority is applied.

Within a dependency node, its exact term map and its arithmetic/guard combination
order remain canonical. Root priority does change the final accumulation order
in the present one-pass engine: each completed root is collected immediately.
Consequently final intermediate coefficient sizes and the first-seen order of
final guards may differ. Requiring canonical final root accumulation as well
would need a separate design, such as pinning all completed roots and collecting
in a second pass. That exchanges memory and cache pressure for a fixed aggregation
order and is not part of this implementation.

## Source contracts that scheduling must preserve

The following observations are bounded to the audited source. They do not prove
closure of an arbitrary candidate family.

| Contract | Baseline reference and implication |
| --- | --- |
| Entry checks precede input cancellation | `crates/rustred-core/src/solver/candidate_reduction/routed/application/engine.rs:880-934`: entry admission, coefficient validation, input denominator retention and duplicate-key accumulation precede root traversal. A zero combined input must not bypass the checks attached to the original inputs. |
| Roots complete before collection | The same `engine.rs:935-1005`: each root is pinned, `run_stack` completes, and terminal terms and guards are then collected. The scheduling patch reorders surviving roots at this boundary. |
| Ordered formula dispatch | `crates/rustred-core/src/solver/candidate_reduction/evaluator.rs:161-189`: first applicable `Partition` establishes the baseline; the first applicable whole-piece alternative may replace it only after baseline coverage exists. |
| Applicability uses the saved conditions | `evaluator.rs:200-263`: fixed coordinates, equalities, AND-zero exceptional branches and every original RHS denominator are checked before coefficient cancellation or RHS coalescing. An undefined expression is not treated as a zero identity. |
| Alternative guards include baseline coverage | `evaluator.rs:151-157`: retain the selected baseline's conditions and, when different, the selected alternative's conditions. Rejected alternatives supply no obligations. |
| Owner batches have ordered stopping semantics | `crates/rustred-core/src/solver/candidate_reduction/owners/evaluation.rs:47-50,83-100`: a later terminal must not shadow an earlier applicable formula. Only an exact `Uncovered` advances to the next batch; other errors remain errors. |
| Candidate formula edges descend | `evaluator.rs:266-300`: every surviving specialized child must compare strictly `Less` under the installed ordering, otherwise execution returns `NonDescending`. |
| Route and Apply are distinct states | `crates/rustred-core/src/solver/candidate_reduction/routed/trace.rs:12-16,89-108`: same-owner children use Apply; a proper sub-support with fewer propagators uses Route; unsupported transitions fail. Same-owner formula descent is checked separately. |
| Parent combination needs completed children | `routed/application/engine.rs:1070-1124`: combine reads completed child values, merges ordered guards, releases their dependency references after arithmetic/guard copying, then admits the parent. |
| DFS scheduling pins each dependency | `routed/application/engine.rs:1400-1417`: the baseline pushes reverse sorted child keys onto a LIFO stack. A changed visitation vector must retain these pin and frame operations. |
| Live dependencies are protected from eviction | `routed/application/cache_policy.rs:57-87,121-158`: reference-counted dependency pins control eligibility, including cleanup on failure. Partially computed nodes are never cached. |
| Closed-artifact priority is also explicit | `crates/rustred-core/src/reduction/reducer.rs:507-550,828-906`: seeded completed terminals precede zero/factorization/formula expansion; ordered cells precede direct rules. This is a separate execution path, not a rule-reordering permission. |
| Persisted mathematical order is authority | `crates/rustred-core/src/foundry/artifact/model.rs:225-228` and `install.rs:188-225,541-573`: closed rules, cells and canonicalizers share one persisted order; descent witnesses bind that order, domain, pivot and shift. |

Saved rule dispatch is explicitly independent of a rule's algebraic identity
(`crates/rustred-core/src/solver/sector.rs:25-34`). Reordering rules, owner batches,
terminals, exceptional branches, factorization owners or whole-piece alternatives
changes this contract; it is outside the scheduling-only experiment. Changing
the installed mathematical order or coordinate priority similarly requires
separate admission and descent work. Source guards, original denominators,
zero authority, entry scope, routes, terminal substitutions and unresolved policy
must remain fixed.

The `rustred-order` crate establishes integer ordering semantics, not rule
applicability, identities or descent on an application domain
(`crates/rustred-order/src/lib.rs:1-13`). Numerator rank, positive-power sum and
the other scheduling scores are not interchangeable with this authority.
Candidate reducers explicitly remain experimental and preserve unresolved gaps
rather than certifying a closed artifact
(`crates/rustred-core/src/solver/candidate_reduction/reducer.rs:44-46`).

## Cache identities and exact-result validation

The persistent binding contains the authenticated artifact/package digest,
family fingerprint, physical arity and solver layer. The digest must cover the
complete immutable runtime package, including routes, scope, zeros and terminal
substitutions (`crates/rustred-core/src/persistence/application_cache/mod.rs:19-27`).
The exact key is stage, owner and powers (`:46-82`); Route and Apply never alias.
Routed saved applications use layer `saved-application-route-apply-v1`, and a
producer revision label alone is insufficient
(`routed/application/persistent.rs:71-101`).

A visitation-only policy leaves those mathematical identities intact. Its
selection must nevertheless be recorded in the experiment manifest because
residency, access stamps, eviction, recomputation, first failure and resource
consumption can change. Cache policy is explicitly operational
(`crates/rustred-core/src/reduction/cache_policy.rs:7-18`). A cache hit is not
evidence that a different rule ordering or terminal convention is compatible.
Those changes require a new semantic binding and are excluded here.

Records contain completed exact expansions and ordered conditions, not eviction
metadata, work counters, pins or partial stacks
(`persistence/application_cache/mod.rs:1-6,142-147`). Routed lookup cannot invent a
missing owner or transport and validates the returned key and complete record
(`routed/application/persistent.rs:347-386`). Completed export materializes the
individual cached node and validates declared terminal ownership
(`routed/application/persistent.rs:465-501`). These contracts remain in force
under every schedule.

Correctness compares every terminal's full exact coefficient in the same
authenticated terminal convention, including exact zeros and terminal ownership.
Counts, selected coefficients, sampled numerical evaluation and payload hashes
alone are insufficient identity checks. Hashes authenticate inputs and outputs;
they do not replace the exact coefficient comparison.

Guard validation must compare the exact nonzero obligations or establish their
domain equivalence with the existing exact checker. Raw vector order is not the
sole correctness criterion. Duplicate obligations and first-seen order may
differ; canonical exact obligation-set equality is sufficient when representation
is shared. If representations or factorizations differ, require a checked exact
equivalence, not equal guard counts, string sorting or numerical samples. Any
missing source, input denominator, selected-rule or imported-subtree obligation
fails the experiment. Preserve raw vectors as diagnostic evidence. The source
already documents that imported completed guarded subtrees can produce a
different deterministic guard sequence while preserving obligations
(`crates/rustred-core/src/reduction/guarded_cache/program.rs:148-156`).

## Hypotheses to test, without assuming a direction

| Hypothesis | Mechanism and competing cost |
| --- | --- |
| More complex roots first improve reuse | A large root may populate completed descendants shared by later roots. With bounded eviction it can also evict precisely the subtrees later roots need. With unlimited retention and fixed dispatch, ordering should not be advertised as avoiding necessary graph work merely because it changes when it happens. |
| More complex siblings first reduce repeated work | Siblings may overlap downstream. Their first completion can serve later siblings, but DFS still fully solves each selected subtree and dependency pins may retain broad completed branches. |
| Higher incoming coefficient size first helps | Root output collection may cancel a large rational expression earlier; alternatively it may create a larger accumulator or expensive simplification before cancellation becomes available. Incoming sparse size is a weak proxy for final coefficient cost. |
| Rank/support/distance predict expensive branches | Rules with identical rank or support can differ greatly in fanout, guard cost, route expansion and coefficient growth. The simple score may pay sorting/allocation overhead without predicting useful work. |
| A schedule lowers memory | It might shorten the lifetime of expensive completed results, or instead leave more pinned branches resident. Retained coefficient bytes and process peak RSS measure different things. |

Record selected-rule fanout before and after equal-child coalescing, route-image
fanout, incoming coefficient sizes and completed coefficient sizes. Relate them
to overlap: repeated physical integral keys are not necessarily repeated
operational nodes because Route and Apply have different meanings. Measure
shared descendants using exact operational keys and record when completed cache
reuse actually occurs. A separate graph survey, if later authorized, must have
its own resource allowance and must not silently warm a measured application.

Cancellation claims must name their level: duplicate input roots, equal children
inside one evaluated rule/route, zero intermediate terminal coefficients during
combination, or zero terminal coefficients during final root collection. This
implementation introduces no cross-root nonterminal frontier cancellation.

## Staged protocol: TUNING only; execution HELD

All actual-five-loop scientific work, including graph surveys and uncached
comparisons, remains **HELD**. A local fixture pass or an idle machine does not
release the hold. Any later launch must use the current actual5L resource guard
and the existing serialized `serialhold` ownership protocol, including the
current resource-owner state and run ledger. Do not invent a second launch path,
reset consumed allowances, overlap a compiler or another scientific job, or
reinterpret an earlier pilot authorization as authorization for this study.
This note supplies no launch commands and changes no coordination state.

The study enables no new default solver work, Route, Apply or cardinality caps.
An explicitly registered fast diagnostic comparison targets at most 240 seconds
with a 300-second hard wall envelope, and no silent retries. The current resource
owner must register the actual launch and before-Native host-memory floor:
actual five-loop work must stop if host `MemAvailable < 50,000,000,000` bytes.
Neither this note nor a generic example invocation grants that admission.

1. **Freeze implementation and engineering checks.** Keep `Canonical` as default;
   verify deterministic ties, entry checks before cancellation, exact coefficients,
   valid guard obligations, cleanup of dependency pins, typed budget/cancellation
   exits and cache interchange under the unchanged semantic binding. Pure fixtures
   are engineering evidence only. Record the final source revision and diff,
   executable digest, dependencies, exact package/convention binding and checker
   revision. Compilation/testing follows the separate resource-owner permission.
2. **Freeze a bounded TUNING manifest before launch.** Use only explicitly
   designated TUNING identities and register their raw/direct-owner/route-alias
   exclusions against held-out identities. The frozen12 held-out inputs are
   excluded from selection, exploration, adaptation, graph surveys and schedule
   ranking. Do not reinstate failed qualification cases or treat correlated
   variants of held-out cases as new tuning inputs. Register finite wall, CPU,
   RSS, coefficient-payload, output/disk and request-count caps, stop rules, number
   of cases, repeats and all candidate policies before any mathematical work.
3. **Establish the canonical control.** Once released by the current coordination
   process, run the same authenticated TUNING requests with independently empty
   application/cache state, fixed exact-algebra limits, route preparation workers,
   terminal convention, cache representation, eviction policy and backing mode.
   Preserve failures and resource exits. An incomplete or incorrect control is
   not a successful timing datum and is not permission for unbounded retries.
4. **Test one dimension at a time.** The bounded candidate list contains at most
   five root-only policies with canonical dependencies, and at most five
   dependency-only policies with canonical roots. Use the frozen subset that
   fits the registered allowance; do not silently expand to a full cross product.
   Pair each candidate with a canonical control in alternating/counterbalanced
   order. Use independent empty sessions for cold comparisons. Separate RAM-warm
   and persistent-backing regimes with explicit identical initial state.
5. **Choose bounded combined pilots on TUNING evidence only.** Predeclare a rule
   for choosing at most two combinations of promising independent dimensions,
   including a correctness and memory constraint. Do not choose combinations
   after observing held-out behavior. Run only within the remaining registered
   allowance; additional repeats need a separately recorded resource decision.
6. **Freeze a decision before any held-out assessment.** Retain the canonical
   default unless the scoped evidence justifies a separately reviewed change.
   Record all attempted policies, overhead, failures and uncertainty, including
   a valid no-benefit result. Any frozen12 functional or performance assessment
   is a later separately released protocol with no schedule reselection, and is
   not authorized by this design.

Current result table: canonical scientific baseline **HELD**; independent root
policies **HELD**; independent dependency policies **HELD**; combined policies
**HELD**; held-out assessment **EXCLUDED FROM TUNING / HELD**. There are no measured
five-loop speed, memory or cancellation claims in this note.

## Generation-free sampling and exact checking

The existing `saved_application` example has two additional command forms:

```text
saved_application scheduling-sample PACKAGE REQUEST_JSON ROOT_ORDER DEPENDENCY_ORDER OUTPUT_JSON [LIMITS_JSON]
saved_application scheduling-check PACKAGE REQUEST_JSON EXPECTED_JSON ACTUAL_JSON
```

The request schema is `rustred.saved-application-scheduling-request.v1`: a named
physical arity, an optional `entry_mode` (`direct` by default, or `routed`), and
`terms` containing full `powers` arrays and exact `coefficient` strings in the
authenticated coefficient field. Multi-root requests are necessary to exercise
root priorities. A newly constructed exact sum must be labeled as a synthetic
scheduling microbenchmark rather than as its source public integrand.

One sample creates an empty application, parses the request outside the timer,
and makes exactly one strict reduction call. It has no persistent backing,
capture, warm-up, repeat or retry. It writes full family-qualified coefficients,
raw guards, common operational options, package/request hashes, work/cache
counters and separate load, preparation and ingress times. The reported
application timer includes terminal substitutions; unavailable narrower phases
remain null. Linux `VmHWM` supplies process peak RSS through application return,
including setup. Source/cohort compile-time labels are nullable, so an external
source/ELF/affinity manifest is still required.

The check command authenticates the same package and reparses both complete
outputs in its coefficient maps. It checks declared terminal identities,
completeness, exact full coefficients and exact guard sets outside application
timing. It additionally requires ordered guard equality when root priority is
unchanged, and always reports any vector-order difference. It performs no
reduction, but loading a five-loop package still requires the externally
registered Native-data admission. These generated receipts compare executions;
they are not new independent identity or coverage proofs.

The synthetic branching fixture also records a negative scheduling result:
canonical sibling visitation succeeds with a two-entry resident cache, whereas
positive-power-first requires three pinned entries and returns the existing
typed cache-limit error at two. With sufficient capacity, their exact output
and ordered guards agree. That fixture checks failure cleanup and illustrates
why higher complexity first is not an unconditional memory improvement. It
establishes no five-loop performance result.

## Measurements and acceptance evidence

Every run needs an immutable manifest with request identity, policy pair, source
and executable identity, semantic binding, machine/resource cohort, limits,
cache/backing state, counter definitions, start/end status and complete output
locations. Collect bounded observations consistently across candidates; measure
observer overhead separately and do not rank an instrumented candidate against
an uninstrumented control.

| Group | Required evidence |
| --- | --- |
| Exact result | Full terminal-key/coefficient comparison, declared terminal convention, completeness/unresolved status, exact guard-obligation validity/equivalence, raw guard vector and checker receipt. |
| Separated phases | Wall and CPU for package read/decompression/authentication, native decode/replay/preparation, entry/routing preparation, exact application, root materialization/collection, terminal substitutions/normalization, output serialization and independent validation. Report available phases honestly; unavailable instrumentation is missing evidence. |
| Nodes and transitions | Requested roots after input coalescing, unique physical keys and unique operational nodes, Route and Apply counts, rule applications, route operations and endpoints, maximum depth/pending frames, attempted/recomputed nodes and typed failures. |
| Exact arithmetic | Coalescing/addition counts, multiplication/normalization counts where available, selected-rule and route fanout, sparse numerator/denominator term counts, coefficient bytes/degree growth, and largest observed intermediate and completed payloads. Missing counters must be identified, not inferred from node counts. |
| Memory | Process peak RSS with an attributable process/cohort measurement, retained coefficient bytes/terms, cache census, pending/pinned dependency census, scheduling-vector overhead and observer/output buffers. Cache budgets are not process RSS limits. |
| Cancellation | Counts and coefficient-size changes at the specific input/local-child/combination/final-root stages; zero roots and zero output terms; timing of cancellation relative to expensive work. Record user-requested cancellation and safe-exit latency separately from algebraic cancellation. |
| Cache | Cold/RAM-warm/backing mode, root versus intermediate hits, Route versus Apply hits, misses, insertions, evictions and evicted bytes, recomputations, pins, backing read/decode cost and peak residency. Imported records must have the same semantic identity and exact validation. |
| Performance | All complete paired observations, order of runs, wall/CPU distributions, variability and resource exits. Report timeouts/caps as censored failures, not successful fast samples. A small-sample median does not erase growth or correctness regressions. |

Accept a scoped policy only after exact checks pass for every completed paired
request, resource/cleanup behavior remains valid, and the predeclared performance
and memory criteria are met without hiding sorting, preparation, materialization
or validation costs. If a metric cannot be obtained within the bounded observer
allowance, narrow the supported conclusion. Pure fixture timings, graph-node
counts alone and a small number of known master coefficients cannot substitute
for these measurements.

## Separate design outline: weighted frontier fusion (not implemented)

The stronger cancellation hypothesis needs an algorithm that brings weighted
nonterminal contributions together before their full DFS expansions. Root or
sibling visitation priorities cannot provide that behavior. The following is
one possible research outline, with its mathematical and engineering obligations
still open; it is not implemented, admitted or authorized for a pilot here.

1. Discover a finite exact operational DAG for the admitted request using the
   existing checked one-step evaluator, immutable dispatch and Route/Apply
   transitions. Keep exact edge weights, selected-rule/source/denominator guard
   obligations and terminal conventions. Discovery must detect cycles and retain
   the existing local descent/route checks. Numerator rank is not a globally
   decreasing scalar across owner changes and transport; rank sorting cannot
   replace these checks or establish a valid global processing order.
2. Process a topological, predecessor-complete frontier of that discovered DAG.
   Accumulate all incoming weighted contributions to the same exact operational
   node before expanding its weighted contribution. Track predecessor completion
   explicitly; a later parent must not add an unseen contribution after the node
   has been treated as canceled or complete. Route and Apply, including Apply
   owner, remain distinct keys even for the same physical integral.
3. Use an existing completed individual cache identity as a complete subtree
   when its binding, coverage, entry/source checks and obligations permit it.
   Only fully completed individual node expansions may be published under the
   existing cache schema. A request-dependent weighted frontier sum, a partial
   global accumulator or a sum that becomes zero is not an individual integral
   identity and must never be stored as one.
4. Preserve conditions from every evaluated contribution, including contributions
   whose weights cancel to zero later. Keep original source and denominator
   checks at their existing admission/evaluation boundaries, before any algebraic
   coalescing could erase them. A net zero coefficient is not permission to claim
   that an uncovered or undefined identity was valid. Any proposal to skip
   never-evaluated canceled descendants needs a separate exact domain and coverage
   argument; this outline grants none.
5. Produce terminal coefficients from the weighted frontier only after its
   predecessor-completeness and obligation rules have been established. Compare
   the complete output and exact guard domain against the checked ordinary
   engine, including zero-cancelled inputs, denominator singularities, route
   images, cache hits, gaps, cycles and typed resource/cancellation exits.

Discovery can cost as much one-step work as ordinary execution, retain a large
graph, and reduce the very back-substitution work the study hopes to avoid only
after paying substantial bookkeeping. A predecessor-complete frontier can be
wide; its weighted rational coefficients can grow faster than DFS coefficients.
Exact fanout, route expansion, duplicate-edge coalescing, guard storage and
topological bookkeeping need budgets. Discovering the DAG and then expanding it
also risks duplicate specialization unless immutable checked edge payloads can
be retained economically. A full graph is not known in advance merely because
each same-owner formula edge descends.

Fusion can change cache coverage: avoiding a zero weighted subtree may leave no
completed individual identity to export, and consuming a completed subtree may
skip node-level survey detail. Report those effects separately from reduced
arithmetic. Cache completion counts and exported identities must remain honest;
never synthesize an individual cached result from a globally canceled sum.
Persisted caches remain acceleration evidence, not a new proof of rule coverage.

A future fusion study therefore needs a separate implementation design, exact
coverage/guard argument, bounded instrumentation and TUNING protocol. Its gains
and costs cannot be inferred from a favorable root/sibling scheduling pilot.
