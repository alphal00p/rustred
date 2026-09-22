# RustRed–Feynkit IBP bridge

`rustred-feynkit` embeds RustRed in the same native Symbolica kernel as
Feynkit. The community host registers `IBPFamily`, `IBPRule`, and
`IBPSolution` in `symbolica.community.hep`. No standalone `rustred` Python
extension, Kira executable, or expression string conversion is involved.

```python
from symbolica import S
from symbolica.community import hep

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
Provide independent external momenta and their scalar products in
`Kinematics`. Masses, invariants, and the dimension can be arbitrary rational
functions of scalar Symbolica symbols, including namespaced symbols.
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

`reduce_laporta(targets, max_depth=2, max_targets=1024, include_lorentz=False)`
searches integer seed neighborhoods, exactly replays the discovered rules,
follows their right-hand sides, and back-substitutes the solved integrals.
`max_depth` bounds the signed L1 seed radius; `max_targets` bounds the number
of distinct integrals searched. Exhausting the latter raises an error.
`residuals` explicitly lists the remaining basis at that search depth; these
are not certified independent masters. Increasing depth can reduce the basis
further. Unmatched integrals passed to `solution.reduce` remain unchanged.
Scaleless subsectors are removed only after RustRed's sector analyzer proves
them zero. Family symmetries can further identify residuals using Feynkit's
verified momentum maps, as demonstrated in the notebook.

The runtime adapter accepts 1–12 denominators. Concrete search targets and
fixed coordinates must lie in RustRed's compact power range `-64..63`;
applying a discovered symbolic recurrence accepts signed 64-bit powers.
It keeps all kinematic scales
symbolic and uses the existing native exact sparse backend. RustRed's separate
experimental reconstruction backends depend on newer vendored Symbolica APIs;
the core's default `reconstruction` feature keeps those available to existing
RustRed users, while this bridge disables that feature to share the community
host's pinned Symbolica version.

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
```

The Python tests generate connected two-loop propagator and vertex graphs,
introduce two independent masses and external virtualities, and check native
identity conversion, both solving modes, exact analytic reductions, and error
handling. Notebook execution additionally needs IPython; interactive execution
uses a normal Python Jupyter kernel.

The [two-loop phi4 notebook](../../examples/notebooks/feyncalc_phi4_two_loop.ipynb)
reproduces the [FeynCalc Kira example](https://feyncalc.github.io/FeynCalcExamples/Phi4/TwoLoops/Renormalization-SS),
including the UV poles and renormalization constants. It verifies denominator
permutations with Feynkit, performs the reductions through RustRed, and uses
the reference's analytic master-integral expansions.
