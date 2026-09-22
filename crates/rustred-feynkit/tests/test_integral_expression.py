"""Native integral-expression output shares the bridge's existing guarded rules."""

import pytest
from symbolica import E, Expression, S
from symbolica.community import hep


@pytest.fixture(scope="module")
def tadpole():
    d, k, mass, integral = S(
        "expression::d", "expression::k", "expression::m2", "family_a::I"
    )
    kin = hep.Kinematics(d, momenta=[k])
    family = hep.IBPFamily(
        hep.IntegralFamily([k], [], [kin.scalar_product(k, k) - mass], kinematics=kin)
    )
    solution = family.solve_parametric([True], max_depth=1)
    return family, solution, d, mass, integral


def test_tadpole_native_expression_and_legacy_tuple_order(tadpole):
    _, solution, d, mass, integral = tadpole
    for operation in (solution.reduce, solution.rules[0].apply):
        terms = operation([2])
        assert isinstance(terms, list)
        assert len(terms) == 1 and terms[0][0] == [1]
        assert (terms[0][1] - (d - 2) / (2 * mass)).together() == E("0")
        assert operation([2], integral=None) == terms
        result = operation([2], integral=integral)
        assert isinstance(result, Expression)
        assert (result - (d - 2) / (2 * mass) * integral(1)).together() == E("0")
        # The native function head must retain its namespace, not just its name.
        other = S("family_b::I")
        assert result != operation([2], integral=other)
        assert result.replace(integral(1), other(1)) == operation([2], integral=other)
        # Application of a recurrence is not limited to compact solver indices.
        assert (
            operation([100], integral=integral)
            - (d - 198) / (198 * mass) * integral(99)
        ).together() == E("0")


def test_two_mass_bubble_returns_full_integral_sum():
    d, k, p, s, a, b, integral = S(
        "expression::D",
        "expression::q",
        "expression::p",
        "expression::s",
        "mass_a::m2",
        "mass_b::m2",
        "bubble::I",
    )
    kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, s)
    family = hep.IBPFamily(
        hep.IntegralFamily(
            [k],
            [p],
            [kin.scalar_product(k, k) - a, kin.scalar_product(k - p, k - p) - b],
            kinematics=kin,
        )
    )
    solution = family.reduce_laporta([[2, 1]], max_depth=2)
    # The two total-derivative identities in k and p, followed by the tadpole
    # recurrence, give this independently specified three-integral result.
    discriminant = (s - a - b) ** 2 - 4 * a * b
    expected = (
        (d - 3) * (a - b - s) * integral(1, 1)
        + (d - 2) * integral(0, 1)
        - (d - 2) * (a + b - s) / (2 * a) * integral(1, 0)
    ) / discriminant
    actual = solution.reduce([2, 1], integral=integral)
    assert (actual - expected).together() == E("0")
    rule = next(rule for rule in solution.rules if rule.target == [E("2"), E("1")])
    assert (rule.apply([2, 1], integral=integral) - expected).together() == E("0")


def test_unresolved_and_empty_solutions_keep_integrals_explicit(tadpole):
    family, generic, _, _, integral = tadpole
    empty = family.reduce_laporta([], max_depth=0, max_targets=0)
    unresolved = family.reduce_laporta([[1]], max_depth=0)
    assert [1] in unresolved.residuals
    for solution in (empty, unresolved, generic):
        assert solution.reduce([1], integral=integral) == integral(1)
        assert solution.reduce([1]) == [([1], E("1"))]
    assert empty.reduce([-2], integral=integral) == integral(-2)
    # Rule application still rejects exceptional indices and incompatible sectors.
    for powers in ([1], [0], [-1]):
        with pytest.raises(ValueError, match="does not apply"):
            generic.rules[0].apply(powers, integral=integral)


def test_zero_rules_return_native_zero_without_losing_conditions():
    d, k, scale, integral = S("zero::D", "zero::k", "zero::scale", "zero::I")
    kin = hep.Kinematics(d, momenta=[k])
    family = hep.IBPFamily(
        hep.IntegralFamily([k], [], [kin.scalar_product(k, k) / scale], kinematics=kin)
    )
    for solution in (
        family.solve_parametric([True], max_depth=0),
        family.reduce_laporta([[1]], max_depth=0),
    ):
        rule = solution.rules[0]
        conditions = rule.nonzero_conditions
        exceptions = rule.exceptions
        assert any(c.replace(scale, E("0")).expand() == E("0") for c in conditions)
        for operation in (solution.reduce, rule.apply):
            assert operation([1]) == []
            assert operation([1], integral=None) == []
            assert operation([1], integral=integral) == E("0")
            with pytest.raises(ValueError, match="bare Symbolica symbol"):
                operation([1], integral=integral(1))
        assert rule.nonzero_conditions == conditions
        assert rule.exceptions == exceptions


@pytest.mark.parametrize("bad_head", [E("0"), E("1"), S("bad::I")(1), S("bad::I") + 1])
def test_expression_heads_must_be_bare_symbols_even_for_empty_solution(
    tadpole, bad_head
):
    family, solution, _, _, _ = tadpole
    empty = family.reduce_laporta([], max_depth=0, max_targets=0)
    for operation in (solution.reduce, solution.rules[0].apply, empty.reduce):
        with pytest.raises(ValueError, match="bare Symbolica symbol"):
            operation([2], integral=bad_head)


def test_integral_head_is_keyword_only_and_requires_native_expression(tadpole):
    _, solution, _, _, integral = tadpole
    for operation in (solution.reduce, solution.rules[0].apply):
        with pytest.raises(TypeError):
            operation([2], integral)
        with pytest.raises(TypeError):
            operation([2], integral="I")
        with pytest.raises(ValueError, match="one integer power"):
            operation([2, 1], integral=integral)


@pytest.mark.parametrize("bad_power", [True, False, 1.5, "2", 2**100])
def test_expression_output_preserves_power_validation(tadpole, bad_power):
    _, solution, _, _, integral = tadpole
    for operation in (solution.reduce, solution.rules[0].apply):
        with pytest.raises((TypeError, ValueError, OverflowError)):
            operation([bad_power], integral=integral)
