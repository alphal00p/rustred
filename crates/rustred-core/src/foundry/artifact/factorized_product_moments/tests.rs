//! Executable boundary proofs for the cold product-moment prototype.

use std::sync::{Arc, OnceLock};

use crate::algebra::{
    Coefficient, CoefficientContext, ExactAlgebraLimits,
    coefficient_clone_owned_retained_byte_bound,
};
use crate::family::IntegralKey;
use crate::foundry::artifact::{ClosedTerminalAuthority, derive_k6_terminal_authority};

use super::resources::{CoefficientBudget, constant_integer_magnitude_bits};
use super::{
    FactorizedProductMomentChart, FactorizedProductMomentError, FactorizedProductMomentLimits,
    ProductMomentExpansion, ProductMomentMonomial, compile_factorized_product_moment_chart,
};

const PATH_SECTOR: [i64; 6] = [0, 0, 1, 0, 1, 1];
const STAR_SECTOR: [i64; 6] = [0, 0, 1, 1, 0, 1];
const K3_TIMES_K1_SECTOR: [i64; 6] = [0, 0, 1, 1, 1, 1];
const PATH_TRIPLE_NUMERATOR: [i64; 6] = [-1, -1, 1, -1, 1, 1];
const STAR_TRIPLE_NUMERATOR: [i64; 6] = [-1, -1, 1, 1, -1, 1];
const PATH_A: [i64; 6] = [-2, -6, 1, -2, 3, 3];
const HELD_OUT_PATH_B: [i64; 6] = [-4, -6, 7, 0, 3, 3];

fn authority() -> &'static ClosedTerminalAuthority {
    static AUTHORITY: OnceLock<Arc<ClosedTerminalAuthority>> = OnceLock::new();
    AUTHORITY
        .get_or_init(|| derive_k6_terminal_authority().unwrap())
        .as_ref()
}

fn factorization_ordinal(sector: &[i64]) -> usize {
    authority()
        .factorization_rules()
        .iter()
        .position(|rule| {
            rule.application_domain()
                .sector()
                .active_bits()
                .iter()
                .zip(sector)
                .all(|(&active, &power)| active == (power >= 1))
        })
        .unwrap()
}

fn chart(sector: &[i64]) -> FactorizedProductMomentChart<'static> {
    compile_factorized_product_moment_chart(
        authority(),
        factorization_ordinal(sector),
        FactorizedProductMomentLimits::default(),
    )
    .unwrap()
}

fn coefficient_context() -> &'static CoefficientContext {
    authority().family().coefficient_context()
}

fn assert_coefficient(actual: &Coefficient, expected: &str) {
    let context = coefficient_context();
    let expected = context.coefficient_fixture(expected);
    assert!(
        context
            .try_sub(actual, &expected, ExactAlgebraLimits::default())
            .unwrap()
            .is_zero(),
        "expected {expected:?}, received {actual:?}",
    );
}

fn terminal_coefficient<'expansion>(
    chart: &FactorizedProductMomentChart<'_>,
    expansion: &'expansion ProductMomentExpansion,
) -> &'expansion Coefficient {
    assert_eq!(expansion.terms().len(), 1);
    expansion
        .terms()
        .get(chart.terminal())
        .expect("the product route must emit only its authenticated parent terminal")
}

#[test]
fn authenticated_path_and_star_charts_preserve_signed_singleton_blocks_and_scalar_corners() {
    for sector in [&PATH_SECTOR, &STAR_SECTOR] {
        let chart = chart(sector);
        assert_eq!(chart.loop_factor_count(), 3);
        assert_eq!(chart.cross_coordinate_count(), 3);
        assert_eq!(chart.parent_by_vector().len(), 3);
        assert_eq!(chart.dependency_by_vector().len(), 3);

        let original = chart.rule().loop_basis().row_major();
        let signed = chart.signed_loop_basis();
        for row in 0..chart.loop_factor_count() {
            let range = row * chart.loop_factor_count()..(row + 1) * chart.loop_factor_count();
            let installed = &original[range.clone()];
            let compiled = &signed[range];
            assert!(
                installed == compiled
                    || installed
                        .iter()
                        .zip(compiled)
                        .all(|(&left, &right)| left.checked_neg() == Some(right))
            );
        }

        let source = IntegralKey::try_new(*sector).unwrap();
        let expansion = chart
            .try_evaluate_parent(&source, FactorizedProductMomentLimits::default())
            .unwrap();
        assert!(expansion.belongs_to_chart(&chart));
        assert_eq!(
            terminal_coefficient(&chart, &expansion),
            chart.normalization()
        );
        assert!(expansion.guards().is_empty());
        assert_eq!(expansion.statistics().numerator_polynomial_terms(), 1);
    }
}

#[test]
fn routed_path_and_star_samples_reproduce_the_known_exact_angular_products() {
    for (sector, source, expected) in [
        (&PATH_SECTOR, PATH_TRIPLE_NUMERATOR, "2*(d+2)^2/d^2"),
        (&STAR_SECTOR, STAR_TRIPLE_NUMERATOR, "(d^2-8)/d^2"),
    ] {
        let chart = chart(sector);
        let source = IntegralKey::try_new(source).unwrap();
        let expansion = chart
            .try_evaluate_parent(&source, FactorizedProductMomentLimits::default())
            .unwrap();
        assert_coefficient(terminal_coefficient(&chart, &expansion), expected);
        assert!(expansion.statistics().angular_transitions() > 0);
        assert!(
            expansion
                .guards()
                .iter()
                .all(|guard| guard.rank() >= 2 && guard.rank() % 2 == 0)
        );
    }
}

#[test]
fn isotropic_incidence_dp_proves_odd_zero_rank_two_and_rank_four() {
    let chart = chart(&PATH_SECTOR);
    let limits = FactorizedProductMomentLimits::default();

    let odd = ProductMomentMonomial::try_new([1, 1, 1], [0, 0, 0], [1, 0, 0]).unwrap();
    let odd = chart.try_evaluate_monomial(&odd, limits).unwrap();
    assert!(odd.terms().is_empty());
    assert!(odd.guards().is_empty());

    let rank_two = ProductMomentMonomial::try_new([1, 1, 1], [0, 0, 0], [2, 0, 0]).unwrap();
    let rank_two = chart.try_evaluate_monomial(&rank_two, limits).unwrap();
    assert_coefficient(terminal_coefficient(&chart, &rank_two), "1/d");
    assert_eq!(rank_two.guards().len(), 1);
    assert_eq!(
        (rank_two.guards()[0].vector(), rank_two.guards()[0].rank()),
        (0, 2)
    );
    assert_coefficient(rank_two.guards()[0].nonzero_polynomial(), "d");

    let rank_four = ProductMomentMonomial::try_new([1, 1, 1], [0, 0, 0], [4, 0, 0]).unwrap();
    let rank_four = chart.try_evaluate_monomial(&rank_four, limits).unwrap();
    assert_coefficient(terminal_coefficient(&chart, &rank_four), "3/(d*(d+2))");
    assert_eq!(
        rank_four
            .guards()
            .iter()
            .map(|guard| (guard.vector(), guard.rank()))
            .collect::<Vec<_>>(),
        [(0, 2), (0, 4)],
    );

    let rank_two_with_dot =
        ProductMomentMonomial::try_new([2, 1, 1], [0, 0, 0], [2, 0, 0]).unwrap();
    let rank_two_with_dot = chart
        .try_evaluate_monomial(&rank_two_with_dot, limits)
        .unwrap();
    assert_coefficient(terminal_coefficient(&chart, &rank_two_with_dot), "1/2");
    assert_eq!(rank_two_with_dot.guards().len(), 1);
    assert_coefficient(rank_two_with_dot.guards()[0].nonzero_polynomial(), "d");
}

#[test]
fn radial_q_squared_over_d_squared_uses_the_sealed_tadpole_reducer() {
    let chart = chart(&PATH_SECTOR);
    let monomial = ProductMomentMonomial::try_new([2, 1, 1], [1, 0, 0], [0, 0, 0]).unwrap();
    let expansion = chart
        .try_evaluate_monomial(&monomial, FactorizedProductMomentLimits::default())
        .unwrap();
    assert_coefficient(terminal_coefficient(&chart, &expansion), "d/2");
    assert!(expansion.statistics().dependency_rule_applications() > 0);
    assert_eq!(expansion.statistics().dependency_requests(), 3);
    assert_eq!(expansion.statistics().radial_summands(), 3);
    assert!(expansion.statistics().dependency_cache_hits() > 0);
    assert!(expansion.statistics().coalescing_additions() > 0);
}

#[test]
fn expansions_are_deterministic_but_remain_bound_to_one_compiled_chart() {
    let first = chart(&PATH_SECTOR);
    let second = chart(&PATH_SECTOR);
    let source = IntegralKey::try_new(PATH_TRIPLE_NUMERATOR).unwrap();
    let limits = FactorizedProductMomentLimits::default();
    let left = first.try_evaluate_parent(&source, limits).unwrap();
    let replay = first.try_evaluate_parent(&source, limits).unwrap();
    let foreign = second.try_evaluate_parent(&source, limits).unwrap();

    assert_eq!(left, replay);
    assert_ne!(left, foreign);
    assert!(left.belongs_to_chart(&first));
    assert!(!left.belongs_to_chart(&second));
    assert_eq!(left.terms(), foreign.terms());
    assert_eq!(left.guards(), foreign.guards());
}

#[test]
fn both_persistent_path_frontiers_end_in_one_authenticated_terminal() {
    let chart = chart(&PATH_SECTOR);
    let limits = FactorizedProductMomentLimits::default();
    for (source, expected, expected_support, expected_guards) in [
        (
            PATH_A,
            "(110960640+305012736*d+325530112*d^2+141681216*d^3+30444304*d^4+3547696*d^5+230992*d^6+8236*d^7+147*d^8+d^9)/(16*d^2*(d+2)*(d+4)*(d+6))",
            3_886,
            &[(0, 2), (0, 4), (0, 6), (0, 8), (1, 2), (1, 4)][..],
        ),
        (
            HELD_OUT_PATH_B,
            "(11134402560+9909301248*d-5772933120*d^2-4342053120*d^3+434170304*d^4+623143088*d^5+134009472*d^6+12849384*d^7+634356*d^8+16419*d^9+208*d^10+d^11)/(184320*d)",
            4_396,
            &[(0, 2), (0, 4), (0, 6), (0, 8), (0, 10), (1, 2), (1, 4)][..],
        ),
    ] {
        let source = IntegralKey::try_new(source).unwrap();
        let first = chart.try_evaluate_parent(&source, limits).unwrap();
        let replay = chart.try_evaluate_parent(&source, limits).unwrap();
        assert_eq!(first, replay);
        let coefficient = terminal_coefficient(&chart, &first);
        assert_coefficient(coefficient, expected);
        assert!(
            first
                .terms()
                .keys()
                .all(|key| key == chart.terminal() && key.powers().iter().all(|power| *power >= 0))
        );
        assert_eq!(
            first
                .guards()
                .iter()
                .map(|guard| (guard.vector(), guard.rank()))
                .collect::<Vec<_>>(),
            expected_guards,
        );
        assert_eq!(
            first.statistics().numerator_polynomial_terms(),
            expected_support
        );
        assert!(first.statistics().angular_states() > 0);
        assert!(first.statistics().radial_states() > 0);
    }
}

#[test]
fn caller_limits_reject_angular_radial_guard_key_and_coalescing_work() {
    let chart = chart(&PATH_SECTOR);
    let rank_four = ProductMomentMonomial::try_new([1, 1, 1], [0, 0, 0], [4, 0, 0]).unwrap();
    let angular_limits = FactorizedProductMomentLimits {
        max_angular_degree: 3,
        ..FactorizedProductMomentLimits::default()
    };
    assert!(matches!(
        chart.try_evaluate_monomial(&rank_four, angular_limits),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "angular degree",
            requested: 4,
            limit: 3,
        })
    ));

    let radial = ProductMomentMonomial::try_new([1, 1, 1], [1, 0, 0], [0, 0, 0]).unwrap();
    let radial_limits = FactorizedProductMomentLimits {
        max_radial_power: 0,
        ..FactorizedProductMomentLimits::default()
    };
    assert!(matches!(
        chart.try_evaluate_monomial(&radial, radial_limits),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "radial power",
            requested: 1,
            limit: 0,
        })
    ));

    let rank_two = ProductMomentMonomial::try_new([1, 1, 1], [0, 0, 0], [2, 0, 0]).unwrap();
    let guard_limits = FactorizedProductMomentLimits {
        max_guards: 0,
        ..FactorizedProductMomentLimits::default()
    };
    assert!(matches!(
        chart.try_evaluate_monomial(&rank_two, guard_limits),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "product moment guards",
            requested: 1,
            limit: 0,
        })
    ));

    let key_limits = FactorizedProductMomentLimits {
        max_output_key_power_entries: 0,
        ..FactorizedProductMomentLimits::default()
    };
    assert!(matches!(
        chart.try_evaluate_parent(&IntegralKey::try_new(PATH_SECTOR).unwrap(), key_limits),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "product output key power entries",
            requested: 18,
            limit: 0,
        })
    ));

    let coefficient_limits = FactorizedProductMomentLimits {
        max_retained_coefficient_terms: 100,
        ..FactorizedProductMomentLimits::default()
    };
    assert!(matches!(
        chart.try_evaluate_parent(
            &IntegralKey::try_new(PATH_TRIPLE_NUMERATOR).unwrap(),
            coefficient_limits,
        ),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "product retained coefficient terms",
            requested,
            limit: 100,
        }) if requested > 100
    ));

    // The old raw `ceil(bits/8)` projection fit exactly at this boundary,
    // even though cloning the >i128 backend integer may retain rounded and
    // spare limbs.  The pre-native envelope must reject before construction.
    let context = coefficient_context();
    let large = context
        .coefficient_fixture("1606938044258990275541962092341162602522202993782792835301376");
    let integer_bits = constant_integer_magnitude_bits(&large).unwrap();
    assert_eq!(integer_bits, 201);
    let unit = context.one();
    let unit_bytes = coefficient_clone_owned_retained_byte_bound(&unit).unwrap();
    let old_raw_magnitude_bytes = integer_bits.div_ceil(8);
    let old_tight_limit = unit_bytes
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(old_raw_magnitude_bytes))
        .unwrap();
    let native_byte_limits = FactorizedProductMomentLimits {
        max_retained_coefficient_clone_owned_bytes: old_tight_limit,
        ..FactorizedProductMomentLimits::default()
    };
    let mut native_budget = CoefficientBudget::new(native_byte_limits);
    native_budget.retain(&unit).unwrap();
    assert!(matches!(
        native_budget.admit_native_integer_envelope(1, integer_bits, &unit),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "product retained coefficient clone-owned bytes",
            requested,
            limit,
        }) if requested > limit && limit == old_tight_limit
    ));

    let dotted = ProductMomentMonomial::try_new([2, 1, 1], [1, 0, 0], [0, 0, 0]).unwrap();
    let addition_limits = FactorizedProductMomentLimits {
        max_coalescing_additions: 0,
        ..FactorizedProductMomentLimits::default()
    };
    assert!(matches!(
        chart.try_evaluate_monomial(&dotted, addition_limits),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "product coefficient coalescing additions",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn compiler_and_evaluator_reject_foreign_shapes_with_typed_errors() {
    let limits = FactorizedProductMomentLimits::default();
    assert!(matches!(
        compile_factorized_product_moment_chart(authority(), usize::MAX, limits),
        Err(FactorizedProductMomentError::MissingFactorizationRule {
            ordinal: usize::MAX,
        })
    ));
    assert!(matches!(
        compile_factorized_product_moment_chart(
            authority(),
            factorization_ordinal(&K3_TIMES_K1_SECTOR),
            limits,
        ),
        Err(FactorizedProductMomentError::UnsupportedFactorCount {
            expected: 3,
            actual: 2,
        })
    ));

    let chart = chart(&PATH_SECTOR);
    let wrong_width = ProductMomentMonomial::try_new([1, 1], [0, 0, 0], [0, 0, 0]).unwrap();
    assert_eq!(
        chart.try_evaluate_monomial(&wrong_width, limits),
        Err(FactorizedProductMomentError::WrongMonomialWidth {
            component: "active powers",
            expected: 3,
            actual: 2,
        })
    );
    let nonpositive = ProductMomentMonomial::try_new([0, 1, 1], [0, 0, 0], [0, 0, 0]).unwrap();
    assert_eq!(
        chart.try_evaluate_monomial(&nonpositive, limits),
        Err(FactorizedProductMomentError::NonpositiveActivePower {
            vector: 0,
            power: 0,
        })
    );

    let operation_limits = FactorizedProductMomentLimits {
        max_native_polynomial_operations: 0,
        ..limits
    };
    assert!(matches!(
        chart.try_evaluate_parent(
            &IntegralKey::try_new(PATH_TRIPLE_NUMERATOR).unwrap(),
            operation_limits,
        ),
        Err(FactorizedProductMomentError::ResourceLimit {
            resource: "product native polynomial operations",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}
