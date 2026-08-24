# Exact four-loop next-shell elimination design

Date: 2026-08-13

## Scope and present status

This document describes the accepted certificate after
[`FourLoopNextClosedRows`](four_loop_parent_row_assembly_design.md). The closed
parent matrix, reusable `ExactSparseElimination` engine, and typed
`FourLoopNextElimination` adapter are landed and have completed a production
build and composed replay. The adapter authenticates the frozen parent
boundary, projects `[d,m2]` coefficients structurally into `[d]`, regenerates
the three frozen modular images, and projects the exact indexed result back to
typed columns.

The separate `FourLoopNextCornerCrossAuth` composition is also landed and
replayed. It borrows the independently replayable native corner and next-shell
certificates, proves that all 160 typed scalar-corner rows embed exactly after
structural projection to `Q(d)`, and records how the native corner shell's 64
unresolved coordinates sit in the larger shell's exact pivot/free partition.
It performs no new elimination and does not widen either certificate's
fixed-seed scope.

The accepted `1968 x 1734` certificate proves generic rank 1,588 over `Q(d)`
and leaves 146 ordered `free_unresolved_columns`. All 1,588 unit pivots are
reconstructed from recursive exact source traces, and all 1,968 authenticated
rows reduce to exact zero during construction and replay. This freezes the
fixed-matrix result described below; it does not supply expanded source-weight
vectors, dependent-row roots, a complete factored exceptional-condition
inventory, or an unrestricted four-loop reduction.

The immutable input facts are:

| quantity | exact value |
|---|---:|
| parent rows | 1,968 |
| authenticated columns | 1,734 |
| retained nonzero entries | 22,424 |
| maximum input-row width | 45 |
| zero input rows | 0 |
| parent-row checksum | `fnv1a64:a55ce4ffda6f8f5c` |
| exact rank / pivot rules | 1,588 |
| free unresolved columns | 146 |
| projected-source checksum | `fnv1a64:89008a253f6289fa` |
| exact-engine checksum | `fnv1a64:97c089efcd1b808d` |
| typed-adapter checksum | `fnv1a64:2e723cec8b36c8de` |

The accepted semantic composition freezes:

| composed corner fact | exact value |
|---|---:|
| native corner rows / columns / entries | 160 / 223 / 1,334 |
| native corner rank / free complement | 159 / 64 |
| exactly embedded rows / entries | 160 / 1,334 |
| structural coefficient projections | 2,668 |
| next-shell columns / rank / free complement | 1,734 / 1,588 / 146 |
| inherited disposition, pivoted / retained | 48 / 16 |
| retained scalar corners / products | 10 / 6 |
| pivoted `D1/N0` / `D1/N1` | 22 / 26 |
| cross-authentication checksum | `fnv1a64:a359ccf83fd1eb5c` |

The 48 coordinates are exposed as `pivoted_nonterminals`: their exact rules may
have support, directly or recursively, on other members of the global
146-column free complement. The sixteen retained inherited coordinates and the
complete global free complement remain unresolved fixed-shell coordinates, not
master-basis claims.

The 1,734 columns are the 1,728 authenticated genuine coordinates and six
canonical lower-loop products in the existing `FourLoopCornerColumnId`
order.  All retained coefficients are literally `m2`-free and therefore lie
in `Q(d)`.

The exact rank of this closed matrix is 1,588. This is an exact theorem for the
authenticated matrix: 1,588 distinct unit leading columns prove the lower
bound, while reconstruction of every recursive trace and exact-zero reduction
of all 1,968 source rows prove the upper bound. The ordered complement contains
146 free unresolved columns.

The three denominator-screened finite-field images independently agree at rank
1,588 and nullity 146 and retain the same hardest-first
`column@source-row` skeleton; their report checksum remains
`fnv1a64:2cca473b7966324a`. They selected and regression-test the skeleton but
do not supply the exact rank proof. The historical rank 1,762 remains confined
to the older 2,644-column opaque-boundary probe.

Recursive traces provide exact provenance in the authenticated source rows,
but they are not materialized expanded source-weight lists. The condition
surface remains a conservative unfactored inversion-slot census. Accordingly,
this result completes only the fixed shell and neither proves an unrestricted
four-loop reduction nor identifies a minimal master basis.

## 1. Proof domain and invariants

Let

```text
K = Q(d),
R = Z[d],
A in K^(1968 x 1734).
```

`A` is read directly from the ordered canonical rows in the parent
certificate.  The exact layer must authenticate the parent schema, checksum,
coefficient variable map `[d,m2]`, column catalog, row order, shape, nonzero
count, and maximum width before doing coefficient-heavy work.  It must inspect
every numerator and denominator again and reject any nonzero `m2` exponent.

Internally, the landed engine uses RustRed's Symbolica-backed
`RationalPolynomial<IntegerRing, u16>` coefficient type with the exact
one-variable map `[d]`.  Conversion from the parent `Coefficient` is
structural: copy the `d` exponent and integer coefficient of every numerator
and denominator term after proving the `m2` exponent is zero.  Expression
parsing, string normalization, and numerical sampling are forbidden on the
exact path.

The proof has four invariants after every accepted row operation:

1. every working row is a sparse, sorted map from a catalog column index to a
   nonzero exact coefficient in `K`;
2. every retained unit-pivot row has a recursive derivation trace whose
   expansion is exactly that row as a `K`-linear combination of the 1,968
   parent rows;
3. retained pivots have distinct columns and every active row is zero in all
   previously retained, harder pivot columns; and
4. every reduction and nonzero pivot normalization is exact over `K`, even
   though a stored rational rule may be undefined after specializing `d` to an
   exceptional value.

These invariants prove a generic `Q(d)` result.  They do not infer an identity
from agreement at finite-field images.

## 2. Deferred alternative: lift from `Q(d)` to primitive `Z[d]` rows

The landed engine does not perform this lift: it operates directly and exactly
over `Q(d)`.  The construction in this section is a deferred fraction-free
alternative that may reduce coefficient growth or expose transformation
factors more directly.  It is not required for the correctness of a completed
exact-Gaussian certificate.

For source row `A_i`, write every nonzero entry as `n_ic/q_ic` with canonical
`n_ic,q_ic in R`.  Compute a positive-leading common polynomial denominator

```text
D_i = lcm_c(q_ic)
```

by polynomial GCD and checked exact division.  Form

```text
B_i[c] = n_ic * (D_i / q_ic) in R.
```

Let `C_i` be the canonical common content of the nonzero `B_i[c]`: the
positive integer content followed by the primitive polynomial GCD.  Divide
every entry by `C_i` with a quotient-and-zero-remainder check and choose the
unit sign so the leading coefficient of the hardest entry is positive.  The
stored polynomial source row is

```text
P_i = epsilon_i * B_i / C_i,
lambda_i = epsilon_i * D_i / C_i in K,
P_i = lambda_i * A_i.                                  (2.1)
```

No source row may become zero.  Since `lambda_i` is nonzero in `K`, this lift
does not change the row space over `K`.  The exact `D_i`, `C_i`, sign, and
`lambda_i` are retained rather than recomputed from a formatted coefficient.
They are the first provenance edges and are also inputs to the exceptional-
condition inventory.

Canonical primitive normalization means:

- zero entries are removed;
- coefficient content is positive;
- the common primitive polynomial GCD is removed exactly;
- the hardest nonzero entry has positive leading coefficient; and
- terms use Symbolica's canonical univariate order.

In the deferred fraction-free implementation this normalization would be
applied after every row combination.  A
failed exact division is a correctness error, not a cue to continue with a
rational fallback.

## 3. Modular discovery is advisory only

A finite-field stage is useful for forecasting fill and proposing a pivot
skeleton before expensive exact polynomial arithmetic.  It may evaluate the
authenticated closed rows at several explicit `(prime,d)` images, reject an
image whenever an input denominator is zero, and run deterministic sparse
elimination.  Its report may contain:

- the source and column-catalog checksums;
- accepted and rejected images;
- modular ranks and pivot columns;
- proposed source-row slots at each pivot;
- peak live nonzeros, row widths, fill, cancellations, and update counts; and
- independent matrix, pivot, and report checksums.

The landed adapter regenerates exactly the three frozen images and requires
them to agree on rank, pivot columns, and the complete hardest-first
`(source_row,column)` skeleton.  It then passes that skeleton to the exact
engine.  Every proposed pivot must survive exact reduction at the advertised
column, and the complete proposed set must reduce all source rows to exact
zero.  Any modular disagreement, unusable exact pivot, residual source row, or
rank/skeleton mismatch is a hard error and yields no certificate.  A future
implementation may add deterministic exact fallback selection, but the landed
adapter does not silently diverge from the frozen proposal.

Modular coefficients never enter an exact row, derivation weight, exceptional
factor, or checksum payload as algebraic data.  Agreement across images does
not establish rank, dependence, a free column, or a nonzero minor over
`Q(d)`.  Conversely, disagreement is diagnostic evidence and must not cause
the exact path to drop a row.  The regenerated advisory report is hashed so
the resulting certificate remains deterministic and replayable.

Fresh modular images may validate a finished exact certificate as regression
evidence, but they remain secondary to exact replay.  Screening them against a
published exceptional set is deferred until that complete factor inventory
exists.

## 4. Landed exact Gaussian elimination and deferred fraction-free alternative

### 4.1 Landed exact Gaussian proof

Columns are indexed by `closed.columns()`, easiest to hardest.  The advisory
skeleton lists distinct pivots in strictly descending column order.  For each
proposed `(source_row,pivot_column)`, the generic engine:

1. clones the authenticated exact `Q(d)` source row;
2. eliminates every retained prior unit pivot in ordinal order;
3. requires the hardest surviving column to equal the proposed pivot;
4. divides the complete row by its nonzero pivot coefficient; and
5. stores the resulting unit row together with the base source-row index,
   prior-pivot reduction factors, and normalization divisor.

Construction then independently reconstructs every stored pivot from those
traces and reduces all 1,968 authenticated source rows through all unit pivots.
The builder returns no certificate if any residual coefficient remains.  The
distinct unit leading columns prove independence, while exact zero residuals
for every source row prove spanning.  Thus a successful build proves the
generic rank over `Q(d)`; the modular images only selected the skeleton.

The accepted production build and composed replay instantiate this argument on
the complete matrix: 1,588 distinct unit pivots and exact-zero residuals for
all 1,968 source rows prove rank 1,588 over `Q(d)`. The exact free complement
contains 146 columns.

### 4.2 Stable ordering for a deferred fraction-free implementation

The following ordering policy and sections 4.3--4.4 describe a possible
fraction-free implementation.  They are stronger implementation guidance, not
a description of the landed rational-Gaussian kernel.

The exact layer preserves these total orders:

1. source rows are numbered in the 1,968-entry parent-manifest order;
2. columns are numbered by `closed.columns()`, easiest to hardest under
   `FourLoopCornerColumnId::Ord`;
3. pivot search always selects the hardest active column, the greatest column
   index with nonzero incidence;
4. an authenticated advisory row is tried first; otherwise restricted
   Markowitz selection minimizes
   `(row_width-1)*(column_incidence-1)`, then row width, then immutable source
   row slot; and
5. target rows are updated in increasing immutable row-slot order, entries in
   increasing column order, source weights in increasing source-row order,
   and exceptional factors in their canonical polynomial order.

An active row stays in its original row slot even after replacement.  This
makes Markowitz ties and provenance stable.  Parallel polynomial operations
may be introduced only if their results are merged in the same deterministic
order.

### 4.3 One deferred fraction-free elimination step

Let `P` be the selected primitive pivot row, let `p=P[c]` be its nonzero
coefficient in the hardest active column `c`, and let another active row `R`
have `a=R[c] != 0`.  In `R=Z[d]`, compute

```text
h = gcd(p,a),
u = p/h,
v = -a/h,
S = u*R + v*P.                                        (4.1)
```

All divisions by `h` are checked exact polynomial divisions.  Equation (4.1)
cancels column `c` without introducing a rational-function entry.  If `S` is
nonzero, compute its canonical common content `g`, choose its canonical sign
`epsilon`, and retain

```text
R' = epsilon*S/g.                                     (4.2)
```

Every division in (4.2) is again checked term by term.  Over `K`, replacing
the pair `(P,R)` by `(P,R')` has determinant

```text
epsilon*u/g != 0,
```

so the row span is unchanged.  If `S` is exactly zero, `R` is dependent on
the retained pivot row and is removed with a zero-derivation record.  Exact
zero means an empty polynomial map after collection; no heuristic zero test
is permitted.

In this deferred algorithm the pivot row is removed from the active set and
retained without forward normalization.  All remaining active rows are
cleared at `c`; therefore every later pivot is strictly easier.  Forward
elimination stops only when no
active nonzero row remains.  The number of retained pivots is then the exact
rank over `K`: the distinct leading columns prove independence, while the
invertible transformations and exact zero rows prove spanning.

### 4.4 Common triangular output

For output, either exact algorithm represents a retained row `P_k` with pivot
coefficient `p_k` as a unit rule:

```text
E_k = P_k/p_k
    = e_(pivot_k) + sum_(j < pivot_k) q_kj e_j,
pivot_k = -sum_(j < pivot_k) q_kj column_j.            (4.3)
```

The stored right-hand side is `-q_kj`.  It may contain easier pivot columns;
recursive triangular reduction terminates because every right-hand-side
column is strictly easier than the rule's pivot.  A free-only RREF is not
required for this certificate and should not be paid for unless a later API
needs it.

The free set is the ordered complement of the exact pivot columns.  The API
must call it `free_unresolved_columns`, not masters.

## 5. Landed recursive provenance and deferred expanded weights

The landed generic engine stores one compact recursive trace per unit pivot.
A trace names its authenticated base source-row index, a strictly ordered list
of prior-pivot ordinals and exact reduction factors, and the final nonzero
normalization divisor.  The typed adapter adds the base
`FourLoopNextRawRowId` and typed prior-pivot columns.  Construction and replay
rederive the complete unit row from this trace and compare it exactly with the
stored row.  This is sufficient provenance for the landed fixed-matrix rank
proof and can be expanded recursively into source-row weights when needed.

Explicitly materialized source-weight lists, roots for every dependent source
row, and a separately retained append-only DAG are deferred strengthening.
They are not currently implied by `CompleteFixedSeedShell`.  A stronger future
certificate may retain two equivalent provenance representations.  Its compact
representation could be an append-only DAG whose parent IDs are always smaller
than the child ID:

```text
Source {
    row_index,
    raw_id,
    common_denominator: D_i,
    common_content: C_i,
    sign: epsilon_i,
    source_multiplier: lambda_i,
}

Combine {
    target_parent,
    pivot_parent,
    target_multiplier: u,
    pivot_multiplier: v,
    primitive_divisor: g,
    sign: epsilon,
    canceled_column,
}

ZeroCombine {
    target_parent,
    pivot_parent,
    target_multiplier: u,
    pivot_multiplier: v,
    canceled_column,
}

NormalizePivot {
    parent,
    pivot_column,
    pivot_coefficient: p_k,
}
```

In that deferred representation the source leaf denotes equation (2.1).  If
`w_R,i` and `w_P,i` are the weights of the two parents in source row `A_i`, a
nonzero combine node denotes

```text
w'_i = (epsilon/g) * (u*w_R,i + v*w_P,i).              (5.1)
```

A normalized pivot denotes

```text
w^(k)_i = w_P,i / p_k.                                (5.2)
```

For the stronger deferred contract, the DAG is an optimization and audit
trail; it is not a substitute for explicit source weights.  Every expanded
`FourLoopNextExactPivotRule` would also retain a sorted, zero-pruned list

```text
FourLoopNextSourceWeight {
    source_row_index,
    source_raw_id,
    coefficient,
}
```

relative to the 1,968 canonical parent rows.  That construction would expand
(5.1) and (5.2) topologically, with memoization and independent coefficient
limits, and compare the result with the stored list.  For every pivot rule, replay must
establish the vector identity

```text
e_pivot - rhs = sum_i source_weight_i * A_i.           (5.3)
```

The parent certificate can then replay each `A_i` to the native IBP,
component transport, and lower-loop closures.  Thus the two certificates
compose without flattening all 26,078 path coefficients into every pivot.

Final roots for dependent row slots would be retained as well.  They authenticate
which exact operations made each remaining active row vanish and prevent a
resource failure or missing row from being mislabeled as dependence.

## 6. Exceptional conditions in `d`

Division by a nonzero polynomial is valid in `Q(d)`, so the exact generic-rank
proof needs no numerical assumption about `d`.  Specializing the certificate
at a concrete dimension is different.  The selected row transformations and
rule normalization may cease to be invertible or even defined there.

The landed adapter is deliberately conservative here.  It reports only an
unfactored inversion-slot census covering parent row scales and coefficient
denominators, trace divisors and factor denominators, and triangular-rule
denominators.  Its status is
`ConservativeUnfactoredInversionSlotCensusOnly`, and
`is_complete_exceptional_dimension_inventory()` always returns `false`.
Those counts cannot be used to specialize the generic certificate at a
numerical value of `d`.

A stronger future elimination certificate would retain canonical, distinct,
primitive irreducible factors in `Z[d]`, with every use site and multiplicity.
That deferred inventory would include nonconstant factors from:

1. every input coefficient denominator and observable parent `row_scale`;
2. the numerator and denominator of each source lift `lambda_i`;
3. the determinant `epsilon*u/g` of every nonzero row replacement;
4. every normalized pivot coefficient `p_k`;
5. every final rule or explicit source-weight denominator; and
6. every other exact inversion introduced by provenance expansion or replay.

In that stronger certificate, factor normalization would remove rational
content and choose positive leading coefficient.  Exact factorization must
multiply back to the original
polynomial up to its recorded nonzero rational unit.  Hitting a factor-count,
degree, term, or byte cap fails construction; an unfactored or truncated list
must not be advertised as complete for the elimination layer.

The deferred public record should distinguish the factor from its uses:

```text
FourLoopNextExceptionalFactor {
    factor,
    uses: [
        InputDenominator { row, column },
        ParentRowScale { row, part },
        SourceLift { row, part },
        RowTransform { node, part },
        PivotNormalization { pivot },
        RuleDenominator { pivot, column },
        SourceWeightDenominator { pivot, source_row },
    ],
}
```

The resulting future set would be a sufficient exclusion set for replaying
this chosen closed-matrix representation and elimination at fixed `d`; it is not a claim
that rank drops at every listed root.  Factors can be removable artifacts of
row scaling or pivot choice.  Nor is it yet a complete exceptional-dimension
classification of the underlying native IBPs: upstream closure layers do not
all expose every canceled intermediate inversion as a composable factor-use
inventory.  A listed root is therefore reported as unsupported by this
generic certificate.  Rank or reductions there require a separately built
specialized exact certificate, not substitution into a pole.

## 7. Landed Rust API and status boundary

The reusable engine is `exact_sparse_elimination`; the four-loop adapter is
`four_loop_next_elimination` and borrows the immutable parent certificate.  Its
landed constructor is:

```text
FourLoopNextElimination::build(
    closed: &FourLoopNextClosedRows,
    config: FourLoopNextEliminationConfig,
) -> Result<FourLoopNextElimination, FourLoopNextEliminationError>
```

`FourLoopNextEliminationConfig` contains independent modular-discovery and
exact-engine resource envelopes.  The adapter exposes the authenticated
columns, advisory report, indexed exact-engine certificate, typed pivot rules,
recursive traces, ordered `free_unresolved_columns`, conservative conditions,
statistics, checksum, and composed `replay()`.

The only constructed status is:

```text
FourLoopNextEliminationStatus::CompleteFixedSeedShell
```

The builder reaches this status only after the exact engine has reconstructed
all proposed pivots and reduced every one of the 1,968 source rows to zero over
`Q(d)`.  It means completion of this authenticated finite matrix only.  It
does not mean `CompleteFourLoopReduction`, does not prove that free unresolved
columns are masters, does not include expanded source-weight lists, and does
not certify any numerical specialization of `d`.  Construction failure returns
an error rather than a partial object.

An accepted production object has now been built and composed-replayed for the
complete `1968 x 1734` matrix. Its frozen rank is 1,588 and its ordered free
unresolved complement has size 146. `FourLoopNextClosedRowsStatus` correctly
remains `ExactFixedSeedParentRowsGenericQdEliminationPending`, because that
status describes the parent-row object in isolation rather than the separately
constructed elimination certificate.

The projected-source, exact-engine, and typed-adapter checksums are
`fnv1a64:89008a253f6289fa`, `fnv1a64:97c089efcd1b808d`, and
`fnv1a64:2e723cec8b36c8de`. The adapter checksum covers the parent checksum,
both configurations, modular report, exact certificate, typed rules and
traces, free complement, conservative condition census, and statistics. These
checksums are regression metadata; the algebraic proof remains exact pivot
reconstruction plus all-source-zero replay.

### 7.1 Landed inherited-corner composition

The semantic bridge is constructed from two already proved objects:

```text
FourLoopNextCornerCrossAuth::compose(
    corner: &FourLoopCornerShellCertificate,
    elimination: &FourLoopNextElimination,
) -> Result<FourLoopNextCornerCrossAuth, FourLoopNextCornerCrossAuthError>
```

`compose` authenticates the native corner certificate's 160 rows, 223 used
columns, 1,334 entries, rank 159, and 64-column free complement; maps the first
ten scalar-corner seeds' 160 next-shell rows by typed raw-row identity; checks
equal seed mass weights; and compares the two canonical normalized sparse rows
exactly after structural `[d,m2] -> [d]` projection. The two sides account for
2,668 coefficient projections. It then proves that all 223 native columns occur
in the 1,734-column next catalog and partitions the inherited 64 coordinates
into 48 next-shell pivots and sixteen next-shell free coordinates.

The only constructed status is:

```text
FourLoopNextCornerCrossAuthStatus::CompleteInheritedCornerDispositionFixedSeedShell
```

The retained sixteen are exactly ten scalar reference corners and six canonical
products. The 48 `pivoted_nonterminals` split as 22 `D1/N0` and 26 `D1/N1`.
This is a disposition certificate, not a terminal reduction claim: recursively
applying those 48 rules may reach other coordinates in the larger 146-column
free complement. Neither free set is called a master basis. `replay()` composes
both borrowed public replays, rebuilds every semantic comparison, and
reproduces checksum `fnv1a64:a359ccf83fd1eb5c`.

A stronger future API may add source lifts, a separately retained derivation
DAG, explicit expanded source weights, dependent-row roots, and factored
exceptional conditions.  Such fields and any stronger status must be versioned
separately; they are not silently promised by `CompleteFixedSeedShell`.

## 8. Resource contracts

Static authentication uses the exact landed dimensions rather than the older
opaque-boundary census:

| resource | fixed input or conservative bound |
|---|---:|
| input rows | 1,968 |
| columns | 1,734 |
| input nonzeros | 22,424 |
| maximum input width | 45 |
| dense live-nonzero universe | `1,968*1,734 = 3,412,512` |
| dense coefficient-update opportunities | `1,968*1,967*1,734 = 6,712,411,104` |
| row-combination opportunities | `1,734*1,967 = 3,410,778` |
| deferred explicit pivot/source weights | at most `1,734*1,968 = 3,412,512` |
| deferred full row-square weights | `1,968^2 = 3,873,024` |

These are allocation and operation ceilings, not expected workloads.  Sparse
incidence and the advisory fill profile should keep the actual census much
smaller.  The landed modular configuration independently limits images,
initial and live nonzeros, cumulative fill, and finite-field work.  The landed
exact configuration independently limits source rows, columns, input entries
and serialized bytes, construction and replay reductions and sparse updates,
retained entries, coefficient terms and bytes, coefficient degree, actual
operation terms, the dense monomial universe, integer bit length, coefficient
pair products, and canonicalization work.

A stronger implementation with the deferred features would additionally need
independent limits on:

- separately classified polynomial additions, multiplications, GCDs, exact
  divisions, and factorizations beyond the landed aggregate pair-product and
  canonicalization-work envelopes;
- source-lift LCMs, derivation nodes and edges, expanded source weights, and
  memoized provenance payload;
- exceptional factors, factor uses, degree, terms, and factorization work;
- retained rational coefficients, terms, and bounded serialized bytes; and
- replay operations and reconstructed sparse entries.

The landed engine uses checked counters, preflights coefficient degree and
dense/actual-term bounds before arithmetic, and uses bounded writers for input
and retained serialization.  As with the parent layer, these counters bound
RustRed-owned payload and operation requests; they cannot claim to cap all
transient allocator or Symbolica workspace.

Resource exhaustion is a hard error.  It cannot truncate a coefficient, skip
a source row, accept a modular zero, mark a row dependent, or promote an
unresolved column to the free set.  The landed builder returns no certificate
until all source rows, pivot traces, all-row exact-zero replay, and retained
metadata fit their configured envelopes.  Deferred expanded weights and
exceptional factors would require their own hard acceptance gates.

## 9. Replay and acceptance tests

Landed `FourLoopNextElimination::replay()` re-authenticates and reprojects the
borrowed source, replays the parent certificate, regenerates the three modular
images, replays the indexed exact certificate against the fresh source matrix,
reprojects the typed rules, and reproduces the conditions, statistics, and
checksum. The accepted production path has composed this complete proof.

The accepted exact-Gaussian production replay establishes all of the
following:

1. the input is exactly `1968 x 1734`, has 22,424 nonzeros, maximum width 45,
   and checksum `fnv1a64:a55ce4ffda6f8f5c`;
2. every input entry converts losslessly to `Q(d)` and has literal zero `m2`
   degree;
3. the reports use exactly the three frozen finite-field images, agree at
   modular rank 1,588 and nullity 146, and retain an identical complete
   source-row/pivot-column skeleton; these values remain advisory for skeleton
   selection, while item 4 and the all-source-zero proof independently
   establish the exact rank;
4. every proposed exact pivot has the advertised hardest surviving column
   after prior exact reductions, and exact rank and skeleton equal the
   proposal before projection;
5. pivot columns are unique and strictly descend, pivot entries normalize to
   one, and every right-hand-side column is strictly easier;
6. every recursive trace rederives its stored unit row from the authenticated
   base source-row index and strictly prior pivots, and each typed trace carries
   the corresponding `FourLoopNextRawRowId`;
7. all 1,968 parent rows reduce to exact zero under the triangular rules;
8. the pivot columns and free-column complement partition the authenticated
   column catalog, while exact statistics account for all 1,968 source rows;
9. free columns remain named `free_unresolved_columns`, and the condition
   census explicitly remains incomplete; and
10. all dynamic statistics and the deterministic checksum reproduce.

The independently accepted corner-composition replay additionally authenticates
all 160 embedded raw-row identities, 1,334 equal projected entries, 2,668
coefficient projections, the 223-column native support, both exact pivot/free
partitions, the 48/16 inherited disposition with its 22/26 and 10/6
refinements, and checksum `fnv1a64:a359ccf83fd1eb5c`. This integration check is
downstream of, and not a premise for, the exact rank-1,588 proof.

Acceptance of deferred strengthening would separately require source lifts
and primitive normalization, fraction-free combination identities, expanded
source weights and dependent roots, a complete rescanned exceptional-factor
inventory, and optional fresh modular validations.  None of those deferred
items is a prerequisite for the mathematical exactness of the landed
rational-Gaussian fixed-matrix proof.

Current negative tests cover cheap frozen-shape preflights plus small exact
matrices with malformed coefficients, normalization, cancellations, dependent
rows, incomplete or incorrect skeletons, source tampering, certificate
tampering, and replay.  Future hardening should add efficient candidate-replay
hooks so every measured production cap can be tested one below its accepted
request—including pivot/trace/RHS retention and replay high-water marks—without
rebuilding the complete matrix once per cap.  It should likewise add focused
adapter tampering for parent context/order, modular disagreement, typed RHS,
trace, and free-complement metadata. Deferred provenance and factorization
features will need their own negative tests when implemented.

The complete production build and composed replay have passed and freeze exact
rank 1,588, 146 free unresolved columns, the measured statistics below, and the
three certificate checksums. The modular report remains secondary regression
evidence even though its proposed rank agrees with the exact result.

| measured resource | accepted value |
|---|---:|
| pivot reductions | 3,646 |
| verification reductions | 23,993 |
| arithmetic updates | 46,580 |
| retained pivot-row entries | 22,283 |
| retained coefficient terms | 50,956 |
| retained coefficient bytes | 95,067 |
| maximum row width | 173 |
| maximum coefficient degree | 5 |
| replay reductions / updates | 27,639 / 232,446 |
| projected RHS entries | 15,461 |
| recursive trace reductions, total / maximum | 3,646 / 169 |
| projected coefficient terms / bytes | 47,780 / 94,202 |
| conservative inversion slots | 45,087 |

The conservative slot total consists of 1,968 parent-row-scale slots, 22,424
parent-coefficient-denominator slots, 1,588 trace-divisor slots, 3,646
trace-factor-denominator slots, and 15,461 rule-RHS-denominator slots. These
counts include trivial slots and repeated factors and do not constitute a
factored exceptional-dimension inventory.

## 10. Incremental implementation phases

1. **Freeze the boundary — landed.** The adapter authenticates the exact
   `1968 x 1734 / 22,424 / width-45` source, parent checksum, ordered typed
   column catalog, unique raw-row provenance, and literal `m2` absence.  No
   exact rank constant is inferred from this step.
2. **Build the generic exact engine — landed and production-validated.** The reusable
   `ExactSparseElimination` kernel performs bounded rational-Gaussian
   elimination over `Q(d)`, retains recursive traces, independently
   reconstructs pivots, and proves spanning by reducing every source row to
   exact zero.  Small exact fixtures exercise good and bad skeletons and source
   tampering.
3. **Add advisory plumbing — landed.** The four-loop adapter regenerates the
   three frozen images, requires their complete skeleton agreement, and treats
   rank 1,588/nullity 146 only as an exact-proof proposal.  Disagreement or an
   unusable exact proposal returns an error; there is no silent modular-to-exact
   fallback.
4. **Project typed rules and provenance — landed and production-validated.** Indexed unit
   rows become typed `pivot = rhs` rules with the correct sign, recursive
   source-row/raw-ID traces, an ordered unresolved free complement,
   conservative condition counts, statistics, checksum, and composed replay.
5. **Run the frozen matrix — landed.** Exact elimination of the authenticated
   `1968 x 1734 / 22,424` input proves rank 1,588 and retains 146 free
   unresolved columns.
6. **Replay and freeze the landed certificate — landed.** Parent, modular,
   exact all-row-zero, and typed-adapter replay compose successfully and
   reproduce all checksums and measured statistics.
7. **Cross-authenticate the inherited corner block — landed and
   production-validated.** `FourLoopNextCornerCrossAuth` maps all 160
   scalar-corner source rows by typed identity and proves exact equality of
   their canonical normalized `Q(d)` rows, reproducing the native
   223-column/rank-159/64-free result inside the larger catalog. It partitions
   the inherited 64 coordinates into 48 pivoted nonterminals and sixteen
   retained inherited coordinates, while keeping them distinct from the global
   146-column free complement. The 48 rules may still depend on other global
   free coordinates, and no free coordinate is promoted to an unrestricted
   master.
8. **Expand source weights and dependent roots — deferred.** Materialize the
   recursive traces as explicit source-row weights and optionally retain a
   separate provenance DAG and zero roots under independent caps.
9. **Evaluate a fraction-free alternative — deferred.** Implement the
   `Z[d]` lifts, primitive normalization, fixed-slot restricted-Markowitz
   kernel, and equations (4.1)--(4.2) only if they provide a measured advantage
   or are needed for stronger transformation metadata.  Compare their exact
   row space against the landed rational-Gaussian certificate.
10. **Compose complete exceptional conditions — deferred.** Record and factor
   every relevant inversion under independent caps, rescan uses, and publish a
   separately versioned completeness claim that retains the upstream caveat.
11. **Integrate reduction without widening the claim.** Expose reduction only
   for the authenticated column catalog, retain free coordinates as
   unresolved, and use separately versioned larger seed shells or exceptional-
   dimension certificates for any broader statement.

This sequence keeps finite-field speedups, exact `Q(d)` proof, and native
source provenance separate.  For the landed baseline, the exact fixed-matrix
stage is accepted only when the rational-Gaussian certificate and composed
replay succeed on the entire source.  Fraction-free arithmetic, expanded
weights, and complete factored conditions are separately versioned
strengthening, not retroactive prerequisites for that exact rank proof.
