"""Certificates: exact replay, master counts, and the stable search depth."""

import pytest
from symbolica import E, S
from symbolica.community import hepkit as hep


def bubble(masses=(E("0"), E("0")), invariant=None, namespace="certificate"):
    d, k, p, s = S(
        f"{namespace}::d", f"{namespace}::k", f"{namespace}::p", f"{namespace}::s"
    )
    kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(
        p, p, s if invariant is None else invariant
    )
    family = hep.IntegralFamily(
        [k],
        [p],
        [
            kin.scalar_product(k, k) - masses[0],
            kin.scalar_product(k - p, k - p) - masses[1],
        ],
        kinematics=kin,
    )
    return family


def test_ludy_bubble_shallow_search_is_verified_but_only_count_consistent():
    ibp = hep.IBPFamily(bubble())
    shallow = ibp.reduce_laporta([[3, 1]], max_depth=0)
    certificate = shallow.certify()
    assert shallow.residuals == [[3, 1]]
    # I(3,1) is a valid single master, so the count cannot object to it.
    assert (certificate.reduction, certificate.masters) == (
        "verified",
        "count-consistent",
    )
    assert certificate.master_counts == {(True, True): 1}
    assert certificate.residual_counts == {(True, True): 1}
    assert certificate.stable_depth is None
    # The stable search reaches the conventional master.
    stable = ibp.reduce_laporta([[3, 1]], max_depth=4, until_stable=True)
    assert (stable.residuals, stable.stable_depth, stable.depth) == ([[1, 1]], 1, 3)
    assert stable.certify().stable_depth == 1
    assert repr(stable.certify()) == (
        "IBPCertificate(reduction='verified', masters='count-consistent', "
        "excess_sectors=0, stable_depth=1)"
    )


def test_more_residuals_than_masters_is_incomplete():
    ibp = hep.IBPFamily(bubble())
    shallow = ibp.reduce_laporta([[3, 1], [1, 1]], max_depth=0)
    certificate = shallow.certify(replay=False)
    assert shallow.residuals == [[1, 1], [3, 1]]
    assert certificate.masters == "incomplete"
    assert certificate.excess_sectors == [[True, True]]
    assert certificate.reduction == "unchecked"
    deeper = ibp.reduce_laporta([[3, 1], [1, 1]], max_depth=1).certify()
    assert (deeper.masters, deeper.excess_sectors) == ("count-consistent", [])


def test_singular_gram_gives_no_verdict_instead_of_a_false_excess():
    m2 = S("certificate::m2")
    ibp = hep.IBPFamily(bubble(masses=(m2, m2), invariant=E("0"), namespace="gram"))
    certificate = ibp.reduce_laporta([[2, 1]], max_depth=2).certify()
    assert (certificate.masters, certificate.excess_sectors) == ("no-verdict", [])
    assert set(certificate.no_verdict) == set(certificate.residual_counts)
    assert set(certificate.no_verdict.values()) == {"singular external Gram matrix"}
    assert set(certificate.master_counts.values()) == {None}


def test_certificates_cover_preferred_masters_cuts_and_seeds():
    m2 = S("certificate::m2")
    family = bubble(masses=(m2, E("0")), namespace="covered")
    preferred = hep.IBPFamily(family).reduce_laporta(
        [[1, 1], [1, 2]], max_depth=1, preferred_masters=[[2, 1]]
    )
    certificate = preferred.certify(seed=7)
    assert (certificate.reduction, certificate.masters) == (
        "verified",
        "count-consistent",
    )
    assert certificate.seed == 7 and certificate.replayed_rules > 0
    cut = hep.IBPFamily(family, cut=[True, True]).reduce_laporta(
        [[2, 1], [1, 0]], max_depth=1
    )
    cut_certificate = cut.certify()
    assert (cut_certificate.reduction, cut_certificate.masters) == (
        "verified",
        "count-consistent",
    )
    # Inside the cut the uncut count applies: one master in the bubble sector.
    assert cut.residuals == [[1, 1]]
    assert cut_certificate.master_counts == {(True, True): 1}
    assert cut.certify(count_masters=False).masters == "unchecked"
    assert "IBPCertificate(" in repr(cut.certify(count_masters=False))


def test_parametric_solutions_are_not_certified():
    ibp = hep.IBPFamily(bubble())
    parametric = ibp.solve_parametric([True, True], max_depth=1)
    assert parametric.depth is None and parametric.stable_depth is None
    with pytest.raises(ValueError, match="requires a reduce_laporta solution"):
        parametric.certify()


def test_integrals_proved_zero_by_ibp_are_certified():
    d, k, p, s, m2 = S("zero::d", "zero::k", "zero::p", "zero::s", "zero::m2")
    kin = hep.Kinematics(d, momenta=[k, p]).with_scalar_product(p, p, s)
    family = hep.IntegralFamily(
        [k], [p], [kin.scalar_product(k, k) - m2], kinematics=kin
    ).complete()
    ibp = hep.IBPFamily(family)
    for targets, depth in (([[1, -1]], 1), ([[2, -1], [1, -2]], 2)):
        solution = ibp.reduce_laporta(targets, max_depth=depth)
        # Odd powers of k.p vanish; I(1,-2) does not.
        assert solution.reduce(targets[0]) == []
        assert solution.certify(count_masters=False).reduction == "verified"


def test_sunrise_with_several_preferred_masters_is_certified():
    d, k1, k2, p, s, m2 = S(
        "sunrise::d",
        "sunrise::k1",
        "sunrise::k2",
        "sunrise::p",
        "sunrise::s",
        "sunrise::m2",
    )
    kin = hep.Kinematics(d, momenta=[k1, k2, p]).with_scalar_product(p, p, s)
    family = hep.IntegralFamily(
        [k1, k2],
        [p],
        [
            kin.scalar_product(k1, k1) - m2,
            kin.scalar_product(k2, k2) - m2,
            kin.scalar_product(k1 + k2 - p, k1 + k2 - p) - m2,
        ],
        kinematics=kin,
    ).complete()
    solution = hep.IBPFamily(family).reduce_laporta(
        [[2, 1, 1, 0, 0], [1, 1, 1, 0, 0]],
        max_depth=1,
        preferred_masters=[[1, 1, 1, -1, 0], [2, 1, 0, 0, 0]],
    )
    assert [status for _, status in solution.preferred_masters] == [
        "replaced",
        "replaced",
    ]
    assert solution.certify(count_masters=False).reduction == "verified"
