# Automatic ISP completion audit

Date: 2026-08-13

RustRed's production implementation is
[`src/automatic_isps.rs`](../../src/automatic_isps.rs); black-box tests are in
[`tests/automatic_isps.rs`](../../tests/automatic_isps.rs).

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

RustRed implements this algorithm over the authenticated Symbolica base field:

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
the retained input prefix, checks every appended unit row/zero shift, and now
compares all rank-test and operation statistics exactly.

The audit added allocation hardening.  The family scalar-product limit is
checked before the scan; the supplied rank matrix and every candidate matrix
are bounded before row-vector allocation/cloning; aggregate numerator and
denominator sparse terms plus canonical-display bytes are bounded before every
rank-matrix clone; and the rank routine repeats the matrix, coefficient-payload,
and operation checks before exact Symbolica work.  Tests cover dependent inputs,
matrix and coefficient-payload limits before cloning, operation limits, and
internal tampering of both retained work counters.

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
