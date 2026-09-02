//! Compact executable-boundary regressions for exact K6 product preimages.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::algebra::{Coefficient, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedTerminalAuthority, derive_k6_terminal_authority, k6_product_reducer_fixture,
};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::reduction::{
    CacheCensus, Reducer, ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    SharedCacheBudget, coefficient_cache_weight,
};
use crate::sector::{InteriorBounds, Mask, SectorInteriorDomain};

use super::angular::{AngularEvaluator, cross_radial_powers};
use super::limits::FactorizedProductMomentLimits;
use super::resources::CoefficientBudget;
use super::runtime::add_weighted_terms;

const PATH: [i64; 6] = [0, 0, 1, 0, 1, 1];
const STAR: [i64; 6] = [0, 0, 1, 1, 0, 1];
const K3_TIMES_K1: [i64; 6] = [0, 0, 1, 1, 1, 1];

fn authority() -> &'static Arc<ClosedTerminalAuthority> {
    static AUTHORITY: OnceLock<Arc<ClosedTerminalAuthority>> = OnceLock::new();
    AUTHORITY.get_or_init(|| derive_k6_terminal_authority().unwrap())
}

fn factorization_ordinal(sector: [i64; 6]) -> usize {
    authority()
        .factorization_rules()
        .iter()
        .position(|rule| {
            rule.application_domain()
                .sector()
                .active_bits()
                .iter()
                .zip(sector)
                .all(|(&active, power)| active == (power >= 1))
        })
        .unwrap()
}

fn execute(
    sector: [i64; 6],
    target: [i64; 6],
    limits: ReductionLimits,
    request: &mut ReductionRequest,
    statistics: &mut ReductionStatistics,
) -> Result<BTreeMap<IntegralKey, Coefficient>, ReductionError> {
    let authority = authority();
    let ordinal = factorization_ordinal(sector);
    let program = authority.factorized_product_programs()[ordinal]
        .as_ref()
        .unwrap();
    let shared_cache = Arc::new(SharedCacheBudget::default());
    let mut reducers = authority
        .dependencies()
        .iter()
        .map(|dependency| Reducer::with_shared_cache(dependency, limits, Arc::clone(&shared_cache)))
        .collect::<Result<Vec<_>, _>>()?;
    let result = program.reduce_parent(
        authority.family(),
        &authority.factorization_rules()[ordinal],
        authority.dependencies(),
        &mut reducers,
        &IntegralKey::try_new(target)?,
        request,
        statistics,
        shared_cache,
        limits,
    );
    // Mirror the public Reducer::statistics() hierarchy: nested lower-artifact
    // work belongs to the same top-level request even in this direct program
    // harness.
    for reducer in &reducers {
        statistics.merge_work(reducer.statistics());
    }
    result
}

fn dependency_seed_census() -> CacheCensus {
    let shared_cache = Arc::new(SharedCacheBudget::default());
    let _reducers = authority()
        .dependencies()
        .iter()
        .map(|dependency| {
            Reducer::with_shared_cache(
                dependency,
                ReductionLimits::default(),
                Arc::clone(&shared_cache),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    shared_cache.snapshot()
}

fn assert_coefficient(actual: &Coefficient, expected: &str) {
    let context = authority().family().coefficient_context();
    let expected = context.coefficient_fixture(expected);
    assert!(
        context
            .try_sub(actual, &expected, ExactAlgebraLimits::default())
            .unwrap()
            .is_zero(),
        "expected {expected:?}, received {actual:?}",
    );
}

fn sole_coefficient(terms: &BTreeMap<IntegralKey, Coefficient>) -> &Coefficient {
    assert_eq!(terms.len(), 1);
    terms.values().next().unwrap()
}

#[test]
fn exact_product_domains_are_executable_owner_authority_under_s4() {
    let authority = Arc::clone(authority());
    let snapshot = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    for (sector, target) in [
        (PATH, [-2, -4, 2, -2, 3, 4]),
        (STAR, [-2, -4, 2, 3, -2, 4]),
        (K3_TIMES_K1, [-4, -2, 2, 3, 4, 2]),
    ] {
        let ordinal = factorization_ordinal(sector);
        let program = authority.factorized_product_programs()[ordinal]
            .as_ref()
            .unwrap();
        assert!(program.contains(&target).unwrap());
        assert!(
            !authority.factorization_rules()[ordinal]
                .application_domain()
                .contains(&target)
                .unwrap()
        );
        assert!(program.branch_width(0).unwrap() <= authority.family().coordinates().len() + 1);
        assert_eq!(program.cross_coordinate_count(), 3);
        assert_eq!(program.signed_loop_basis().len(), 9);
        if sector == K3_TIMES_K1 {
            assert_eq!(program.singleton_factor_count(), 1);
            assert_eq!(program.correlated_factor_loop_count(), Some(2));
        } else {
            assert_eq!(program.singleton_factor_count(), 3);
            assert_eq!(program.correlated_factor_loop_count(), None);
        }

        let target = IntegralKey::try_new(target).unwrap();
        for image in authority
            .canonicalizer()
            .unwrap()
            .orbit(&target)
            .unwrap()
            .images()
        {
            assert!(
                snapshot
                    .authenticates_explicit_terminal(image.integral())
                    .unwrap(),
                "exact product image was not authenticated: {:?}",
                image.integral().powers(),
            );
        }
    }
    assert!(
        snapshot
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
}

#[test]
fn dependency_root_preimages_enforce_exact_max_min_and_aggregate_boundaries() {
    let authority = Arc::clone(authority());
    let snapshot = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let cases = [
        (PATH, [i64::MIN, -1, 1, 0, i64::MAX, i64::MAX], true),
        (PATH, [i64::MIN, -2, 1, 0, i64::MAX, i64::MAX], false),
        (PATH, [i64::MIN, -1, 1, -1, 1, 1], false),
        (PATH, [i64::MIN, 0, 1, -1, 1, 1], true),
        (PATH, [i64::MIN, -1, 1, 0, 1, 1], true),
        // The inactive path source in slot 3 has no incidence on the first
        // singleton. A total-rank box would reject this valid sparse-support
        // point even though every exact dependency preimage contains it.
        (PATH, [-2, 0, 1, i64::MIN, i64::MAX, i64::MAX], true),
        (K3_TIMES_K1, [0, 0, 1, i64::MAX - 1, 1, 1], true),
        (K3_TIMES_K1, [0, 0, 1, i64::MAX, 1, 1], false),
        // A numerator cannot rescue an excessive K3 base: the nonzero
        // constant branch still invokes the unshifted dependency root.
        (K3_TIMES_K1, [-1, 0, 1, i64::MAX, 1, 1], false),
        // Slot 0 cannot shift K3 coordinates 1 or 2; their exact support is
        // only slot 1, while the K1/K3-coordinate-0 rows see both sources.
        (
            K3_TIMES_K1,
            [i64::MIN, -2, i64::MAX, i64::MAX - 1, 1, 1],
            true,
        ),
    ];
    for (sector, target, expected) in cases {
        let ordinal = factorization_ordinal(sector);
        let program = authority.factorized_product_programs()[ordinal]
            .as_ref()
            .unwrap();
        assert_eq!(program.contains(&target).unwrap(), expected, "{target:?}");
        let target_key = IntegralKey::try_new(target).unwrap();
        let routed_expected = authority
            .canonicalizer()
            .unwrap()
            .orbit(&target_key)
            .unwrap()
            .images()
            .iter()
            .any(|image| {
                authority
                    .factorized_product_programs()
                    .iter()
                    .filter_map(Option::as_ref)
                    .any(|program| program.contains(image.integral().powers()).unwrap())
            });
        assert_eq!(
            snapshot
                .authenticates_explicit_terminal(&target_key)
                .unwrap(),
            routed_expected,
            "immutable owner disagrees for {target:?}",
        );
        assert!(!expected || routed_expected);
    }

    // The accepted K3 upper endpoint reaches nested dispatch without a late
    // dependency-root rejection. Its astronomical recurrence is deliberately
    // stopped by the caller's work limit.
    let mut request = ReductionRequest::default();
    let mut statistics = ReductionStatistics::default();
    let error = execute(
        K3_TIMES_K1,
        [0, 0, 1, i64::MAX - 1, 1, 1],
        ReductionLimits {
            max_rule_applications: 0,
            ..ReductionLimits::default()
        },
        &mut request,
        &mut statistics,
    )
    .unwrap_err();
    assert!(matches!(error, ReductionError::RuleApplicationLimit { .. }));
}

#[test]
fn k6_stabilizer_union_cover_reduces_to_one_route_on_boundary_grid() {
    let authority = authority();
    let canonicalizer = authority.canonicalizer().unwrap();
    for sector_powers in [PATH, STAR, K3_TIMES_K1] {
        let ordinal = factorization_ordinal(sector_powers);
        let program = authority.factorized_product_programs()[ordinal]
            .as_ref()
            .unwrap();
        let sector = program.application_hull().sector();
        let routes = canonicalizer
            .routing_witnesses()
            .filter(|route| {
                route
                    .source_for_target()
                    .iter()
                    .enumerate()
                    .all(|(owner, &raw)| sector.active_bits()[owner] == sector.active_bits()[raw])
            })
            .collect::<Vec<_>>();
        assert!(!routes.is_empty());

        // Every route in this fixed-sector stabilizer has the same raw
        // rectangular carrier.  This includes the nonuniform K3-times-K1
        // upper cap: its unique singleton edge is fixed by the stabilizer.
        let raw_carriers = routes
            .iter()
            .map(|route| {
                let mut raw = vec![InteriorBounds::new(0, 0); sector.arity()];
                for (owner, &raw_position) in route.source_for_target().iter().enumerate() {
                    raw[raw_position] = program.application_hull().bounds()[owner];
                }
                raw
            })
            .collect::<Vec<_>>();
        assert!(raw_carriers.windows(2).all(|pair| pair[0] == pair[1]));
        let carrier = &raw_carriers[0];

        let choices = carrier
            .iter()
            .map(|bounds| {
                if bounds.lower() == i64::MIN {
                    vec![0, -1, i64::MIN + 1, i64::MIN]
                } else {
                    let mut values = vec![
                        bounds.lower(),
                        bounds.upper().saturating_sub(1),
                        bounds.upper(),
                    ];
                    values.retain(|&value| bounds.lower() <= value);
                    values.sort_unstable();
                    values.dedup();
                    values
                }
            })
            .collect::<Vec<_>>();
        let mut lower = vec![0_i64; sector.arity()];
        visit_boundary_grid(&choices, 0, &mut lower, &mut |lower| {
            let rectangle = SectorInteriorDomain::try_new(
                Mask::try_new(sector.active_bits().iter().copied()).unwrap(),
                lower
                    .iter()
                    .zip(carrier)
                    .map(|(&lower, upper)| InteriorBounds::new(lower, upper.upper())),
            )
            .unwrap();
            let lower_in_route_union = routes.iter().any(|route| {
                let owner_point = route
                    .source_for_target()
                    .iter()
                    .map(|&raw| lower[raw])
                    .collect::<Vec<_>>();
                program.contains(&owner_point).unwrap()
            });
            let covered_by_one_route = routes.iter().any(|route| {
                program
                    .exact_application_domain()
                    .covers_transported_domain(&rectangle, route.source_for_target())
            });
            assert_eq!(
                covered_by_one_route, lower_in_route_union,
                "persisted-cover one-route lemma failed for {sector_powers:?} at {lower:?}",
            );
        });
    }
}

fn visit_boundary_grid(
    choices: &[Vec<i64>],
    position: usize,
    current: &mut [i64],
    visit: &mut impl FnMut(&[i64]),
) {
    let Some(values) = choices.get(position) else {
        visit(current);
        return;
    };
    for &value in values {
        current[position] = value;
        visit_boundary_grid(choices, position + 1, current, visit);
    }
}

#[test]
fn stabilizer_routed_product_authority_has_an_executable_orbit_image() {
    let authority = Arc::clone(authority());
    let snapshot = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    // The ordinary canonical representative fails the native K3×K1 x3
    // aggregate row. The authenticated stabilizer (0<->1, 3<->5) maps it to
    // the exact MIN boundary of an executable product preimage.
    let raw =
        IntegralKey::try_new([-i64::MAX, -i64::MAX, i64::MAX - 1, 1, 1, i64::MAX - 1]).unwrap();
    let canonicalizer = authority.canonicalizer().unwrap();
    assert_eq!(
        canonicalizer.canonicalize(&raw).unwrap().canonical(),
        &raw,
        "the regression must exercise canonical-direct failure",
    );
    let ordinal = factorization_ordinal(K3_TIMES_K1);
    let program = authority.factorized_product_programs()[ordinal]
        .as_ref()
        .unwrap();
    assert!(!program.contains(raw.powers()).unwrap());
    assert!(snapshot.authenticates_explicit_terminal(&raw).unwrap());

    let routed = canonicalizer
        .orbit(&raw)
        .unwrap()
        .images()
        .iter()
        .find(|image| program.contains(image.integral().powers()).unwrap())
        .expect("authenticated product union must retain an executable image")
        .integral()
        .clone();
    assert_ne!(routed, raw);
    let routed: [i64; 6] = routed.powers().try_into().unwrap();
    let mut request = ReductionRequest::default();
    let mut statistics = ReductionStatistics::default();
    let error = execute(
        K3_TIMES_K1,
        routed,
        ReductionLimits {
            max_rule_applications: 0,
            ..ReductionLimits::default()
        },
        &mut request,
        &mut statistics,
    )
    .unwrap_err();
    assert!(matches!(error, ReductionError::RuleApplicationLimit { .. }));

    // Exercise the actual public dispatch seam on the raw canonical key.  A
    // direct-only selector would fall through to `UncoveredIntegral`; the
    // authenticated orbit selector finds the safe image first and therefore
    // reaches the caller's deliberately zero work allowance.
    let artifact = k6_product_reducer_fixture();
    let mut reducer = Reducer::with_limits(
        &artifact,
        ReductionLimits {
            max_rule_applications: 0,
            ..ReductionLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        reducer.reduce_unit_mass(&raw),
        Err(ReductionError::RuleApplicationLimit {
            requested: 1,
            limit: 0,
        })
    );
}

#[test]
fn wide_valid_star_preimage_uses_checked_wide_descent_measure() {
    let target = [
        -i64::MAX,
        -i64::MAX,
        i64::MAX,
        i64::MAX,
        -i64::MAX,
        i64::MAX,
    ];
    let ordinal = factorization_ordinal(STAR);
    let program = authority().factorized_product_programs()[ordinal]
        .as_ref()
        .unwrap();
    assert!(program.contains(&target).unwrap());
    let mut request = ReductionRequest::default();
    let mut statistics = ReductionStatistics::default();
    let error = execute(
        STAR,
        target,
        ReductionLimits {
            max_rule_applications: 0,
            ..ReductionLimits::default()
        },
        &mut request,
        &mut statistics,
    )
    .unwrap_err();
    assert!(matches!(error, ReductionError::RuleApplicationLimit { .. }));

    // Two individually representable moment powers can produce an incidence
    // beyond u64.  The retained key and exact Symbolica multiplicity stay wide
    // without a signed or unsigned machine-word downcast.
    let beyond_u64 = u128::from(u64::MAX) + 1;
    let radial = cross_radial_powers(3, &[(0, 1), (0, 2), (1, 2)], &[beyond_u64, beyond_u64, 0])
        .unwrap()
        .unwrap();
    assert_eq!(
        radial.as_ref(),
        &[beyond_u64, beyond_u64 / 2, beyond_u64 / 2]
    );
    assert_coefficient(
        &authority()
            .family()
            .coefficient_context()
            .unsigned_integer(beyond_u64),
        "18446744073709551616",
    );
}

#[test]
fn path_star_and_correlated_products_replay_known_exact_reductions() {
    for (sector, target, expected) in [
        (PATH, [-1, -1, 1, -1, 1, 1], "2*(d+2)^2/d^2"),
        (STAR, [-1, -1, 1, 1, -1, 1], "(d^2-8)/d^2"),
    ] {
        let mut request = ReductionRequest::default();
        let mut statistics = ReductionStatistics::default();
        let terms = execute(
            sector,
            target,
            ReductionLimits::default(),
            &mut request,
            &mut statistics,
        )
        .unwrap();
        assert_coefficient(sole_coefficient(&terms), expected);
        assert!(statistics.rule_applications() > 0);
        assert!(
            terms
                .keys()
                .all(|key| authority().master_terminals().any(|master| master == key))
        );
    }

    for (target, product, sunset) in [
        ([-2, 0, 1, 1, 1, 1], Some("(3*d+4)/d"), "(d+4)/d"),
        ([-1, -1, 1, 1, 1, 1], Some("2*(d+1)/d"), "(d+2)/d"),
    ] {
        let mut request = ReductionRequest::default();
        let mut statistics = ReductionStatistics::default();
        let terms = execute(
            K3_TIMES_K1,
            target,
            ReductionLimits::default(),
            &mut request,
            &mut statistics,
        )
        .unwrap();
        let product_key = IntegralKey::try_new(PATH).unwrap();
        let sunset_key = IntegralKey::try_new(K3_TIMES_K1).unwrap();
        if let Some(expected) = product {
            assert_coefficient(terms.get(&product_key).unwrap(), expected);
        }
        assert_coefficient(terms.get(&sunset_key).unwrap(), sunset);
        assert!(statistics.rule_applications() > 0);
    }
}

#[test]
fn held_out_arbitrary_rank_products_terminate_deterministically_on_masters() {
    for (sector, target) in [
        (PATH, [-2, -3, 2, -2, 2, 3]),
        (STAR, [-2, -3, 2, 2, -2, 3]),
        (K3_TIMES_K1, [-3, -2, 2, 2, 3, 2]),
    ] {
        let mut first_request = ReductionRequest::default();
        let mut first_statistics = ReductionStatistics::default();
        let first = execute(
            sector,
            target,
            ReductionLimits::default(),
            &mut first_request,
            &mut first_statistics,
        )
        .unwrap();
        let mut replay_request = ReductionRequest::default();
        let mut replay_statistics = ReductionStatistics::default();
        let replay = execute(
            sector,
            target,
            ReductionLimits::default(),
            &mut replay_request,
            &mut replay_statistics,
        )
        .unwrap();
        assert_eq!(first, replay);
        assert!(!first.is_empty());
        assert!(
            first
                .keys()
                .all(|key| authority().master_terminals().any(|master| master == key))
        );
        assert_eq!(first_statistics, replay_statistics);
        assert!(first_statistics.rule_applications() > 0);
        assert!(first_statistics.cache_hits() > 0);
    }
}

#[test]
fn angular_guards_and_product_resource_limits_are_exact() {
    let context = authority().family().coefficient_context();
    let edges = [(0, 1), (0, 2), (1, 2)];
    let moment_limits = FactorizedProductMomentLimits::default();
    let mut budget = CoefficientBudget::new(moment_limits);
    let mut angular = AngularEvaluator::new(
        context,
        authority().family().dimension(),
        3,
        &edges,
        moment_limits,
    );
    let coefficient = angular.evaluate(&[4, 0, 0], &mut budget).unwrap();
    assert_coefficient(&coefficient, "3/(d*(d+2))");
    let guards = angular.finish(&mut budget).unwrap();
    assert_eq!(
        guards
            .iter()
            .map(|guard| (guard.vector(), guard.rank()))
            .collect::<Vec<_>>(),
        [(0, 2), (0, 4)],
    );
    assert_coefficient(guards[0].nonzero_polynomial(), "d");

    let corner_limits = ReductionLimits {
        max_cached_integrals: 0,
        ..ReductionLimits::default()
    };
    assert!(matches!(
        execute(
            PATH,
            PATH,
            corner_limits,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ),
        Err(ReductionError::CacheLimit { limit: 0, .. })
    ));

    let coefficient_limits = ReductionLimits {
        max_cached_coefficient_terms: 0,
        ..ReductionLimits::default()
    };
    assert!(matches!(
        execute(
            PATH,
            PATH,
            coefficient_limits,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ),
        Err(ReductionError::CacheCoefficientTermLimit { limit: 0, .. })
    ));

    let byte_limits = ReductionLimits {
        max_cached_coefficient_bytes: 0,
        ..ReductionLimits::default()
    };
    assert!(matches!(
        execute(
            PATH,
            PATH,
            byte_limits,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ),
        Err(ReductionError::CacheCoefficientByteLimit { limit: 0, .. })
    ));

    let work_limits = ReductionLimits {
        max_rule_applications: 1,
        ..ReductionLimits::default()
    };
    let mut precharged = ReductionRequest::default();
    precharged
        .record_rule_application(work_limits.max_rule_applications)
        .unwrap();
    let mut failed_statistics = ReductionStatistics::default();
    assert!(matches!(
        execute(
            PATH,
            [-1, 0, 1, 0, 1, 1],
            work_limits,
            &mut precharged,
            &mut failed_statistics,
        ),
        Err(ReductionError::RuleApplicationLimit {
            requested: 2,
            limit: 1,
        })
    ));
    assert_eq!(
        failed_statistics.rule_applications(),
        1,
        "the rejected recurrence attempt must remain visible in telemetry"
    );
}

#[test]
fn angular_cache_limits_are_aggregate_and_exactly_one_below() {
    let seed = dependency_seed_census();
    let one = authority().family().coefficient_context().one();
    let one_weight = coefficient_cache_weight(std::iter::once(&one)).unwrap();

    let state_limits = ReductionLimits {
        max_cached_integrals: seed.integrals,
        ..ReductionLimits::default()
    };
    assert_eq!(
        execute(
            PATH,
            PATH,
            state_limits,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ),
        Err(ReductionError::CacheLimit {
            requested: seed.integrals + 1,
            limit: seed.integrals,
        })
    );

    // The zero-rank angular DP retains one exact `1` and clones it for its
    // return value.  Admit exactly one sparse term less than that live peak.
    let coefficient_peak = one_weight.coefficient_terms.checked_mul(2).unwrap();
    let coefficient_limit = seed
        .coefficient_terms
        .checked_add(coefficient_peak)
        .unwrap()
        - 1;
    let coefficient_limits = ReductionLimits {
        max_cached_coefficient_terms: coefficient_limit,
        ..ReductionLimits::default()
    };
    assert_eq!(
        execute(
            PATH,
            PATH,
            coefficient_limits,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ),
        Err(ReductionError::CacheCoefficientTermLimit {
            requested: seed.coefficient_terms + coefficient_peak,
            limit: coefficient_limit,
        })
    );

    let byte_peak = one_weight.coefficient_bytes.checked_mul(2).unwrap();
    let byte_limit = seed.coefficient_bytes.checked_add(byte_peak).unwrap() - 1;
    let byte_limits = ReductionLimits {
        max_cached_coefficient_bytes: byte_limit,
        ..ReductionLimits::default()
    };
    assert_eq!(
        execute(
            PATH,
            PATH,
            byte_limits,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        ),
        Err(ReductionError::CacheCoefficientByteLimit {
            requested: seed.coefficient_bytes + byte_peak,
            limit: byte_limit,
        })
    );
}

#[test]
fn pending_frames_are_aggregate_and_restore_the_caller_baseline() {
    let limits = ReductionLimits {
        max_pending_frames: 1,
        ..ReductionLimits::default()
    };
    let mut request = ReductionRequest::default();
    request
        .retain_pending_frame(limits.max_pending_frames)
        .unwrap();
    assert_eq!(
        execute(
            PATH,
            PATH,
            limits,
            &mut request,
            &mut ReductionStatistics::default(),
        ),
        Err(ReductionError::PendingFrameLimit {
            requested: 2,
            limit: 1,
        })
    );
    assert_eq!(request.pending_frame_count(), 1);
    request.release_pending_frame().unwrap();
    execute(
        PATH,
        PATH,
        limits,
        &mut request,
        &mut ReductionStatistics::default(),
    )
    .unwrap();
    assert_eq!(request.pending_frame_count(), 0);
}

#[test]
fn coalescing_limit_is_global_across_monomials_and_counts_failed_attempt() {
    let target = [-3, -2, 2, 2, 3, 2];
    let mut baseline_request = ReductionRequest::default();
    let mut baseline_statistics = ReductionStatistics::default();
    execute(
        K3_TIMES_K1,
        target,
        ReductionLimits::default(),
        &mut baseline_request,
        &mut baseline_statistics,
    )
    .unwrap();
    let required = baseline_statistics.coalescing_additions();
    assert!(
        required > 1,
        "held-out correlated rank must exercise coalescing"
    );

    let limits = ReductionLimits {
        max_coalescing_additions: required - 1,
        ..ReductionLimits::default()
    };
    let mut failed_request = ReductionRequest::default();
    let mut failed_statistics = ReductionStatistics::default();
    assert_eq!(
        execute(
            K3_TIMES_K1,
            target,
            limits,
            &mut failed_request,
            &mut failed_statistics,
        ),
        Err(ReductionError::CoalescingAdditionLimit {
            requested: required,
            limit: required - 1,
        })
    );
    assert_eq!(failed_statistics.coalescing_additions(), required);
}

#[test]
fn post_angular_master_merge_obeys_the_request_wide_zero_limit() {
    let context = authority().family().coefficient_context();
    let master = IntegralKey::try_new(PATH).unwrap();
    let input = BTreeMap::from([(master.clone(), context.one())]);
    let mut output = BTreeMap::from([(master.clone(), context.one())]);
    let limits = ReductionLimits {
        max_coalescing_additions: 0,
        ..ReductionLimits::default()
    };
    let mut request = ReductionRequest::default();
    let mut statistics = ReductionStatistics::default();

    assert_eq!(
        add_weighted_terms(
            context,
            &mut output,
            &input,
            &context.one(),
            limits,
            &mut request,
            &mut statistics,
        ),
        Err(ReductionError::CoalescingAdditionLimit {
            requested: 1,
            limit: 0,
        })
    );
    assert_eq!(statistics.coalescing_additions(), 1);
    assert_coefficient(output.get(&master).unwrap(), "1");
}

#[test]
fn post_angular_master_merges_reject_the_one_beyond_limit_before_algebra() {
    let context = authority().family().coefficient_context();
    let first_master = IntegralKey::try_new(PATH).unwrap();
    let second_master = IntegralKey::try_new(STAR).unwrap();
    let input = BTreeMap::from([
        (first_master.clone(), context.one()),
        (second_master.clone(), context.one()),
    ]);
    let mut output = BTreeMap::from([
        (first_master.clone(), context.one()),
        (second_master.clone(), context.one()),
    ]);
    let limits = ReductionLimits {
        max_coalescing_additions: 1,
        ..ReductionLimits::default()
    };
    let mut request = ReductionRequest::default();
    let mut statistics = ReductionStatistics::default();

    assert_eq!(
        add_weighted_terms(
            context,
            &mut output,
            &input,
            &context.one(),
            limits,
            &mut request,
            &mut statistics,
        ),
        Err(ReductionError::CoalescingAdditionLimit {
            requested: 2,
            limit: 1,
        })
    );
    assert_eq!(statistics.coalescing_additions(), 2);
    assert_coefficient(output.get(&first_master).unwrap(), "2");
    assert_coefficient(output.get(&second_master).unwrap(), "1");
}
