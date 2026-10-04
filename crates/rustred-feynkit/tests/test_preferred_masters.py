"""Strict preferred masters: an exact basis change to the requested integrals."""

import pytest
from symbolica import E, S
from symbolica.community import hepkit as hep


def equal(left, right):
    assert (left - right).together() == E("0")


@pytest.fixture(scope="module")
def tadpole():
    d, k, m2, T = S("preferred::d", "preferred::k", "preferred::m2", "preferred::T")
    kin = hep.Kinematics(d, momenta=[k])
    family = hep.IntegralFamily(
        [k], [], [kin.scalar_product(k, k) - m2], kinematics=kin
    )
    return hep.IBPFamily(family), d, m2, T


@pytest.fixture(scope="module")
def bubble():
    """D1 = k.k - m2 and D2 = (k-p).(k-p), with p.p = s."""
    d, k, p, s, m2, I = S(
        "preferred::d",
        "preferred::k",
        "preferred::p",
        "preferred::s",
        "preferred::m2",
        "preferred::I",
    )
    kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, s)
    family = hep.IntegralFamily(
        [k],
        [p],
        [kin.scalar_product(k, k) - m2, kin.scalar_product(k - p, k - p)],
        kinematics=kin,
    )
    return family, I


def test_tadpole_reduces_to_the_preferred_dotted_master(tadpole):
    ibp, d, m2, T = tadpole
    solution = ibp.reduce_laporta([[1], [3]], max_depth=1, preferred_masters=[[2]])
    assert solution.residuals == [[2]]
    assert solution.preferred_masters == [([2], "replaced")]
    assert solution.replaced == [[1]]
    # T(n+1) = (d-2n)/(2 n m2) T(n) gives T(1) and T(3) in terms of T(2).
    equal(solution.reduce([1], integral=T), 2 * m2 / (d - 2) * T(2))
    equal(solution.reduce([3], integral=T), (d - 4) / (4 * m2) * T(2))
    assert solution.reduce([2], integral=T) == T(2)
    rule = next(r for r in solution.rules if r.target == [E("1")])
    assert any(
        (c - (d - 2)).expand() == E("0") or (c + (d - 2)).expand() == E("0")
        for c in rule.nonzero_conditions
    )


def test_bubble_preferred_master_maps_back_to_the_default_basis(bubble):
    family, I = bubble
    targets = [[1, 1], [1, 2], [1, 0]]
    default = hep.IBPFamily(family).reduce_laporta(targets + [[2, 1]], max_depth=1)
    solution = hep.IBPFamily(family).reduce_laporta(
        targets, max_depth=1, preferred_masters=[[2, 1]]
    )
    assert solution.residuals == [[1, 0], [2, 1]]
    assert solution.replaced == [[1, 1]]
    preferred = default.reduce([2, 1], integral=I)
    for target in targets:
        mapped = solution.reduce(target, integral=I).replace(I(2, 1), preferred)
        equal(mapped, default.reduce(target, integral=I))


def test_a_preferred_residual_keeps_the_default_reductions(bubble):
    family, I = bubble
    targets = [[2, 1], [1, 2]]
    default = hep.IBPFamily(family).reduce_laporta(targets, max_depth=1)
    solution = hep.IBPFamily(family).reduce_laporta(
        targets, max_depth=1, preferred_masters=[[1, 1]]
    )
    assert solution.preferred_masters == [([1, 1], "residual")]
    assert solution.replaced == []
    for target in targets:
        equal(solution.reduce(target, integral=I), default.reduce(target, integral=I))


def light_like_triangle():
    d, k, p1, p2, s = S(
        "triangle::d", "triangle::k", "triangle::p1", "triangle::p2", "triangle::s"
    )
    kin = (
        hep.Kinematics(d, momenta=[k, p1, p2])
        .with_scalar_product(p1, p1, E("0"))
        .with_scalar_product(p2, p2, E("0"))
        .with_scalar_product(p1, p2, s / 2)
    )
    denominators = [
        kin.scalar_product(k, k),
        kin.scalar_product(k - p1, k - p1),
        kin.scalar_product(k - p1 - p2, k - p1 - p2),
    ]
    return hep.IBPFamily(
        hep.IntegralFamily([k], [p1, p2], denominators, kinematics=kin)
    )


@pytest.mark.parametrize(
    ("case", "match"),
    [
        ("duplicate", "listed twice"),
        ("zero_sector", "zero sector"),
        ("outside_cut", "outside the cut"),
        ("dependent", "depends on preferred master"),
        ("lower_sectors", "lower-sector integrals"),
        ("arity", "coordinates"),
        ("range", "compact range"),
    ],
)
def test_strict_preferred_masters_explain_failures(bubble, tadpole, case, match):
    family, _ = bubble
    tadpole_ibp = tadpole[0]
    run = {
        "duplicate": lambda: tadpole_ibp.reduce_laporta(
            [[1]], preferred_masters=[[2], [2]]
        ),
        "zero_sector": lambda: tadpole_ibp.reduce_laporta(
            [[1]], preferred_masters=[[0]]
        ),
        "outside_cut": lambda: hep.IBPFamily(family, cut=[True, True]).reduce_laporta(
            [[2, 1]], preferred_masters=[[1, 0]]
        ),
        "dependent": lambda: hep.IBPFamily(family).reduce_laporta(
            [[1, 1]], max_depth=1, preferred_masters=[[2, 1], [1, 2]]
        ),
        "lower_sectors": lambda: light_like_triangle().reduce_laporta(
            [[1, 1, 1]], preferred_masters=[[1, 1, 1]]
        ),
        "arity": lambda: tadpole_ibp.reduce_laporta([[1]], preferred_masters=[[1, 1]]),
        "range": lambda: tadpole_ibp.reduce_laporta([[1]], preferred_masters=[[64]]),
    }[case]
    with pytest.raises(ValueError, match=match):
        run()


def test_preferred_masters_are_validated_and_budgeted(tadpole):
    ibp = tadpole[0]
    with pytest.raises(TypeError):
        ibp.reduce_laporta([[1]], preferred_masters=[[True]])
    with pytest.raises(ValueError, match="max_targets"):
        ibp.reduce_laporta([], max_depth=1, max_targets=0, preferred_masters=[[2]])
    assert ibp.reduce_laporta([[1]], max_depth=1).preferred_masters == []
