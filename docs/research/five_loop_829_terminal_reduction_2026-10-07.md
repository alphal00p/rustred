# How to reduce the current 829 five-loop terminal candidates

Research and independent implementation audit, 7 October 2026. Scope: identify
additional relations and the next useful experiment, not implement a new
reducer, change the user's campaign, or claim a smaller basis already obtained.

## Executive conclusion

The strongest immediate opportunity is **saved-rule-assisted, terminal-targeted
Laporta reduction**, followed by higher-sector cancellation identities. There
is no evidence that the current 829 keys require a new mathematical identity
class before ordinary IBP can reduce them. Their published artifact is explicitly
unrefined, and the separate refinement is still running.

Dimensional shifts remain legitimate tools, especially within low-loop
subgraphs. They should not be the first global pass: a standard inverse-shift
composition is already in the momentum-space IBP ideal, and our earlier global
shift experiment produced very large, poorly oriented equations.

This is a research recommendation, **not a measured reduction from 829**. An
individual new exact relation among those particular binary-stored keys has
not been exhibited in this investigation.

## 1. Frozen evidence and the actual question

The campaign is `campaigns/five-loop-rank-ladder-THE-ONE`. At inspection its
latest published directory was
`master-reduction/scopes/52afacba4abc580fae79caf552cc7fc29b500071b7b7d46bc399ca6d1a06f025`.
Local copies of the pointer, artifact metadata and live status were saved in
`TMP/terminal-research-20261007/`; these are untracked evidence, not distributed
artifacts. No production state was changed.

| Published artifact property | Observed value |
| --- | ---: |
| Starting rank / power-difference caps | R <= 2, D <= 9 |
| Accumulated starting-query records | 201 |
| Raw / normalized / remaining terminal keys | 829 / 829 / 829 |
| Processed refinement rows | 0 |
| Publication status | `published_unrefined` |
| Encountered distinct rules | 5,962 |
| Installed owners / batches / rules | 67 / 69 / 9,982 |
| Family contexts in the inventory | 1 |

The 201 records include successive scope amendments; they are not 201 distinct
topologies. The terminal census covers inspected symbolic domains and explicitly
sets `concrete_target_reachability_claim=false`. Conditional routing and earlier
inspected domains can overapproximate the terminal set needed by the current
concrete targets. Removing unreachable keys is a separate exercise from proving
linear relations; neither should be described as the other.

A separate depth-one refinement was verified live, native PID 3167813. The
preserved snapshot at Unix time 1791409435.3478198 reports:

| Running refinement property | Observed value |
| --- | ---: |
| Seeds | 21,353 |
| Ordinary sources per seed | 25 |
| Processed / planned source rows | 38,831 / 533,825 |
| Independent rows | 38,175 |
| Integral columns / auxiliary columns | 99,827 / 98,998 |
| Terminal-only relations so far | 0 |
| Elapsed / observed cores / process-tree RSS | 1,660.60 s / 1.01 / 277.41 MB |

This is **incomplete**, not a failed completed refinement or a benchmark of a
parallel Laporta implementation. Generated-column equivalence normalization
and rebuilding also occur after source generation, so the intermediate zero
does not predict the final count. No completion ETA was inferred.

## 2. The most concrete missing connection

[TerminalRelationSession](../../crates/rustred-core/src/reduction/terminal_relations/mod.rs)
generates exact ordinary IBPs at signed-L1 neighborhoods of terminal keys.
Its state contains no saved owner/routing program. Generated equations receive
only finite terminal normalization before Symbolica elimination. Thus the
refinement can spend time rediscovering reductions already present in the
saved parametric rules.

For terminal vector T and newly encountered integrals U, ordinary equations
have the schematic form

    M_T T + M_U U = 0.

If saved rules supply U = C T, substitution gives a much smaller equation
`(M_T + M_U C) T = 0`. Where such a full substitution is not available, retain
every unresolved column.

### A smaller implementation than a full recursive applier

Insert each applicable saved rule directly as a homogeneous equation:

    I(target) - sum_j c_j(d) I(child_j) = 0.

Also insert exact, coefficient-carrying routing identities. This is enough to
reuse the rules in a joint sparse matrix; **it does not require proving that
recursive rule application terminates**, and cycles do not invalidate linear
equations. Full coefficient back-substitution remains useful for later Vakint
application, but should not be a prerequisite for this experiment.

The existing [candidate evaluator](../../crates/rustred-core/src/solver/candidate_reduction/evaluator.rs)
already implements exact coefficient specialization and applicability checks.
Its private one-step API is the right basis for a narrow identity interface:
reuse fixed-index checks, equalities, exception conjunctions and original
denominator checks. Do not duplicate that logic. The public
[integral transporter](../../crates/rustred-core/src/sector/symmetry/integral_transport/transport.rs)
provides weighted endpoint expansions. Domain successor sets are not equations
and cannot replace those coefficients.

In identity-discovery mode, two independently applicable rules can both be
useful even if normal execution selects only one. Their difference may relate
terminals. A terminal declaration alone supplies no identity and never means
zero. Only exact applicable equations may enter the matrix; local loop-count
or topology names must play no role.

Exact specialization is not a provenance check: the candidate evaluator does
not replay an original-source derivation. Relations inferred from candidate
rules must retain that authority status, unless the input identities have
independently verified source provenance. Validate such provenance once at the
appropriate boundary, not repeatedly in the elimination hot path. Also,
`apply_selected` currently enforces descent. Reuse its coefficient and guard
logic through a narrow identity-specialization interface below that execution
policy if non-descending identities or cycles are to enter the joint matrix.
Current callers also select an owner batch by the target's positive support
before testing rule applicability. An all-rules enumerator must preserve that
sector/root restriction and source conditions. Its return type should retain
generic-d pole/nonzero conditions alongside coefficients and provenance;
normal execution's coefficient-map-only result is not sufficient on its own.

Kira's support for user-supplied equation systems provides an established
precedent for mixing specialized recurrence rows with ordinary IBP rows.
[Kira 2.0, sections 3.2 and 3.11](https://arxiv.org/pdf/2008.06494).

## 3. Which additional relationships are worth testing?

### First: weighted symmetries and factorized lower-loop recurrences

Current normalization includes positive-power parameter equivalences and a
restricted single-quadratic-numerator circuit projection. It is not a general
IBP reduction of every numerator or every dotted factorized product. In
particular, tadpole product aliasing compares equal power multisets; different
dot patterns are not collapsed by that alias operation alone.

For Euclidean massive tadpoles, ordinary IBP gives

    m^2 n J_d(n+1) = (n - d/2) J_d(n).

Products with many dot assignments can therefore share one product master,
with rational coefficients. General factorized components should similarly
reuse lower-loop reductions, with tensor contractions retained. Whether this
particular redundancy occurs among the 829 still requires a per-key census.

For a physical common-mass family with a mass-independent homogeneous numerator N of
momentum degree 2R, another inexpensive row is

    m^2 sum_a a_a I(a+e_a; N) = (A - R - L*d/2) I(a; N).

It is a sum of Euler IBPs, not an independent law. The derivative acts on
actual mass-dependent propagators; do not apply it blindly to auxiliary
numerator slots. Minkowski denominator conventions must be translated.

These are derived illustrations of applicable relation mechanisms, not
already verified relations for named entries in the 829-key binary inventory.

### Second: higher-sector or "magic" relations

It can be necessary to generate equations in a containing sector even though
the final identity contains only its subsectors. A schematic example is

    U + a T1 + b T2 = 0,
    U + c T1 + e T2 = 0,

which eliminates U and gives `(a-c) T1 + (b-e) T2 = 0`. Independent searches
restricted to the two terminal sectors may never generate that pair.
There is no need to add U as a numerical master or enlarge the physical
starting scope: it is a temporary elimination column.

Kira documents an example in which including a containing sector reduces five
apparent masters to four. Smirnov and Smirnov discuss hidden relations obtained
by reducing symmetry-related samples or equating different reductions.
[Kira 1.2, section 3.13](https://arxiv.org/pdf/1812.01491);
[How to choose master integrals, sections 3 and 5](https://arxiv.org/pdf/2002.08042).

The newer *Magic Relations and Critical Varieties of Feynman Integrals* gives
a targeted direction: seek polynomial vectors with both
`sum_i phi_i partial_i G = 0` and `sum_i partial_i phi_i = 0`, where G=U+F.
Bulk terms cancel and boundary-sector terms survive. Its proposed connection
to higher-dimensional critical varieties remains conjectural with stated
assumptions; use it to propose exact equations, not declare independence or
closure. The paper does not demonstrate compression of our five-loop set.
[Crisanti et al., sections 4.4 and 6](https://arxiv.org/html/2605.29789v2).

For a first pilot, generating a few containing-sector IBPs and retaining
their lower-sector remainders is simpler than implementing a general syzygy
search. Verify nontrivial rows exactly, including all boundary terms.

### Third: dimensional shifts within selected subgraphs

Tarasov/Lee dimension recurrences can reorganize the search, but introducing
new d+2 integrals alone does not reduce the numerical input count. Eliminate
them again or supply an exact map to the retained basis.

Bitoun et al., Proposition 31, prove that the standard inverse-dimensional
shift composite belongs to the momentum-space IBP ideal. This does not prove
that every possible integral identity is an ordinary IBP, but it does rule out
treating that composition as inherently new fixed-d independence.
[Feynman integral relations from parametric annihilators](https://arxiv.org/html/1712.09215).

Our [October 1 dimensional experiment](dimensional_recurrence_shortcuts_2026-10-01.md)
already produced 4,157/4,195-term paired rows, with over 2,200 terms harder than
the intended target. That negative result is specific to its target, not a
general impossibility theorem. Reopen through a demonstrably sparse low-loop
subgraph, inspired by FMFT's convolution approach, rather than expanding global
five-loop Gram determinants indiscriminately.
[FMFT, sections 2.2–2.4](https://arxiv.org/html/1707.01710).

Never set the five-loop Gram determinant to zero merely because there are five
vectors in four dimensions: the artifacts live at d=4-2epsilon. Evanescent
terms can multiply poles. Nor does a fully massive bubble generally integrate
to one epsilon-powered propagator; the distinction is explained in
[EPSILON.md](../../EPSILON.md).

## 4. Proposed experiment and acceptance

1. Freeze the source publication and expose its exact per-key census without
   rerunning the campaign. Group keys by support, dots, negative-index powers,
   verified symmetry orbit and factorization.
2. On a representative frozen subset, compare equal ordinary-source budgets:
   current refinement versus the same rows plus applicable saved-rule and
   weighted-routing equations. Preserve the 829-key union as the full target.
3. Canonicalize generated columns before or periodically during elimination,
   measuring normalization cost instead of assuming it is free.
4. Probe at generic d over several finite fields to select useful source
   support. Rebuild exact rows over Q(d), or use Symbolica reconstruction and
   exact replay. Never accept a sampled relation as the artifact authority.
5. Only where the count stalls, add selected dot/numerator neighbors and
   containing sectors. Do not clip generated integrals by the physical R/D cap.
6. Test sparse subgraph identities on the remaining stubborn clusters.

The pinned Symbolica source already exposes
`poly::reconstruction::reconstruct_rational_function_over_q`; current RustRed
uses Symbolica sparse exact/finite-field reducers. No new CAS, interpolation
or rational-reconstruction kernel is called for. This audit did not implement
an algebraic primitive or exhaustively audit an upstream API change.

Measure exact eliminated-terminal rank, new unresolved columns, coefficient
size, fill, wall/CPU time and peak memory. A faster matrix with the same basis
is a performance gain; fewer exact terminal labels is a compression gain.
An exchange between equal-dimensional bases requires an explicit invertible
transformation. Compressing a redundant spanning list instead requires exact
substitution identities expressing the removed labels in the retained list;
there is no invertible 829-to-smaller matrix. New numerical unknowns must be
included when reporting the final count.

Published five-loop numbers are useful scale only: the 2016 work evaluated
103 masters in 63 sectors and listed nine more in four twelve-line sectors;
other thesis/QCD counts differ. There is no verified map that makes 110 or112
an acceptance requirement for these 829 candidates.
[Five-loop massive tadpoles, section 3](https://arxiv.org/pdf/1609.06786).

## 5. Delivery and audit

- Root: inspected current publication/live process, existing refinement and
  normalization, primary papers, and integrated recommendations.
- `terminal_reducer_audit_20261008`: independent read-only code/campaign audit;
  checked exact-row injection versus a full recursive applier.
- `terminal_literature_20261008`: [literature and concrete pilot study](five_loop_terminal_relations_literature_2026-10-07.md).
- `dimension_relation_audit_20261008`: [independent dimensional/subgraph critique](five_loop_dimensional_relations_audit_2026-10-07.md).
- Final synthesis audit: `terminal_literature_20261008` independently checked
  the mathematical claims and references; `terminal_report_code_crosscheck`
  checked the saved-rule interface, authority boundary and descent-policy
  caveats. Their corrections are incorporated above.

No reduction count was fabricated from bibliography or a modular rank. No
solver implementation, production process, campaign or artifact was modified.
The remaining work is a separately authorized implementation/measurement of
these proposed terminal-compression passes, not unfinished literature search.
