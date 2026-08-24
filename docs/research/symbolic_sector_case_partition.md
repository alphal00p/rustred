# Symbolic integer sector-case partition: first production slice

Date: 2026-08-13

RustRed now has a standalone, loop-count-independent substrate for the case
partition used by LiteRed's `SolvejSector`/`WhenBad` workflow.  The production
implementation is
[`src/symbolic_sector_cases.rs`](../../src/symbolic_sector_cases.rs), with
public black-box tests in
[`tests/symbolic_sector_cases.rs`](../../tests/symbolic_sector_cases.rs).

## LiteRed source contract

The relevant Mathematica behavior is:

- the sector corner becomes the integer orthant `n_i >= 1` for active lines
  and `n_i <= 0` for inactive lines
  ([`LiteRed2026.m:2384-2386`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2384));
- `SolvejSector` maintains a list of uncovered cases
  ([`LiteRed2026.m:2430-2465`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2430));
- a successful rule is accepted under the current case and the complement of
  its `WhenBad` condition
  ([`LiteRed2026.m:2484-2505`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2484));
- covered/bad conditions are folded back into the remaining cases
  ([`LiteRed2026.m:2519-2523`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2519)); and
- `WhenBad` forms exceptional coefficient and sector-leak conditions, then
  simplifies them over the integer sector domain
  ([`LiteRed2026.m:2565-2569`](../../vendor/LiteRed2/Source/LiteRed2026.m#L2565)).

This document records the first, module-local partition slice.  Subsequent
production layers now derive algebraic `WhenBad` conditions
([`when_bad.rs`](../../src/when_bad.rs)), authenticate them against freshly
generated identities ([`generated_when_bad.rs`](../../src/generated_when_bad.rs)),
and attach supplied candidates to a finite replayed cover
([`parametric_sector_coverage.rs`](../../src/parametric_sector_coverage.rs)).
Automatic adaptive discovery, coordinate-locus extraction, generated partial
re-elimination, and the replayable live exceptional-leaf queue are implemented
in the later generated-sector modules.  Recursive closure of those exceptional
leaves to LiteRed's full `SolvejSector` fixed point remains open.

## Implemented semantics

`SymbolicSectorOrthant` expands a `SectorMask` into one typed constraint per
unshifted index:

```text
active line i:   n_i >= 1, n_i in Z
inactive line i: n_i <= 0, n_i in Z
```

The arity is bound to the caller's existing
`ParametricCoefficientContext`, so predicates live on exactly the same
Symbolica `K(n)` variable map as generated IBPs.  A leaf is the orthant
intersected with a conjunction of authenticated `ParametricPolynomial`
predicates of the forms `p = 0` and `p != 0`.

For a live case `C`, `split_on_bad_polynomial` deterministically creates the
two neutral complementary branches:

```text
equal-zero child: C and p = 0
nonzero child:    C and p != 0
```

The equality child is always allocated first.  Case IDs, split ordinals, branch
order, predicate order, and final leaf order are deterministic.  The helper
`split_on_pivot_coefficient` extracts the normalized pivot numerator.  Pivot
denominators remain separate domain conditions and must be discharged or split
explicitly by a later `WhenBad` compiler.

The splitter deliberately assigns no semantic meaning such as “safe” or
“bad” to either branch.  A pivot numerator is unsafe on its equality branch,
whereas an RHS leak coefficient is safe there.  The public accessors are
therefore `equal_zero_case` and `nonzero_case`; the old `bad_case`/`good_case`
spellings are deprecated compatibility aliases.

Identically zero and nonzero coefficient-field-constant split polynomials are
rejected.  In particular, a nonzero polynomial involving only base variables
is an element of `K=Q(theta)` and is constant with respect to `K[n]`; RustRed
does not manufacture a generic-kinematics branch such as `theta=0`.  Exact
repetition of a polynomial on one lineage is also rejected.  Polynomial
authentication and sparse-term counts use the existing checked Symbolica API;
no string parsing or Mathematica/FORM runtime is involved.

## Coverage proof and replay

`SymbolicSectorCasePartitionCertificate` retains:

- the schema and exact `K(n)` context fingerprint;
- the sector and its expanded orthant constraints;
- every parent, split polynomial, branch ordinal, and deterministic child ID;
- the exact final leaf conjunctions; and
- split, leaf, depth, predicate-occurrence, and retained-polynomial term/byte
  statistics.

Replay takes both the expected context and expected sector.  It starts from the
single orthant case, repeats the complete split transcript through the public
checked builder, and compares the reconstructed certificate exactly.

Coverage and pairwise disjointness follow by induction on this transcript.  A
split preserves the parent union because every value satisfies exactly one of
`p = 0` or `p != 0`.  Any two final leaves first diverge at one retained split,
where they contain complementary predicates.  Thus the final leaves are a
finite disjoint cover **of the recorded orthant**, regardless of whether some
individual leaves are empty.

## Resource and adversarial coverage

All mutations are preflighted before the live-case map changes.  Independent
limits cover index arity, context-fingerprint bytes, exact polynomial
authentication, split count, live leaves, predicates per leaf, total
leaf-predicate occurrences, and all sparse polynomial terms and
canonical-display bytes retained by leaves plus the replay transcript.

Tests cover:

- a two-index active/inactive orthant split by two polynomials into four leaves;
- exact replay and deterministic reconstruction;
- sampled uniqueness of the matching leaf over the integer orthant;
- context, sector, arity, dead-case, integer/base-field-constant polynomial,
  and repeated-predicate rejection;
- pivot-numerator extraction;
- transactional resource failures; and
- internal adversarial mutations of the schema, child IDs, branch predicate,
  orthant, and retained statistics.

## Precise limitations of this substrate

This module alone is not full LiteRed symbolic reduction coverage.  Items 1,
2, and the syntactic-only part of item 4 below are boundaries of the low-level
`symbolic_sector_cases.rs` API; higher generated `WhenBad`, coverage, and
provider layers now implement those responsibilities:

1. This splitter does not compile `WhenBad`; the higher compiler combines the
   pivot numerator, coefficient denominators, inherited guards, and
   coefficient-aware same-sector leakage.
2. This splitter does not attach rules; the higher sector-certificate provider
   applies generated descending rules, with a separate conditional provider
   for supported exceptional loci.
3. There is no integer-locus simplifier.  The code does not run `Reduce`, a
   Gröbner basis, Presburger arithmetic, or nonlinear Diophantine solving; it
   neither proves a leaf inhabited nor removes contradictory leaves.
4. The low-level splitter itself treats loci structurally.  The higher V3
   coverage composer now proves exact associates over `K`, compresses a finite
   disjunction `p_1=0 || ... || p_k=0` to the exactly equivalent product locus
   `p_1...p_k=0`, and uses only the two sound directional consequences of exact
   divisibility in the integral domain `K[n]`.  It still deliberately does not
   infer radical-ideal equivalence or either invalid converse implication.
5. Base parameters may occur as coefficients of an index-dependent
   authenticated `K(n)` polynomial.  They remain generic elements of `K`; this
   slice does not partition exceptional kinematic parameter loci.  A nonzero
   base-only polynomial is therefore rejected as a trivial split.
6. Initial bounded search, exceptional coordinate-case re-elimination, and
   condition-bound rule selection are implemented.  Iterated depth growth on
   every residual locus and a final certificate that every inhabited sector
   case has a rule or an explicitly justified terminal remain future work.

The repository-wide next integration step is now to iterate the existing
automatic work queue to a fixed point, feed solved subsectors into later
supersectors, and attach only replayable zero/symmetry or caller-authorized
master terminals.  General polynomial loci remain typed unsupported.
`Uncovered` stays explicit for any leaf not discharged by that process.
