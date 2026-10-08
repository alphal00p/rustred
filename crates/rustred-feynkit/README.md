# RustRed–Feynkit IBP bridge

`rustred-feynkit` embeds RustRed in the same native Symbolica kernel as
Feynkit. The community host registers `IBPFamily`, `IBPRule`, and
`IBPSolution` in `symbolica.community.hepkit`. No standalone `rustred` Python
extension, Kira executable, or expression string conversion is involved.

The bridge also supports the shared HEPKit **Pyodide/WebAssembly** host. Build
with `default-features = false` and `features = ["wasm", "campaign-api"]` to use
Symbolica's portable arithmetic and the same rule/artifact APIs. Browser
generation is single-worker and synchronous, with explicit execution
capabilities for notebook controls; see [the WASM contract](../../docs/wasm.md).

```python
from symbolica import S
from symbolica.community import hepkit as hep

d, k, m2 = S("d", "k", "m2")
kin = hep.Kinematics(d, momenta=[k])
family = hep.IntegralFamily(
    [k], [], [kin.scalar_product(k, k) - m2], kinematics=kin,
)
ibp = hep.IBPFamily(family)
parametric = ibp.solve_parametric([True])
print(parametric.rules[0].target, parametric.rules[0].terms)
print(parametric.reduce([3]))
laporta = ibp.reduce_laporta([[3]])
print(laporta.reduce([3]))
```

For graph input, start with `diagram.integral_family(kinematics=kin)`.
Provide independent external momenta and assign all their scalar products in
`Kinematics`. Masses, invariants, and the dimension can be arbitrary rational
functions of scalar Symbolica symbols, including namespaced symbols, and of
loop-independent invariants such as the tensor contraction `dot(q, q)`:

```python
from symbolica.community.tensor import Representation, TensorName, dot

p = S("p")
q = TensorName.vector("probe::q")(1, Representation.mink(dimension=d))
kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, dot(q, q))
bubble = hep.IntegralFamily(
    [k], [p], [kin.scalar_product(k, k), kin.scalar_product(k - p, k - p)],
    kinematics=kin,
)
print(hep.IBPFamily(bubble).reduce_laporta([[2, 1]]).reduce([2, 1]))
```

An invariant is a call to a function declared `Scalar`, such as `dot`, that
mentions none of the family's momenta, labeled like `K(0)` or plain like `k`,
and no routed edge momentum `Q(e)`. These checks are syntactic: any other
vector inside an invariant must itself be loop-independent. Each distinct
invariant is an independent parameter and appears unchanged in coefficients
and conditions; relations between invariants, or with symbols inside them
such as the `d` in `mink(d)`, are not used. Contracted indices normalize to
`dot`, and Symbolica orders its arguments, but `dot` over `mink(d)` and over
`mink(4)` are different invariants. Uncontracted tensor structures, such as an
explicit metric or a factor left inside `dot`, Symbolica built-ins such as
`log`, and non-integer or symbolic exponents such as `sqrt(s)` are rejected.
Call `.to_expression()` on a tensor invariant before combining it with
`kin.scalar_product(...)` in a denominator.
Complete an independent but incomplete family with `family.complete()`;
the appended slots represent irreducible scalar products and normally have
nonpositive powers. Dependent propagators require partial fractions first.
The bridge preserves denominator order and propagator signs.

`ibp_identities()` returns the ordinary `L*(L+E)` zero equations as lists of
`(powers, coefficient)` pairs. Powers and coefficients are native Symbolica
expressions. `index_symbols` identifies the formal integral indices; these
symbols are distinct from physical parameters.

`solve_parametric(sector, fixed=None, max_depth=2, include_lorentz=False)`
uses RustRed's sector search to find a reusable symbolic recurrence. `sector`
is a Boolean vector specifying positive and nonpositive coordinates. `fixed`
optionally fixes absolute integer powers, with `None` retaining a symbolic
index. Each rule exposes its target, terms, sector, nonzero conditions, and
exceptional branches. All polynomials in an exceptional branch vanishing
forbids the rule. `rule.apply(powers)` checks the integer-domain conditions;
remaining symbolic kinematic conditions must still be nonzero. A parametric
`solution.reduce(powers)` performs one recurrence step. It does not certify
closure of every sector or every exceptional case.

`reduce_laporta(targets, max_depth=2, max_targets=1024, include_lorentz=False,
preferred_masters=None, until_stable=False)`
searches integer seed neighborhoods, exactly replays the discovered rules,
follows their right-hand sides, and back-substitutes the solved integrals.
`max_depth` bounds the signed L1 seed radius; `max_targets` bounds the number
of distinct integrals searched. Exhausting the latter raises an error.
`residuals` explicitly lists the remaining basis at that search depth; these
are not certified independent masters. Increasing depth can reduce the basis
further; `preferred_masters`, `until_stable`, and `certify()` are described
below. Unmatched integrals passed to `solution.reduce` remain unchanged
unless they lie outside a declared cut, which reduces them to zero.
Scaleless subsectors are removed only after RustRed's sector analyzer proves
them zero. Family symmetries can further identify residuals using Feynkit's
verified momentum maps, as demonstrated in the notebook. In a cut family, use
only maps that send cut denominators to cut denominators.

`hep.IBPFamily.compiled_runtime_arities()` reports the solver entry points
compiled into the host (by default 1–16 denominators). This is a build
capability, not a mathematical bound. To choose a different finite registry,
enable the `runtime-arity-selection` Cargo feature and set, for example,
`RUSTRED_RUNTIME_ARITIES=1,2,13,14,15,21` when rebuilding
the community host; setting it only at Python runtime cannot add compiled
entry points. The list must contain distinct positive integers, separated by
commas. Unsupported-arity errors report the actual compiled list.

The independent, opt-in `capacity-dispatch` Cargo feature uses storage
capacities 4, 8 and 16 for all supported physical arities. It changes no
denominators or powers: padding is frozen to zero and removed from results.
`hep.IBPFamily.compiled_runtime_capacities()` reports the selected capacities.
Campaign generation, matching, routing, feedback, both walking policies,
checkpoint restore and cold closure verification compile only these same three
capacities. Prepared basis ordinals, public saved programs, progress keys and
certified artifacts retain the original physical arity. Private checkpoints
record their storage width; manifest-bound readers validate padding before
converting between physical coordinates and shared storage.
Neither feature is enabled by default. Community forwards them as
`ibp-capacity-dispatch` and `ibp-runtime-arity-selection`, respectively.

Rust hosts can bypass the runtime registry using the checked
`solve_parametric_for::<N>`, `solve_laporta_for::<N>`, and
`certify_laporta_for::<N>` entry points in `rustred::solver::bridge`, with the
same cuts, options, preferred-master and certificate contracts. Hosts using
`SectorSolver` directly can reuse `rustred::dispatch_arity!` with a caller's
generic function and either the compiled registry or an explicit literal
list. Neither mechanism changes algebra, ordering, or resource limits.

Concrete search targets and fixed coordinates must lie in RustRed's compact
power range `-64..63`;
applying a discovered symbolic recurrence accepts signed 64-bit powers.
It keeps all kinematic scales
symbolic and uses the existing native exact sparse backend. RustRed's separate
experimental reconstruction backends depend on newer vendored Symbolica APIs;
the core's default `reconstruction` feature keeps those available to existing
RustRed users, while this bridge disables that feature to share the community
host's pinned Symbolica version.

## Reverse-unitarity cuts

`IBPFamily(family, cut=[True, False, ...])` flags cut denominators in family
order, like Kira's `cut_propagators` (1-based numbers there) or LiteRed's
`CutDs`. Every integral with a nonpositive power on a cut denominator
vanishes. Both solvers and `solution.reduce` return zero for such integrals,
whether or not they were requested:

```python
from symbolica import E

d, k, p, s, m2, I = S("d", "k", "p", "s", "m2", "I")
kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, s)
bubble = hep.IntegralFamily(
    [k], [p], [kin.scalar_product(k, k) - m2, kin.scalar_product(k - p, k - p)],
    kinematics=kin,
)
solution = hep.IBPFamily(bubble, cut=[True, True]).reduce_laporta([[2, 1]], max_depth=1)
expected = (3 - d) / (s - m2) * I(1, 1)
assert (solution.reduce([2, 1], integral=I) - expected).together() == E("0")
assert solution.reduce([1, 0]) == []
```

The cut family keeps the uncut integral order. A cut reduction is therefore
the uncut reduction with every integral outside the cut removed, and its
residuals are the uncut residuals inside the cut. `ibp_identities()` returns
the ordinary identities; they hold for cut integrals once the terms outside
the cut are set to zero. In `solve_parametric`, discovery drops terms that
leave the cut through a fixed cut index, so fix the cut indices where
possible, for example `fixed=[None, 1]`. Terms that leave the cut through a
symbolic cut index stay in `rule.terms`; `rule.apply` and `solution.reduce`
drop them as zero. With every cut index symbolic, the rule is the uncut rule.
Masks built from a diagram's cut edges follow the family's denominator order.
Never cut the auxiliary slots appended by `complete()`.

## Preferred masters

`reduce_laporta(targets, preferred_masters=[[...], ...])` expresses the
reduction through the listed integrals. They are searched like targets and
count against `max_targets`:

```python
from symbolica import E

d, k, m2, T = S("d", "k", "m2", "T")
kin = hep.Kinematics(d, momenta=[k])
tadpole = hep.IntegralFamily([k], [], [kin.scalar_product(k, k) - m2], kinematics=kin)
solution = hep.IBPFamily(tadpole).reduce_laporta([[1], [3]], max_depth=1, preferred_masters=[[2]])
assert solution.residuals == [[2]] and solution.replaced == [[1]]
assert (solution.reduce([1], integral=T) - 2 * m2 / (d - 2) * T(2)).together() == E("0")
```

Each preferred integral that the search reduces replaces a residual of its own
sector by an exact basis change: per sector, from the lowest, the reductions
of its preferred integrals are brought to reduced echelon form over the
sector's other residuals, hardest residual first. Dividing by a pivot, here
`d - 2`, adds its numerator to the `nonzero_conditions` of every rule that
uses the replaced residual. Mapping each preferred integral back to its
original reduction must restore every rule of the search exactly; the bridge
checks this. A preferred integral that the search leaves unreduced already is
a residual and stays one; `solution.preferred_masters` reports `"replaced"` or
`"residual"` for each. `ValueError` is raised, and no basis change returned,
when a preferred integral is listed twice, lies outside the cut or in a zero
sector, reduces to zero or only to lower-sector integrals, or depends on other
preferred integrals of its sector modulo lower sectors. These are exact
relations, not depth effects. Masters are counted modulo IBP (and Lorentz)
identities without symmetries, so integrals equal by a graph symmetry are
distinct masters. `residuals` may include integrals needed only to express a
preferred integral.

## Checking a reduction

`until_stable=True` searches depths `0..=max_depth` and stops once two deeper
searches reproduce the residual set; `solution.stable_depth` is the first depth
of that plateau, or `None` if `max_depth` came first. This is a heuristic.

`solution.certify(count_masters=True, replay=True, seed=0)` returns an
`IBPCertificate` of a `reduce_laporta` solution:

- `replay` recomputes the zero-sector census, derives every rule of the
  search again from the family's original IBP (and, if used, Lorentz)
  identities at the seeds the search recorded, checks strict descent, and
  checks that every returned rule uses only residuals and, including any basis
  change, follows from the derived rules. A failure raises `ValueError`;
  success sets `certificate.reduction = "verified"`. The nonzero conditions
  are not checked against the coefficients' poles.
- `count_masters` counts the master integrals of each residual sector without
  symmetries from critical points of the Lee–Pomeransky polynomial `U + F`,
  computed exactly at random finite-field kinematics chosen by `seed` and
  required to agree on two independent samples. `certificate.masters` is
  `"incomplete"` when a sector holds more residuals than masters, which shows
  that the search missed relations; `"count-consistent"` when no sector
  exceeds its count, which does not prove the residuals independent; or
  `"no-verdict"` when a count is unreliable, for example with a singular
  external Gram matrix (light-like or forward kinematics), where the
  parametric count can be lower than what IBP reaches. `master_counts`,
  `residual_counts`, `excess_sectors`, and `no_verdict` give the details.
  Counts compare sector by sector, so a relation the search misses between
  integrals of different sectors, such as two equal-mass tadpoles related by
  a light-like shift, is not detected. Counting computes one Groebner basis
  per subsector of each residual sector: milliseconds at one loop and about a
  second for a massive two-loop top sector in a release build.

```python
d, k, p, s = S("d", "k", "p", "s")
kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, s)
bubble = hep.IntegralFamily(
    [k], [p], [kin.scalar_product(k, k), kin.scalar_product(k - p, k - p)], kinematics=kin
)
ibp = hep.IBPFamily(bubble)
assert ibp.reduce_laporta([[3, 1], [1, 1]], max_depth=0).certify().masters == "incomplete"
stable = ibp.reduce_laporta([[3, 1]], max_depth=4, until_stable=True)
assert stable.residuals == [[1, 1]] and stable.stable_depth == 1
assert stable.certify().reduction == "verified"
```

A depth-zero search of `I(3,1)` alone keeps `I(3,1)`, a valid single master:
it is `"count-consistent"`, and only the deeper search or `until_stable`
reveals the conventional master `I(1,1)`.

## Build and validation

The companion `symbolica-community` host adds a native dependency on this crate
and calls `rustred_feynkit::register_hep_module(module)` in its HEP registration.
Build and install that host with its existing Maturin workflow. Its top-level
Symbolica patch must unify all participating crates to one kernel. Installing
the standalone RustRed wheel alone does not add the HEP classes.

From the RustRed checkout, using Python with the rebuilt community host:

```sh
python -m pytest crates/rustred-feynkit/tests
cargo test -p rustred --test feynkit_bridge -- --test-threads=1
cargo test -p rustred --test bridge_arity --test arity_dispatch -- --test-threads=1
```

The Python tests generate connected two-loop propagator and vertex graphs,
introduce two independent masses and external virtualities, and check native
identity conversion, both solving modes, exact analytic reductions,
reverse-unitarity cuts, tensor invariants, preferred masters, certificates,
and error handling. High-arity tests also check nonzero massive tadpole
reductions with linear auxiliary slots at arities 13, 14, and 15 and replay
their original-source certificates; these are API/algebra regressions, not
large-family performance evidence. Notebook
execution additionally needs IPython; interactive execution uses a normal
Python Jupyter kernel.

The [two-loop phi4 notebook](../../examples/notebooks/feyncalc_phi4_two_loop.ipynb)
reproduces the [FeynCalc Kira example](https://feyncalc.github.io/FeynCalcExamples/Phi4/TwoLoops/Renormalization-SS),
including the UV poles and renormalization constants. It verifies denominator
permutations with Feynkit, performs the reductions through RustRed, and uses
the reference's analytic master-integral expansions.
