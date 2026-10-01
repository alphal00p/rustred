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

### Support-pruning diagnostic: acceptance failure retained

A two-arm diagnostic was prepared using the existing joint-support option,
with unchanged queries, owners and full-orthant anchors. This option prunes
simultaneous pinch masks; it does not make the individual inactive-coordinate
image bounds exact. The required stronger cold check deliberately disabled
those reference levers.

The first A1 arm exhausted 24,185 domains and 17,898 native inspections with
zero frontiers or native errors, pruning 70,778 masks. Its reference cold check
then rejected exactly 70,778 conservative Route covers. All 17,898 native
records were reinspected without native errors. This is **not an accepted
optimization result** and does not, by itself, prove the pruning or IBPs
mathematically unsound: it exposes a mismatch with this required independent
coverage check. The donor arm was not run, and no weaker as-run check was
substituted after seeing the failure. The corrected diagnostic drained in
16.470 seconds; an earlier staging-directory setup failure is also retained.

Evidence: `joint-support-on-pair/execution-r2/` and
`JOINT_SUPPORT_DIAGNOSTIC_RESULTS.md` under the local evidence root above.
This routing experiment is parked; current development remains rule selection.

### One next opt-in score, not a claimed improvement

The next registered rule preference is `total-positive-shift-excursion`.
For each RHS term, sum the existing positive displacement proxy on active
axes, then sum over terms. The maximum already available cannot distinguish
one child increasing a power from several children doing so. Symbolic powers
use target-relative offsets; fixed powers use differences of their positive
degrees. No algebraic primitive or custom polynomial operation is added.

For the first two saved differing rules, maximum numerator excursion ties but
this total positive excursion is 1 versus 4, and 1 versus 2 (A1 versus donor).
The proposed order puts maximum numerator excursion first, this new total
second, then total numerator excursion and RHS size. It preserves the A1
choice for those two saved comparisons while preferring the donor on the
other seven coordinate-case-matched comparisons, whose maximum numerator
excursion improves. New case queues may differ: these are predictions from
saved pairs, not observations of new generation.

Independent source review accepts the narrow generic implementation. Its
native/API validation and the fresh same-binary full four-loop comparison
are still pending. Existing A1 defaults, exact admission, guards, exceptional
recursion and mathematical order are unchanged. The falsifier is whole-cohort
work: reproducing A1 or reducing a local score without reducing downstream
cost is not a win. One useful pair still needs replication before transfer.
See `ACTIVE_REPLENISHMENT_CANDIDATE.md` and `positive-shift-control/PLAN.json`
for the fixed recipe and acceptance boundary.

A separate read-only census supplies context for the longer-term question.
Eight saved incoming responsibilities in the qualified Ready control favor
different active-coordinate faces; inactive axes 0 and 8 have identical literal
bounds in those eight stored boxes. Their owner corpus, routes and query bytes
match A1, but the Ready
graph is not the Epoch graph. A broad reusable anchor dominates incoming
dependencies, three G2 records inspect only narrower residual power bands,
and two records are delegated. Counts of these records are therefore neither
event frequencies nor region volumes. Exact native first-match diagnostics
are required before using this asymmetry to motivate a contextual guard score.

### Actual incoming-domain dispatch: guard fallback is real, but not sufficient

The no-follow native diagnostic subsequently completed under both saved programs,
keeping the eight stored responsibilities separate from the three exact G2
residual scopes. Every input and echoed rank/box/A/D constraint matches the
saved Ready evidence. All 22 query responses are complete and locally applicable,
with no gaps, unresolved conditions, invalid source conditions or output limits.
An initial 100,000-predicate query limit was reached; a single documented increase
to 1,000,000 through the existing CLI option sufficed. Both receipts are retained.
Four phases drained in 16.505 seconds while a compiler ran on disjoint CPUs;
this is a diagnostic duration, not a performance comparison.

| Exact matching scope | A1 pieces | Donor pieces |
| --- | ---: | ---: |
| Eight stored responsibilities | 6,943 | 5,352 |
| Actual residual 20920 | 769 | 645 |
| Actual residual 30560 | 27 | 34 |
| Actual residual 30569 | 6 | 9 |

Stored responsibility totals include 49 terminal pieces on each side; actual
residuals contain no terminal pieces. These scopes overlap, so sums are
diagnostic work counts, not distinct-integral counts or arrival probabilities.

The smallest residual gives a concrete example. Its active physical indices
are `(n1,n2,n3,n4,n9)=(1,2,2,1,1)`, so the total positive power is 7. Its
fixed power difference is −6, hence the five nonnegative inactive coordinates
sum to numerator rank 13. There are exactly 2,380 integer tuples. Independent
integer membership checks confirm that each program's native partition covers
every tuple exactly once.

A1 rule2 handles the 1,820 tuples with `x0>=1`. The donor cannot use its rule2
because `n9=1` throughout this domain. It handles the same region with rule5
in two pieces: `x8=0` (455 tuples) and `x8>=1` (1,365 tuples). This demonstrates
an actual ordered guard/fallback split, not merely a guessed coordinate corner.
The larger residual still partitions into fewer donor pieces. Thus even exact
local incoming-domain fragmentation fails to predict the donor's measured
whole-campaign regression; no guard-count score is justified by these results.

Evidence: `owner-481-incoming-match/execution-r2/RESULT.json`. The prior budget
failure remains in `execution/`. Runner: `frontier_probe_runner`; independent
scope, receipt and finite-partition audit: `rule_quality_audit`. The new
total-positive score now passes 19 focused core and 9 application tests; the
matching CLI, installed-Python and complete new-score campaign gates remain
pending. No production switch is recommended.

### Total-positive-excursion result: valid, but no useful win over A1

The fresh same-binary comparison is complete. Both arms used frozen optimized
CLI `e32fcc63…`, W16 on CPUs32–47, the same 16-owner/508-route/58-query/32-root
control, and unchanged integral order and terminal policy. Fresh A1's two
parent payloads are byte-identical to the previously qualified A1 payloads.
Both arms pass independent cold-All reinspection of every native record and
all 58 queries/32 roots, with zero violations, pending work or frontiers.

| Measurement | Fresh A1 | New total-positive preference |
| --- | ---: | ---: |
| Selected alternatives | 0 | 23 |
| Generated rules / finite residuals | 523 / 28 | 519 / 28 |
| Discovered domains | 26,025 | 26,488 |
| Native inspections | 17,957 | 17,890 |
| Dependency edges | 495,898 | 486,398 |
| Native traversal, seconds | 4.193 | 4.166 |
| Inclusive primary arm, seconds | 29.503 | 29.076 |
| Waited pipeline CPU, seconds | 36.658 | 37.984 |
| Sampled peak pipeline RSS, MB | 222.9 | 238.3 |
| Separate source/guard replay, seconds | 42.089 | 41.903 |

Exact source/guard replay independently passes 523 rules/16,778 source entries
for A1 and 519 rules/16,711 entries for the candidate, across all 16 sectors.
The primary raw receipts retain their original `SOURCE_REPLAY_PENDING` label;
their separate successful qualification receipts complete that gate. Likewise
the raw traversal exits with its stale cached closure snapshot; the successful
full cold check is the authority for this bounded control's discharged queries.
Neither raw observation was rewritten to look more favorable.

The candidate makes a nontrivial change and largely avoids the earlier rank
portfolio's regression, but domains still increase 1.78%, while native work
decreases only 0.37%. One small timing difference is not a demonstrated gain,
and pipeline CPU is higher. **Keep A1 as the recommendation.** Retain the
tested generic feature as an opt-in research control, not a promoted preset;
do not repeat this recipe or transfer it to five loops on these results.
Six owner payloads differ from A1, including the earlier implicated lower owner.
The registered diagnostic completes in 4.282 seconds: the new owner has 146
rules and the same 11 ordered terminal keys. Its first seven coordinate cases,
targets, integer RHS inventories and displayed guards match A1 (not a claim
of coefficient equality). The three exact G2 residuals partition into 768/26/5
pieces versus A1's 769/27/6, all fully applicable with no gaps. Restoring the
early guard structure therefore does not suffice to select a successful whole
recipe. Because five other owners also change, this is not an isolated causal
test of that lower owner. No further recipe sweep is justified.

Resource boundaries are nested: native generation/walk requests use a 150 GB
hard ceiling; the existing outer process-tree controllers for pipeline, cold
and source replay use 600 GB. Both arms use the same limits and a 150 GB host
reserve; actual memory use is small as shown above. Compilation is separate.
Independent implementation gates also passed: 19 focused core tests, 9 app
portfolio tests, 7 pure Python descriptor tests and 2 fresh installed-wheel/
CLI/checkpoint tests, with no skips in the native public gate.

Evidence: `positive-shift-control/receipts/{a1-r1,positive-r1}/RESULT.json`
together with each `qualification/RESULT.json`, baseline `DEFAULT_IDENTITY.json`,
and `ACTIVE_REPLENISHMENT_VALIDATION.md`. Implementation:
`frontier_replay_implementation`; execution: `frontier_probe_runner`;
independent source, scope and measurement audit: `rule_quality_audit`.

### Parked experiment: avoid a cut from an impossible earlier rule

The exact residual example exposes a concrete dispatcher mechanism: a rule's
fixed coordinate case can partition an incoming domain before its first guard
proves the rule inapplicable everywhere on that intersection. Both fragments
then reach later rules, preserving a boundary that had no mathematical effect.
The donor's `n8=0` split on residual30569 is the smallest retained falsifier.

The implemented default-off `pre-cut-case-rejection` experiment was not a new rule
order or algebra kernel. On a proper normalized fixed-coordinate intersection,
with no affine case equalities, test only the first singleton excluded-zero
condition using the existing exact native guard resolver and Symbolica
specialization. Only uniform zero permits advancing the original unsplit cell
to the next saved rule. Other results retain ordinary dispatch. Source and
terminal precedence, selected-rule denominators, exact rank/A/D constraints,
cancellation and strict operational errors remain mandatory. Multi-atom,
later-guard and general affine lookahead are deliberately not part of this slice.

The expected benefit is fewer inherited boundaries and therefore fewer redundant
successor obligations; the extra guard probe may instead cost more than it
saves. First test generic pointwise dispatch and the saved smallest residual.
Then compare complete frozen A1 and donor controls with flag-off identity,
independent cold reinspection and unchanged source replay. Stop or park the
experiment if it fails these checks or does not improve whole-cohort work/cost.
Local piece counts alone do not justify a five-loop recommendation. No result
or deployment gain is claimed before those tests finish.

The first native boundary test now passes: all 2,380 concrete-point dispatch
labels remain identical, while the predicted later-rule region coalesces from
two pieces to one. Total pieces decrease 9→8, predicate work 117→98, splits
88→66 and charged cells 480→416. The owned diagnostic completes in 5.778 seconds
including both preparations and drainage. This establishes the local mechanism
only; full-control performance and independent recursive reinspection remain
the acceptance gates. Evidence: `pre-cut-case-rejection/boundary-r1/RESULT.json`.

The first full A1 pair **does not pass acceptance**. OFF cold-verifies all
58 queries/32 roots. ON exhausts its worklist and reduces domains from 26,025
to 24,158 and native inspections from 17,957 to 16,730, but reference-Off
reinspection finds 557 uncovered successor images. Every native was reinspected
with zero native errors/frontiers; the failed coverage test still verifies zero
roots. Forty-four count mismatches are separate informational diagnostics.
The printed `Route successor` denotes the destination phase, not necessarily
the phase of the inspected source node. This result alone neither proves
concrete algebraic unsoundness nor establishes a harmless union-cover mismatch.
Preserve the failure; no as-run substitute or donor continuation is accepted.

Even before that failure, traversal increases 4.128843→4.274225 seconds in this
single pair, with waited walk CPU 22.350→22.547 seconds and sampled RSS
244.4→294.9 MB. Fewer domains therefore are not an accepted performance gain.
Do not retain this unqualified option in the main implementation. Evidence and raw cold output:
`pre-cut-case-rejection/RESULTS.md` and `a1-on-r1/cold-all.json` below that study.
Read-only diagnosis is separate from the next mathematical-order experiment;
no new certification framework is authorized by this failed pilot.

After the remaining native pilots drained, its fifteen modified implementation,
steering and API-documentation files were restored byte-for-byte to their
previous committed versions; only its three new implementation/test files were
removed. The complete recoverable patch, manifest and restoration plan remain
in `pre-cut-parking/` (patch SHA256
`87ecfeeca02f46a56ed13c36de0b9f716f9e2f7bf88f4948403ff4b544a13ee3`).
Independent review checked exact scope and recovery before removal. Reopen only
with evidence resolving the full-control discrepancy and a plausible net gain.
The frozen experiment binaries remain intact; subsequent ordering measurements
using CLI2072 keep that provenance, with the option OFF. They are not relabelled
as measurements of a new clean build.

### Numerator-primary mathematical order: first qualified four-loop pilot

This is distinct from the unsuccessful **rank-first rule-quality score** above:
that score still generated and admitted rules under the original E-then-R
mathematical comparator. A new data-only comparator experiment is justified by
a concrete native witness, after the current pre-cut test.

In the saved A1 lower-owner program, ordered piece364 selects rule117 and its
guarded native output contains the nonzero same-support successor

```text
I( 0, n1,   2, 2, 1, 0, 0, 0, 0, 1)
  -> I(-1, n1-2, 2, 2, 1, 0, 0, 0, 0, 1),  n1 >= 4.
```

Thus actual numerator rank R rises from 0 to 1 while total positive power A
falls by two; total excess E=A−number_of_positive_indices+R falls by one.
This is lawful under E-first and would be forbidden within the same support
under R-first. It is not evidence of an indefinitely repeating recurrence:
the child no longer satisfies this particular rule's `n0=0` case. Nor does
this four-loop observation identify the cause of live five-loop growth.
Evidence: `owner-481-ordered-geometry-r3/a1-r1/{ordered,guarded}.json` under the
local study directory, independently checked against native shift convention,
piece geometry, nonzero coefficient status and successful error-free finish.

The raw domain rank cap is not the attainable rank used by every lookup. In
this example a stored cap can increase from 5 to 6, while existing exact power
projection tightens the child's attainable maximum to R=1. Ready/Epoch semantic
containment already uses that summary and may reuse a containing R5 result.
If reuse misses, the raw cap may remain in an admitted domain; nonliteral routing
does not universally replace it with the projected bound. This particular first
child stays in its owner and goes to Apply. Consequently the witness proves a
rank-increasing rule, not additional queued work or a missing normalization
service. Keep raw-cap telemetry separate from actual rank and measure the full
campaign before attributing any benefit to the proposed comparator.

The proposed descriptor retains support ordering and coordinate ties and uses
two degree rows: first active weights zero/inactive weights one (R), then
active weights one/inactive weights zero (A-excess). Existing runtime order
APIs can express it without changing or rebuilding the Rust solver. Within a
fixed support, exact descent then forbids R growth, and tied R cannot increase
A-excess. This could reduce unnecessary excursions from rank-capped helpers.

There are important countervailing effects. The order is well-founded but has
infinite initial segments when R is positive: smaller R permits arbitrarily
large dot powers. Search depth, exact expression size or guard complexity may
increase. Earlier support priorities still permit degree changes at pinches or
other lower supports, and routing changes coordinates. Neither global rank
boundedness nor finite symbolic closure follows. In the live input, only 13
of the 183 queries have no A upper bound; all 116 required queries and 67
auxiliary declarations must remain untouched. Their relation to current closed
root counts is not established. The optional E-primary envelope certificate
does not apply to this comparator and must not be claimed.

No executed R-primary comparator was found in the audited earlier pilot set;
this is not a claim about every historical scratch run. The registered complete
four-loop test uses A1 source visitation/FirstValid, original
16 owners/508 routes/58 queries/32 roots, unchanged backend and budgets, pre-cut
OFF, fresh generation of both parent programs and a contemporaneous A1 control.
Every new rule needs native order binding and exact source/guard replay, and
the whole control must cold-verify. Old saved rules cannot simply be relabelled
with the new order. Failure to finish under the existing pilot budget, new
unresolved guards, or greater total work/dot/route inflation is negative evidence;
do not automatically enlarge limits or promote the order. The existing selected
preparation driver rejects nonlegacy orders, so use the earlier audited generic
order-pilot path or a separately reviewed small steering extension, not a
reserved-argument bypass.

That test now completes with every gate passed in both arms. Fresh reports,
checkpoints and decoded owners retain the actual programmed order; source replay
checks the new rules rather than relabelling the old programs. Independent
review confirms exact scope and order binding, all-native cold reinspection,
58/32 discharged queries/roots and zero uncovered/errors/frontiers. Both pre-cut
and joint-support pruning are OFF throughout.

| Matched four-loop workload | Fresh A1 | R-primary |
| --- | ---: | ---: |
| Rules / finite residuals | 523 / 28 | 524 / 28 |
| Scheduled domains | 26,025 | 24,971 |
| Native inspections | 17,957 | 16,877 |
| Successor admissions | 410,681 | 384,039 |
| Traversal, seconds | 4.305 | 3.898 |
| Waited walk CPU, seconds | 21.906 | 21.222 |
| Native generation solve, seconds | 5.141 | 7.339 |
| Parent payloads, MB | 4.912 | 6.168 |
| Original-source replay phase, seconds | 41.062 | 86.401 |
| Full inclusive run, seconds | 66.166 | 111.867 |

This first pair reduces domain work 4.05%, native inspections 6.01% and traversal
wall time 9.47%, with a smaller 3.13% walk-CPU reduction. It also increases
generation, payload size and source-replay cost. Retain those costs; do not
present it as an overall logical-cold speedup. The unchanged finite-residual
count is not a comparison of terminal integer-key sets or master bases.
One counterbalanced fixed-program repeat is justified to check sustained
performance without regenerating identical rules. No production/default switch
or five-loop gain follows from this one pair. Evidence:
`r-primary-order-v1/receipts/{a1-r1,r-primary-r1}/RESULT.json` below the study
directory; independently reviewed by `rule_quality_audit`.

That reverse-order repeat now also passes strict cold-All for every native and
all 58/32 queries/roots. Existing qualified programs are restaged unchanged;
generation and source replay are deliberately not repeated or retimed. Exact
domain, native and edge counts reproduce. Candidate-then-A1 traversal is
3.947239 versus4.141971 seconds (4.70% lower), supervised walk6.529515 versus
6.642619 seconds (1.70% lower), and waited walk CPU21.851772 versus22.582122
seconds (3.23% lower). Sampled RSS is256.7 versus248.6 MB. This supports a modest
work/traversal benefit on this control, not a universal9.47% speedup. Retain the
larger one-time generation/replay costs above. Evidence:
`r-primary-order-v1/RESULTS.md` and `receipts/{r-primary-r2,a1-r2}/RESULT.json`.

### Limited five-loop transfer: compatible, no useful work reduction

Both freshly generated orders also pass one five-loop transfer control: the
entire fourteen-sector downset of the seven-line input root
`101101100101000`, fourteen identity routes, and one fixed 784-point starting
query with R≤1, A≤10 and A−R≥9. Descendants are not clipped. This small query is
not the original production query with R≤13 and A≤22, nor the full frozen
116-required/67-helper scope. In particular, this control does not test
nonidentity cross-family routing or establish production scaling.

| Matched limited five-loop workload | Fresh A1 | R-primary |
| --- | ---: | ---: |
| Fresh sectors / reused sectors | 14 / 0 | 14 / 0 |
| Rules / finite residual declarations | 696 / 16 | 696 / 16 |
| Scheduled domains | 1,647 | 1,648 |
| Native inspections, all reinspected | 1,504 | 1,506 |
| Dependency edges | 8,669 | 8,673 |
| Native traversal, seconds | 0.097717 | 0.100990 |
| Supervised walk, seconds | 2.705 | 2.684 |
| Generation phase, seconds | 13.782 | 17.806 |
| Original-source replay phase, seconds | 11.068 | 15.601 |
| Full inclusive run, seconds | 31.788 | 40.309 |

All 696 source identities and guards replay in each arm. Native order admission
accepts every fresh shard, and cold-All verifies the sole required query/root
with zero uncovered images, errors, frontiers or unfinished admission. Both
owned process trees fully drain. W16, CPUs32–47, identical A1 source visitation
and generation/walk policies; both pruning experiments are OFF. These are new
Epoch-versus-Epoch measurements, not a comparison with old Ready timings.

There is no useful work reduction, and approximately 0.1-second traversal
times are too short for a stable speed claim. Generation and replay increase,
so do not repeat this tiny timing pair or recommend production migration on it.
The modest four-loop result remains valid but has not demonstrated a five-loop
gain. The next useful transfer question concerns an unchanged actual required
production domain, using the already qualified rules rather than regenerating
them or testing a blind ordering grid.

Evidence: `r-primary-five-loop-transfer-v1/RESULTS.md` and
`receipts/{a1-r1,r-primary-r1}/RESULT.json` under the study directory.
Execution: `five_loop_order_runner`; independent actual-receipt review:
`r_primary_transfer_audit`. No production input, process or checkpoint changed.
