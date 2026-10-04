"""Boundary and guard tests for the installed native community bridge."""

import pytest
from symbolica import E, S
from symbolica.community import hepkit as hep


def tadpole(mass=None, scale=None):
    d, k, default_mass = S("validation::d", "validation::k", "validation::m2")
    mass = default_mass if mass is None else mass
    kin = hep.Kinematics(d, momenta=[k])
    denominator = kin.scalar_product(k, k) - mass
    if scale is not None:
        denominator /= scale
    family = hep.IntegralFamily([k], [], [denominator], kinematics=kin)
    return hep.IBPFamily(family), d, mass


def test_index_names_cannot_capture_physical_parameters():
    # The old public index name is a legitimate user-owned mass parameter.
    mass = S("hep::n1")
    family, d, _ = tadpole(mass=mass)
    assert family.index_symbols[0] != mass
    solution = family.solve_parametric([True], max_depth=1)
    terms = solution.reduce([2])
    assert terms[0][0] == [1]
    assert (terms[0][1] - (d - 2) / (2 * mass)).together() == E("0")
    # Reuse the actual index from one family as a different family's mass.
    # Its native Atom identity must still survive the second solver boundary.
    second, d, _ = tadpole(mass=family.index_symbols[0])
    assert second.index_symbols[0] != family.index_symbols[0]
    terms = second.solve_parametric([True], max_depth=1).reduce([2])
    assert (terms[0][1] - (d - 2) / (2 * family.index_symbols[0])).together() == E("0")


def test_fixed_indices_are_absolute_constraints():
    family, d, mass = tadpole()
    rule = family.solve_parametric([True], fixed=[2], max_depth=1).rules[0]
    assert rule.target == [E("2")]
    assert rule.apply([2])[0][0] == [1]
    assert (rule.apply([2])[0][1] - (d - 2) / (2 * mass)).together() == E("0")
    with pytest.raises(ValueError, match="does not apply"):
        rule.apply([3])
    with pytest.raises(ValueError, match="outside its sector"):
        family.solve_parametric([True], fixed=[0], max_depth=0)


def test_rational_family_conditions_survive_cancellation_and_zero_rules():
    scale = S("validation::scale")
    family, d, mass = tadpole(scale=scale)
    solution = family.solve_parametric([True], max_depth=1)
    rule = solution.rules[0]
    assert any(
        c.replace(scale, E("0")).expand() == E("0") for c in rule.nonzero_conditions
    )
    assert (solution.reduce([2])[0][1] - scale * (d - 2) / (2 * mass)).together() == E(
        "0"
    )
    scaleless, _, _ = tadpole(mass=E("0"), scale=scale)
    for zero in [
        scaleless.solve_parametric([True], max_depth=0),
        scaleless.reduce_laporta([[1]], max_depth=0),
    ]:
        assert zero.reduce([1]) == []
        assert any(
            c.replace(scale, E("0")).expand() == E("0")
            for c in zero.rules[0].nonzero_conditions
        )


@pytest.mark.parametrize("bad_power", [True, False, 1.5, "2", 2**100])
def test_noninteger_and_overflowing_powers_rejected(bad_power):
    family, _, _ = tadpole()
    with pytest.raises((TypeError, ValueError, OverflowError)):
        family.reduce_laporta([[bad_power]], max_depth=0)
    with pytest.raises((TypeError, ValueError, OverflowError)):
        family.solve_parametric([True], fixed=[bad_power], max_depth=0)
    solution = family.solve_parametric([True], max_depth=1)
    with pytest.raises((TypeError, ValueError, OverflowError)):
        solution.reduce([bad_power])
    with pytest.raises((TypeError, ValueError, OverflowError)):
        solution.rules[0].apply([bad_power])


@pytest.mark.parametrize("bad_power", [-65, 64, 32768])
def test_concrete_solver_compact_power_bound_is_explicit(bad_power):
    family, _, _ = tadpole()
    with pytest.raises(ValueError, match="compact range"):
        family.reduce_laporta([[bad_power]], max_depth=0)


def test_symbolic_rule_application_exceeds_compact_search_power_range():
    family, d, mass = tadpole()
    rule = family.solve_parametric([True], max_depth=1).rules[0]
    terms = rule.apply([100])
    assert terms[0][0] == [99]
    assert (terms[0][1] - (d - 198) / (198 * mass)).together() == E("0")


def test_empty_targets_and_explicit_target_budget():
    family, _, _ = tadpole()
    empty = family.reduce_laporta([], max_depth=0, max_targets=0)
    assert empty.rules == []
    assert empty.residuals == []
    with pytest.raises(ValueError, match="max_targets"):
        family.reduce_laporta([[2]], max_depth=0, max_targets=0)


def test_unresolved_target_remains_explicit_at_finite_depth():
    family, _, _ = tadpole()
    solution = family.reduce_laporta([[1]], max_depth=0)
    assert [1] in solution.residuals
    assert solution.reduce([1]) == [([1], E("1"))]


def tadpole_with_auxiliaries(arity):
    """One massive propagator plus an independent k.p_i basis, not a zero family."""
    d, k, mass = S("arity::d", "arity::k", "arity::m2")
    externals = [S(f"arity::p{i}") for i in range(1, arity)]
    kin = hep.Kinematics(d, momenta=[k] + externals)
    for i, left in enumerate(externals):
        for j, right in enumerate(externals[i:], start=i):
            kin = kin.with_scalar_product(left, right, E("1" if i == j else "0"))
    denominators = [kin.scalar_product(k, k) - mass]
    denominators.extend(kin.scalar_product(k, p) for p in externals)
    family = hep.IBPFamily(
        hep.IntegralFamily([k], externals, denominators, kinematics=kin)
    )
    return family, d, mass


def test_runtime_arity_capabilities_are_explicit():
    arities = hep.IBPFamily.compiled_runtime_arities()
    assert arities and arities == sorted(set(arities))
    assert all(type(n) is int and n > 0 for n in arities)
    family, _, _ = tadpole()
    assert family.compiled_runtime_arities() == arities


def test_unsupported_solver_arity_reports_actual_registry_before_search():
    arities = hep.IBPFamily.compiled_runtime_arities()
    absent = next(n for n in range(1, max(arities) + 2) if n not in arities)
    family, _, _ = tadpole_with_auxiliaries(absent)
    assert family.denominator_count == absent
    # The unsupported build capability is rejected before source generation.
    for call in [
        lambda: family.solve_parametric([True] * absent, max_depth=0),
        lambda: family.reduce_laporta([[1] * absent], max_depth=0),
    ]:
        with pytest.raises(ValueError, match="runtime bridge was compiled") as error:
            call()
        assert str(arities) in str(error.value)
        assert f"received {absent}" in str(error.value)
        assert "RUSTRED_RUNTIME_ARITIES" in str(error.value)


def test_fourteen_slot_nonzero_reduction_and_exact_certificate():
    if 14 not in hep.IBPFamily.compiled_runtime_arities():
        pytest.skip("host explicitly built without the 14-slot runtime entry point")
    family, d, mass = tadpole_with_auxiliaries(14)
    assert family.denominator_count == 14
    target, master = [2] + [0] * 13, [1] + [0] * 13
    solution = family.reduce_laporta([target], max_depth=1)
    assert solution.residuals == [master]
    terms = solution.reduce(target)
    assert len(terms) == 1 and terms[0][0] == master
    expected = (d - 2) / (2 * mass)
    assert (terms[0][1] - expected).together() == E("0")
    assert solution.certify(count_masters=False, replay=True).reduction == "verified"
    parametric = family.solve_parametric(
        [True] + [False] * 13, fixed=target, max_depth=1
    )
    terms = parametric.reduce(target)
    assert len(terms) == 1 and terms[0][0] == master
    assert (terms[0][1] - expected).together() == E("0")
