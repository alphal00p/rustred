# Five-loop scalar shell after the `D=2` candidate-terminal certificate

Date: 2026-08-12

## Outcome

The finite-field orbit search identifies a smallest orbit-complete seed
addition that closes every six-line scalar target of total dot degree `D<=3`:
it is **numerator-free**.  Starting from the already certified corner and
one-dot layers, add both scalar `D=2` orbits

```text
A2 = B(3,1,1,1,1,1),
B2 = B(2,2,1,1,1,1).
```

The modular native-IBP matrices reduce all three `D=3` scalar orbits

```text
A3 = B(4,1,1,1,1,1),
B3 = B(3,2,1,1,1,1),
C3 = B(2,2,2,1,1,1)
```

to the declared finite-shell terminals `{M,B2}`.  Each of the two new seed
orbits is necessary in this orbit-complete scalar construction: an `A2` seed
alone pivots only `A3`, while a `B2` seed alone pivots only `B3`; `C3` remains
free in either one-orbit shell.  The complete pair pivots all three.

This result does **not** prove that `B2` is an unrestricted master.  The first
numerator seed shell capable by degree of feeding back into scalar `D=2` was
also tested.  It leaves `B2` free at three independent finite-field
specializations.  Larger checked numerator shells do the same.  These are
finite-shell discovery results, not an exact non-reducibility theorem.

All calculations behind this note used native Rust algebra or static
identities.  No FORM or Mathematica program was run.

## 1. Family, grading, and joint orbits

Use the oriented physical lines

```text
l0=k0, ..., l4=k4,
l5=-(k0+k1+k2+k3+k4),
sum_i li=0,
Di=li^2+m2.
```

For a column with physical powers `a` and a numerator multigraph `G`, define

```text
D(a) = sum_i max(ai-1,0),
N(G) = number of multigraph edges.
```

An edge `[u,v]` denotes the scalar product `lu.lv`.  Loops `[u,u]` are allowed
inside the algebraic halo.  Physical `S6` acts jointly on the six powers and
the multigraph vertices.  Canonicalizing the powers and numerator graph
separately would identify inequivalent incidence classes and is forbidden.

The exact number of scalar `S6` orbits at dot degree `D` is the number of
partitions of `D` into at most six parts.  Hence the exact scalar census through
degree three is

| exact degree | canonical orbits | representatives | labelled images |
|---:|---:|---|---:|
| 0 | 1 | `M=[1,1,1,1,1,1]` | 1 |
| 1 | 1 | `A=[2,1,1,1,1,1]` | 6 |
| 2 | 2 | `A2`, `B2` | 6, 15 |
| 3 | 3 | `A3`, `B3`, `C3` | 6, 30, 20 |

Thus the closing seed set has four canonical orbits and 28 labelled scalar
assignments through `D=2`.  The incremental work beyond the existing
corner/one-dot certificate is exactly two canonical seeds, 21 labelled
assignments, and `2*25=50` canonical raw IBP origins that a production shell
must authenticate.  Building the complete four-orbit shell uses
`4*25=100` canonical raw origins.

For numerator seeds the exact count is obtained by exhaustive `S6` orbit
enumeration, equivalently Burnside's lemma on the joint pair `(a,G)`:

```text
U(Dmax,Nmax) = (1/720) sum_(sigma in S6)
               # { (a,G) fixed by sigma : D(a)<=Dmax, 1<=N(G)<=Nmax }.
```

The standalone oracle enumerates all 720 permutations and asserts uniqueness
through a `BTreeSet`.  The relevant numerator layers have three canonical
seeds in total:

| `(D,N)` | incidence class | labelled orbit size |
|---:|---|---:|
| `(0,1)` | one off-diagonal edge | 15 |
| `(1,1)` | dot incident to the edge | 30 |
| `(1,1)` | dot disjoint from the edge | 60 |

The last two representatives account for all 90 labelled assignments in the
exact `N=1,D=1` layer.  They are the smallest orbit-complete numerator layer
capable of reaching scalar `D=2`; together with the four scalar seeds, the
minimal audit has six canonical seeds and `6*25=150` raw origins.  Adding the
first row gives a useful nested `N=1,D<=1` diagnostic: three numerator orbits,
105 labelled assignments, seven total canonical seeds, and 175 raw origins.
It is not needed for the minimal `B2` probe because that lower seed is already
inside the exact `(D,N)<=(1,1)` boundary domain.

## 2. Exact one-row halo and why `D=1,N=1` is the first useful probe

For a seed `(a,G)`, a native row `partial_(k_i).k_j` contains:

- its divergence term at `(D,N)`;
- numerator-derivative terms at `(D,N)`; and
- denominator-derivative terms at `(D+1,N+1)`.

The diagonal relation

```text
li.li = Di-m2
```

removes one graph edge and either lowers the corresponding physical power by
one or leaves the physical powers unchanged with an `m2` coefficient.
Momentum conservation adds

```text
sum_j li.lj = 0.
```

Consequently a numerator seed with `D=0,N=1` can descend only to scalar
degree at most one and cannot affect the `B2` column.  A `D=1,N=1` seed emits
`D=2,N=2`; taking two mass branches of diagonal lowering can reach scalar
`D=2`.  Therefore the two `D=1,N=1` incidence orbits are the smallest
orbit-complete numerator seed layer that can possibly change whether `B2` is
a pivot.  The optional `D=0,N=1` orbit makes a larger nested `N=1,D<=1` shell,
but cannot affect `B2` by this grading argument.

This is a finite row-space probe, not a closure theorem for arbitrary
numerator degree.  Its exact emitted support lies within

```text
scalar seeds:             D<=2, N=0
numerator seeds:          D=1, N=1
raw numerator halo:       D<=2, N<=2
algebraic descendants:    N<=1 and proper five-line columns.
```

Proper five-line columns are retained as free factorized-boundary columns by
the discovery oracle; columns with at most four active lines are dropped only
after the rank-deficiency/scaleless rule.  A production certificate must
instead call the exact five-line boundary reducer and record that provenance.

## 3. Exact shell census and modular rank witnesses

The pure-`std` oracle is
[`five_loop_scalar_shell_rank.rs`](../../tools/five_loop_scalar_shell_rank.rs).
Its arguments are

```text
scalar_D numerator_D numerator_N prime d m2 d2_subset numerator_min_D
```

where `d2_subset` is `none`, `a2`, `b2`, or `all`.  It primality-checks the
modulus, canonicalizes joint `S6` orbits, constructs all rows, and prints a
directly replayed nonzero rank minor.  The row count called `raw_origins`
includes zero rows before symmetry collection; `raw_rows` is the exact number
of nonzero collected rows.  Setting `numerator_min_D=numerator_D` selects an
exact numerator-dot layer instead of all lower layers.

At `(p,d,m2)=(1000003,17,19)`, the minimal scalar closure is

```text
canonical seeds       4
raw origins           100
nonzero raw rows       36
algebraic rows          26
total rows              62
columns                 42
rank                    34
nullity                  8
```

The exact orbit-column census `(active lines,D,N) -> count` is

```text
raw:
  (6,0,0):1 (6,1,0):1 (6,1,1):2
  (6,2,0):2 (6,2,1):5 (6,3,1):10

after diagonal/momentum closure:
  (5,1,0):1 (5,2,0):2 (5,3,0):3
  (6,0,0):1 (6,1,0):1 (6,1,1):4
  (6,2,0):2 (6,2,1):9
  (6,3,0):3 (6,3,1):16.
```

The oracle records a nonzero order-34 minor.  In deterministic sorted-row
order its selected row indices are

```text
[0,1,2,6,7,8,9,10,11,16,17,23,24,26,28,29,30,31,
 35,36,37,38,42,43,44,45,46,47,52,53,54,55,56,61]
```

and its determinant is `288654 mod 1000003`.  This proves rank at least 34 at
that specialization.  The reported rank 34 follows from the explicit
elimination at that specialization.  It does not prove generic rank 34 over
`Q(d,m2)`.

The smallest orbit-complete numerator-capable audit, selected with
`numerator_D=numerator_min_D=1`, has the following exact census at the same
specialization:

```text
canonical seeds          6 = 4 scalar + 2 numerator
raw origins            150
nonzero raw rows         82
algebraic rows           90
total rows              172
columns                 102
rank                     92
nullity                  10
```

Its closed orbit-column census is

```text
(5,1,0):1 (5,2,0):4 (5,2,1):15 (5,3,0):3
(6,0,0):1 (6,1,0):1 (6,1,1):4
(6,1,2):17 (6,2,0):2 (6,2,1):9 (6,2,2):43
(6,3,0):3 (6,3,1):16.
```

The direct order-92 modular minor is nonzero (`391759 mod 1000003`).
The tool prints its 92 sorted row indices; the matching pivot-column order is
constructed deterministically in the source, and the determinant is replayed
directly from that matrix slice rather than trusted from elimination state.

Both ranks, the target pivot bitmap, and the free top scalar list were
identical at the independently primality-checked specializations

```text
(p,d,m2) = (1000003,17,19),
           (1000033,23,29),
           (1000037,31,37).
```

For the scalar shell their nonzero order-34 minor determinants are respectively
`288654`, `8872`, and `-434967`.  For the minimal numerator audit the order-92
minor determinants are `391759`, `346778`, and `256714`, in their corresponding
fields.

## 4. Minimality experiment and `B2` verdict

At all three specializations the scalar target pivot bitmap is:

| added exact-`D=2` scalar seed orbits | `A3` | `B3` | `C3` |
|---|---|---|---|
| neither | free | free | free |
| `A2` only | pivot | free | free |
| `B2` only | free | pivot | free |
| both | pivot | pivot | pivot |

Adding either the exact `N=1,D=1` numerator layer or the nested `N=1,D<=1`
shell to any row of this table does not change the bitmap.  Thus both scalar
`D=2` seeds are necessary and sufficient for this smallest orbit-complete
`D=3` scalar closure.  Numerator seeds neither replace a missing scalar seed
orbit nor remove `B2`.

The following larger finite numerator shells were also checked at
`(1000003,17,19)`:

| numerator seeds | numerator seed orbits | columns | rank | free top scalar `B2`? |
|---|---:|---:|---:|---|
| `N=1,D=1` | 2 | 102 | 92 | yes |
| `N<=1,D<=0` | 1 | 70 | 60 | yes |
| `N<=1,D<=1` | 3 | 130 | 118 | yes |
| `N<=1,D<=2` | 8 | 243 | 229 | yes |
| `N<=1,D<=3` | 17 | 493 | 471 | yes |
| `N<=2,D<=0` | 4 | 197 | 184 | yes |
| `N<=2,D<=1` | 13 | 551 | 533 | yes |
| `N<=3,D<=0` | 12 | 746 | 729 | yes |
| `N<=3,D<=1` | 43 | 2459 | 2432 | yes |

At `N<=1,D<=3` an additional scalar `D=4` column is free, as expected when a
deeper halo is opened without also adding its complete scalar seed layer.
That extra column is not evidence about the closed `D<=3` target box.

The honest conclusion is therefore:

- `B2` is **not reducible in any finite shell listed above**;
- the smallest numerator shell capable of touching scalar `D=2` already
  leaves it free; but
- no finite sequence of failed pivot searches proves that a still deeper seed
  shell cannot reduce it.

An exact proof that `B2` is an unrestricted master would require a lower-bound
certificate in a quotient that covers all later IBPs, for example a proved
critical-point/master-count argument or a parametric module functional that
annihilates every native IBP while evaluating nontrivially on `B2`.  RustRed
does not yet have such a theorem.  Its API must continue to call `B2` a
`candidate_terminal`.

## 5. Reconstructed `D=3` rules and exact-proof status

Finite-field interpolation followed by checks at the three specializations
above gives the following candidate rules (write `mu=m2` and
`M=B(1,1,1,1,1,1)`):

```text
A3 =  5*(11*d-50)/(72*mu) * B2
    + (-125*d^3+1225*d^2-3830*d+3864)/(864*mu^3) * M,

B3 =  (19*d-46)/(24*mu) * B2
    + (-50*d^3+385*d^2-986*d+840)/(288*mu^3) * M,

C3 =  (47-17*d)/(12*mu) * B2
    + (50*d^3-385*d^2+986*d-840)/(288*mu^3) * M.
```

The coefficients obey the independent single-scale derivative check

```text
B3+C3 = (16-5*d)/(8*mu) B2,
```

including exact cancellation of their `M` coefficients.  They also satisfy
the derivative of the already exact `A2` rule:

```text
3*A3 + 15*B3 + 10*C3
  = (25*d^2-130*d+168)*(16-5*d)/(96*mu^3) M.
```

These identities were strong reconstruction checks during discovery. They are
now production theorems of
[`five_loop_d3.rs`](../../src/five_loop_d3.rs): the module regenerates and
authenticates 100 native identities, closes the exact oriented-line algebraic
and five-line boundary halo, eliminates over `Q(d,m2)`, and replays every
pivot from source-row weights. In contrast, the existing exact `D=2`
relations

```text
A2 = -5/2 B2 + (25*d^2-130*d+168)/(48*mu^2) M,
R  = 5*mu B2 + (-10*d^2+49*d-60)/(12*mu) M
```

and the nonzero minor `-2*mu/5` already have exact provenance in
`five_loop_d2.rs`.

## 6. Implemented exact certificate

The production milestone is an exact shell, not hard-coded rules accepted
because modular tests pass. Its public type is `FiveLoopBananaD3Shell`, with
the following surface:

```text
FiveLoopBananaD3Shell::build(config) -> Result<Shell, Error>

Shell::seeds()              // M, A, A2, B2 with stable orbit IDs
Shell::normalized_rows()    // every authenticated native row
Shell::pivots()             // triangular exact rules + source weights
Shell::free_columns()       // includes typed M and B2 terminals
Shell::nonzero_conditions() // every exact divisor
Shell::reduce_target(I)     // public scalar D<=3 target reduction
Shell::replay()             // regenerate rows and replay proof
```

The certificate performs this sequence:

1. Authenticate the equal-mass six-line family and `m2!=0` domain exactly as
   `FiveLoopBananaD2Reducer` does.
2. Enumerate the four stable scalar orbit IDs `M,A,A2,B2`, assert the exact
   seed census `1+1+2` and labelled orbit sizes `1,6,6,15`, and generate all
   100 raw IDs `(seed_orbit,i,j)` with `0<=i,j<5` using
   `IbpGenerator::try_generate_raw_identity`.
3. Jointly canonicalize every physical-power/multigraph column under `S6`.
   Never treat the nine auxiliary basis positions as a permutation
   representation of `S6`; transform scalar products through the oriented
   `li` lines.
4. Add authenticated algebraic row IDs for every required
   `li^2=Di-m2` and `sum_j li.lj=0` relation in the exact emitted halo.  Reduce
   all five-line terms with `FiveLoopBananaBoundaryReducer`, retaining the
   boundary witness in the row provenance.
5. Eliminate over Symbolica-backed `Q(d,m2)` with a deterministic hardness and
   stable-column order.  Record each pivot as a sparse source-row combination,
   plus every factor divided from a raw/algebraic row or pivot.  The generic
   coefficient domain must explicitly include `m2!=0` and all reconstructed
   exceptional polynomials in `d`.
6. Require exact pivots for `A3,B3,C3`, require normal forms containing only
   `{M,B2}` plus already certified factorized boundary terminals, and compare
   the exact output to the reconstructed rules above.
7. Replay from scratch: regenerate every row by its stable ID; verify every
   normalized row; reconstruct each pivot from recorded source weights;
   require a strictly triangular right-hand side; and reduce every source row
   to literal zero. This replay, not the modular rank, is the exact proof. The
   resulting closed system has 43 typed columns, rank 40, and exactly three
   free columns: `T1^5`, `M`, and the explicitly named `B2` candidate.

The smaller `FiveLoopBananaB2AuditShell` should remain a discovery/debug API.
It adds the two exact `N=1,D=1` joint-orbit seed IDs and the complete emitted
`N<=2,D<=2` algebraic halo, using the existing boundary certificate for lower
seeds, and reports only

```text
UnknownCandidateTerminal { integral: B2, checked_shell, modular_witnesses }
```

when `B2` is free.  It must not return `Master` or `Irreducible`.

## 7. Resource limits and regression commands

Before construction, preflight exact caps for seed orbits, raw origins,
collected term incidences, algebraic graph multisets, global columns,
elimination updates, coefficient exponent growth, source-row weights, and
boundary reductions.  Counts in Sections 1--3 are hard assertions for the
fixed shell, not tuning suggestions.  Every exponent shift must use checked
integer arithmetic.

The standalone tool is intentionally independent of Cargo and Symbolica.  A
direct `rustc` build is sufficient.  Recommended bounded invocations are:

```text
five_loop_scalar_shell_rank 2 0 0 1000003 17 19 all
five_loop_scalar_shell_rank 2 1 1 1000003 17 19 all 1
five_loop_scalar_shell_rank 2 0 0 1000033 23 29 all
five_loop_scalar_shell_rank 2 1 1 1000033 23 29 all 1
five_loop_scalar_shell_rank 2 0 0 1000037 31 37 all
five_loop_scalar_shell_rank 2 1 1 1000037 31 37 all 1
```

The expected headline outputs are respectively `rank/nullity=34/8` for the
scalar shell and `92/10` for the minimal numerator audit, with `A3,B3,C3`
pivot and exactly `M,B2` free among scalar top columns.  Run the scalar command
with `none`, `a2`, `b2`, and `all` to regression-test the minimality bitmap in
Section 4.
