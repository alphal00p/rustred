# Getting past the five-loop exceptional domain

Date: 2026-09-20. Research and bounded diagnostics, not a claim of five-loop
closure. The exact captured equations and previous solver measurements are
in [the exceptional-geometry record](tide_exceptional_geometry.md).

## Conclusion

A rank-scoped hybrid is the most direct practical next step: retain symbolic
rules on their valid domains, enumerate only the bounded exceptional integer
targets, and derive fresh exact relations for them. This follows TIDE's
finite-target approach more closely than proving coverage of all integer
indices by piecewise symbolic rules. Unrestricted symbolic closure remains a
separate objective.

The current problem is not evidence of an infinite master basis. It is a
nonempty infinite set of indices where one selected formula is inapplicable.
Alternative formulas, specialized source systems, or a finite requested domain
can get past that formula without classifying every integer point on the
unbounded surface. None of these alternatives is yet demonstrated to close
the complete connected five-loop sector.

## What fails, schematically

An elimination step has the schematic form

```text
g(n,d) I(n) = a1(n,d) J1(n) + ... + ak(n,d) Jk(n)

g nonzero at the requested indices       g identically zero as a function of d
                |                                         |
divide and obtain a descending rule      this formula cannot solve for I(n)
                |                                         |
continue reducing the RHS                derive a different relation on this case
```

This diagram illustrates pivot specialization, not the literal full 781-term
identity. RustRed also checks that a rule does not activate forbidden sector
coordinates. Parameter poles at particular dimensions remain distinct from an
index specialization making a denominator identically zero for generic `d`.

The fast search initially chooses useful rows at modular sample points, then
materializes an exact rule. The exact formula can contain index-dependent
denominators. Their zero conditions generate additional cases. Coordinate and
affine cases can be searched again; a genuine curved integer surface currently
exceeds the supported case representation. More time in the same failed call
does not add that representation.

This is not solely a RustRed discrepancy: the narrowed original SpIRed run
returned the same 781 physical RHS integrals and exactly equal coefficients,
including proportional denominators. Its exceptional decomposition also
contains the same conic. SpIRed's `dioSys::linearize` instead offers a finite
coordinate scan, with default `MAX_NLIN_SEARCH=30`; the reference source warns
that this can replace an infinite set with an inequivalent finite set. That
scan is not a total-numerator-degree-30 coverage contract.

## The actual natural-order exception

The physical sector is `111000000001110`. Six active powers are fixed to one;
`n5=n6=n9=n14=0`. The remaining five indices are nonpositive. With `u=1+n10`,
the captured branch is

```text
n3 = (2*u + 3*n8 + n7)/3
n4 = (-3*u + n8 + 10*n7)/8
164*n7^2 - 308*n7*n8 + 189*n8^2
  + 124*n7*u - 94*n8*u - 75*u^2 = 0.
```

For every integer `t>=1`, it contains
`(n3,n4,n7,n8,n10)=(-2t,-t,-t,-t,-t-1)`. The numerator degree is `6t+1`.
The nondegenerate indefinite quadratic has infinitely many admissible
directions, not just this displayed ray. Declaring every such integral a
separate master would not deliver a finite universal basis.

Here define numerator degree and excess denominator degree separately:

```text
R = sum_i max(-n_i,0)
D = sum_i max(n_i-1,0).
```

For quadratic numerator factors, `R` counts scalar-product powers; their
momentum degree is `2R`, not `R`. Auxiliary shifted propagators can also
produce lower-degree terms on expansion.

At `R<=30`, exhaustive exact integer enumeration gives **six** points on this
specific conic and affine parent:

| R | n3 | n4 | n7 | n8 | n10 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 0 | 0 | 0 | 0 | -1 |
| 7 | -1 | -3 | -2 | -1 | 0 |
| 7 | -2 | -1 | -1 | -1 | -2 |
| 13 | -4 | -2 | -2 | -2 | -3 |
| 19 | -6 | -3 | -3 | -3 | -4 |
| 25 | -8 | -4 | -4 | -4 | -5 |

Exhaustiveness here is elementary: every free nonpositive index is at least
`-30`, so it suffices to visit `n7,n8 in [-30,0]`, `u in [-29,1]`, recover
`n3,n4` by the exact chart, and test divisibility, signs, the quadratic and
total degree. Root and an independent agent reproduce the same six tuples.
This is geometry enumeration, **not** six established masters or a completed
reduction. Other exceptional branches and subsequent cases remain separate.

The Q-first ordering has a different parent with one fixed dot. Its surviving
branch can be written

```text
(7*a-4*b)*w = 15*a^2-18*a*b+5*b^2
a,b >= 0, w >= 1, b <= 2*a+1, R=3*a+w.
```

For `R<=30` there are **41** points: 30 on `a=b=0`, and 11 with
`7*a-4*b != 0`. For total excess `R+D<=30`, its fixed `D=1` instead leaves
40. These counts concern different ordered parent cases and are not additive.

## What TIDE actually establishes

The supplied Luthe thesis is also available from the
[Bielefeld repository](https://noah.nrw/ubbihs/download/pdf/5131192).
The relevant printed-page references are:

- **Section 2.3, p.16:** define the wanted finite domain by separate maximum
  dot and numerator powers. Both large seed sets and missing boundary
  relations can cause severe cost, including expression growth.
- **Section 2.4, pp.21–23:** ordering matters; incomplete boundary information
  is helped by symmetries, mass-derivative combinations and syzygies that
  avoid increasing dots. Such combinations reorganize IBP information,
  rather than creating independent new physics identities.
- **Section 5, pp.43–44:** the numerical-master method introduces one
  symbolic propagator exponent; other powers remain integer labels. This is
  not an all-index symbolic closure result.
- **Section 8.1.2, pp.88–89:** distinguish wanted bounds from generated bounds.
  Extra seed layers supply information at the wanted boundary; a one-layer
  margin is an empirical starting point, not a theorem.
- **Section 8.1.4, pp.92–94:** use modular runs to select useful equations and
  delay lower-sector algebra until the main-sector operations are known.
- **Section 8.1.5, pp.94–95:** test alternative bases/orderings cheaply before
  committing to the full symbolic reduction.

The inference for RustRed is to specialize difficult index targets **before**
selecting their modular support, reuse immutable lower-sector reductions, and
materialize only successful dependency traces. TIDE does not demonstrate that
our present conic is empty, nor supply an all-index rule bypassing it.
Its section 8.1.4 also cautions that higher-sector seeds can occasionally
reveal additional lower-sector relations. Delayed lower-sector work must
preserve that information, not assume every lower-sector-only row redundant.

## Related primary literature and implications

- [Kira 3, sections 3.4–3.5](https://arxiv.org/html/2505.20197v1#S3.SS4):
  equation selection at generic samples need not remain sufficient on a
  constrained slice; the chosen slice must inform selection. Fully symbolic
  powers are expensive, while one or a few symbolic powers can be useful.
  This supports specialized probes and partly symbolic fallback, not a
  guarantee for our sector.
- [Smith–Zeng, sections 2.3 and 3.1](https://arxiv.org/html/2507.11140v2#S3.SS1):
  use syzygy-constrained operators, operator-level elimination, then small
  local symbolic systems for unresolved targets. This provides a complementary
  way to control dot growth and support size. Their demonstrations do not
  settle our nonlinear exceptional surface or guarantee a sufficient seed
  radius.
- [Nabeshima, sections 1–3](https://link.springer.com/article/10.1007/s00200-023-00620-8):
  generic rational-function elimination must retain specialization conditions;
  comprehensive systems separate parameter loci. This is the algebraic
  precedent for conditional case ownership. It is not a ready-made integer
  IBP/Ore solver or a promise of practical five-loop termination.
- [Smirnov–Petukhov](https://arxiv.org/abs/1004.4199): finiteness of the master
  space is not contradicted by infinitely many indices where one formula
  fails. It does not tell this search the sufficient seed depth or the best
  finite terminal basis.

## Recommended next experiment and longer-term route

1. Use the completed bounded source-visitation probes below as a control.
   Different formulas can have different bad loci, but these three did not.
   Combining formulas requires preserving each formula's own applicability
   conditions; identical bad loci offer no extra coverage.
2. Probe the six bounded natural-order points directly with input-generated
   IBP sources, symbolic `d`, and the existing native sparse machinery. No
   Diophantine solver is required after the target indices are fixed. Start
   with a small finite seed neighborhood and expand only measured misses.
3. If successful, generalize the *input scope*, not the topology: enumerate
   requested bounded exceptional targets, deduplicate their reachable
   dependencies and prove the resulting exact reduction graph ends in an
   explicit manageable finite terminal set. Do not accept every requested
   integral as a terminal merely to make coverage vacuous.
4. Retain source-generation headroom beyond the requested bound. A rule may
   lower the full integral ordering while increasing numerator degree. An
   input rank bound is not permission to discard such RHS terms or truncate
   source equations. A budget miss stays incomplete.
5. For unrestricted progress, test a nonlinear coefficient-field diagnostic
   using Symbolica's existing rational-function and algebraic-quotient
   operations. The Q-first equation is rationally solvable for `w` away from
   `7*a-4*b=0`; its denominator-zero piece is `a=b=0`, with `w` free. The
   natural conic permits a quadratic function field. These can generate
   identities on a variety without first listing all its integer points.
   Integer/sector membership, new poles, exceptional components, shifted
   source replay and strict descent still require explicit treatment.

A numerator-only cap leaves the whole family's positive denominator powers
unbounded. Use fixed/capped dots, a total-excess scope, or separately proved
symbolic dot-lowering rules. The immediate six-point study has fixed unit
denominators, so that caveat does not prevent it. It must not silently expand
its success claim to all numerator-rank-30 integrals of all five-loop families.

More memory or a ten-hour run is justified for an advancing exact solve, not
for repeating an unsupported-case error. All new pilots remain resource
bounded with phase/progress and memory observations; no long campaign is
admitted solely on the strength of this research.

## Fresh source-visitation results

The release client uses the current core, prepared family-bound zero census,
natural integral ordering, depth at most two, one worker on CPU94 and sparse
exact materialization. Each isolated case had a 300-second/8-GiB ceiling and
returned before it. These are three sequential single observations on a shared
host, not a repeated performance benchmark or a completed-sector timing.

| Source visitation | Search time | Process wall | RHS terms | Selected sources | Exceptional geometry |
| --- | ---: | ---: | ---: | ---: | --- |
| Identity | 36.934 s | 38.02 s | 781 | 496 | Unsupported conic |
| Reverse | 37.418 s | 38.32 s | 653 | 418 | Same unsupported conic |
| Half-rotation | 27.507 s | 28.23 s | 725 | 454 | Same unsupported conic |

All three returned raw exceptional branch/equation lists are byte-identical.
Thus reverse/rotation change source support and the formula, but their union
does not remove this bad locus. The identity result also passes an independent
native Symbolica comparison against the older 781-term control: all
coefficients, variable maps and structure agree. No candidate in this table
was published as a closing sector artifact. Evidence, resource observations,
and the independent runner audit are in
`TMP/tide-source-order-pilots.BEebXm/`.

## Direct bounded-point experiment: five useful relations

All six concrete targets above were then passed to the already-built generic
client, keeping `d` symbolic, natural integral ordering, identity source
visitation, sparse exact arithmetic and seed depth at most two. The process
budget was 60 seconds and 8 GiB per point, sequentially on CPU93. No timeout
occurred and no compilation or family-specific implementation was needed.
The client uses the ordinary input-generated **preconditioned** source basis;
this is not an unpreconditioned-original-source rank experiment.

| Target | Solver core including preconditioning | Process wall | RHS terms | Result |
| --- | ---: | ---: | ---: | --- |
| R=1 | 0.822 s | 0.85 s | — | Depth-2 search exhausted after 3,775 rows |
| R=7, off the displayed ray | 3.436 s | 4.17 s | 217 | Exact descending local candidate |
| R=7, ray t=1 | 6.934 s | 7.69 s | 259 | Exact descending local candidate |
| R=13, ray t=2 | 2.095 s | 2.82 s | 262 | Exact descending local candidate |
| R=19, ray t=3 | 2.139 s | 2.87 s | 262 | Exact descending local candidate |
| R=25, ray t=4 | 2.203 s | 2.95 s | 262 | Exact descending local candidate |

All five returned candidates have zero extracted index-exception branches,
successful `Ok([])` exceptional geometry and strictly descending fixed
physical RHS keys. The dimension remains symbolic and its possible poles
remain visible in coefficients. Each candidate comes from one native exact
materialization with stored preconditioned-source/seed provenance. The batch
status is one because of the single bounded miss, not a crashed or timed-out
solve. GNU-time peak RSS ranges from 27,708 to 40,008 KiB. Timing observations
are single runs with prepared zero data, not full logical-cold family timings.

This is direct evidence that useful exact rules exist at five points where
the generic formula fails. It does **not** yet recursively reduce their RHS
to a terminal basis. The R=1 miss does not prove master independence, absence
of deeper IBPs, or completeness of the specialized preconditioned source span.
No rank-30 artifact or connected five-loop closing bundle is published.

The RHS analysis makes the scope distinction concrete: each formula reaches
numerator degree one greater than its target (8, 8, 14, 20, 26 respectively),
and up to two dots. Maximum total excess is the input degree plus three.
The full integral ordering still descends, including transitions to pinched
sectors. Thus using a requested numerator degree as an intermediate cutoff
would discard genuine terms even in these successful local tests.
Across these five formulas there are 1,262 distinct immediate physical RHS
keys. None exceeds the overall degree-30 ceiling in this first step, but they
leave the unit-denominator-only input scope. Their later descendants remain
unchecked; the finite geometry census does not by itself make reduction free.

Receipts and frozen inputs:
`TMP/exceptional-domain-rank30-audit.20260920/native-six-point-run/`.
The next bounded test is the deduplicated RHS dependency graph and the R=1
target with an expanded or independently checked source neighborhood. Preserve
the five successful rules rather than regenerating their generic parent.
