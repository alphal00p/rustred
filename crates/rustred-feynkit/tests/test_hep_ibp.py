"""Exercise the installed community bridge, including generated two-loop graphs."""

from pathlib import Path

import pytest
from symbolica import E, Expression, S
from symbolica.community import hep

MODEL = Path(__file__).parent / "fixtures" / "scalar_phi3.json"


def coefficients(terms):
    result = {}
    for powers, coefficient in terms:
        key = tuple(powers)
        result[key] = result.get(key, E("0")) + coefficient
    return {key: value for key, value in result.items() if value != E("0")}


def equal(left, right):
    assert (left - right).together() == E("0")


@pytest.fixture(params=[2, 3], ids=["propagator", "vertex"])
def graph_family(request):
    """Connected two-loop graphs, two masses and all external virtualities."""
    legs = request.param
    model = hep.Model(str(MODEL))
    diagrams = model.generate_diagrams(
        ["scalar_0"],
        ["scalar_0"] * (legs - 1),
        loops=2,
        max_vertices=legs + 2,
        allow_self_loops=False,
        threads=1,
    ).diagrams
    diagram = next(g for g in diagrams if g.integral_family().is_independent)
    original = diagram.integral_family()
    d, s, t, u, ma, mb = S(
        "ibp_test::d",
        "ibp_test::s",
        "ibp_test::t",
        "ibp_test::u",
        "mass_a::m2",
        "mass_b::m2",
    )
    external = original.external_momenta
    kin = hep.Kinematics(d, momenta=original.loop_momenta + external)
    kin = kin.with_scalar_product(external[0], external[0], s)
    if legs == 3:
        kin = kin.with_scalar_product(external[1], external[1], t)
        kin = kin.with_scalar_product(external[0], external[1], (u - s - t) / 2)
    routed = diagram.integral_family(kinematics=kin)
    # Assign two independent masses to the graph's ordered internal edges.
    physical = len(routed.denominators)
    family = hep.IntegralFamily(
        routed.loop_momenta,
        external,
        [den - (ma if i % 2 else mb) for i, den in enumerate(routed.denominators)],
        kinematics=kin,
    ).complete()
    assert diagram.loop_count == 2
    assert len(family.denominators) == (5 if legs == 2 else 7)
    return family, physical


def test_generated_multiscale_graph_parametric(graph_family):
    family, physical = graph_family
    ibp = hep.IBPFamily(family)
    arity = len(family.denominators)
    identities = ibp.ibp_identities()
    assert len(identities) == 2 * (2 + len(family.external_momenta))
    assert all(isinstance(c, Expression) for row in identities for _, c in row)
    # Independently differentiate the original graph denominators. Dividing
    # an IBP row by I(n) replaces I(n+shift) with prod(D**(-shift)); this
    # checks the conversion's LL/LE coordinate order, Gram data and signs.
    kin = family.kinematics
    k, q = family.loop_momenta
    products = [
        (kin.scalar_product(k, k), 2 * kin.scalar_product(k, k)),
        (kin.scalar_product(k, q), kin.scalar_product(k, q)),
    ]
    products += [
        (kin.scalar_product(k, p), kin.scalar_product(k, p))
        for p in family.external_momenta
    ]
    indices = ibp.index_symbols
    expected = kin.dimension
    for n, den in zip(indices, family.denominators):
        contraction = sum(
            (den.expand().coefficient(sp) * derivative for sp, derivative in products),
            E("0"),
        )
        expected -= n * contraction / den
    actual = E("0")
    for powers, coefficient in identities[0]:
        term = coefficient
        for den, power, index in zip(family.denominators, powers, indices):
            term *= den ** int(str((index - power).expand()))
        actual += term
    equal(actual, expected)
    solution = ibp.solve_parametric(
        [True] * physical + [False] * (arity - physical), max_depth=1
    )
    assert solution.rules
    powers = [3] * physical + [-2] * (arity - physical)
    reduced = coefficients(solution.rules[0].apply(powers))
    assert tuple(powers) not in reduced
    assert reduced
    assert solution.rules[0].nonzero_conditions or solution.rules[0].exceptions


def test_generated_multiscale_graph_laporta(graph_family):
    family, physical = graph_family
    ibp = hep.IBPFamily(family)
    arity = len(family.denominators)
    corner = [1] * physical + [0] * (arity - physical)
    target = corner.copy()
    target[0] = 2
    solution = ibp.reduce_laporta([corner, target], max_depth=0)
    reduced = coefficients(solution.reduce(target))
    assert tuple(target) not in reduced
    assert reduced
    assert solution.stats["rows"] > 0
    assert set(reduced) <= {tuple(p) for p in solution.residuals}


def test_exact_two_mass_pinch_and_namespace_preservation():
    d, k, q, ma, mb = S("pinch::d", "pinch::k", "pinch::q", "a::m2", "b::m2")
    kin = hep.Kinematics(d, momenta=[k, q])
    family = hep.IntegralFamily(
        [k, q],
        [],
        [
            kin.scalar_product(k, k) - ma,
            kin.scalar_product(q, q) - mb,
            kin.scalar_product(k - q, k - q) - ma,
        ],
        kinematics=kin,
    )
    ibp = hep.IBPFamily(family)
    solution = ibp.reduce_laporta([[2, 2, 0]], max_depth=1)
    reduced = coefficients(solution.reduce([2, 2, 0]))
    assert set(reduced) == {(1, 1, 0)}
    equal(reduced[1, 1, 0], (d - 2) ** 2 / (4 * ma * mb))
    assert solution.reduce([1, 0, 0]) == [([1, 0, 0], E("1"))]
    # An unrequested integral is left alone; an explicitly requested scaleless
    # sector is recognized and reduced to zero by the native sector analyzer.
    zero = ibp.reduce_laporta([[1, 0, 0]], max_depth=0)
    assert zero.reduce([1, 0, 0]) == []


def test_input_validation_and_exceptional_indices():
    d, k, m = S("guard::d", "guard::k", "guard::m2")
    kin = hep.Kinematics(d, momenta=[k])
    den = kin.scalar_product(k, k) - m
    family = hep.IntegralFamily([k], [], [den], kinematics=kin)
    ibp = hep.IBPFamily(family)
    generic = ibp.solve_parametric([True], max_depth=1)
    equal(coefficients(generic.reduce([2]))[1,], (d - 2) / (2 * m))
    with pytest.raises(ValueError, match="exceptional"):
        generic.rules[0].apply([1])
    with pytest.raises(ValueError):
        ibp.reduce_laporta([[1, 2]])
    with pytest.raises((ValueError, OverflowError)):
        ibp.solve_parametric([True], max_depth=-1)
    with pytest.raises(ValueError, match="dependent"):
        hep.IBPFamily(hep.IntegralFamily([k], [], [den, den + m], kinematics=kin))
    incomplete = hep.IntegralFamily([k], [S("guard::p")], [den], kinematics=kin)
    with pytest.raises(ValueError, match="complete"):
        hep.IBPFamily(incomplete)
