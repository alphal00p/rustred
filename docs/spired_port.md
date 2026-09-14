# Executable SpIRed reference port

## Objective

Port `vendor/spired/src/solver.tpp::solveSector` faithfully to Rust, using
Symbolica/numerica for all CAS and finite-field arithmetic. The acceptance
scope is every supplied one- through three-loop (2–4PM) example, with equal or
better release timing and identical or algebraically smaller rules. This is
not satisfied by a fast single case, an incomplete sector, or a vacuum-only
subset. Reference source and generated reference artifacts remain local.

## Why the existing notes-based search is not the reference algorithm

The executable reference exposes several material differences:

- Integral order is harder-first SpIRed/LiteRed order, including symbolic vs
  fixed powers, sector signs, cuts, total degree, numerator degree, and
  coordinate tie-breaks. RustRed V1 order cannot be used as a substitute.
- Sources are preconditioned once per sector by GCD-scaled polynomial forward
  and backward cancellation. The reference does not first make a rational RREF.
- Any leading integral compatible with the current equalities solves the case.
  Its symbolic shifts are removed afterward. Thus an unconstrained symbolic
  case accepts the first nonzero prepared row directly at seed depth zero.
- Otherwise one incremental finite-field reducer processes all columns. A
  matching pivot triggers L-pattern ancestor extraction, ordered by pivot
  column, followed by a compact exact forward solve. No full RREF is needed.
- Fixed-index cases have a separate bounded seed search; unresolved ones are
  designated masters. Multiple fixed cases share seeding and elimination.
- Coupled linear equalities are executable cases, not sampled rectangles.

The reference's nonlinear exceptional simplification uses a bounded integer
search (`|n_i| <= 30`). This is a heuristic, not a general integer zero-locus
proof. The policy for this behavior and for residual-master classification has
been submitted to the user; neither should be silently resolved by claiming
stronger closure than the search establishes.

## Architecture and implementation sequence

1. Add compact solver primitives: one-byte symbolic/fixed powers, inline
   const-generic integral arrays, reference ordering, and allocation-free L1
   seeding in reference chronology. Structural integer arithmetic is checked;
   arithmetic on coefficients and field values belongs to Symbolica/numerica.
2. Lower existing generic RustRed IBPs once to immutable native polynomial
   rows. Share their variable maps and avoid per-term context wrappers. Port
   reference polynomial source preconditioning, including coefficient tie-breaks
   in indices-first DEGLEX order.
3. Implement direct compatible-leading extraction and canonical translation.
   Add the single native modular reducer, accepted-source tracking, L-pattern
   pruning, and native exact replay. Keep source conditions and case boundaries
   explicit; a returned equation is not yet an unconditional rewrite rule.
4. Implement the ordered equality-case queue, exact denominator/activation
   exceptions, subsumption, coupled linear cases, and shared numerical search.
   Port cut-derivative preparation and LI sources needed by the PM fixtures.
5. Exercise complete reference examples, compare rules algebraically and case
   coverage explicitly, and profile release runs. Fix the dominant measured
   costs rather than impose previous search machinery on the reference flow.
6. Connect accepted solver results to artifact certification/publication and
   user interfaces without adding that ownership bookkeeping to the hot rows.

The new cohesive `rustred::solver` module is the source-port implementation.
The existing `foundry::completion::spired` remains the prior notes-based
implementation during this transition; it must not be confused with a verified
port or used to claim reference-performance acceptance. Remove/replace redundant
search paths only once their consumers have moved, preserving independent
artifact mathematics and the optional Janet/Ore strategy.

## Native API audit

The checked vendored public APIs provide the required primitives:

- `SparseRowReducer::add_row`, `add_cols`, `LuLMode::Pattern`, `l`, `u`, and
  `pivots` implement the same dense-scratch/first-free-pivot streaming algorithm
  as C++ `gpluRow`. Do not write another elimination kernel.
- Native L includes rows for dependent inputs; accepted U-row ordinals need a
  separate mapping to L input slices. A trailing unused column avoids native
  full-rank early-return corner cases during dynamic insertion.
- `RationalPolynomial<IntegerRing,u16>` and `MultivariatePolynomial` are the
  coefficient types. Native GCD, exact division, `shift_var`, `replace`, and
  `evaluate_with_coeff_map` provide polynomial operations. Coefficient ordering
  is application scheduling, not a replacement polynomial representation.
- Exact replay uses `RationalPolynomialField<IntegerRing,u16>` with the same
  native sparse reducer. No custom rational reconstruction is introduced.

One measured-storage caveat in the current native API: Pattern L retains a
slice for every dependent input, whereas the C++ reducer discards that slice.
The Rust mapping handles accepted/dependent row identities correctly, but this
is not yet equivalent memory scaling. Both retained L rows and entries are
reported explicitly. The current public API has no safe way to drop only a
dependent slice while retaining U, pivots, and ongoing pattern recording; a
small upstream opt-in operation would remove this difference without changing
any arithmetic kernel. Do not substitute a custom Rust elimination routine.

Native polynomial parsing may construct a new, equal variable-map `Arc`.
Ordering and variable identities, not pointer equality to a caller template,
are the mathematical invariant. Let Symbolica own its context sharing.

## Reference corrections and explicit differences

The user approved preserving a complete homogeneous identity in the shared
numerical solver (2026-09-14). In the uploaded C++ `solveNumCases`, the direct
hit path calls the mutating `ibp::minusRest()` and can then submit that same
modified object to GPLU. The object now contains only the RHS, not an identity
equal to zero. Rust clones only for direct-rule extraction and keeps the
original row intact for the shared reducer. A regression uses
`I(n+1)-I(n)=0`: discovering `I(2)=I(1)` must not falsely imply `I(1)=0`.

The shared numeric row cutoff also uses the actual easiest requested integral
under the integral ordering. The last case in equality-case lexicographic
order is not necessarily that integral. Case visitation order itself remains
the reference's order. Fully numerical initial cases, when every coordinate
is a removed cut, use the bounded numerical phase directly; this edge differs
from C++ starting every initial case through `solveCase`.

These differences are not performance optimizations justified by the vacuum
fixture. They preserve exact equations and explicit bounded-search semantics
for the generic API and are recorded rather than hidden as reference parity.

## Validation and current baseline

Each implementation slice gets independent audit and focused differential
tests. Cover compact overflow, exact ordering, seed chronology, source span,
dynamic columns, dependent input rows, matching shifted pivots, exact replay,
guard branches, and genuinely numerical residual cases. Test real family
preparation end to end, not just hand-authored rows.

Current C++ release measurements on the AMD EPYC 9754 host:

| Workload | Serial process wall | Six-worker process wall | Rules |
| --- | --- | --- | --- |
| `fam1_11` | median 0.75 s (3 runs) | median 0.14 s (3 runs) | 802 sector + 2 pre-rules |
| `vac3` | 1.44 s (1 run) | 0.31 s (1 run) | 617 |
| `vac4` | 72.67 s (1 run) | 45.40 s (1 run) | 1272 |

The `vac4` example selects 27 sectors, not all 743 nonzero sectors. Counts of
residual masters are not independence proofs. Logs, output files, build options,
and exact workload details are local under
`vendor/spired/build-release/benchmarks`. Timings exclude compilation but include
process initialization and output; compare the same boundary on both sides.
The newer paired `vac3` comparison below covers that complete reference run;
the PM suite and Rust `vac4` comparison remain outstanding.

## First implemented slice (2026-09-14)

The compact coordinate-case search, source adapter/preconditioner, native
discovery, pruned exact replay, and exact denominator/activation exception
extractor are implemented. All 46 focused tests passed in the integrated run;
the first 34 and subsequent exception mathematics received separate delegated
verification/audits. Compiled C++ fixtures validate 20,000
mixed-power order comparisons, 377 seeds through three-coordinate L1 depth six,
and four polynomial-preconditioning examples.

The release `spired-solve-case` example ran the genuine ordinary-source K1,
K3, and K6 generic cases and the five successive trailing-one coordinate faces
of the full K6 sector. These all produced a compatible source-row candidate at
seed depth zero. For one observed K6 generic run, source/family preparation was
1.501 ms, sector preconditioning 0.393 ms, and case search 0.119 ms. These are
**single-case component measurements**, not family closure timings or a claim
of benchmark parity. The all-one K6 corner exhausted depth three after 756
rows in a separately timed 0.06 s process; it remains a numerical residual, not
an automatically certified master.

An example-only comparator then matched the exact RHS of **all six** full-top
K6 coordinate cases against the C++ `111111.dat` export with explicit `m=1`
specialization. Each Rust candidate is generated independently before the
reference is read. This comparison is coefficient-by-coefficient native CAS,
not string equality. Five separate example-comparator tests exercise native
normalization, coordinate mapping, explicit mass substitution, pole/mismatch
rejection, and syntax validation. It does not compare guard coverage or claim
sector closure.

The subsequent automatic-sector slice implements equality-case traversal,
exact coordinate intersections, guard-aware subsumption, and the reference's
shared numerical search with one native modular system and one union-trace
exact solve. Its first combined run passed 70 focused tests. A full optimized
`vac3` campaign then independently generated all 38 sectors, 617 rules, and
38 finite residuals; all 617 RHS equations matched C++ with symbolic mass, and
an independent audit matched the coordinate guard domains and residual set.
Three latest interleaved serial repeats from the normal Cargo release driver
measured median process times of 0.66 s Rust and 1.47 s C++, with output-format
and shared-host caveats documented in
[the complete benchmark report](spired_vac3_results.md).

An LI source adapter and expanded guard comparator have also been implemented
and independently reviewed. The expanded integrated solver suite passes all
77 tests, including LI source order/signs and the actual vacuum numerical
boundary equation. The updated `--release --locked` driver again generated
all 617 rules and passed programmatic exact RHS, coordinate-guard, and
sector-sign comparison. Its equations and residual outputs are byte-identical
to the first run. All 12 example-comparator tests also passed in release mode,
and `cargo check --locked --workspace` passed with an explicitly selected
installed Python interpreter. The targeted Symbolica variable-map ownership
migration regression passed as well. Coupled affine cases, cut-derivative source preparation,
and complete PM fixture coverage remain outstanding. `RuleCandidate` still
does not claim unconditional applicability or publish artifacts; `SectorRule`
adds its exact exceptional conditions, and `SectorSolution` explicitly retains
bounded finite residuals without declaring certified master independence.

Keep user worktree changes intact. Every Git operation uses the requested
ValentinHirschi name/email, and no reference-only material is committed or pushed.

## Next source-preparation slice: removed linear cuts

The reference's `family::improveIBPs` pipeline must precede the PM sector
benchmarks. For a simple linear cut `D_i = c k_a·u`, solve the ordinary identity
`u·∂/∂k_a` for the raised cut, substitute positive cut shifts once throughout
the ordinary/LI source frame (translating coefficients as well as integral
keys), export the canonical pre-rule, then fix each removed cut index to one.
The exported pre-rule is exceptional at `n_i=1`, not zero as one C++ comment
suggests. Native division, `shift_var`, `replace`, GCD, and exact division are
already available; no new CAS framework is needed.

The structural change needed next is a common fixed-coordinate source
pattern: improved C++ rows store numerical cut powers equal to one. Source
admission and instantiation must distinguish these absolute fixed powers from
symbolic shifts, validate the removed-cut mask, and never apply such a source
to a free-cut case. Start with the reference's diagonal cut-derivative incidence
and reject unsupported coupled cuts explicitly. Also retain existing noninteger
power offsets when preparing the cut derivative: raw C++ `deltaReplRule` omits
that shift, although none of the supplied removed-cut examples exercises it.

The first exact differential fixture uses `D0=k·u`, `D1=k·v`, `D2=-k²`,
`u²=v²=1`, `u·v=γ`. Its pre-rule is
`I(n) = -γ n1/(n0-1) I(n-e0+e1) + 2 n2/(n0-1) I(n-2e0+e2)`.
Then compare both `fam1_11` pre-rules and the improved source frame before
running its complete sector census. Coupled affine case support remains a
separate required part of the all-PM reference-port objective.
