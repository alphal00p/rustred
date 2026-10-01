# Bounded exact-rule portfolios: first matched campaign measurements

Status: all three campaigns pass full cold reinspection and original-source/
guard replay. Neither candidate demonstrates an improvement over A1. Do not
recommend a production switch.

## Question and controlled workload

Can selecting among a few independently valid rules reduce the subsequent
recursive domain traversal enough to repay the extra generation work?

The existing combined FG/BMW/H/X four-loop control is kept intact: 16 owners,
508 routes, 58 required queries, no auxiliary queries, and 32 verification roots.
This is the bounded physics-input control, not unrestricted family certification.
All three arms generate the same 16 sectors freshly, with no checkpoint reuse.
They use sparse exact generation, numerical depth 2, finite-case policy `search`,
the same mathematical integral order and the same A1 source basis.

All arms use the same frozen default-release CLI (`4e76707b…`), 16 workers on
CPUs 32–47, and the same discovery, staging, walking and verification drivers.
The new Epoch prepared-retirement code is **not** in these binaries: it has its
own fixed-rule comparison. Builds are excluded. Other users' host contention is
unknown; these are single-arm screening measurements, not confidence intervals.

The comparison changes only the optional rule portfolio:

- **A1:** first valid rule from active-first sectors and source rows ordered by
  term count, then coefficient monomial count.
- **Branch-first:** compare against one input-order source trial, prioritizing
  exceptional cases, maximum numerator-shift excursion, RHS terms, coefficient
  monomials and source rows, in that order.
- **Rank-first:** the one registered correction; prioritize maximum and total
  numerator-shift excursion, RHS terms, exceptional cases, guard predicates,
  coefficient monomials and source rows.

Both optional trials have depth 0, at most 2,048 search rows, 1,024 exact-trace
rows and 262,144 exact-trace terms. The baseline remains mandatory, and all
competing rules pass the existing exact admission gates. Ties keep the incumbent.
These scores are structural heuristics, not proofs about descendant cost.

## Results

| Quantity | A1 | Branch-first | Rank-first |
| --- | ---: | ---: | ---: |
| Selected alternative rules | — | 11 | 25 |
| Generated rules | 523 | 511 | 530 |
| Finite residuals | 28 | 28 | 28 |
| Discovered domains | 26,025 | 25,982 | 29,768 |
| Dependency edges | 495,898 | 495,588 | 558,153 |
| Native reinspection candidates | 17,957 | 17,914 | 21,199 |
| Native traversal, seconds | 4.177 | 4.203 | 4.014 |
| Fresh preparation/generation/walk phase, seconds | 19.929 | 19.891 | 20.407 |
| Full cold-All phase, seconds | 7.651 | 8.141 | 7.834 |
| Inclusive primary job, seconds | 29.278 | 29.949 | 30.370 |
| Waited descendant CPU, seconds | 73.240 | 73.671 | 72.861 |
| Sampled peak process-tree RSS, MB | 246.8 | 236.9 | 234.3 |

The inclusive primary boundary starts before input preparation and ends after
cold verification and evidence checks; it is not merely the sum of the two
timed child phases. CPU includes waited descendants and their supervisors, not
the outer harness. RSS is sampled, not an exact allocation high-water mark.
Every arm drained its owned process groups before releasing its locks.

All 58 queries and 32 roots pass full native cold reinspection in each arm,
with zero errors, frontiers or uncovered obligations. This separate oracle finds
all discovered domains closed; the walk's cached closure counters can remain
stale at exhaustion. Raw process exit codes remain recorded rather than rewritten.

Baseline qualification independently replays all 523 rules against regenerated
ordinary IBP sources and checks their declared guards (16,778 source entries).
The qualification job took 44.200 seconds, separately from the 29.278-second
primary boundary; the combined cost is 73.477 seconds. It also checks full
decoded equality of the two historical/current A1 programs. Source replay does
not independently prove descent, coverage, terminal minimality or unrestricted
closure. The two changed candidate programs require their own source replay.

Both changed candidates subsequently passed their own source/guard replay:

| Candidate | Rules / sectors | Original-source entries | Separate qualification, seconds |
| --- | ---: | ---: | ---: |
| Branch-first | 511 / 16 | 16,623 | 42.971 |
| Rank-first | 530 / 16 | 16,728 | 42.921 |

These jobs drained cleanly. Qualification timings are not matched solver
comparisons: the baseline job additionally checks historical decoded equality.

## Interpretation and decision

Branch-first reduced the number of rules by 12 but the number of discovered
domains by only 43 (0.17%). Its inclusive time was 2.29% higher. This is not a
useful downstream gain; the small timing difference alone is not strong evidence
of a regression on a shared host.

Rank-first increased domains by 14.38% and native reinspection candidates by
18.05%. Immediate successor events decreased (322,881 versus 410,681), while
routed admissions increased (499,640 versus 443,631). Thus fewer immediate
successors did not imply less global work. The result is consistent with changed
routing and sharing, but these counters alone do not identify the exact causal
rules. Slightly lower traversal time or CPU is not a whole-campaign improvement.

**Park both tested recipes.** Retain the generic opt-in portfolio API, but keep
A1 for current recommendations. Do not proceed to five-loop transfer or a blind
priority sweep on these results. Reopening requires evidence tying the proposed
selection criterion to actual downstream ownership/reuse cost, not simply a
different permutation of the same local scores.

Following the user's clarification, one-off preparation/generation time is not
the decisive full-campaign criterion: it may be amortized over a very long walk.
Retain its separate cost, but emphasize sustained traversal and total domain
work. That does not change this screening decision: branch-first has no measured
traversal gain, while rank-first's single small traversal-time decrease comes
with substantially more domains and inspections and needs evidence before any
scaling claim.

## Reproduction and remaining checks

Evidence root: `TMP/postlaunch-20261001/rule-quality-portfolio/`.
The frozen `PLAN.json`, input recipes and `run_arm.py` record exact commands,
input identities, resource allocation and the 1,800-second inclusive pilot limit.
Arm receipts are `receipts/{a1-r1,portfolio-branch-r1,portfolio-rank-r1}/RESULT.json`.
Baseline source qualification is under `receipts/a1-r1/qualification/`.

Native implementation tests pass: 16 focused core tests and 1,320 application
tests, with separately documented external/scale and wide-worker exclusions.
That native suite used `9477044d` plus the reviewed P3/test working-tree changes;
it is not a clean-checkout test result for a later documentation/test commit.
The added candidate replay tests themselves do not depend on the P3 code.
Fresh installed Python parity now passes all six targeted native API/CLI/
checkpoint tests with no skips, using the matching new CLI and an isolated
installed wheel (0.231s test execution). The gate is under
`TMP/postlaunch-20261001/portfolio-public-python/acceptance-r2/`.
Matched old/new Epoch-versus-Ready performance remains a distinct check.
No running production campaign was changed.

### Follow-up: downstream work, not just local rule size

Read-only inspection of the saved three runs and their checkpoint metadata
finds that rank-first adds3,271 Route inspections and removes29 Apply inspections.
Its immediate successor events fall21.4%, but Route-emitted Apply obligations
rise12.6%. Thus a smaller immediate expansion is not a reliable proxy for a
smaller shared closure graph. Matching all32 starting-query geometries makes the
root comparison valid; it does not match every reached descendant geometry.
Root dependency cones overlap and their cost differences cannot be summed.

Growth appears already in lower-owner cones and propagates through higher owners
whose saved payloads are unchanged. This supports a targeted causal experiment,
not an identified rule-level explanation or a new performance claim. The proposed
smallest experiment swaps one already replayed lower-owner payload in both
directions while preserving all other owners, routes and queries. Both arms
have now completed with native mixed-load checks and full cold acceptance.

Evidence and independent critique:
`TMP/postlaunch-20261001/rule-quality-portfolio/SAVED_CANDIDATE_DIAGNOSTIC.md`.
Both tested scoring recipes remain parked. The objective is to discover a
generic criterion for reusable routed descendants, not a topology-name-specific
score or another blind permutation sweep.

### Two-way intervention: a lower-owner program accounts for most extra routing

The single changed payload is owner`0111100001`, published sector481 in the
input family, not a hard-coded solver choice. Native mixed import validates
the family/order/Symbolica context. All other15 payloads, all508 route records
and the complete query bytes are unchanged from the respective base. Every
arm uses the same previous4e76707b CLI and native walk policies. Both full
cold checks pass58/58 queries and32/32 roots with no frontiers or uncovered
obligations. Original-source replay applies to the unchanged payload bytes.

| Input program collection | Domains | Route inspections | Apply inspections | Conditional successors |
| --- | ---: | ---: | ---: | ---: |
| A1 | 26,025 | 16,438 | 1,519 | 2,911 |
| A1 with this rank-first owner | 29,107 | 19,474 | 1,490 | 5,651 |
| Rank-first | 29,768 | 19,709 | 1,490 | 5,651 |
| Rank-first with this A1 owner | 26,666 | 16,673 | 1,519 | 2,911 |

The substitution transfers exactly3,036 Route inspections in either direction,
92.8% of the full portfolio's3,271 extra Route inspections. It transfers the
entire2,740 conditional-successor difference. Routed Apply emissions change
by47,816 in each direction,85.4% of the full difference. Stored domains change
by3,082/3,102,about82–83% of that difference; overlapping root-cone totals are
not summed.

This strongly localizes *program-level influence*, but not the offending rule
or predicate. Terminal declarations travel with the donor program; matching
terminal counts/policy is not yet exact terminal-key equality. No new terminals
were synthesized or promoted. Single traversal observations3.667s and4.193s
do not establish a speedup, and the extra Route jobs may be cheaper than Apply
jobs. The next test is native ordered-rule/guard/shift/terminal inspection,
using existing Symbolica operations and the native binary reader—not another
priority grid or a custom artifact decoder.

Evidence: `owner_swap_analysis.json` and
`receipts/owner-swap-{a1-with-rank,rank-with-a1}-r1/RESULT.json` under the same
evidence root. Author:`frontier_replay_implementation`; independent donor-only
input and authority audit:`rule_quality_audit`.

### Identical-domain follow-up: ordered rule selection and RHS geometry

The next diagnostic holds one original R5 convenience domain fixed, including
all original coordinate bounds. It first uses the native ordered matcher, then
applies each selected rule only on the piece assigned to it. This avoids treating
a nominated rule as the actual dispatcher winner. It uses the successful
campaign's finite-axis refinement and degree64 guard policy, rather than the
more restrictive bare-command defaults. No positive-power/total-degree bounds
are dropped: this particular anchor has neither. No rules are regenerated.

| Same source anchor | A1 | A1 with rank-first owner |
| --- | ---: | ---: |
| Selected source pieces | 500 | 544 |
| Immediate successor events | 14,499 | 14,536 |
| Distinct literal target-image keys | 9,565 | 10,167 |
| Duplicate image events | 4,934 | 4,369 |
| Conditional successor events | 240 | 285 |
| Exact zero terms | 3,366 | 3,735 |

Both sides have zero gaps/unresolved pieces, application problems or optional
refusals; each matches11 terminal pieces. Terminal-piece counts do not establish
equality of all declared terminal keys. Image keys include sector, endpoints and
rank, but are not canonical routed images, algebraic equivalence or containment
proofs. The diagnostic completes in12.479s inclusive onCPU32/W1 with the same
4e76707b executable and fully drained owned processes. This is not a timing
comparison or a family-closure claim.

The result shows8.8% more selected source pieces and6.3% more literal successor
images despite almost identical immediate successor counts. Extra fragmentation
is visible before recursive traversal, but the exact changed guards and routing
mechanism are not yet isolated. More zero terms also prevent interpreting
coefficient factors or conditionality as uniformly harmful. Next compare native
saved cases and align actual overlapping source pieces, rather than matching
rule ordinals across different programs or inventing a weighted score now.

Evidence: `owner-481-ordered-geometry-r3/RESULT.json` in the same local evidence
root. Earlier r1/r2 receipts retain the diagnostic-policy mismatches, not hidden
failed solver runs. Author:`frontier_probe_runner`; independent review by
`rule_quality_audit` confirms the reported scope and counts.

### Implementation boundary for the next selection experiment

The independent architecture review favors selection among complete saved
owner-program **cohorts** in the steering layer as the first experiment. Existing
native mixed-program loading already keeps each program's ordered cases, guards,
exceptional descendants and terminal declarations intact. A shared native walk
can measure their interaction without coupling the core sector solver to global
application routing state. Choosing each owner independently and combining the
winners is not justified: the two-way swap has already shown non-additive domain
costs. The combined cohort must be tested.

The current per-rule portfolio only exposes trial summaries externally; it
compares private fully admitted candidates and discards losers before publishing
the winner. A routed per-rule callback would therefore be a new API, not just
another cheap feature enum. It would also require an explicit frozen downstream
snapshot: active-first generation does not guarantee that lower owners have
already been generated. This added engine coupling is premature while the
useful scoring mechanism is unproven.

A cohort experiment must bind its saved program bytes, unchanged routes, query
scope, mathematical order and matching limits. Ties, unsupported probes and
censored runs keep the baseline; incomplete work must never score as cheap.
Local diagnostic panels were chosen after observing this regression, so success
there supports a mechanism but does not validate a general heuristic. Any new
candidate still needs a full unchanged four-loop comparison, followed by a
representative five-loop transfer test. This is an architecture decision, not
a newly successful strategy or a production-switch recommendation.

### Structural interpretation of the implicated input

The five active momenta of this supplied four-loop owner are
`k2, k3, k4, k1-k4, k1-k2-k3`. Their unique linear dependence has nonzero
coefficients on all five:
`k2 + k3 - k4 - (k1-k4) + (k1-k2-k3) = 0`.
Thus this is a connected five-line momentum circuit, not a two-loop component
times two one-loop factors. Removing any one line leaves four independent
unit-Jacobian propagator momenta. This explains why all five one-pinches map
to the same installed lower owner: four through explicit routing and one
already in its coordinates. Nontrivial numerators can still couple these
one-loop factors tensorially; arbitrary numerator integrals are not simply
scalar products of tadpoles.

The lower owner has both R9 and R12 unconstrained orthant helpers. In the
existing admitted transport path, the root image preserves actual numerator
rank and maps active coordinates bijectively; proper pinches decrease rank.
Consequently a natively validated root Apply image of rank at most7 fits those
helpers. This does not cover the preceding different-mask Route obligation,
source-validity checks or every further-pinch output. It explains why repeatedly
transporting fragmented boxes may cost work even when their common destination
is already represented.

Explicit pre-routing helpers are a possible input-only falsifier, not a proven
optimization. Historical broad Apply+Route anchoring experiments had mixed or
negative results; a broader helper can create more work and dependencies than
it removes. Test saved transport work against the complete extra helper/pinch
cost before recommending any such change.

The native source-order audit also rules out a shortcut based on row ordinals.
All ordinary vacuum sources are initially differentiated-loop-major, but
sector specialization/preconditioning occurs before the current terms/monomial
sort. A visited row need not be a raw single-loop derivative identity. Any new
locality criterion must inspect prepared-row structure or retained exact source
provenance; it must not label rows by presumed derivative positions. These
observations concern this input and existing generic code, not new hard-coded
topology logic.

### Native program inspection and the six-panel falsifier

The new read-only native inspector and its Rust/CLI/Python adapters pass two
public native tests and three fresh-installed Python/CLI tests (zero skips).
The release build takes760.434s separately from execution. Test plus four
bounded program exports takes16.887s. No identities are generated by inspection;
the K1/K3 test fixtures themselves are generated normally. Internal structural,
writer and parser unit tests were typechecked but not executed in a new full
library harness. All structural reports use existing native import and Symbolica
display, not an alternative coefficient format or algebra implementation.

Exact terminal inspection now resolves the earlier caveat: both implicated
programs contain the **same ordered eleven integer keys**, not just equal counts.
Three later candidate pairs61/62,23/25,41/42 also have identical coordinate cases,
targets and ordered RHS integral keys. This does not assert equality of their
exact coefficients, and affine cases remain unmatched without native comparison.

Six small, identical finite source panels were selected around those late pairs.
Native ordered matching first rechecked their dispatch. Each side then ran an
ordinary serial Ready walk with the same complete routing/owner collection.
These six diagnostic roots are not the full58-query control, and do not inherit
its entire initial-helper context. Both finish with zero pending/frontiers/errors;
all owned processes drain in14.990s inclusive.

| Six-panel diagnostic | A1 | Rank-first donor |
| --- | ---: | ---: |
| Discovered domains | 4,371 | 4,275 |
| Native inspections | 3,592 | 3,516 |
| Route inspections | 2,552 | 2,489 |
| Conditional successors | 365 | 352 |

Every root has identical depth-one phase/count summaries; depth-two differences
are small and mixed. The prediction that these panels would reproduce the
donor's adverse whole-campaign direction is falsified. Do not implement a
conditional-term or local-image score from this result. Conversely it does not
disprove every route-aware approach: the panels were post-hoc, and both reached
geometry and shared helper context differ from the original full campaign.

The native saved-rule comparison finds a much earlier structural difference:
rule2 on both sides has the same case (physical index4 fixed to1) and target,
but10 versus13 RHS integral keys with no common key. Its displayed exceptional
predicates involve different axes; saved case order diverges immediately after.
This earlier rule/exception tree is the next diagnostic lead, not an established
cause for all extra cost. The terminal-set confound is removed, while coefficient
equivalence and affine-case identification are not inferred from local IDs or
display strings.

Evidence: `native-inspection-run1/RESULT.json`,
`native-inspection-python-run1/RESULT.json`, and
`owner-481-six-panel-routing/execution/RESULT.json` beneath the same evidence
root. Separate source, coordinate, receipt and interpretation audits passed.
Both candidate scoring recipes remain parked; no campaign restart is recommended.

### Early-dispatch panels and selective-trial ablation

The earlier rule2 difference is real, but its local cost still does not predict
the complete shared campaign. Five disjoint finite R5 panels contain37 concrete
entry tuples. Native matching checks the expected first-rule pairs2/2,2/9,5/2,
2/5 and1/1 before ordinary recursive traversal. They keep the same saved owner
collection and routes; they are not the original58-query acceptance workload.
Both walks exhaust their work with no frontiers/errors in13.870s inclusive.

| Early-panel measurement | A1 | Rank-first donor |
| --- | ---: | ---: |
| Complete panel walk domains | 3,379 | 2,590 |
| Native inspections | 3,068 | 2,512 |
| Route inspections | 2,126 | 1,781 |
| Both-rule2 root: native successors | 4 | 8 |
| Both-rule2 root: depth-two graph domains | 28 | 128 |

The local rule2 expansion grows while total panel work shrinks. Furthermore,
the unchanged-rule1 control emits exactly6 native successors on both sides but
has graph outdegree6 versus4. Graph edges include containment/sharing effects;
they must not be read as raw rule fanout. These results disqualify the tested
shallow panel as a sufficient predictor of the full-cohort regression.

Inspection of integer shifts also explains one rank-first preference: the two
early changed rules tie on maximum numerator excursion but reduce the summed
excursion2→1, before their larger RHS counts10→13 and11→12 are compared.
All nine uniquely coordinate-case-matched pairs with different RHS inventories
retain one seeded **preconditioned** basis row. That is not necessarily one
ordinary IBP: preconditioning can combine sources.

One generic, input-only ablation therefore activated the existing rank portfolio
only when the mandatory baseline retained at least two such rows. The trigger
does not restrict the eventual alternative's row count. The full unchanged
four-loop control, not just the panels, was regenerated and checked:

- Zero alternatives selected;249 trigger skips.
- Both parent program payloads are byte-identical to A1:523rules and28residuals.
- The same26,025domains,17,957native inspections and495,898edges.
- Cold-All passes58queries/32roots with no uncovered obligations or frontiers.
- Separate exact source/guard replay passes523rules across16sectors and
  16,778source entries in43.140s. The primary arm is29.688s; the combined
  charged boundary is72.828s. All owned processes drained.

This is a **negative optimization result**. Disabling trials on single-seed
baselines removes the program changes on the resulting A1 case path; it does
not reconstruct all25 original winner decisions, because earlier choices alter
later visited cases. Equal payloads/work counts do not imply identical checkpoint
graphs: the edge digests differ. No performance improvement or five-loop transfer
is claimed, and no default changes.

Evidence beneath the same local root: `owner-481-early-dispatch/`,
`OWNER_481_GEOMETRY_DIAGNOSTIC.md`, `MULTISEED_ABLATION_RESULTS.md`, and
`EARLY_RULE_SELECTION_DIAGNOSIS.md`. Runner:`frontier_probe_runner`;
independent authority/interpretation audit:`rule_quality_audit`.

### Next mechanism: shared exception geometry, not topology-only pinch counts

All five edges of the implicated circuit pinch to the same lower owner. Thus a
topology-only destination count cannot distinguish the two early choices. The
numerator coordinate chart and routing witnesses are asymmetric, but their raw
matrix density is not an exact prediction of cancellation support or domain
reuse. Prior independent per-route support minimization already reduced local
edges while increasing total campaign work, so it is not reopened blindly.

The current rank-only diagnostic also has a precision caveat: with unconstrained
A/D predicates and joint-support pruning off, the route cover does not construct
the per-axis `NumeratorDegrees` service. One cannot claim that a particular
inactive-coordinate boundary survives transport precisely on that path.
Adding a mathematically redundant D lower bound would activate the constrained
path, but also disables initial-full-orthant reuse. Independent review caught
this confound before execution; that experiment is held rather than interpreted
as an isolated support test. Existing runtime options and coherent source/rule
preferences are being evaluated first. No new scoring default, CAS implementation
or production modification follows from these observations.
