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
