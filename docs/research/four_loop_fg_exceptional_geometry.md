# Four-loop FG: the exact exceptional-geometry obstruction

Date: 2026-09-16. This is a diagnosis of an external input family, not a
four-loop closure claim or an engine special case.

## Reproduction and the previously missing diagnostic

The historical `family-close` error said `sector 397`, but this number is the
zero-based ordinal in the sorted **nonzero-sector manifest**, not a bitmask.
The full typed error already owned the original and unresolved polynomial
conjunctions; its `Display` implementation did not print them. Consequently,
the retained full-campaign logs did not identify the actual mathematical case.

A temporary public-Rust-API driver loaded
`examples/input/four_loop_fg.toml`, performed the same native zero analysis as
`family-close`, sorted the nonzero masks with the same ordering, and solved
only manifest entry 397. The native census contains 743 nonzero sectors and
281 proved-zero sectors. Entry 397 is the physical-coordinate mask

```text
1001110010
```

The driver is optimized and links the existing release RustRed and Symbolica
libraries; it does not rebuild dependencies or modify solver behavior. Source,
binary, exact stdout/stderr and measurements are retained at
`/tmp/rustred-fg-sector-diagnostic.6o3yj9/`. The complete input remains the
ordinary external TOML file, with no topology-name dispatch.

One isolated natural-order run took **0.20 s wall**, **0.15 s user CPU**, and
9,236 KiB peak RSS. Its internal census/input preparation took 84 ms and the
sector search took 106 ms. These are single diagnostic observations, not
controlled performance benchmarks and not timings for complete-family closure.
The earlier roughly 234-second full-family failure should not be interpreted
as time spent solving this one exceptional polynomial.

## Exact case

The failing parent integral is

```text
I(1, n1, 0, 1, 1, 1, n6, n7, 1, 0),   n1,n6,n7 <= 0.
```

There is one exceptional equation:

```text
g = 1 + 2*n7 + n7^2 + n6^2 + n1 + n1*n7 - 3*n1*n6 + 2*n1^2 = 0.
```

The original and unresolved conjunctions are identical. The existing exact
intersection engine reports one work item, one restriction, zero affine
admissions, zero Gröbner normalizations, one factorization, and zero factor
children. Its single equation has eight terms. Symbolica's native integer
factorization returns only `g`, with multiplicity one. No Gröbner normalization
is attempted because there is only one equation; a Gröbner basis of a principal
ideal would not remove its genuinely nonlinear zero locus.

This is therefore not the earlier H problem involving an affine equation and
an overly large rectangular carrier. Nor is it a missing affine sibling that
would be recovered merely by retaining all equations from a transformed basis.

## Infinite integer solutions inside the requested sector

The exact identity

```text
g = (n6-n1)*(n6-2*n1) + (n7+1)*(n7+n1+1)
```

shows why one cannot replace this equation by the separate vanishing of its
factors: the two products can cancel without either being zero.

Set `x=-n1`, `y=-n6`, and `v=-n7-1`. For the admissible subdomain `n7<=-1`,
these variables are nonnegative and

```text
g = (y-x)*(y-2*x) + v*(v+x).
```

For any integers `p>q>=0`, the assignments

```text
x = p^2+q^2
y = 2*p^2-p*q+q^2
v = q*(p-q)
```

give an exact zero. Equivalently,

```text
n1 = -(p^2+q^2)
n6 = -(2*p^2-p*q+q^2)
n7 = -1-q*(p-q).
```

Examples are `(-5,-7,-2)`, `(-13,-16,-3)`, and `(-25,-29,-4)`. They are not
the simpler `n7=-1` rays. This displayed parametrization supplies an infinite
subset of the exceptional locus; it is **not asserted to be an exhaustive
integer parametrization**.

The obstruction cannot be represented by a finite union of affine cases.
After shifting `n7+1`, `g` is a nondegenerate homogeneous quadratic in three
variables, irreducible over the rational field. It cannot contain an affine
plane, because that plane's linear polynomial would divide it. Thus each
proper affine subspace contained in its surface has dimension at most one.
Taking `q=p-1` above gives the non-collinear quadratic curve

```text
n1 = -(2*p^2-2*p+1),  n6 = -(2*p^2-p+1),  n7 = -p,  p>=1.
```

No finite set of affine lines covers this curve. In particular, adding a few
integer seeds or additional affine rays cannot certify coverage of the whole
exceptional domain.

## Existing Symbolica facilities and their authority boundary

The pinned dependency is the local Symbolica 3.0 development tree. The API
audit checked the public definitions, implementations, and independent
Rust/Python API tests/examples:

- `vendor/symbolica/src/poly/factor.rs`: native `Factorize::factor` and
  `is_irreducible`; RustRed already uses native factorization here.
- `vendor/symbolica/src/poly/groebner.rs`: native F4 normalization, FGLM order
  conversion, algebraic `solve`, and `solve_parametric`.
- `vendor/symbolica/src/atom/core.rs` and `src/solve/solution_set.rs`:
  `Atom::solve(...).over(Integers).wrt(...)`, exact branch assignments,
  conditions, `SolveCoverage::{Complete,Generic}`, and fallible `is_empty`.
- `vendor/symbolica/src/lib.rs` exports these interfaces publicly;
  `src/solve.rs`, `tests/solve_api.py`, and
  `examples/solve_linear_system.rs` exercise their actual semantics.

Native integer solving is available and must be tried before considering a
RustRed algebra implementation. However, a generic parametric solution set is
not a complete integer classification. Its coverage guard and branch
conditions cannot be discarded. The native API deliberately returns an error
from `is_empty()` when coverage or validity conditions are unresolved.
Likewise, finitely many returned *branches* need not represent finitely many
integrals: a branch can have free variables and unresolved integer-membership
conditions.

### Observed native solve results

An isolated optimized Symbolica-only probe verified both displayed polynomial
identities using native substitution and expansion, then called the public
integer solver in two forms:

1. With `n1,n6,n7` as unrestricted integer unknowns.
2. With `ni=1-Pi`, each `Pi` tagged as a positive integer, representing the
   exact requested nonpositive orthant without discarding inequalities.

Both return `coverage=Complete`, an empty coverage guard, and two radical
branches. Each branch has two free variables and retains an integer-membership
condition on its square-root expression. The orthant run also retains the
positivity conditions. Both return

```text
is_empty = Err(IncompleteCoverage("Emptiness depends on unresolved solution conditions"))
dimension = None
```

This is a complete *conditional symbolic representation*, not a complete
classification into unconditional affine-integer charts. `Complete` must not
be interpreted as permission to discard branch conditions. The solve took
19 ms for the positive-shift orthant and 1 ms for unrestricted integers; the
whole process took 0.05 s wall with 6,144 KiB peak RSS. A 60-second wall limit
was imposed and was not reached. Logs are `native.stdout.log` and
`native.measurements.txt` in the temporary diagnostic directory above.

### Bounded ordering comparison

The same physical mask `1001110010` was solved independently with natural and
fully reversed coordinate priorities. Both use the same input and native zero
census; the manifest ordinal was resolved before applying either permutation.

| Coordinate priority | Sector search | Process wall | Peak RSS | Result |
| --- | ---: | ---: | ---: | --- |
| Natural | 106 ms | 0.20 s | 9,236 KiB | Unsupported irreducible quadratic `g` above |
| Reversed `9,8,...,0` | 193 ms | 0.29 s | 12,308 KiB | A different unsupported irreducible quadratic |

The reversed run was capped at 180 seconds and did not reach the limit. Its
intersection engine performed six work items, five affine admissions, two
native Gröbner normalizations, five factorizations, and six factor children.
It ended on the face

```text
I(1,0,0,1,1,1,n6,n7,1,0),  n6,n7<=0,
1 - 5*n7 + 3*n7^2 + 2*n6 - 5*n6*n7 + n6^2 = 0.
```

The reversed condition has a crucially different integer geometry. The exact
identity

```text
4*h = (2*n6+2-5*n7)^2 - 13*n7^2
```

implies that its only rational, hence only integer, solution is
`n6=-1,n7=0`. If `n7` were nonzero, the rational number
`(2*n6+2-5*n7)/n7` would have square 13, which is impossible. Thus this
irreducible polynomial's integer locus is a **single point**, whereas the
natural-order `g` has the infinite nonlinear family proved above. Polynomial
irreducibility over the rationals alone does not determine integer-locus
dimension.

Native Symbolica expansion verified the displayed completed-square identity.
Its public solve API was then tried over both `Integers` and `Rationals`.
Both return two complete but conditional branches,

```text
n6 = -1 + 5*n7/2 +/- sqrt(13*n7^2)/2,
```

with free variable `n7` and a retained domain-membership condition on `n6`.
Both again report `dimension=None` and refuse unconditional emptiness
classification. Neither call simplifies these conditions to `n7=0,n6=-1`.
The exact outputs are in `reverse-native.stdout.log`; this final semantic
probe linked the existing release Symbolica library through an unoptimized
small harness, and no timing claim is made for it. It had a 60-second wall cap
and completed successfully. An optimized independent run agreed for the
integer domain (`native-expanded.stdout.log`).

Consequently, reverse ordering still triggers the current unsupported-case
error, but it turns this particular failure into a potentially much smaller
exact integer-geometry admission problem. This observation is not a solver
implementation or a complete-sector result. It does not establish that all
other exceptional branches are already representable. The existing C++
`vac4.cpp` is a different
denominator family, so its output cannot serve as a matched FG oracle; no
apples-to-oranges comparison was made.

## Physical root domains versus an all-coordinate family

There is a separate, important scope issue. The external FG input explicitly
labels `D1`--`D8` as physical propagators and `D9,D10` as auxiliary scalar-product
coordinates. Physical FG integrals therefore need arbitrary powers of the
first eight coordinates but only nonpositive powers of the last two. The
captured failing mask has **`n8=1`**: it introduces an auxiliary coordinate as
a positive-power propagator and is outside that physical root domain.

The H diagnostic obstruction similarly has its auxiliary `D10` active. This
does **not** make either failure irrelevant to the current command's promise:
`family-close` presently promises all `2^K` sectors of its input coordinate
family. The program audit, artifact installer, and cold loader enforce that
all-sector promise. Removing these sectors while keeping the old promise
would be incorrect.

An explicitly requested root-domain contract is nevertheless mathematically
natural and remains fully generic. For any chosen auxiliary coordinate `a`
with unshifted integer powers, an ordinary IBP can increase its index only
through differentiating that denominator, giving terms of the forms

```text
-n_a*C*I(n+e_a),        -n_a*C_b*I(n+e_a-e_b).
```

If `n_a<=-1`, the new index is still nonpositive. If `n_a=0`, the coefficient
vanishes exactly. Other differentiated denominators cannot increase `n_a`.
This is implemented directly in
`identity/generator/ordinary.rs`: every raising derivative is multiplied by
the corresponding index. Hence ordinary sources started inside the
no-positive-auxiliary domain do not leave it. This argument requires the
unshifted-power setup; a nonzero power shift must be analyzed explicitly.

Discovery also uses *translated* sources and may temporarily visit outside
the requested root domain. Therefore source invariance alone is insufficient:
the final published rules must independently prove that each nonzero RHS
remains in the admitted sector manifest. RustRed's existing supplied-domain
sector-monotone admission already proves the stronger property that an
inactive parent line cannot become active; this is the appropriate proof
boundary to reuse, not bypass.

A sound smaller closure objective would explicitly declare a downward-closed
set of sectors, such as all masks with `n8,n9<=0` for FG, then:

- enumerate and prove coverage of every nonzero sector in that declared set;
- retain exact zero-sector proofs and strict-descent/RHS containment;
- serialize the declared root domain and authenticate it again during cold
  loading, instead of reconstructing unrestricted root bounds;
- reject out-of-domain application requests and out-of-domain terminals;
- bind the scope into artifact identity and inspection output;
- retain unrestricted all-sector closure as the default or an explicit
  separate objective, never silently narrow it based on names or comments.

At the time of the initial diagnosis, `supported_root_power_bounds` and reducer
root-input checking offered some infrastructure, but the source-port loader replaced these with
unrestricted bounds and both census gates require `2^K` masks. Thus this is an
explicit generic schema/coverage extension, not merely filtering the search
queue. It could avoid artificial positive-ISP obstructions for the physical
four-loop application, but no scoped artifact has yet been generated, and
the unperformed exact publication checks may still expose other obstructions.

### Bounded physical-domain search experiment

A follow-up diagnostic explicitly selected all FG sectors with `n8,n9<=0`,
using the same native zero census, natural coordinate order, exact input, and
shared source system as `family-close`. It used the public `SectorExecutor`
with two workers and a 120-second wall limit. All **256** selected masks are
accounted for: **132 native-proved-zero sectors** and **124 nonzero sectors**.
All **124 nonzero sector searches succeeded**, generating **9,264 candidate
rules** and **145 fully fixed finite residuals** in aggregate.

| Boundary | Observation |
| --- | ---: |
| Driver preparation + selected search, internal clock | 33.427 s |
| Whole process wall time | 33.47 s |
| User CPU time | 65.91 s |
| System CPU time | 0.39 s |
| Peak RSS | 114,080 KiB |
| Workers | 2 |
| Process exit status | 0 |

Each completion is logged with its actual sector mask, ordinal-independent
completion count, rule count, and residual count. Logs and external
measurements are `physical.stdout.log`, `physical.stderr.log`, and
`physical.measurements.txt` in the temporary diagnostic directory. The generic
driver option was `--nonpositive-axes 8,9`; no family-name dispatch was added.

This is **search completion only**. The run did not invoke the artifact
installer, original-source audit, uniform-descent audit, exact ownership
coverage, or cold loading. Finite residuals are not automatically authenticated
masters, and their count is not a minimality claim. No artifact was produced.
It therefore supports implementing and testing the explicit root-domain
contract as a concrete next step, while leaving unrestricted all-sector K10
closure open.

The existing `family-solve --sectors ...` CLI can select a manifest, but its
current application adapter supplies `SectorConfig::default()` without the
native zero-sector census. The temporary driver was used here to keep source
projection identical to `family-close`, and to retain per-sector completion
records. This API/configuration difference should be addressed generically
when adding the explicit closure-scope interface.

### Full physical-domain publication attempt

The subsequently implemented generic root-scope contract was tested through
`family-close --nonpositive-indices 8,9`, with natural priority and two workers
on physical CPUs 34/35. This is explicitly scoped closure, not a redefinition
of the unrestricted family. All 281 native global zero-sector proofs remained
available to replay; the requested downset contained 132 zero and 124 nonzero
masks.

All 124 searches finished by **32.9 s**, again proposing **9,264 rules and 145
finite residual entries**. Unlike the search-only diagnostic, this run then
invoked exact artifact checks. Twenty sectors passed before sector mask
**106**, physical-coordinate string **`0101011000`**, failed with **93/96
replayed and descending rules, zero uncovered boxes, and three original-source
residuals**:

```text
(-1/n8) I(n0+1,1,-1,2,0,0,1,0,n8,0)
(-1/n8) I(n0+1,1,-1,2,0,0,1,-1,n8,0)
(-1/n8) I(n0+1,1,-2,2,0,0,1,0,n8,0)
```

The process exited 8 after **50.11 s wall**, **81.61 s CPU** (79.94 user +
1.67 system), with **196,640 KiB peak RSS**. It did not reach the 1200-second
bound. No artifact, cold inspection, or reduction canary was produced. Evidence
and the frozen release binary are in `/tmp/rustred-four-loop-fg-scope.4ueeaV/`;
the executable SHA-256 is
`b01f0c4c8e96d41e1deaf53b34aed36fdae9672f725d824a6e5e402fe988b64b`.
These are shared-host diagnostic observations, not benchmark medians.

This obstruction is inside the physical domain and does not involve the
nonlinear exceptional geometry above. The first useful source-level lead is
boundary activation: when `n0<=-1`, the displayed integrals have only the
three active denominators `D2,D4,D7` (mask 74), leaving a scaleless loop
direction. At `n0=0`, the shifted exponent `n0+1` activates `D1` and gives
mask 75. The four momentum forms `k1`, `k2`, `k1-k3`, `k1-k3+k4` are independent;
the native census includes that sector among its nonzero search tasks. Thus
the whole shifted column cannot be removed solely from the parent sign of
`n0`, and `-1/n8` does not vanish on this boundary for negative `n8`.

Discovery's `vanishes_in_subsector` currently retains the parent sector sign
for symbolic shifted powers; the independent source audit deliberately
regenerates unprojected rows and checks actual sign cells. Their discrepancy
is a concrete source-projection hypothesis, not authority to weaken replay.
The complete candidate cases, recentering shifts and dependency traces still
need a selected-sector diagnostic to establish precisely where the boundary
was lost. Neither the 145 finite residual count nor zero uncovered coordinate
boxes substitutes for a replayed identity.

### Conditional nonzero boundary check

For the first displayed residual (reported rule 85), consider `n0=0,n8=-1`.
**Admission of this point by that rule's complete case and exclusions has not
yet been confirmed.** The following calculation identifies a useful exact
diagnostic target; it does not establish the full cause of the replay failure.

The residual coefficient is then `+1`, multiplying

```text
I(1,1,-1,2,0,0,1,0,-1,0).
```

Use the unit-Jacobian loop transformation
`a=k1, b=k2, c=k1-k3, e=k1-k3+k4`. Its denominators are four independent
unit-mass tadpoles, with the `c` denominator squared. The numerator is
`((a-c)^2-1)*((b-e+c)^2-1)`. Write
`T_r = integral d^d l / (l^2-1)^r`, with the same per-loop measure convention
throughout. Odd terms integrate to zero; terms canceling the only denominator
of `a`, `b`, or `e` leave a scaleless polynomial integral. Consequently,

```text
I(1,1,-1,2,0,0,1,0,-1,0)
  = T_1^3 * integral d^d c [c^2*(c^2+1)/(c^2-1)^2]
  = T_1^3 * (3*T_1 + 2*T_2)
  = (d+1)*T_1^4.
```

The final step uses the ordinary one-loop IBP `2*T_2=(d-2)*T_1`. This is
generically nonzero, so full rank here is not the only evidence against
discarding the term. If the selected-rule diagnostic confirms that `n0=0`
and `n8=-1` satisfy the actual case, recentering and guards, this point is a
concrete counterexample to treating the entire shifted column as scaleless.
Until those conditions are checked, the pruning diagnosis remains conditional.

## Minimal generic direction

Do not try to make the existing finite OR-of-affine engine accept this locus
as an affine hull, a set of samples, or a finite terminal set. Raising its
factorization/Gröbner budgets cannot turn this irreducible cone into finitely
many affine cases.

The smallest near-term strategy to investigate is **candidate selection that
accounts for exceptional geometry**. A proposed IBP may be exact yet introduce
an exceptional locus outside the supported case model. Preserve the failure
and try an alternative exact rule, pivot/source support, or bounded ordering
choice on the same uncovered parent. Every alternative must still pass the
ordinary-source replay, guard, descent, ownership and cold-artifact gates.
Failure to find a representable candidate remains incomplete, not closure.

The reversed two-variable example suggests a separately bounded exact geometry
lemma. Suppose native operations establish that a rational-coefficient
polynomial involves exactly two free variables, has total degree two, is
irreducible over the rationals, and has a unique rational stationary point
`v` where its value is zero. Translation to `v` gives an irreducible binary
homogeneous quadratic. A nonzero rational zero would imply a rational
projective root and hence a rational linear factor, contradicting
irreducibility. Its rational zero locus is therefore exactly `v`; the integer
locus is that singleton if integral and in the parent domain, otherwise empty.

Derivative construction, exact linear solving, factorization, and substitution
are native Symbolica operations. RustRed would orchestrate the restricted
geometry proof, not implement another factorizer or general Diophantine
solver. This is a proposed, unimplemented admission rule requiring its own
adversarial tests. It must not be generalized to three free variables: the
natural-order `g` is an explicit counterexample to that extension. Nor may the
zero-value-at-the-stationary-point condition be omitted; general translated
quadratics can have substantially different integer loci.

A larger alternative is a true nonlinear equality-case service, using native
Symbolica ideal reduction/solution sets and retaining exact integer conditions
through search, replay, guards, ownership and persistence. That is an
architectural extension, not a factorization patch. It should not be undertaken
before the actual native solve and ordering experiments show that it is needed.

The diagnostics improvement made during this study changes neither approach:
errors now retain a bounded printable exact context (16 KiB, with explicit
truncation) and the real physical-order sector mask. The complete typed payload
remains unchanged. No new CAS or case-engine algorithm is implemented here.
