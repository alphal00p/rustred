"""Loop-independent scalar invariants are opaque IBP parameters with exact round trips."""

import re

import pytest
from symbolica import E, S
from symbolica.community import hepkit as hep
from symbolica.community.tensor import Representation, TensorName, dot


def coefficients(terms):
    result = {}
    for powers, coefficient in terms:
        key = tuple(powers)
        result[key] = result.get(key, E("0")) + coefficient
    return {key: value for key, value in result.items() if value != E("0")}


def equal(left, right):
    assert (left - right).together() == E("0")


def vector(d, label, name="hep_probe::q"):
    """A rank-one probe tensor such as q(1, mink(d))."""
    return TensorName.vector(name)(label, Representation.mink(dimension=d))


def bubble(d, value, mass=E("0"), namespace="invariant"):
    """[k.k - mass, (k-p).(k-p)] with p.p assigned to ``value``."""
    k, p = S(f"{namespace}::k", f"{namespace}::p")
    kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, value)
    family = hep.IntegralFamily(
        [k],
        [p],
        [kin.scalar_product(k, k) - mass, kin.scalar_product(k - p, k - p)],
        kinematics=kin,
    )
    return hep.IBPFamily(family), kin.scalar_product(p, p)


def generic_identities(ibp, replacements=()):
    """IBP rows with caller atoms and private index symbols renamed."""
    generic = [S(f"generic::n{i + 1}") for i in range(ibp.denominator_count)]
    replacements = [*replacements, *zip(ibp.index_symbols, generic)]

    def rename(expression):
        for old, new in replacements:
            expression = expression.replace(old, new)
        return expression

    return [
        [([rename(p) for p in powers], rename(c)) for powers, c in row]
        for row in ibp.ibp_identities()
    ]


@pytest.mark.parametrize("kind", ["square", "mixed", "scalar_function"])
def test_invariant_matches_scalar_control(kind):
    d, s = S("invariant::d", "invariant::s")
    q1, q2 = vector(d, 1), vector(d, 2)
    value = {
        # The tensor invariant itself, as passed to with_scalar_product.
        "square": dot(q1, q1),
        "mixed": dot(q2, q1),
        "scalar_function": S("invariant::g", is_scalar=True)(s),
    }[kind]
    tensor, invariant = bubble(d, value, namespace=f"invariant_{kind}")
    control, _ = bubble(d, s, namespace=f"invariant_{kind}_control")
    if kind != "scalar_function":
        assert invariant == value.to_expression()
    targets = [[1, 1], [2, 1]]
    actual = coefficients(tensor.reduce_laporta(targets).reduce([2, 1]))
    expected = coefficients(control.reduce_laporta(targets).reduce([2, 1]))
    assert set(actual) == set(expected) == {(1, 1)}
    equal(actual[1, 1], -(d - 3) / invariant)
    equal(actual[1, 1].replace(invariant, s), expected[1, 1])


def test_invariants_round_trip_through_identities_rules_and_conditions():
    d, s, m2 = S("round_trip::d", "round_trip::s", "round_trip::m2")
    mass = dot(vector(d, 3), vector(d, 3)).to_expression()
    tensor, invariant = bubble(
        d, dot(vector(d, 1), vector(d, 1)), mass=mass, namespace="round_trip"
    )
    control, _ = bubble(d, s, mass=m2, namespace="round_trip_control")
    # Whole invariant atoms are renamed, so d inside mink(d) is untouched.
    assert generic_identities(tensor, [(invariant, s), (mass, m2)]) == (
        generic_identities(control)
    )
    solution = tensor.reduce_laporta([[1, 1], [2, 1], [1, 0]], max_depth=1)
    rule = next(r for r in solution.rules if r.target == [E("2"), E("1")])
    conditions = rule.nonzero_conditions
    # Only the caller's atoms come back: d, and the invariants built on d.
    for expression in [c for _, c in rule.terms] + conditions:
        assert set(expression.get_all_symbols(False)) <= {d}
    reduced = coefficients(solution.reduce([2, 1]))
    assert set(reduced) == {(1, 1), (1, 0)}
    equal(reduced[1, 1], -(d - 3) / (invariant - mass))
    equal(reduced[1, 0], (d - 2) / (2 * mass * (invariant - mass)))
    # Guards s - m2 and m2 are reported in the caller's invariant atoms.
    assert any(c.replace(invariant, mass).expand() == E("0") for c in conditions)
    assert any(c.replace(mass, E("0")).expand() == E("0") for c in conditions)


def test_two_mass_bubble_with_tensor_mass_and_mixed_invariant():
    d, b, integral = S("two_mass_tensor::d", "two_mass_tensor::b", "two_mass_tensor::I")
    k, p = S("two_mass_tensor::k", "two_mass_tensor::p")
    qa, qb = vector(d, 1, "hep_probe::qa"), vector(d, 1, "hep_probe::qb")
    a = dot(qa, qa).to_expression()
    kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, dot(qa, qb))
    s = kin.scalar_product(p, p)
    family = hep.IBPFamily(
        hep.IntegralFamily(
            [k],
            [p],
            [kin.scalar_product(k, k) - a, kin.scalar_product(k - p, k - p) - b],
            kinematics=kin,
        )
    )
    # test_integral_expression.py's three-integral result, with a and s tensors.
    discriminant = (s - a - b) ** 2 - 4 * a * b
    expected = (
        (d - 3) * (a - b - s) * integral(1, 1)
        + (d - 2) * integral(0, 1)
        - (d - 2) * (a + b - s) / (2 * a) * integral(1, 0)
    ) / discriminant
    actual = family.reduce_laporta([[2, 1]], max_depth=2).reduce(
        [2, 1], integral=integral
    )
    equal(actual, expected)


def test_tensor_invariant_mass_keeps_the_tadpole_recurrence():
    d, k = S("invariant_tadpole::d", "invariant_tadpole::k")
    mass = dot(vector(d, 1), vector(d, 1)).to_expression()
    kin = hep.Kinematics(d, momenta=[k])
    family = hep.IBPFamily(
        hep.IntegralFamily([k], [], [kin.scalar_product(k, k) - mass], kinematics=kin)
    )
    parametric = family.solve_parametric([True], max_depth=1)
    for solution in (parametric, family.reduce_laporta([[2]], max_depth=1)):
        terms = solution.reduce([2])
        assert terms[0][0] == [1]
        equal(terms[0][1], (d - 2) / (2 * mass))
    terms = parametric.rules[0].apply([100])
    assert terms[0][0] == [99]
    equal(terms[0][1], (d - 198) / (198 * mass))
    assert any(
        c.replace(mass, E("0")).expand() == E("0")
        for c in parametric.rules[0].nonzero_conditions
    )


def test_index_symbols_cannot_hide_inside_an_invariant():
    d, k = S("hidden_index::d", "hidden_index::k")
    kin = hep.Kinematics(d, momenta=[k])

    def tadpole(mass):
        return hep.IBPFamily(
            hep.IntegralFamily(
                [k], [], [kin.scalar_product(k, k) - mass], kinematics=kin
            )
        )

    # Families take index scopes in order; predict the next one and hide its
    # index inside an opaque mass. Rule application must not substitute it.
    def scope(family):
        index = family.index_symbols[0].to_canonical_string()
        return int(re.search(r"indices_(\d+)::", index).group(1))

    hidden_scope = scope(tadpole(S("hidden_index::m2"))) + 1
    hidden = S(f"rustred_feynkit::indices_{hidden_scope}::n1")
    mass = S("hidden_index::w", is_scalar=True)(hidden)
    family = tadpole(mass)
    # The colliding scope was skipped, not merely missed.
    assert scope(family) == hidden_scope + 1
    terms = family.solve_parametric([True], max_depth=1).rules[0].apply([5])
    assert terms[0][0] == [4]
    equal(terms[0][1], (d - 8) / (8 * mass))


def rejected_family(case):
    """Assign p1.p2 to the tested value; it enters only the Gram matrix.

    FeynKit itself rejects a loop-dependent value that reaches a denominator,
    so only a Gram-only entry exercises the bridge's own guard.
    """
    d, k, p1, p2, s, a = S(
        "reject::d", "reject::k", "reject::p1", "reject::p2", "reject::s", "reject::a"
    )
    kin = hep.Kinematics(d, momenta=[k, p1, p2])
    q1, q2 = vector(d, 1, "hep_probe::reject_q"), vector(d, 2, "hep_probe::reject_q")
    edge = hep.Symbols.edge_momentum()(2)
    value = {
        # FeynKit accepts these families, but each Gram entry depends on the
        # loop momentum, so treating it as a constant would be wrong.
        "loop_product": lambda: kin.scalar_product(k, k),
        "bare_loop": lambda: k,
        "scalar_of_loop": lambda: S("reject::g", is_scalar=True)(k),
        # A routed edge momentum Q(e) may hide any loop momentum.
        "edge_momentum": lambda: kin.scalar_product(edge, edge),
        "vector": lambda: q1,
        "function": lambda: S("reject::f")(s),
        "log": lambda: E("log(reject::s)"),
        "builtin_namespace": lambda: S("symbolica::bridge_probe", is_scalar=True)(s),
        # sqrt and exp normalize to s^(1/2) and e^s, not to functions.
        "sqrt": lambda: E("sqrt(reject::s)"),
        "exp": lambda: E("exp(reject::s)"),
        "root_of_invariant": lambda: dot(q1, q1).to_expression() ** E("1/2"),
        "symbolic_exponent": lambda: dot(q1, q1).to_expression() ** d,
        # Only Scalar factors leave a linear function, so a plain factor would
        # make dot(a*q, q) a second parameter for a*dot(q, q).
        "linear_factor": lambda: dot(a * q1, q1),
        "linear_sum_factor": lambda: dot(q1 + a * q2, q1),
    }[case]()
    kin = (
        kin.with_scalar_product(p1, p1, s)
        .with_scalar_product(p2, p2, s)
        .with_scalar_product(p1, p2, value)
    )
    return hep.IntegralFamily(
        [k],
        [p1, p2],
        [
            kin.scalar_product(k, k),
            kin.scalar_product(k - p1, k - p1),
            kin.scalar_product(k - p2, k - p2),
        ],
        kinematics=kin,
    )


@pytest.mark.parametrize(
    ("case", "match"),
    [
        ("loop_product", "loop momentum"),
        ("bare_loop", "loop momentum"),
        ("scalar_of_loop", "loop momentum"),
        ("edge_momentum", "routed edge momentum"),
        ("vector", "declared Scalar"),
        ("function", "declared Scalar"),
        ("log", "Symbolica built-in"),
        ("builtin_namespace", "Symbolica built-in"),
        ("sqrt", "non-integer or symbolic exponent"),
        ("exp", "non-integer or symbolic exponent"),
        ("root_of_invariant", "non-integer or symbolic exponent"),
        ("symbolic_exponent", "non-integer or symbolic exponent"),
        ("linear_factor", "keeps a factor"),
        ("linear_sum_factor", "keeps a factor"),
    ],
)
def test_invariants_must_be_loop_free_scalars(case, match):
    family = rejected_family(case)
    assert family.is_complete and family.is_independent
    with pytest.raises(ValueError, match=match):
        hep.IBPFamily(family)


def test_unassigned_external_scalar_products_must_still_be_assigned():
    d, k, p = S("unassigned::d", "unassigned::k", "unassigned::p")
    kin = hep.Kinematics(d, momenta=[k, p])
    family = hep.IntegralFamily(
        [k],
        [p],
        [kin.scalar_product(k, k), kin.scalar_product(k - p, k - p)],
        kinematics=kin,
    )
    with pytest.raises(ValueError, match="assign all external scalar products"):
        hep.IBPFamily(family)


def test_labeled_momenta_are_matched_by_their_labels():
    """FeynKit names momenta K(i) and P(i); only the family's own labels count."""
    K, P = hep.Kinematics.loop_momentum(), hep.Kinematics.external_momentum()
    d, s = S("labeled::d", "labeled::s")

    def bubble_family(loop, external, value):
        kin = hep.Kinematics(d, momenta=[loop, external])
        if value is not None:
            kin = kin.with_scalar_product(external, external, value)
        denominators = [
            kin.scalar_product(loop, loop),
            kin.scalar_product(loop - external, loop - external),
        ]
        return hep.IntegralFamily([loop], [external], denominators, kinematics=kin)

    # Loop by loop, K(1) is external to the inner family: its square must be
    # assigned, and K(0) does not make it a loop momentum.
    with pytest.raises(ValueError, match="assign all external scalar products"):
        hep.IBPFamily(bubble_family(K(0), K(1), None))
    # K(1) is not a momentum of this family, so K(1).K(1) is an invariant.
    outer = hep.Kinematics(d, momenta=[K(1)]).scalar_product(K(1), K(1))
    ibp = hep.IBPFamily(bubble_family(K(0), P(0), outer))
    reduced = coefficients(ibp.reduce_laporta([[1, 1], [2, 1]]).reduce([2, 1]))
    assert set(reduced) == {(1, 1)}
    equal(reduced[1, 1], -(d - 3) / outer)
