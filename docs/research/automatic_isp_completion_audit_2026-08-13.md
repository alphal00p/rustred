# Automatic ISP completion audit

Date: 2026-08-13; updated 2026-08-24

RustRed's production implementation is
[`src/automatic_isps.rs`](../../src/automatic_isps.rs); black-box tests are in
[`tests/automatic_isps.rs`](../../tests/automatic_isps.rs) and the independent
maximal-minor oracle in
[`tests/automatic_isps_symbolica_rank_oracle.rs`](../../tests/automatic_isps_symbolica_rank_oracle.rs).

LiteRed first builds the denominator coefficient matrix in its scalar-product
basis, rejects dependent input rows, and computes the number of missing basis
directions
([`LiteRed2026.m:780-789`](../../vendor/LiteRed2/Source/LiteRed2026.m#L780)).
Its private `append` helper scans identity rows, retaining a row exactly when
it increases the current matrix rank
([`LiteRed2026.m:316`](../../vendor/LiteRed2/Source/LiteRed2026.m#L316)); the
accepted scalar products are appended as denominators and their power shifts
are padded with zero
([`LiteRed2026.m:790-797`](../../vendor/LiteRed2/Source/LiteRed2026.m#L790)).

RustRed implements this algorithm over the authenticated Symbolica base field.
Every generic-rank test crosses the checked coefficient-matrix boundary and
calls public `Matrix::partial_row_reduce` over the full rectangular matrix;
RustRed contains no pivot search, row normalization, or elimination arithmetic.
The surrounding LiteRed semantics are:

1. authenticate the supplied affine rows and require generic row rank equal to
   the supplied denominator count;
2. scan scalar-product identity rows from left to right;
3. keep exactly a rank-increasing row;
4. append it with zero affine constant and zero power shift; and
5. construct and replay the resulting complete `IntegralFamily`.

The coordinate-order qualifier is important.  LiteRed constructs `sps` using
Mathematica `Union`, whose canonical expression order is not RustRed's public
coordinate order.  RustRed intentionally retains its documented order:
upper-triangular loop--loop products, followed by loop--external products in
loop-major order.  Therefore the algorithm and completed span agree, but exact
ISP ordinals or the particular complementary ISP basis need not match a
LiteRed session.  Tests and documentation do not claim literal ordinal parity.

The retained transcript contains the accepted RustRed coordinate ordinals,
rank after every accepted row, and the full deterministic rank-test work
census.  Replay verifies the complete family, reruns the identity scan from
the retained input prefix, checks every appended unit row/zero shift, and
compares all rank-test and operation statistics exactly.  New completions use
`rustred-automatic-isp-completion-v2`: its `rank_operations` field counts the
checked exact arithmetic calls made by Symbolica's native rank schedule;
constant construction and predicates are separately censused and excluded.
The V1 schema
identifier remains exported for legacy identification, but its former
RustRed-elimination work census is not silently reinterpreted.
A Symbolica upgrade that changes the native rank arithmetic schedule therefore
requires a new completion schema rather than accepting old V2 work transcripts.

The audit added allocation hardening.  The family scalar-product limit is
checked before the scan; the supplied rank matrix and every candidate matrix
are bounded before row-vector allocation/cloning; aggregate numerator and
denominator sparse terms plus canonical-display bytes are bounded before every
rank-matrix copy; separate clone-owned input and mutated-echelon output byte
limits surround the native call; and every checked scalar operation consumes
the cumulative construction/replay budget before it executes.  Tests cover
dependent inputs, matrix and coefficient-payload limits before copying, native
input/output byte limits, cumulative operation limits, and internal tampering
of both retained work counters.

Independent validation defines generic rank as the largest nonzero square
minor and delegates every determinant to public Symbolica `Matrix::det`; it
does not copy the production elimination.  Parallel licensed runs passed
23/23 adapter tests, 30/30 combined internal tests, 8/8 public automatic-ISP
tests, and 3/3 maximal-minor oracle tests.  The final optimized internal gate
passed 30/30 in nextest run `88208064-7cd3-46b4-b4f5-807953c2232f`.  The
optimized downstream gate passed 13/13 in
`0a0f4f11-09b0-4d0e-a9b8-f9adad877989`, including the public completion and
minor-oracle binaries plus complete four- and five-loop factorized reductions.
The concrete oracle fixtures cover
one loop with two external momenta, a two-loop vacuum row-swap case, and a
dense symbolic two-loop/two-external rectangular family.  These are validation
fixtures only; production has no topology or loop-count dispatch.

Precise remaining scope:

- dependent or overcomplete denominator sets are rejected rather than sent to
  LiteRed-style `NewDsSet` relations/partial fractioning;
- automatic discovery of external momenta from symbolic denominator syntax is
  outside this typed affine API;
- no claim is made that a chosen ISP has the same printed form/order as
  Mathematica; and
- exceptional base-parameter loci where the generic rank drops are represented
  by the completed family's existing nonzero determinant/domain guards rather
  than separate specialized bases.
