"""Reverse-unitarity cuts: integrals with a nonpositive power on a cut line vanish."""

import pytest
from symbolica import E, S
from symbolica.community import hepkit as hep


def coefficients(terms):
    result = {}
    for powers, coefficient in terms:
        key = tuple(powers)
        result[key] = result.get(key, E("0")) + coefficient
    return {key: value for key, value in result.items() if value != E("0")}


def equal(left, right):
    assert (left - right).together() == E("0")


BOTH, MASSLESS_LINE, MASSIVE_LINE = [True, True], [False, True], [True, False]
CUTS = pytest.mark.parametrize(
    "cut", [BOTH, MASSLESS_LINE, MASSIVE_LINE], ids=["both", "massless", "massive"]
)


@pytest.fixture(scope="module")
def bubble():
    """D1 = k.k - m2 and D2 = (k-p).(k-p), with p.p = s."""
    d, k, p, s, m2 = S("cut::d", "cut::k", "cut::p", "cut::s", "cut::m2")
    kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, s)
    family = hep.IntegralFamily(
        [k],
        [p],
        [kin.scalar_product(k, k) - m2, kin.scalar_product(k - p, k - p)],
        kinematics=kin,
    )
    # With IBP_v = d/dk . v, the two ordinary identities are
    #   IBP_k: (d-2n1-n2) I - 2 m2 n1 1+ - n2 1-2+ + n2 (s-m2) 2+ = 0,
    #   IBP_p: (n2-n1) I - n1 (s+m2) 1+ + n1 1+2- - n2 1-2+ + n2 (s-m2) 2+ = 0.
    # Their difference at (1,1) is (d-3) I(1,1) + (s-m2) I(2,1) - I(2,0) = 0,
    # and IBP_k at (1,0) is (d-2) I(1,0) = 2 m2 I(2,0). Hence
    #   I(2,1) = master I(1,1) + tadpole I(1,0).
    master = -(d - 3) / (s - m2)
    tadpole = (d - 2) / (2 * m2 * (s - m2))
    return family, d, s, m2, master, tadpole


@pytest.mark.parametrize(
    "options",
    [{}, {"cut": None}, {"cut": [False, False]}],
    ids=["omitted", "none", "empty"],
)
def test_uncut_control_keeps_the_massive_tadpole(bubble, options):
    family, _, _, _, master, tadpole = bubble
    ibp = hep.IBPFamily(family, **options)
    assert ibp.cut == [False, False]
    assert "cut" not in repr(ibp)
    solution = ibp.reduce_laporta([[1, 1], [2, 1], [1, 0]], max_depth=1)
    reduced = coefficients(solution.reduce([2, 1]))
    assert set(reduced) == {(1, 1), (1, 0)}
    equal(reduced[1, 1], master)
    equal(reduced[1, 0], tadpole)
    assert solution.reduce([1, 0]) == [([1, 0], E("1"))]
    assert solution.residuals == [[1, 0], [1, 1]]


def test_two_cut_bubble_drops_the_tadpole(bubble):
    family, d, s, m2, _, _ = bubble
    integral = S("cut::I")
    ibp = hep.IBPFamily(family, cut=BOTH)
    assert repr(ibp).endswith("cut=[True, True])")
    solution = ibp.reduce_laporta([[1, 1], [2, 1], [1, 0]], max_depth=1)
    equal(
        solution.reduce([2, 1], integral=integral), -(d - 3) / (s - m2) * integral(1, 1)
    )
    assert solution.reduce([1, 0], integral=integral) == E("0")
    assert solution.residuals == [[1, 1]]


@pytest.mark.parametrize(
    ("cut", "tadpole_survives"),
    [(BOTH, False), (MASSLESS_LINE, False), (MASSIVE_LINE, True)],
    ids=["both", "massless", "massive"],
)
def test_cut_reductions_drop_only_integrals_outside_cut_support(
    bubble, cut, tadpole_survives
):
    family, d, s, m2, master, tadpole = bubble
    ibp = hep.IBPFamily(family, cut=cut)
    assert ibp.cut == cut
    solution = ibp.reduce_laporta([[1, 1], [2, 1], [1, 2], [1, 0]], max_depth=1)
    # A cut on D2 removes I(2,0) and I(1,0) from both relations above.
    # A cut on D1 alone keeps them: D1 has a positive power in both, and the
    # only D1-vanishing integral, I(0,2), cancels from IBP_k - IBP_p.
    # IBP_p at (1,1) also gives (s-m2) I(1,2) = (s+m2) I(2,1) - I(2,0) + I(0,2).
    expected = {
        (2, 1): {(1, 1): master},
        (1, 2): {(1, 1): -(d - 3) * (s + m2) / (s - m2) ** 2},
    }
    if tadpole_survives:
        expected[2, 1][1, 0] = tadpole
        expected[1, 2][1, 0] = (d - 2) / (s - m2) ** 2
    for target, terms in expected.items():
        reduced = coefficients(solution.reduce(list(target)))
        assert set(reduced) == set(terms), target
        for key, value in terms.items():
            equal(reduced[key], value)
    if tadpole_survives:
        # The cut massive tadpole is a genuine master, not a zero.
        assert solution.reduce([1, 0]) == [([1, 0], E("1"))]
        assert solution.residuals == [[1, 0], [1, 1]]
    else:
        assert solution.reduce([1, 0]) == []
        assert solution.residuals == [[1, 1]]


@pytest.mark.parametrize("outside", [[1, 0], [3, -1], [0, 1], [-2, 2], [0, 0]])
def test_requested_targets_outside_cut_support_are_zero_rules(bubble, outside):
    family, *_ = bubble
    integral = S("cut::I")
    ibp = hep.IBPFamily(family, cut=BOTH)
    for solution in (
        ibp.reduce_laporta([outside], max_depth=0, max_targets=0),
        ibp.reduce_laporta([[1, 1], [2, 1], outside], max_depth=1),
    ):
        assert solution.reduce(outside) == []
        assert solution.reduce(outside, integral=integral) == E("0")
        assert outside not in solution.residuals
        rule = next(
            r for r in solution.rules if r.target == [E(str(v)) for v in outside]
        )
        assert rule.terms == [] and rule.nonzero_conditions == []
        assert rule.apply(outside) == []


def test_unrequested_integrals_outside_cut_support_are_zero(bubble):
    # Cut support is definitional, unlike a scaleless-sector proof, so
    # reduce() zeros it without a requested target.
    family, *_ = bubble
    ibp = hep.IBPFamily(family, cut=BOTH)
    empty = ibp.reduce_laporta([], max_depth=0, max_targets=0)
    assert empty.reduce([1, 0]) == []
    assert empty.reduce([5, -3]) == []
    assert empty.reduce([1, 1]) == [([1, 1], E("1"))]


@pytest.mark.parametrize(
    ("cut", "boundary_survives"),
    [(BOTH, False), (MASSLESS_LINE, False), (MASSIVE_LINE, True)],
    ids=["both", "massless", "massive"],
)
def test_cut_parametric_recurrence_at_fixed_cut_power(bubble, cut, boundary_survives):
    family, d, s, m2, master, _ = bubble
    ibp = hep.IBPFamily(family, cut=cut)
    rule = ibp.solve_parametric([True, True], fixed=[None, 1], max_depth=1).rules[0]
    # (IBP_k - IBP_p) at (n1-1, 1):
    #   I(n1,1) = -(d-1-n1)/((n1-1)(s-m2)) I(n1-1,1) + 1/(s-m2) I(n1,0).
    # A cut on D2 removes the fixed boundary term I(n1,0) during discovery.
    lines = [i for i, flag in enumerate(cut) if flag]
    for n1 in (2, 3, 4):
        terms = rule.apply([n1, 1])
        assert all(powers[i] > 0 for powers, _ in terms for i in lines)
        expected = {(n1 - 1, 1): -(d - 1 - n1) / ((n1 - 1) * (s - m2))}
        if boundary_survives:
            expected[n1, 0] = 1 / (s - m2)
        reduced = coefficients(terms)
        assert set(reduced) == set(expected)
        for key, value in expected.items():
            equal(reduced[key], value)
    equal(coefficients(rule.apply([2, 1]))[1, 1], master)


def test_cut_parametric_rule_never_returns_integrals_outside_cut_support(bubble):
    family, *_ = bubble
    # Control: one uncut step from I(1,3) lowers n1 to the D1-cut-excluded I(0,3).
    uncut = hep.IBPFamily(family).solve_parametric([True, True], max_depth=1)
    assert (0, 3) in coefficients(uncut.reduce([1, 3]))
    ibp = hep.IBPFamily(family, cut=BOTH)
    terms = ibp.solve_parametric([True, True], max_depth=1).reduce([1, 3])
    assert all(power > 0 for powers, _ in terms for power in powers)
    step = coefficients(terms)
    assert step and (1, 3) not in step
    # The step is an identity among cut integrals: closing it with an
    # independent cut Laporta reduction reproduces the direct reduction.
    laporta = ibp.reduce_laporta([[1, 1], [1, 3], *map(list, step)], max_depth=1)
    direct = coefficients(laporta.reduce([1, 3]))
    closed = {}
    for powers, coefficient in step.items():
        for key, value in coefficients(laporta.reduce(list(powers))).items():
            closed[key] = closed.get(key, E("0")) + coefficient * value
    assert set(direct) == set(closed) == {(1, 1)}
    equal(direct[1, 1], closed[1, 1])


@pytest.mark.parametrize("fixed", [None, [None, 0]])
def test_cut_parametric_sector_outside_cut_support_is_a_zero_rule(bubble, fixed):
    family, *_ = bubble
    # Uncut, the numerator sector of the massive tadpole has a real recurrence.
    uncut = hep.IBPFamily(family).solve_parametric([True, False], max_depth=1)
    assert uncut.rules[0].terms
    ibp = hep.IBPFamily(family, cut=MASSLESS_LINE)
    zero = ibp.solve_parametric([True, False], fixed=fixed, max_depth=1)
    assert len(zero.rules) == 1
    assert zero.rules[0].sector == [True, False]
    assert zero.rules[0].terms == []
    for powers in ([1, 0], [4, 0]):
        assert zero.reduce(powers) == []
    with pytest.raises(ValueError, match="outside its sector"):
        ibp.solve_parametric([True, True], fixed=[None, 0], max_depth=0)


@CUTS
def test_cuts_leave_the_ibp_identities_unchanged(bubble, cut):
    family, *_ = bubble
    uncut = hep.IBPFamily(family)
    ibp = hep.IBPFamily(family, cut=cut)

    def rename(expression):
        for old, new in zip(ibp.index_symbols, uncut.index_symbols):
            expression = expression.replace(old, new)
        return expression

    assert [
        [([rename(p) for p in powers], rename(c)) for powers, c in row]
        for row in ibp.ibp_identities()
    ] == uncut.ibp_identities()


@pytest.mark.parametrize(
    ("cut", "error", "match"),
    [
        ([True], ValueError, "cut has 1 coordinates; expected 2"),
        ([True, True, False], ValueError, "expected 2"),
        ([1, 0], TypeError, None),
        ([0.5, True], TypeError, None),
        (True, TypeError, None),
    ],
)
def test_cut_masks_are_validated(bubble, cut, error, match):
    family, *_ = bubble
    with pytest.raises(error, match=match):
        hep.IBPFamily(family, cut=cut)
