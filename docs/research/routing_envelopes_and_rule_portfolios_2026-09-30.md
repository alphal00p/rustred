# Structural candidate: retain verified numerator-incidence envelopes

Date: 2026-09-30. Author: `/root/s5_fixed_work_pilots`.
Status: **research-only shadow test prepared but not yet linked or run; no
production cover change, native result, or production mutation**.
Independent mathematical/source critique by `/root/final_requirements_audit` agrees
with the conditional derivation; a measured shadow audit is still required. Root
reviewed the cited route/support implementation independently.
The separate allocation-removal implementation is not part of this proposal.

## Follow-through at 20:25 UTC

The bounded metadata-only extractor inspected16 ranges of at most2MiB from the
completed, cold-All-accepted finite-five Ready control. It read39,383 records,
found21,260 eligible nonliteral/nonzero completed Route records and153 source
masks, then selected8 routes/15 boxes. Selection deliberately favors visible
coordinate refinement: this is a biased feasibility sample, not an occurrence-
weighted census. It includes reached rank3 descendants of rank2 starting input;
the starting restriction is never incorrectly reapplied to successors.

Independent review checked all16 range hashes, all15 record hashes/fields,
the8 original witness records and the full67-owner mask list. The original
selection and CP5 request/generation binding match the completed control's cold
receipt. The research test retains the same source-condition checks and requires
exact agreement with recorded visitor classification counts before measuring
any tighter cover. Its synthetic empty-rule owner set must never be used to
apply IBPs, publish an artifact or claim closure. There is no native shadow result
yet. Source and receipts are described in `CODEX_PROGRESS.md`.

### Exact elimination: native API audit and next falsifier

The pinned Symbolica/Numerica public sparse API already provides
`SparseMatrix::solve_parallel` and `SparseRowReducer::back_substitute_parallel`.
Inspection of `vendor/symbolica/lib/numerica/src/tensors/sparse.rs` shows that
the former still constructs its forward reducer serially and then parallelizes
back-substitution. RustRed's observed tail is `SparseRowReducer::add_row` during
**forward** elimination. These existing APIs therefore cannot be advertised as
a direct switch that parallelizes the measured phase. No competing elimination
or polynomial kernel is proposed.

Before changing production search, a new input-driven diagnostic test will use
the existing public `solve_case_with_source_order_and_observer` to compare A1
and A0 on identical decoded saved cases. Start with two four-loop cases, then
one substantial case from each of the already completed five-loop parents30527
and30699. Read only their completed immutable shards, never unfinished3822 state.
Current public bundle inspection does not expose full typed cases, so one native
test build is necessary; subsequent choices use the same input-driven executable.

This intentionally materializes both candidates for a small **quality
experiment**, not as the proposed production design. Preserve saved-A1 exact
candidate equality, the source basis/zero census/mathematical order, source
provenance and exceptional geometry. Charge all discovery, exact lifting and
exception extraction. A candidate result is not a completed sector or campaign.

The earlier isolated five-loop case195 study is a concrete counterexample to
ranking solely by selected-row count:

| Source order | Selected rows | RHS terms | Search wall, including exact (s) | Exact portion (s) |
|---|---:|---:|---:|---:|
| Identity |496|781|36.934|36.019|
| Reverse |418|653|37.418|36.460|
| Half rotation |454|725|27.507|26.928|

All three retained the same unsupported conic. These are historical single,
shared-host observations, not a completed-sector speed qualification. They
motivate measuring candidate quality and retaining negative evidence rather than
assuming smaller support always wins.

If the new panel supports it, the minimal production proposal is a private
discovery/materialization split with default first-hit identity, one optional
sequential alternative probe with an explicit extra-row budget, and exact lifting
of only the selected frame. The baseline remains available on an alternative
budget miss/unlucky sample; none of these events may suppress a case or declare
a terminal. Record both probes' costs and any exact fallback. Bind the selection
policy into generation checkpoint identity, not into stronger proof authority.

Raw pivots can differ by shifts even for the same case. `canonicalize` translates
coefficients and RHS by that pivot; successful canonical targets are checked
against `case.integral()`. Any modular quality proxy must therefore use
**target-relative** shifts and activation offsets. It cannot know exact guard
complexity, cancellation or exceptional geometry before exact lifting. A smaller
frame that produces worse domain fragmentation is a failed optimization.

## Recommendation and scope

First perform a small **shadow applicability census**, using the already verified
denominator maps and actual routed source boxes. If it finds substantial tightening,
extend the existing route overcover with per-inactive-axis upper bounds and,
separately, a proved homogeneous-map upper-D bound. This is a narrow, exact,
topology- and loop-count-generic change; it need not introduce new domain types,
polynomial arithmetic, interpolation, or a proof engine.

This targets **later saved-rule traversal**, not the current production campaign's
exact elimination while generating the new A1 owners. The old LC2's hundreds of
millions of discovered domains are not hundreds of millions of fresh GPLU solves.
The new campaign was still generating owners when this proposal was prepared.
Neither zero frontiers nor its initial generator timings establish scoped closure.
Keep every one of the frozen **116 required queries and 67 auxiliary helpers**.

Rank versus rule selection:

1. **Fastest exact structural falsifier:** this incidence-envelope candidate. The
   required exact metadata already exists; the missing consumer is localized.
   Potential total-work gain is currently **unmeasured**, and may be negligible.
2. **Strongest demonstrated sensitivity / larger potential:** downstream-work-aware
   rule selection. A1 already substantially changed four-loop rule/descendant work;
   extending that into a bounded candidate portfolio is more invasive and needs
   rule-level attribution and exact materialization cost control. Do not delay it
   indefinitely for envelope polishing if the shadow census is weak.

## Current-code audit: what is known, not inferred

Paths below are relative to the repository root.

| Existing code | Audited behavior |
|---|---|
| `crates/rustred-core/src/sector/symmetry/integral_transport/compile.rs` | Compiles only an exactly verified map: rational-constant affine inactive rows, unit active-row bijection, zero analytic shifts, unconditional unit Jacobian. It validates coefficients through the coefficient context, then uses exact `is_zero()` to record inactive-source incidence for **every target column**. |
| `.../integral_transport/model.rs` | `Prepared::numerator_sources_for_target(j)` exposes sorted source rows of that incidence; `Prepared::verified_map().denominators().constant()` exposes the already verified constants. No denominator-map reconstruction is needed. |
| `crates/rustred-core/src/solver/candidate_reduction/routed/domain_overcover/support.rs` | `NumeratorDegrees::from_source` calls `column_cap` only for **active** target columns. The cap uses source coordinate bounds plus the aggregate rank after existing exact projection. |
| `.../domain_overcover/visit.rs` | Literal owner boxes are retained. Nonliteral `target_upper` starts entirely unbounded, and only mapped active axes receive an upper bound. Every inactive axis of each emitted pinch is again assigned upper `None`, lower zero. Existing projection can recover a total-rank bound, not the forgotten per-axis map support. |
| `.../domain_overcover/power.rs` | `mapped_bounds` retains lower D but always discards supplied upper D, because constants in an affine numerator map can increase D. It uses A' as a safe substitute upper bound. |
| `.../domain_overcover/support/tests.rs` | Existing `column_cap` has an exhaustive finite-coordinate test with >10,000 combinations, explicit empty/unbounded/wide cases, and a Symbolica-expanded cancellation test. Reuse this arithmetic and authority rather than inventing another support solver. |

The current constrained route visitor projects the source box before reading these
bounds. With unconstrained A/D it intentionally preserves older behavior and does
not currently construct `NumeratorDegrees`. An initial opt-in prototype can target
the constrained path only, preserving default/off identity. Extending to plain
R/box-only inputs later still requires the same nonempty-source bookkeeping.

### Frozen input census (not an algebraic incidence census)

`CENSUS.json` and the read-only `census.py` in
`TMP/aster-integration-20260930-resumed/structural-routing-envelope/` bind the actual
production shared family/selection/query bytes. The census completed in ~0.13 s;
that is preparation, not an IBP or traversal performance measurement.

- 67 owner masks; 8,246 distinct route source masks; 8,179 request transport.
- All destination masks are among the 67 owners; every raw source/destination mask
  pair has equal active cardinality.
- Routes by active count: 5:974, 6:2,126, 7:2,364, 8:1,679, 9:801, 10:251,
  11:47, 12:4. Thus many registered routes have seven or more inactive coordinates,
  but this alone says nothing about runtime visitation or actual tightness.
- 183 initial queries: 116 required, 67 auxiliary. Their actual R values are
  heterogeneous (1 through 14), so this is not a uniform R=10 experiment.
- All 116 required inputs have finite A and lower D; 98 also have finite upper D.
  However, **none** of the 681 required-input inactive-coordinate uppers is below
  that query's total R; none is fixed zero. All 406 helper inactive-coordinate
  uppers are absent; 54 helpers have finite A and 13 have unconstrained A/D.
  Thus initial boxes do not by themselves demonstrate the toy cap advantage:
  its opportunity must arise in actually visited, subsequently refined boxes.
  Upper-D retention has an input restriction to preserve in 98 queries, but still
  needs actual homogeneous routes and nonredundancy after projection.
- Only 67 routes have identical source/owner raw momentum witnesses; there are
  8,224 distinct raw witness pairs. This is **not** a count of distinct composed
  denominator maps. Signed momentum permutations likewise do not imply denominator
  permutations in this nontrivial quadratic basis.

The JSON stores two loop-momentum witnesses, **not** the verified denominator
incidence matrix. No independent Python matrix inversion / polynomial algebra was
written to fill that gap. A small native shadow observation using `Prepared` is
still needed to know which actual inactive caps are tighter and how often relevant
inactive rows are homogeneous. The source-code audit confirms availability and
meaning of exact metadata, not numerical benefit on all 8,246 routes.

## Exact derivation

Let inactive source powers be `n_i=-r_i`, with `r_i>=0`, coordinate interval
`l_i<=r_i<=u_i` (u may be absent), and total rank `sum r_i<=R` (R may be absent).
The admitted map has

`D_i = c_i + sum_j M_ij D'_j`.

Let `S_j={i inactive : M_ij != 0}` be the existing verified incidence. Every
monomial that survives the exact numerator expansion has exponent `e_j` obeying

`e_j <= C_j = min(sum_(i in S_j) u_i, R - sum_(i not in S_j) l_i)`.

When one of the two caps is unavailable, use the other; when both are unavailable,
keep the axis unbounded. This is exactly the existing `column_cap` logic. Each
factor occurrence supplies at most one unit of degree, and only a row containing
column j can supply that column. Constants supply no target degree. Cancellation
can remove endpoints, never introduce an exponent violating the bound. A finite
cap is necessary, **not a claim that every exponent beneath it is attainable**.

The unit active map gives pre-numerator target denominator powers B_j. For an
originally inactive target, B_j=0. For a root-active target its matched positive
source has B_j >= L_j+1, where L_j is the existing local active lower coordinate.
Every final target index is `n'_j=B_j-e_j`.

Therefore:

- **Originally inactive target j:** its local numerator coordinate is e_j, so add
  upper `C_j`. Retain lower zero unless another already proved bound tightens it.
- **Newly pinched target j:** its local coordinate is `e_j-B_j`, so add upper
  `C_j-(L_j+1)`. If `C_j<L_j+1`, that pinch is impossible; omit it rather than
  saturating it into a fabricated rank-zero branch. Equality permits local zero.
- Keep existing total R/A/D bounds, source-condition obligations, phase changes,
  zero handling and exact projection. This is intersection with an additional
  necessary inequality, not an alternative admission/closure rule.

All sibling covers must derive their bounds from the **source projection**, not
from the already projected all-positive sibling, whose extra assumptions need not
hold after a pinch. Subtract only that target's own positive B lower from C_j;
do not subtract other pinch costs from C_j without a separate disjoint-supply proof.

Use checked u128 degree arithmetic already present. Never cast a wide finite cap
into a smaller integer. If an optional tightening is wider than the coordinate
interface, omitting that tightening is safe; do not narrow it by saturation.
Retain the existing routed finite-rank-above-u32 rejection; optional cap omission
does not authorize bypassing that representation boundary.
Malformed source/route data and impossible mandatory lower totals retain the
existing typed failure/empty behavior. Cancellation still means incomplete work.

### Concrete example: bounds, not mask pruning

Consider three inactive source denominators with verified linear substitutions

`Q1=A+B`, `Q2=C`, `Q3=B`,

and source powers `0<=r1<=1`, `0<=r2<=9`, `r3=0`, `R<=10`. The linear map on these
three axes is invertible; this can be a block of a larger verified family map.

The current generic inactive cover can include all `(a,b,c)>=0` with `a+b+c<=10`:
286 lattice points. Incidence gives `a<=1`, `b<=1`, `c<=9`; intersecting the same
rank bound leaves 39 points. The actual positive-coefficient expansion occupies
30 points because additionally `a+b<=1`. The proposed representation still keeps
some unreachable points; it is not secretly claiming an exact affine image.

**No sector mask was removed here.** The benefit, if this pattern occurs in real
visited boxes, is a smaller emitted domain and fewer later shifted descendants.

For a pinch example, suppose source positive power B_j>=3, total source rank up to
10, but incidence proves e_j<=4. Once j is pinched its excess numerator coordinate
is <=1. The existing generic residual-rank cap of 10-3=7 cannot express this
axis-specific restriction. Other inactive axes may still use that residual rank.

Conversely, if every inactive source upper is merely R and each target column is
fed by such a source, `C_j=R` often adds **nothing** beyond existing projection.
Dense maps / broad boxes are an explicit failure mode for the proposed benefit.
Similarly, collecting the union of all inactive target supports often contains
every inactive source in a full-rank map, so it would not by itself improve total
rank. Do not oversell an aggregate-rank breakthrough from the same incidence.

## Independent optional improvement: homogeneous upper D

Here `D=A-R` is the sum of all signed integral indices, not spacetime dimension.
Before numerator expansion the sum of target denominator powers B equals source
A, because the active rows form a unit bijection.

If **every inactive row that can carry a positive source exponent** has exactly
zero constant `c_i`, every expanded monomial has degree exactly `sum_i r_i`.
Consequently `sum_j n'_j = sum_j B_j - sum_j e_j = A-R = D`.
Sector classification and pinching do not change this signed sum. Polynomial
cancellation only removes same-degree terms. Hence the old source upper D can be
retained, intersected with the existing safe target A upper bound.

Check constants through the exact verified API above. A row whose projected upper
is zero can be ignored; an unbounded or positive upper is relevant. **Unit mass and
vacuum kinematics do not imply this homogeneity:** rewriting quadratic expressions
in denominators `q^2-1` can generate constants. If any relevant row has a nonzero
constant, preserve the current conservative upper-D fallback. A nonhomogeneous
map might still permit a sharper bound, but that is outside this first proposal.

Example failure of naive D preservation: Q=C+1 with r=2 has monomials C^2, C and 1,
so D' can equal D, D+1 or D+2. Keeping the old max D would discard legitimate work.

## Why this is not the previous failed experiment

`docs/research/joint_pruning_independent_campaigns_2026-09-25.md` reports the prior
joint-support mask experiment: 445,442 masks rejected, native work only -0.51%,
traversal +1.14% and whole time +0.90% in one shared-host finite-five pair. Preserve
that negative/inconclusive outcome; do not repeat it unchanged.

That experiment asks whether a chosen **removed-axis subset** can be supplied by
the union of its source rows. This proposal constrains the coordinates of
**surviving emitted domains** and optionally retains a global invariant. It uses
the same trustworthy incidence, not the same intervention. Nevertheless smaller
geometric overcovers need not make the walker faster: they can impair subsumption,
produce more distinct boxes, or spend arithmetic on bounds that projection already
implies. End-to-end descendant work and runtime, not mask/cap counts, decide.

## Smallest tests, evidence gate and stopping rule

1. **Before production implementation:** one bounded shadow observation on the
   existing frozen finite-five control (all 67 owners, one 1,324-point physical
   query, explicit limited scope), or a small fixed sample of its previously
   visited Route domains. Use the existing native map compiler; do not decode
   binary CAS payloads independently. Count nonliteral visits, strictly tightened
   inactive coordinates, fixed-zero destinations, homogeneous eligible visits,
   finite-max-D homogeneous eligible visits, map-cache cost,
   redundant-after-projection caps, and retained-box lattice volume where finite
   enumeration is already admitted. Aggregate by owner/sector; bounded samples
   only. Observation must not change dispatch, authority, or the live campaign.
2. Extend existing core exact-transport tests: every Symbolica-expanded endpoint
   for small bounded source powers belongs to the proposed cover. Include
   constants, homogeneous maps, cancellations, repeated column support, pinches,
   wide/unbounded coordinate caps, R=None, empty boxes, fixed-zero inactive rows,
   and the two toy examples. Compare flag-off output exactly. Cover generic arities,
   not a named five-loop topology.
   Include a zero polynomial inactive row (positive exponent has no endpoints;
   exponent zero contributes degree zero), negative coefficients, a nonhomogeneous
   row fixed to exponent zero, and the existing affine-constant D regression.
3. One opt-in fixed-input pilot using saved rules, **not regeneration**, only if
   the shadow audit shows material opportunity. Use four-loop controls with an
   explicitly measured nonliteral-route count: historical FG/BMW/H/X literal-owner
   controls had zero routed masks and are correctness controls, not efficacy tests.
   Then use the exact existing limited-five control. Preserve cold full reinspection,
   all requested roots, source-validity checks and failure reporting. Fewer output
   domains alone is not independent proof of soundness.
4. Compare native inspections, admitted domains, peak pending, required-query closure,
   work mix, CPU, memory and full wall boundary. Look explicitly for smaller boxes
   **increasing** fragmentation or defeating G2/subsumption. Only a matched repeated
   gain justifies changing the next production launch; do not interrupt this one.

**Stop/defer** before a full implementation if the actual incidence/box census
usually reduces to the existing rank cap, homogeneous cases do not have a finite
max-D to retain, or candidate bounds disappear as redundant after projection.
After one evidence-backed correction/retest, park it if measured descendant work
and whole runtime do not improve. It is not a reason to write a new polyhedral
engine or repeatedly tune mask pruning.

## Larger next candidate: downstream-work-aware rule portfolio

`solver/search.rs::search_validated_case` currently returns the first exact direct
or modular-discovered valid candidate. Runtime source order A1 changes which hit
is encountered. `discovery_strategy.rs::SourceRowFeatures` describes input-row
structure, not the resulting rule's routed descendant obligations.

The older `foundry/completion/spired/target_run/run.rs` already contains a bounded
`BranchCandidateQuality` policy (guards, residual/source terms, etc.); a portfolio
idea is therefore not wholly new, but that seam does not currently select winners
in the production `solver/search.rs` path. Any implementation should reuse the
existing exact authority pipeline and suitable architecture rather than duplicate
the algebra or revive a legacy campaign wholesale.

For a few admissible candidates, compare induced sign/guard pieces, mapped successor
domains, rank/power excursions and reusable lower owners on fixed representative
queries. Those samples only **guide candidate choice**; exact rules, guards, replay,
descent, exceptional cases and finite terminal policies remain authoritative.
Bound the post-hit cost, since materializing several poor exact candidates could
erase the whole gain. Rule-level successor/route/guard attribution is currently
missing; aggregate counts cannot identify the worst recurrence reliably.

A1's measured four-loop required-domain counts fell 69,315→38,680 and
64,231→38,681 in two historical pairs; total times 97.286→84.538 s and
102.024→82.553 s. Limited-five source-policy evidence also improved, but full
67-owner A1 traversal has not yet completed. Generation/admission subsequently
completed for all67 owners; the October1 walk stopped on a local saved-rule
coverage gap after49.918M inspections (see `CODEX_PROGRESS.md`). These establish genuine
sensitivity to generated rules, not a promise that a further portfolio closes the
five-loop scope or wins by the same factor.

Independent read-only mathematical review by `/root/final_requirements_audit`
found no blocking proof issue under the admitted-map assumptions. Its additional
source-projection, overlapping-support, representational-limit and shadow-gate
requirements are incorporated above. This review is not an executed test or a
measured speedup.

The envelope shadow screen is cheaper and can falsify a precise omission quickly;
the rule portfolio remains the better-supported **large-gain direction** if that
screen is weak. Neither candidate is a proven cure for finite-but-enormous domain
growth, and neither changes the frozen physics scope.
