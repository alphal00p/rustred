# Five-loop finite-domain campaign: parallel audit summary

Date: 2026-09-22. This is an audit of existing receipts and the current working
tree, not a new solver campaign or a completion claim. Three independent lanes
reviewed measured performance, implementation/mathematics, and primary
literature. The coordinator ran release diagnostics and validation. No
production implementation was changed during this audit.

**Post-audit validation follow-up:** the two obsolete fixture counts were
corrected with explicit input-to-domain-ID equivalence assertions, retaining
all worker/off-on checks. The semantic-only release application gate now passes
**361 tests, one ignored**, including both complete parity loops; the separate
82 integration tests also pass. Receipt: `app-corrected.log` under the gate
directory below. The earlier failing gate recorded in this report is historical.
The same-input recursive rerun uses frozen binary SHA256
`891cf5ae621760c653645e3ae17e9246111f803f2d8684d8cad942eb85648d10`,
separate from the subsequently implemented affine-support slice.

## Bottom line

The evidence presently favors reducing domain-admission cost and conservative
routing fanout before generating more IBPs or increasing worker count. The
last constrained campaign stopped with unfinished work, not with a demonstrated
missing-rule frontier. Zero observed frontiers is not coverage of its pending
queue. There is still no measured complete five-loop physical-envelope solve
or defensible fifteen-hour completion forecast.

The current pilot starts from only 980 points in one diagonal control with
A<=24, D=A-R>=10 and R<=10. It is not the complete marginal R14 envelope or
the full-jet R15 envelope specified in `../finite_starting_domains.md`.

## Measurements and their boundaries

| Workload | Outcome | Measurement | What it establishes |
|---|---|---|---|
| Previously completed 50-concrete-root stress trace | Complete for those roots | 1,552.875 s traversal; 54,695,037 distinct reachable keys | Those explicit roots and descendants reach existing terminals, without coefficient back-substitution |
| Latest 980-point correlated symbolic pilot | Cooperatively stopped, incomplete | 106.956 s traversal; 49,686 completed / 145,431 queued / one partial cancelled domain | 4.972 billion containment comparisons and later coordinator pressure; no observed frontier |
| New saved-descriptor admission replay | Six completed repeats, both lane orders | Paired median old/new wall ratio 4.271; separate wall medians 1.319154 / 0.308513 s | Faster index-only processing of this saved sample, not campaign speedup |
| Native affine-support diagnostic | Complete for 32 observed maps and 27,807 recorded calls | 1.288465 s internal / 1.36 s command wall | Strictly smaller projected root covers in 17,304 calls (62.23%), not a measured successor reduction |

The stopped pilot configured 50 workers, but its later sampled interval averaged
about 2.08 busy cores while the coordinator admitted results. Its sampled peak
RSS was 9.589 GB, far below the 500 GB ceiling. Neither adding workers nor
enlarging the completed-result buffer fixes that serial throughput problem.
This is a diagnosis from counters/thread state, not a stack-sampled attribution
of an exact fraction of CPU time to a function.

The admission replay uses 49,686 completed descriptors, excluding the partial
record. It does not replay every original rejected scheduling proposal, nor
the unfinished queue. Counts agree across all six repeats:

| Index quantity | Reconstructed old raw index | Current semantic index |
|---|---:|---:|
| Admissions | 49,686 | 25,207 |
| Live containment candidates | 21,711 | 11,597 |
| Total comparisons, including reverse maintenance | 112,362,704 | 22,484,047 |
| Additional semantic reuse hits | — | 24,479 |

Timing includes descriptor cloning and admission; summary construction is
included in the new lane. Parsing, decoding and post-run correctness checks are
separate. The old index is a test reconstruction rather than the old production
binary, and shared-host timings are not dedicated-host benchmark guarantees.

## Implementation review and immediate validation issue

Cached tight coordinate/A/R/D extrema give exact containment for the current
fixed-template domain vocabulary. They do not represent arbitrary affine or
nonlinear guards. Raw IDs, pending obligations and phase/support separation
remain distinct from inclusion reuse; a containing pending domain is not a
completed proof. The explicitly capped comparison lane retains its former
policy.

Release results at the initial audit, before the follow-up above:

- Native core: 2,758 passed, 32 existing diagnostics ignored.
- Standalone actual queue source/tests: 27 passed, one replay diagnostic ignored.
- Application library: 359 passed, two failed, one ignored.
- Separately run application integration targets: all 82 passed.

The two failures are hard-coded baseline counts, before worker-parity loops:
`tests/parallel_walk.rs:47` expects four scheduled nodes but obtains three;
`walking/execution/initial_orthants_tests.rs:231` expects five completed nodes
but obtains four. On their fully active one-coordinate owner, both a finite
rank label and an unlimited label describe actual numerator rank zero, so
semantic inclusion can legitimately merge those requests. This explains the
count discrepancy, but does not substitute for updating the semantic assertions
and rerunning every downstream parity check. The integrated gate is **not yet
green**. No assertions were edited during this audit.

The diagnostics used uncommitted containment changes on HEAD `b15a7e70` and
separate pre-existing scheduler/escrow work. Do not attribute these measurements
to that clean commit alone. No new campaign was launched with this index.

## Highest-value next experiments

1. **Finish the semantic-index validation gate**, preserving strict worker-count
   equivalence assertions, then rerun the identical 980-point pilot. Compare
   actual work, queue growth and coordinator utilization, not merely elapsed
   time at different stopping points.
2. **Retain bounds implied by affine-map sparsity.** If inactive row i can
   contribute only to selected target denominators, its power cannot cancel
   every active denominator. For target j, an exact safe degree budget is
   `B_j=min(sum(relevant upper_i), R_max-sum(irrelevant lower_i))`.
   A surviving active local coordinate has lower bound
   `max(0, source_lower-B_j)`. Use exact verified map support from Symbolica;
   no numerator expansion or new CAS kernel is necessary.
3. **If linear containment scans still dominate, add conservative monotone
   feature filtering** over cached extrema, with exact containment remaining
   authoritative. Measure both forward lookup and reverse-index maintenance.
   Preserve deterministic publication and all pending obligations.
4. **Repair only a genuinely reached missing target.** Broader source solving,
   ordering search, reconstruction, and master minimization do not address the
   currently measured admission bottleneck. Finite-field source selection and
   adaptive target-local seeding remain useful when an actual gap appears.

Before a longer run, also measure publication-chunk duration and cooperative
stop latency: the coordinator currently processes a whole chunk before its
next outer heartbeat/cancellation check. This is an identified risk, not a
measured cancellation failure. The symbolic walker remains a diagnostic,
without durable pending-queue resume; connecting a demonstrated reachable
gap to the existing concrete feedback session is still a separate integration
requirement. Faster containment does not implement that connection by itself.

The support experiment compares native-projected root covers, not raw bounds:
uniform global-rank bounds alone improve 3,696 calls (13.29%), versus 17,304
using map support. Its 20,340 calls with newly impossible single-axis pinches
are counted against the old rank-only test; later A/D projection or weighted
subset checks may already reject some. The associated 634,217 old children
are **not** an eliminated-child count. Subsets were not enumerated.

A full-family permutation shortcut is deliberately deferred. The complete
8,246-map census found 41 nonliteral eligible maps, but **none** belongs to the
32 maps used by the observed 27,807 mapped calls. It cannot improve this sample.

## Literature interpretation

The most directly relevant non-IBP mechanism is monotone feature-vector
indexing to avoid new-versus-all subsumption scans, described in Section 4.1(a)
of [Blanchet, Cheval and Cortier, ProVerif with lemmas, induction, fast
subsumption, and much more](https://bblanche.gitlabpages.inria.fr/proverif/publications/BlanchetEtAlSP22.pdf).
Applying this to RustRed is an engineering proposal, not a transferred proof
or speedup claim. TIDE and Kira additionally motivate limiting expensive
numerator routing rather than treating canonical routing as intrinsically
cheaper than local IBPs. The separate literature audit ranks these mechanisms
and their limits; no source supplies a guarantee of our full five-loop closure.

## Detailed reports and local evidence

- [Performance audit](finite_domain_performance_audit_2026-09-22.md).
- [Implementation audit](finite_domain_implementation_audit_2026-09-22.md).
- [Literature and strategy audit](finite_domain_strategy_literature_2026-09-22.md).
- [Campaign history](bounded_routing_pilot_2026-09-22.md).
- Ignored release/test/replay receipts: `TMP/semantic-containment-gate.qn9MTE/`.
- Ignored routing census/proof/diagnostics: `TMP/route-permutation-census.mWBz2v/`.
- Stopped correlated pilot:
  `TMP/correlated-routing-pilots.jw6Bwo/shared-owner-campaign.jtsg2q0t/`.

The queue diagnostic binary SHA256 is
`b8fbbbe44ff0aac625ffa976e44af85d5f86bd4721e6468062aed8e2d4e65dd3`;
the support diagnostic binary SHA256 is
`19ec69205ff1a2c4e86daa41f1da24aab7396e2c4bd14b364a8750da081fb76c`.
All diagnostics are release/optimized; compilation is outside their reported
timings. No reference-only code, confidential notes or license is reproduced.
