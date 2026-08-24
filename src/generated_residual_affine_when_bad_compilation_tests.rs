//! Generated sunset coverage for the crate-private affine `WhenBad` authority.
//!
//! The fixture supplies no recurrence: its pending row is derived from the
//! ordinary generated IBP/LI, residual-branch, re-elimination, and target-
//! matching pipeline.

use std::sync::Arc;

use symbolica::domains::integer::Integer;

use crate::generated_residual_affine_condition_accumulator::{
    GeneratedResidualAffineConditionInputClass, GeneratedResidualAffineConditionRelationTerm,
    GeneratedResidualAffineConditionScope, GeneratedResidualAffineConditionSourceLocator,
};
use crate::generated_residual_affine_when_bad_compilation::{
    GeneratedResidualAffineSealedApplicationLimits, GeneratedResidualAffineSealedApplicationStats,
    GeneratedResidualAffineWhenBadApplicationError, GeneratedResidualAffineWhenBadCertificate,
    GeneratedResidualAffineWhenBadCompilation, GeneratedResidualAffineWhenBadCompiler,
    GeneratedResidualAffineWhenBadError, GeneratedResidualAffineWhenBadExceptionalKind,
    GeneratedResidualAffineWhenBadExceptionalLeafSourceError, GeneratedResidualAffineWhenBadLimits,
    GeneratedResidualAffineWhenBadPointError, GeneratedResidualAffineWhenBadPointLimits,
    GeneratedResidualAffineWhenBadPointStats, GeneratedResidualAffineWhenBadStats,
    authenticate_generated_residual_affine_when_bad_input,
    check_generated_affine_condition_comparison_limits,
    compile_generated_residual_affine_when_bad_conditions,
    generated_affine_condition_payload_preflight,
    preflight_generated_affine_private_payload_comparison,
};
use crate::generated_residual_affine_when_bad_descent::{
    GeneratedResidualAffineConstantTransitionKind, GeneratedResidualAffineTargetSectorDescentScope,
    GeneratedResidualAffineWhenBadDescentCompilation, GeneratedResidualAffineWhenBadDescentError,
    GeneratedResidualAffineWhenBadDescentReady, GeneratedResidualAffineWhenBadDescentStats,
    GeneratedResidualAffineWhenBadRhsDescentProof,
    compile_generated_residual_affine_when_bad_descent,
};
use crate::generated_sector_affine_effective_coverage::GeneratedSectorAffineSealedLeafAuthorization;
use crate::parametric_relation::{
    ParametricConcreteSpecializationLimits, ParametricConcreteSpecializationPreflight,
};
use crate::when_bad::prove_uniform_same_sector_descent;
use crate::{
    AffineDenominator, AffineParametricOrderingError, AffineParametricOrderingLimits,
    AffinePreparePointScheduleCertificate, AffinePreparePointScheduleLimits,
    AffineStartParametricEliminationOrdering, AffineStartReplayAuthority,
    AffineWhenBadRelativeCaseError, AffineWhenBadRelativeLeafDisposition, CoefficientContext,
    CoefficientPolynomial, ExactAlgebraError,
    GeneratedResidualAffineBranchReeliminationCompilation,
    GeneratedResidualAffineBranchReeliminationCompiler,
    GeneratedResidualAffineBranchReeliminationLimits,
    GeneratedResidualAffineCaseInventoryCertificate, GeneratedResidualAffineCaseInventoryCompiler,
    GeneratedResidualAffineCaseInventoryLimits, GeneratedResidualAffinePendingWhenBad,
    GeneratedResidualAffinePivotTargetMatchingCertificate,
    GeneratedResidualAffinePivotTargetMatchingCompiler,
    GeneratedResidualAffinePivotTargetMatchingLimits, GeneratedResidualAffinePivotTargetOutcome,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricIbpGenerator, ParametricRelationError, SectorMask,
};

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn generated_inventory(
    bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedResidualAffineCaseInventoryCertificate>,
) {
    let family = equal_mass_sunset(&format!("affine-when-bad-sunset-{bits}"));
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_from_bit_string(bits).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    let inventory = Arc::new(
        GeneratedResidualAffineCaseInventoryCompiler::compile(
            &family,
            &context,
            queue,
            GeneratedResidualAffineCaseInventoryLimits::default(),
        )
        .unwrap(),
    );
    (family, context, inventory)
}

fn generated_matcher_for_case(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    source_case_ordinal: usize,
) -> Option<GeneratedResidualAffinePivotTargetMatchingCertificate> {
    let case = &inventory.cases()[source_case_ordinal];
    let ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
        family,
        context,
        case.source_cover().clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        case.source_branch().clone(),
        AffineParametricOrderingLimits::default(),
    )
    .unwrap();
    let schedule = Arc::new(
        AffinePreparePointScheduleCertificate::compile_with_authority(
            AffineStartReplayAuthority::ResidualBooleanBranch {
                family,
                context,
                cover: case.source_cover(),
            },
            ordering,
            0,
            AffinePreparePointScheduleLimits::default(),
        )
        .unwrap(),
    );
    let compilation = GeneratedResidualAffineBranchReeliminationCompiler::compile(
        family,
        context,
        schedule,
        case.guard_composition().clone(),
        GeneratedResidualAffineBranchReeliminationLimits::default(),
    )
    .unwrap();
    let GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(reelimination) =
        compilation
    else {
        return None;
    };
    if reelimination.pivot_count() == 0 {
        return None;
    }
    Some(
        GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
            family,
            context,
            inventory.clone(),
            source_case_ordinal,
            Arc::new(reelimination),
            GeneratedResidualAffinePivotTargetMatchingLimits::default(),
        )
        .unwrap(),
    )
}

struct GeneratedPendingFixture {
    family: IntegralFamily,
    context: ParametricCoefficientContext,
    inventory: Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
    pivot_ordinal: usize,
    target_case_ordinal: usize,
    target_position: usize,
}

impl GeneratedPendingFixture {
    fn pending(&self) -> &GeneratedResidualAffinePendingWhenBad {
        match &self.matcher.outcomes()[self.pivot_ordinal] {
            GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) => pending,
            other => panic!("selected generated outcome is not pending: {other:?}"),
        }
    }
}

fn generated_pending_fixture() -> GeneratedPendingFixture {
    // The generated 001 sunset case is the smallest stable fixture which has
    // a pending row with nonempty translated guards and a matching target.
    let (family, context, inventory) = generated_inventory("001");
    let matcher = Arc::new(
        generated_matcher_for_case(&family, &context, &inventory, 0)
            .expect("001 sunset case 0 must produce an eliminated affine branch"),
    );
    let (pivot_ordinal, target_position, target_case_ordinal) = matcher
        .outcomes()
        .iter()
        .enumerate()
        .find_map(|(pivot_ordinal, outcome)| {
            let GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) = outcome
            else {
                return None;
            };
            pending
                .matching_target_case_ordinals()
                .iter()
                .enumerate()
                .next()
                .map(|(target_position, &target_case_ordinal)| {
                    (pivot_ordinal, target_position, target_case_ordinal)
                })
        })
        .expect("001 sunset must have a generated pending row with a matching target");
    assert_eq!(
        matcher.outcomes()[pivot_ordinal].pivot_ordinal(),
        pivot_ordinal
    );
    GeneratedPendingFixture {
        family,
        context,
        inventory,
        matcher,
        pivot_ordinal,
        target_case_ordinal,
        target_position,
    }
}

fn integer_magnitude_bits(value: &Integer) -> usize {
    match value {
        Integer::Single(value) => {
            usize::try_from(i64::BITS - value.unsigned_abs().leading_zeros()).unwrap()
        }
        Integer::Double(value) => {
            usize::try_from(i128::BITS - value.unsigned_abs().leading_zeros()).unwrap()
        }
        Integer::Large(value) => usize::try_from(value.significant_bits()).unwrap(),
    }
}

fn polynomial_payload(polynomial: &CoefficientPolynomial) -> (usize, usize, usize) {
    (
        polynomial.nterms(),
        polynomial.exponents.len(),
        polynomial
            .coefficients
            .iter()
            .map(integer_magnitude_bits)
            .sum(),
    )
}

fn add_payload(left: (usize, usize, usize), right: (usize, usize, usize)) -> (usize, usize, usize) {
    (left.0 + right.0, left.1 + right.1, left.2 + right.2)
}

fn exact_private_row_limits(
    stats: GeneratedResidualAffineWhenBadStats,
) -> GeneratedResidualAffineWhenBadLimits {
    let mut limits = GeneratedResidualAffineWhenBadLimits::default();
    limits.max_private_relation_terms = stats.private_relation_terms();
    limits.max_private_relation_guards = stats.private_relation_guards();
    limits.max_private_relation_origins = stats.private_relation_origins();
    limits.max_private_relation_manifest_bytes = stats.private_relation_manifest_bytes();
    limits.max_private_relation_shift_components = stats.private_relation_shift_components();
    limits.max_rhs_terms = stats.rhs_terms();
    limits.max_total_source_terms = stats.private_relation_source_terms();
    limits.max_total_source_exponent_entries = stats.private_relation_source_exponent_entries();
    limits.max_total_source_integer_bits = stats.private_relation_source_integer_bits();
    limits.max_retained_bytes = stats.retained_byte_envelope();
    limits
}

fn expect_when_bad_error<T>(
    result: Result<T, GeneratedResidualAffineWhenBadError>,
    case: &str,
) -> GeneratedResidualAffineWhenBadError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("{case} unexpectedly succeeded"),
    }
}

fn coefficient_resource_limit(error: ParametricCoefficientError) -> (&'static str, usize, usize) {
    match error {
        ParametricCoefficientError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        }) => (resource, requested, limit),
        other => panic!("expected a coefficient resource limit, got {other:?}"),
    }
}

fn when_bad_resource_limit(
    error: GeneratedResidualAffineWhenBadError,
) -> (&'static str, usize, usize) {
    match error {
        GeneratedResidualAffineWhenBadError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | GeneratedResidualAffineWhenBadError::Ordering(
            AffineParametricOrderingError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
        | GeneratedResidualAffineWhenBadError::Relation(ParametricRelationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
        | GeneratedResidualAffineWhenBadError::RelativePartition(
            AffineWhenBadRelativeCaseError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => (resource, requested, limit),
        GeneratedResidualAffineWhenBadError::ParametricCoefficient(error)
        | GeneratedResidualAffineWhenBadError::Relation(ParametricRelationError::Coefficient(
            error,
        )) => coefficient_resource_limit(error),
        other => panic!("expected a generated affine WhenBad resource limit, got {other:?}"),
    }
}

fn compile_generated_descent_fixture(
    fixture: &GeneratedPendingFixture,
    limits: GeneratedResidualAffineWhenBadLimits,
) -> Result<
    GeneratedResidualAffineWhenBadDescentCompilation,
    GeneratedResidualAffineWhenBadDescentError,
> {
    let input = authenticate_generated_residual_affine_when_bad_input(
        &fixture.family,
        &fixture.context,
        fixture.matcher.clone(),
        fixture.pivot_ordinal,
        fixture.target_case_ordinal,
        limits,
    )?;
    compile_generated_residual_affine_when_bad_descent(input)
}

pub(crate) fn generated_ready_fixture_for_pullback_gate() -> (
    ParametricCoefficientContext,
    GeneratedResidualAffineWhenBadDescentReady,
) {
    let fixture = generated_pending_fixture();
    let context = fixture.context.clone();
    let descent = compile_generated_descent_fixture(
        &fixture,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .expect("generated pullback/gate fixture must authenticate descent");
    let GeneratedResidualAffineWhenBadDescentCompilation::Ready(ready) = descent else {
        panic!("generated pullback/gate fixture must have complete authoritative descent")
    };
    (context, ready)
}

fn generated_descent_stats(
    compilation: &GeneratedResidualAffineWhenBadDescentCompilation,
) -> GeneratedResidualAffineWhenBadDescentStats {
    match compilation {
        GeneratedResidualAffineWhenBadDescentCompilation::Ready(ready) => ready.stats(),
        GeneratedResidualAffineWhenBadDescentCompilation::Unsupported(unsupported) => {
            unsupported.stats()
        }
    }
}

fn expect_descent_error(
    result: Result<
        GeneratedResidualAffineWhenBadDescentCompilation,
        GeneratedResidualAffineWhenBadDescentError,
    >,
    case: &str,
) -> GeneratedResidualAffineWhenBadDescentError {
    match result {
        Err(error) => error,
        Ok(compilation) => panic!("{case} unexpectedly succeeded: {compilation:?}"),
    }
}

fn descent_resource_limit(
    error: GeneratedResidualAffineWhenBadDescentError,
) -> (&'static str, usize, usize) {
    match error {
        GeneratedResidualAffineWhenBadDescentError::Authority(error) => {
            when_bad_resource_limit(error)
        }
        other => {
            panic!("expected a generated affine WhenBad descent resource limit, got {other:?}")
        }
    }
}

struct OneBelowCase {
    name: &'static str,
    observed: usize,
    set_limit: fn(&mut GeneratedResidualAffineWhenBadLimits, usize),
    accepted_resources: &'static [&'static str],
}

#[test]
fn generated_sunset_authority_binds_private_row_and_enforces_exact_aggregate_limits() {
    let fixture = generated_pending_fixture();
    let pending = fixture.pending();
    let target = &fixture.inventory.cases()[fixture.target_case_ordinal];
    assert!(pending.recentered_guard_count() > 0);
    assert!(Arc::ptr_eq(fixture.matcher.inventory(), &fixture.inventory));

    let baseline = authenticate_generated_residual_affine_when_bad_input(
        &fixture.family,
        &fixture.context,
        fixture.matcher.clone(),
        fixture.pivot_ordinal,
        fixture.target_case_ordinal,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap();
    let binding = baseline.binding();
    let stats = baseline.stats();

    assert!(Arc::ptr_eq(baseline.matcher(), &fixture.matcher));
    assert!(Arc::ptr_eq(baseline.target_cover(), target.source_cover()));
    assert!(Arc::ptr_eq(
        baseline.target_branch(),
        target.source_branch()
    ));
    assert!(Arc::ptr_eq(
        baseline.target_guard_composition(),
        target.guard_composition()
    ));
    assert!(Arc::ptr_eq(
        baseline.relation(),
        pending.relation_for_affine_when_bad()
    ));
    assert!(Arc::ptr_eq(
        baseline
            .target_ordering()
            .residual_branch()
            .expect("target ordering must retain its generated branch"),
        target.source_branch()
    ));

    assert_eq!(
        binding.source_case_ordinal(),
        fixture.matcher.source_case_ordinal()
    );
    assert_eq!(
        binding.source_group_ordinal(),
        fixture.matcher.source_group_ordinal()
    );
    assert_eq!(target.group_ordinal(), binding.source_group_ordinal());
    assert_eq!(binding.pivot_ordinal(), fixture.pivot_ordinal);
    assert_eq!(binding.target_case_ordinal(), fixture.target_case_ordinal);
    assert_eq!(
        binding.target_position_in_matching_list(),
        fixture.target_position
    );
    assert_eq!(binding.target_locator(), target.locator());
    assert_eq!(
        binding.target_ordinal_within_group(),
        target.ordinal_within_group()
    );
    assert_eq!(binding.sector(), baseline.target_ordering().sector());
    assert_eq!(binding.sector(), fixture.inventory.source_queue().sector());
    assert_eq!(
        binding.coefficient_translation(),
        pending.coefficient_translation()
    );
    assert_eq!(binding.key_center(), pending.key_center());
    assert_eq!(
        binding.target_ordering_manifest(),
        baseline.target_ordering().stable_manifest()
    );
    assert_eq!(
        binding.private_relation_manifest_bytes(),
        stats.private_relation_manifest_bytes()
    );
    assert_eq!(binding.rhs_terms(), stats.rhs_terms());

    let relation = baseline.relation();
    assert_eq!(stats.private_relation_terms(), relation.terms().len());
    assert_eq!(
        stats.private_relation_guards(),
        relation.guarded_nonzero_conditions().len()
    );
    assert_eq!(
        stats.private_relation_origins(),
        relation
            .guarded_nonzero_conditions()
            .iter()
            .map(|condition| condition.origins().len())
            .sum::<usize>()
    );
    assert_eq!(
        stats.private_relation_shift_components(),
        relation
            .terms()
            .keys()
            .map(|shift| shift.values().len())
            .sum::<usize>()
    );
    assert_eq!(
        stats.rhs_terms(),
        relation
            .terms()
            .keys()
            .filter(|shift| shift.values().iter().any(|&value| value != 0))
            .count()
    );

    let guard_payload = relation
        .guarded_nonzero_conditions()
        .iter()
        .map(|condition| polynomial_payload(condition.polynomial().raw()))
        .fold((0, 0, 0), add_payload);
    let coefficient_payload = relation
        .terms()
        .values()
        .map(|coefficient| {
            add_payload(
                polynomial_payload(&coefficient.raw().numerator),
                polynomial_payload(&coefficient.raw().denominator),
            )
        })
        .fold((0, 0, 0), add_payload);
    assert!(guard_payload.0 > 0);
    assert!(guard_payload.1 > 0);
    assert!(guard_payload.2 > 0);
    let complete_source_payload = add_payload(guard_payload, coefficient_payload);
    assert_eq!(
        stats.private_relation_source_terms(),
        complete_source_payload.0
    );
    assert_eq!(
        stats.private_relation_source_exponent_entries(),
        complete_source_payload.1
    );
    assert_eq!(
        stats.private_relation_source_integer_bits(),
        complete_source_payload.2
    );

    assert!(!baseline.private_relation_manifest().is_empty());
    assert_eq!(
        baseline.private_relation_manifest().len(),
        stats.private_relation_manifest_bytes()
    );
    let rebuilt_manifest = relation
        .stable_manifest_with_limit(stats.private_relation_manifest_bytes())
        .unwrap();
    assert_eq!(baseline.private_relation_manifest(), rebuilt_manifest);
    assert!(stats.retained_bytes() <= stats.retained_byte_envelope());

    let binding_debug = format!("{binding:?}");
    let pending_debug = format!("{pending:?}");
    assert!(binding_debug.contains("private_relation_manifest_bytes"));
    assert!(pending_debug.contains("private_relation: \"<redacted>\""));
    assert!(!binding_debug.contains(baseline.private_relation_manifest()));
    assert!(!pending_debug.contains(baseline.private_relation_manifest()));
    for forbidden in [
        "ParametricRelation",
        "ParametricCoefficient",
        "ParametricPolynomial",
        "generated-residual-affine-pending-when-bad:",
        "family_fingerprint: \"",
        "context_fingerprint: \"",
        "row_id: ",
        "raw:",
    ] {
        assert!(
            !binding_debug.contains(forbidden),
            "binding Debug leaked private marker {forbidden:?}: {binding_debug}"
        );
        assert!(
            !pending_debug.contains(forbidden),
            "pending Debug leaked private marker {forbidden:?}: {pending_debug}"
        );
    }

    let wrong_pivot = fixture.matcher.outcomes().len();
    assert_eq!(
        expect_when_bad_error(
            authenticate_generated_residual_affine_when_bad_input(
                &fixture.family,
                &fixture.context,
                fixture.matcher.clone(),
                wrong_pivot,
                fixture.target_case_ordinal,
                GeneratedResidualAffineWhenBadLimits::default(),
            ),
            "wrong pivot",
        ),
        GeneratedResidualAffineWhenBadError::PivotOrdinalOutOfRange {
            pivot_ordinal: wrong_pivot,
        }
    );
    let absent_target = fixture.inventory.cases().len();
    assert_eq!(
        expect_when_bad_error(
            authenticate_generated_residual_affine_when_bad_input(
                &fixture.family,
                &fixture.context,
                fixture.matcher.clone(),
                fixture.pivot_ordinal,
                absent_target,
                GeneratedResidualAffineWhenBadLimits::default(),
            ),
            "absent target",
        ),
        GeneratedResidualAffineWhenBadError::TargetNotInMatchingList {
            target_case_ordinal: absent_target,
        }
    );

    // The ordering manifest includes its effective manifest ceiling. Tighten
    // the outer retained-byte limit until that self-description reaches a
    // stable exact fixed point, then test the true exact/one-below boundary.
    let mut exact_limits = exact_private_row_limits(stats);
    let mut retained_limit = stats.retained_byte_envelope();
    let mut tightening_steps = 0usize;
    let exactly_bounded = loop {
        exact_limits.max_retained_bytes = retained_limit;
        let candidate = authenticate_generated_residual_affine_when_bad_input(
            &fixture.family,
            &fixture.context,
            fixture.matcher.clone(),
            fixture.pivot_ordinal,
            fixture.target_case_ordinal,
            exact_limits,
        )
        .unwrap();
        let next = candidate.stats().retained_byte_envelope();
        if next == retained_limit {
            break candidate;
        }
        assert!(next < retained_limit);
        retained_limit = next;
        tightening_steps += 1;
        assert!(
            tightening_steps <= 32,
            "retained-byte exact boundary did not reach a fixed point"
        );
    };
    let exact_stats = exactly_bounded.stats();
    assert_eq!(exactly_bounded.limits(), exact_limits);
    assert_eq!(exact_stats.retained_byte_envelope(), retained_limit);
    assert!(exact_stats.retained_bytes() <= retained_limit);
    for (observed, expected) in [
        (
            exact_stats.private_relation_terms(),
            stats.private_relation_terms(),
        ),
        (
            exact_stats.private_relation_guards(),
            stats.private_relation_guards(),
        ),
        (
            exact_stats.private_relation_origins(),
            stats.private_relation_origins(),
        ),
        (
            exact_stats.private_relation_manifest_bytes(),
            stats.private_relation_manifest_bytes(),
        ),
        (
            exact_stats.private_relation_shift_components(),
            stats.private_relation_shift_components(),
        ),
        (exact_stats.rhs_terms(), stats.rhs_terms()),
        (
            exact_stats.private_relation_source_terms(),
            stats.private_relation_source_terms(),
        ),
        (
            exact_stats.private_relation_source_exponent_entries(),
            stats.private_relation_source_exponent_entries(),
        ),
        (
            exact_stats.private_relation_source_integer_bits(),
            stats.private_relation_source_integer_bits(),
        ),
    ] {
        assert_eq!(observed, expected);
        assert!(observed > 0);
    }

    let one_below_cases = [
        OneBelowCase {
            name: "private relation terms",
            observed: exact_stats.private_relation_terms(),
            set_limit: |limits, value| limits.max_private_relation_terms = value,
            accepted_resources: &["generated affine WhenBad private relation terms"],
        },
        OneBelowCase {
            name: "private relation guards",
            observed: exact_stats.private_relation_guards(),
            set_limit: |limits, value| limits.max_private_relation_guards = value,
            accepted_resources: &["generated affine WhenBad private relation guards"],
        },
        OneBelowCase {
            name: "private relation origins",
            observed: exact_stats.private_relation_origins(),
            set_limit: |limits, value| limits.max_private_relation_origins = value,
            accepted_resources: &["generated affine WhenBad private relation origins"],
        },
        OneBelowCase {
            name: "private relation manifest bytes",
            observed: exact_stats.private_relation_manifest_bytes(),
            set_limit: |limits, value| limits.max_private_relation_manifest_bytes = value,
            accepted_resources: &["parametric relation manifest bytes"],
        },
        OneBelowCase {
            name: "private relation shift components",
            observed: exact_stats.private_relation_shift_components(),
            set_limit: |limits, value| limits.max_private_relation_shift_components = value,
            accepted_resources: &["generated affine WhenBad private relation shift components"],
        },
        OneBelowCase {
            name: "private RHS terms",
            observed: exact_stats.rhs_terms(),
            set_limit: |limits, value| limits.max_rhs_terms = value,
            accepted_resources: &[
                "generated affine WhenBad RHS term upper bound",
                "generated affine WhenBad RHS terms",
            ],
        },
        OneBelowCase {
            name: "aggregate private-row source terms",
            observed: exact_stats.private_relation_source_terms(),
            set_limit: |limits, value| limits.max_total_source_terms = value,
            accepted_resources: &[
                "parametric polynomial validation source terms",
                "parametric coefficient validation source terms",
            ],
        },
        OneBelowCase {
            name: "aggregate private-row source exponent entries",
            observed: exact_stats.private_relation_source_exponent_entries(),
            set_limit: |limits, value| limits.max_total_source_exponent_entries = value,
            accepted_resources: &[
                "parametric polynomial validation source exponent entries",
                "parametric coefficient validation source exponent entries",
            ],
        },
        OneBelowCase {
            name: "aggregate private-row source integer bits",
            observed: exact_stats.private_relation_source_integer_bits(),
            set_limit: |limits, value| limits.max_total_source_integer_bits = value,
            accepted_resources: &[
                "parametric polynomial validation source integer bits",
                "parametric coefficient validation source integer bits",
            ],
        },
        OneBelowCase {
            name: "retained bytes",
            observed: exact_stats.retained_byte_envelope(),
            set_limit: |limits, value| limits.max_retained_bytes = value,
            accepted_resources: &[
                "generated affine WhenBad retained bytes",
                "affine ordering manifest bytes",
            ],
        },
    ];
    for case in one_below_cases {
        assert!(case.observed > 0, "{} fixture must be nonzero", case.name);
        let mut limited = exact_limits;
        (case.set_limit)(&mut limited, case.observed - 1);
        let error = expect_when_bad_error(
            authenticate_generated_residual_affine_when_bad_input(
                &fixture.family,
                &fixture.context,
                fixture.matcher.clone(),
                fixture.pivot_ordinal,
                fixture.target_case_ordinal,
                limited,
            ),
            case.name,
        );
        let (resource, requested, limit) = when_bad_resource_limit(error);
        assert!(
            case.accepted_resources.contains(&resource),
            "{} returned unexpected resource {resource:?}",
            case.name
        );
        assert!(
            requested > limit,
            "{} did not exceed its admitted limit: requested={requested}, limit={limit}",
            case.name
        );
    }
}

#[test]
fn generated_sunset_descent_replays_btree_routes_and_complete_work_census() {
    let fixture = generated_pending_fixture();
    // Authenticate a fresh private input, then derive the expected descent
    // result independently from the exact generated relation's BTree order.
    // No recurrence coefficient or RHS shift is supplied by this test.
    let descent_input = authenticate_generated_residual_affine_when_bad_input(
        &fixture.family,
        &fixture.context,
        fixture.matcher.clone(),
        fixture.pivot_ordinal,
        fixture.target_case_ordinal,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap();
    let descent_arity = descent_input.target_ordering().arity();
    let constant_positions = descent_input.target_ordering().constant_positions().len();
    let target_manifest_bytes = descent_input.target_ordering().stable_manifest().len();
    let authenticated_retained_envelope = descent_input.stats().retained_byte_envelope();
    let authenticated_private_shift_components =
        descent_input.stats().private_relation_shift_components();
    let private_relation_terms = descent_input.relation().terms().len();
    let forbidden_relation_manifest = descent_input.private_relation_manifest().to_owned();
    let expected_descent = descent_input
        .relation()
        .terms()
        .iter()
        .filter(|(shift, _)| shift.values().iter().any(|&component| component != 0))
        .enumerate()
        .map(|(rhs_ordinal, (shift, _coefficient))| {
            (
                shift.clone(),
                prove_uniform_same_sector_descent(
                    descent_input.target_ordering().sector(),
                    rhs_ordinal,
                    shift,
                )
                .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(expected_descent.len(), descent_input.binding().rhs_terms());
    assert!(!expected_descent.is_empty());
    let forbidden_rhs_debug = expected_descent
        .iter()
        .map(|(shift, _)| format!("rhs_shift: {shift:?}"))
        .collect::<Vec<_>>();
    let descent = compile_generated_residual_affine_when_bad_descent(descent_input).unwrap();
    let descent_stats = generated_descent_stats(&descent);
    let rhs_count = expected_descent.len();
    let expected_components = rhs_count
        .checked_mul(descent_arity)
        .expect("generated fixture witness-component count must fit usize");
    let expected_aggregate_components = expected_components
        .checked_mul(6)
        .expect("six complete generated descent component classes must fit usize");
    assert_eq!(descent_stats.rhs_terms(), rhs_count);
    assert_eq!(descent_stats.descent_witnesses_precharged(), rhs_count);
    assert_eq!(
        descent_stats.descent_witness_components(),
        expected_components
    );
    assert_eq!(
        descent_stats.target_sector_rows_precharged(),
        expected_components
    );
    assert_eq!(
        descent_stats.target_sector_formal_mask_components_precharged(),
        expected_components
    );
    assert_eq!(
        descent_stats.target_sector_maximal_mask_components_precharged(),
        expected_components
    );
    assert_eq!(
        descent_stats.target_sector_constant_transition_components_precharged(),
        expected_components
    );
    assert_eq!(
        descent_stats.target_sector_activation_obligation_components_precharged(),
        expected_components
    );
    assert_eq!(
        descent_stats.aggregate_descent_components_precharged(),
        expected_aggregate_components
    );
    assert_eq!(
        descent_stats.target_ordering_manifest_bytes(),
        target_manifest_bytes
    );
    assert!(descent_stats.retained_bytes() <= descent_stats.retained_byte_envelope());
    assert!(descent_stats.retained_byte_envelope() >= authenticated_retained_envelope);

    let GeneratedResidualAffineWhenBadDescentCompilation::Ready(ready) = &descent else {
        panic!("the generated 001 target must be closed by authoritative descent routes")
    };
    assert!(!ready.is_directly_applicable_rule());
    assert_eq!(ready.private_rhs_proofs().len(), rhs_count);
    assert_eq!(descent_stats.descent_witnesses_attempted(), rhs_count);
    assert_eq!(descent_stats.descent_witnesses_proved(), rhs_count);

    let transcript = ready.private_target_sector_transcript();
    let mut next_same_sector = 0usize;
    let mut next_target_sector = 0usize;
    let mut transition_count = 0usize;
    let mut obligation_count = 0usize;
    let mut universal_pinches = 0usize;
    let mut universal_activations = 0usize;
    let mut whole_target = 0usize;
    let mut conditional_target = 0usize;
    let mut expected_target_integer_work_observed = 0usize;
    for (rhs_ordinal, proof) in ready.private_rhs_proofs().iter().copied().enumerate() {
        let (shift, same_sector_attempt) = &expected_descent[rhs_ordinal];
        match proof {
            GeneratedResidualAffineWhenBadRhsDescentProof::SameSector { witness_ordinal } => {
                assert_eq!(witness_ordinal, next_same_sector);
                let actual = &ready.private_witnesses()[witness_ordinal];
                let expected = same_sector_attempt.as_ref().expect(
                    "an authoritative same-sector route needs the independent signed witness",
                );
                assert_eq!(actual.rhs_ordinal(), rhs_ordinal);
                assert_eq!(actual.rhs_shift(), shift);
                assert_eq!(actual.decisive_component(), expected.decisive_component());
                assert_eq!(actual.corner_delta(), expected.corner_delta());
                assert_eq!(actual.dot_delta(), expected.dot_delta());
                assert_eq!(actual.numerator_delta(), expected.numerator_delta());
                assert_eq!(actual.index_excess_deltas(), expected.index_excess_deltas());
                next_same_sector += 1;
            }
            GeneratedResidualAffineWhenBadRhsDescentProof::TargetSector { witness_ordinal } => {
                assert_eq!(witness_ordinal, next_target_sector);
                assert!(same_sector_attempt.is_err());
                let witness = transcript.witnesses()[witness_ordinal];
                assert_eq!(witness.rhs_ordinal(), rhs_ordinal);
                let transitions = transcript.constant_transitions(witness).unwrap();
                let obligations = transcript.symbolic_activation_obligations(witness).unwrap();
                assert_eq!(
                    transcript.formal_sector_bits(witness).unwrap().len(),
                    descent_arity
                );
                assert_eq!(
                    transcript.maximal_sector_bits(witness).unwrap().len(),
                    descent_arity
                );
                transition_count += transitions.len();
                obligation_count += obligations.len();
                universal_pinches += transitions
                    .iter()
                    .filter(|transition| {
                        transition.kind()
                            == GeneratedResidualAffineConstantTransitionKind::UniversalActivePinch
                    })
                    .count();
                universal_activations += transitions
                    .iter()
                    .filter(|transition| {
                        transition.kind()
                            == GeneratedResidualAffineConstantTransitionKind::UniversalInactiveActivation
                    })
                    .count();
                match witness.scope() {
                    GeneratedResidualAffineTargetSectorDescentScope::WholeTarget => {
                        whole_target += 1
                    }
                    GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain => {
                        conditional_target += 1
                    }
                }
                for (constant_ordinal, &position) in ready
                    .input()
                    .target_ordering()
                    .constant_positions()
                    .iter()
                    .enumerate()
                {
                    let displacement = shift.values()[position];
                    expected_target_integer_work_observed += ready
                        .input()
                        .target_ordering()
                        .classify_constant_row_shift_by_ordinal(constant_ordinal, displacement)
                        .unwrap()
                        .integer_bit_work();
                    expected_target_integer_work_observed += ready
                        .input()
                        .target_ordering()
                        .replay_constant_row_shift_integer_bit_work_bound_by_ordinal(
                            constant_ordinal,
                            displacement,
                        )
                        .unwrap()
                        .1;
                }
                next_target_sector += 1;
            }
        }
    }
    assert_eq!(next_same_sector + next_target_sector, rhs_count);
    assert_eq!(next_same_sector, ready.private_witnesses().len());
    assert_eq!(next_target_sector, transcript.witnesses().len());
    assert_eq!(whole_target + conditional_target, next_target_sector);
    assert_eq!(
        ready.requires_symbolic_activation_hazard_seal(),
        conditional_target > 0
    );

    let relation_components = private_relation_terms * descent_arity;
    assert_eq!(authenticated_private_shift_components, relation_components);
    let rhs_components = rhs_count * descent_arity;
    let expected_private_shift_precharged = authenticated_private_shift_components
        + 3 * relation_components
        + 4 * rhs_components
        + rhs_count * constant_positions;
    let expected_private_shift_observed = authenticated_private_shift_components
        + 3 * relation_components
        + 3 * rhs_components
        + next_target_sector * descent_arity
        + rhs_count * constant_positions;
    assert_eq!(
        descent_stats.private_rhs_shift_components_precharged(),
        expected_private_shift_precharged
    );
    assert_eq!(
        descent_stats.private_rhs_shift_components_observed(),
        expected_private_shift_observed
    );
    let expected_payload_precharged =
        128 * rhs_components + 8 * rhs_count * constant_positions + 64 * rhs_count + 16;
    assert_eq!(
        descent_stats.payload_comparison_units_precharged(),
        expected_payload_precharged
    );
    assert!(descent_stats.payload_comparison_units_observed() > 0);
    assert!(
        descent_stats.payload_comparison_units_observed()
            <= descent_stats.payload_comparison_units_precharged()
    );
    assert_eq!(
        descent_stats.descent_witness_components_observed(),
        rhs_components
    );
    assert_eq!(
        descent_stats.target_sector_rows_observed(),
        next_target_sector * descent_arity
    );
    assert_eq!(
        descent_stats.target_sector_formal_mask_components_observed(),
        next_target_sector * descent_arity
    );
    assert_eq!(
        descent_stats.target_sector_maximal_mask_components_observed(),
        next_target_sector * descent_arity
    );
    assert_eq!(
        descent_stats.target_sector_constant_transition_components_observed(),
        transition_count
    );
    assert_eq!(
        descent_stats.target_sector_activation_obligation_components_observed(),
        obligation_count
    );
    assert_eq!(
        descent_stats.aggregate_descent_components_observed(),
        rhs_components
            + 3 * next_target_sector * descent_arity
            + transition_count
            + obligation_count
    );
    assert_eq!(
        descent_stats.target_sector_constant_additions_precharged(),
        rhs_count * constant_positions
    );
    assert_eq!(
        descent_stats.target_sector_constant_additions_observed(),
        next_target_sector * constant_positions
    );
    let expected_target_integer_work_precharged = expected_descent
        .iter()
        .map(|(shift, _)| {
            ready
                .input()
                .target_ordering()
                .constant_positions()
                .iter()
                .enumerate()
                .map(|(constant_ordinal, &position)| {
                    let displacement = shift.values()[position];
                    ready
                        .input()
                        .target_ordering()
                        .constant_row_shift_integer_bit_work_bound_by_ordinal(
                            constant_ordinal,
                            displacement,
                        )
                        .unwrap()
                        .1
                        + ready
                            .input()
                            .target_ordering()
                            .replay_constant_row_shift_integer_bit_work_bound_by_ordinal(
                                constant_ordinal,
                                displacement,
                            )
                            .unwrap()
                            .1
                })
                .sum::<usize>()
        })
        .sum::<usize>();
    assert_eq!(
        descent_stats.target_sector_integer_bit_work_precharged(),
        expected_target_integer_work_precharged
    );
    assert_eq!(
        descent_stats.target_sector_integer_bit_work_observed(),
        expected_target_integer_work_observed
    );
    assert_eq!(
        descent_stats.target_sector_fallbacks_attempted(),
        next_target_sector
    );
    assert_eq!(
        descent_stats.target_sector_fallbacks_proved(),
        next_target_sector
    );
    assert_eq!(
        descent_stats.target_sector_whole_target_proved(),
        whole_target
    );
    assert_eq!(
        descent_stats.target_sector_applicable_nonzero_term_domain_proved(),
        conditional_target
    );
    assert_eq!(
        descent_stats.target_sector_constant_rows_inspected(),
        next_target_sector * constant_positions
    );
    assert_eq!(
        descent_stats.target_sector_symbolic_rows_inspected(),
        next_target_sector * (descent_arity - constant_positions)
    );
    assert_eq!(
        descent_stats.target_sector_universal_active_pinches(),
        universal_pinches
    );
    assert_eq!(
        descent_stats.target_sector_universal_inactive_activations(),
        universal_activations
    );
    assert_eq!(
        descent_stats.target_sector_symbolic_activation_obligations(),
        obligation_count
    );
    let descent_debug = format!("{descent:?}");
    for forbidden in &forbidden_rhs_debug {
        assert!(
            !descent_debug.contains(forbidden),
            "matcher-bound descent Debug leaked a generated private RHS: {descent_debug}"
        );
    }
    assert!(!descent_debug.contains(forbidden_relation_manifest.as_str()));
}

#[test]
fn generated_sunset_descent_structural_resource_boundaries() {
    let fixture = generated_pending_fixture();
    let baseline = compile_generated_descent_fixture(
        &fixture,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap();
    let descent_stats = generated_descent_stats(&baseline);

    // The exact authenticated counts must be admitted.  Their one-below
    // counterparts fail during aggregate precharge, before any core descent
    // proof can be attempted.
    let rhs_count = descent_stats.rhs_terms();
    assert!(rhs_count > 0);
    let expected_components = descent_stats.descent_witness_components();
    let expected_aggregate_components = descent_stats.aggregate_descent_components_precharged();
    let mut count_exact = GeneratedResidualAffineWhenBadLimits::default();
    count_exact.max_rhs_terms = rhs_count;
    count_exact.max_descent_witnesses = rhs_count;
    count_exact.max_descent_witness_components = expected_aggregate_components;
    let count_exact_compilation = compile_generated_descent_fixture(&fixture, count_exact).unwrap();
    let count_exact_stats = generated_descent_stats(&count_exact_compilation);
    assert_eq!(
        (
            count_exact_stats.rhs_terms(),
            count_exact_stats.descent_witnesses_precharged(),
            count_exact_stats.descent_witness_components(),
            count_exact_stats.aggregate_descent_components_precharged(),
        ),
        (
            rhs_count,
            rhs_count,
            expected_components,
            expected_aggregate_components,
        )
    );
    let mut rhs_one_below = count_exact;
    rhs_one_below.max_rhs_terms = rhs_count - 1;
    // Also make the later witness ceiling impossible: observing the RHS
    // resource proves that this earlier precharge has priority.
    rhs_one_below.max_descent_witnesses = 0;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, rhs_one_below),
        "one-below descent RHS limit",
    ));
    assert!(
        [
            "generated affine WhenBad RHS term upper bound",
            "generated affine WhenBad RHS terms",
            "generated affine WhenBad descent RHS terms",
        ]
        .contains(&resource)
    );
    assert!(requested > limit);

    let mut witness_one_below = count_exact;
    witness_one_below.max_descent_witnesses = rhs_count - 1;
    // This impossible later ceiling makes the check ordering observable.
    witness_one_below.max_descent_witness_components = 0;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, witness_one_below),
        "one-below descent witness limit",
    ));
    assert_eq!(resource, "generated affine WhenBad descent witnesses");
    assert_eq!(requested, rhs_count);
    assert_eq!(limit, rhs_count - 1);

    assert!(expected_components > 0);
    let mut component_one_below = count_exact;
    component_one_below.max_descent_witness_components = expected_components - 1;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, component_one_below),
        "one-below descent component limit",
    ));
    assert_eq!(
        resource,
        "generated affine WhenBad descent witness components"
    );
    assert_eq!(requested, expected_components);
    assert_eq!(limit, expected_components - 1);

    assert!(expected_aggregate_components > expected_components);
    let mut aggregate_one_below = count_exact;
    aggregate_one_below.max_descent_witness_components = expected_aggregate_components - 1;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, aggregate_one_below),
        "one-below aggregate descent component limit",
    ));
    assert_eq!(
        resource,
        "generated affine WhenBad aggregate descent components"
    );
    assert_eq!(requested, expected_aggregate_components);
    assert_eq!(limit, expected_aggregate_components - 1);
}

#[test]
fn generated_sunset_descent_work_resource_boundaries() {
    let fixture = generated_pending_fixture();
    let authority = authenticate_generated_residual_affine_when_bad_input(
        &fixture.family,
        &fixture.context,
        fixture.matcher.clone(),
        fixture.pivot_ordinal,
        fixture.target_case_ordinal,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap();
    let inherited_target_integer_bits = authority.stats().target_constant_comparison_integer_bits();
    drop(authority);
    let baseline = compile_generated_descent_fixture(
        &fixture,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap();
    let descent_stats = generated_descent_stats(&baseline);

    let exact_private_shift_components = descent_stats.private_rhs_shift_components_precharged();
    assert!(exact_private_shift_components > 0);
    let mut private_shift_exact = GeneratedResidualAffineWhenBadLimits::default();
    private_shift_exact.max_private_relation_shift_components = exact_private_shift_components;
    let exact = compile_generated_descent_fixture(&fixture, private_shift_exact).unwrap();
    assert_eq!(
        generated_descent_stats(&exact).private_rhs_shift_components_precharged(),
        exact_private_shift_components
    );
    private_shift_exact.max_private_relation_shift_components = exact_private_shift_components - 1;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, private_shift_exact),
        "one-below private shift-component limit",
    ));
    assert_eq!(
        resource,
        "generated affine WhenBad private RHS shift components"
    );
    assert_eq!(requested, exact_private_shift_components);
    assert_eq!(limit, exact_private_shift_components - 1);

    let exact_payload_units = descent_stats.payload_comparison_units_precharged();
    assert!(exact_payload_units > 0);
    let mut payload_exact = GeneratedResidualAffineWhenBadLimits::default();
    payload_exact.max_payload_comparison_units = exact_payload_units;
    let exact = compile_generated_descent_fixture(&fixture, payload_exact).unwrap();
    assert_eq!(
        generated_descent_stats(&exact).payload_comparison_units_precharged(),
        exact_payload_units
    );
    payload_exact.max_payload_comparison_units = exact_payload_units - 1;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, payload_exact),
        "one-below payload-comparison limit",
    ));
    assert_eq!(
        resource,
        "generated affine WhenBad payload comparison units"
    );
    assert_eq!(requested, exact_payload_units);
    assert_eq!(limit, exact_payload_units - 1);

    let local_target_integer_bits = descent_stats.target_sector_integer_bit_work_precharged();
    // This generated target currently has no constant target row, so its
    // descent-local contribution may be zero.  The public ceiling is shared
    // with the already-authenticated target-ordering comparisons; exercise
    // the exact aggregate boundary instead of assuming a particular route or
    // target-row shape.  Dedicated target-sector unit fixtures cover positive
    // local observed work.
    let exact_target_integer_bits = inherited_target_integer_bits
        .checked_add(local_target_integer_bits)
        .unwrap();
    let mut integer_exact = GeneratedResidualAffineWhenBadLimits::default();
    integer_exact.max_target_constant_comparison_integer_bits = exact_target_integer_bits;
    let exact = compile_generated_descent_fixture(&fixture, integer_exact).unwrap();
    assert_eq!(
        inherited_target_integer_bits
            + generated_descent_stats(&exact).target_sector_integer_bit_work_precharged(),
        exact_target_integer_bits
    );
    // Zero is itself the exact boundary when this generated target has no
    // constant rows; there is no representable usize one-below value.  If a
    // future generated fixture contributes positive work, keep exercising the
    // strict one-below error as well.
    if exact_target_integer_bits == 0 {
        return;
    }
    integer_exact.max_target_constant_comparison_integer_bits = exact_target_integer_bits - 1;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, integer_exact),
        "one-below target-sector integer-bit work limit",
    ));
    assert_eq!(
        resource,
        "generated affine WhenBad target constant comparison integer bits"
    );
    assert_eq!(requested, exact_target_integer_bits);
    assert_eq!(limit, exact_target_integer_bits - 1);
}

#[test]
fn generated_sunset_descent_retained_byte_boundary() {
    let fixture = generated_pending_fixture();
    let baseline = compile_generated_descent_fixture(
        &fixture,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap();
    let descent_stats = generated_descent_stats(&baseline);
    let rhs_count = descent_stats.rhs_terms();

    // As in the authority test above, the ordering manifest describes its
    // admitted ceiling.  Iterate downward to the exact matcher-bound descent
    // fixed point, then verify the retained envelope's true one-below edge.
    let mut descent_retained_limit = descent_stats.retained_byte_envelope();
    let mut descent_tightening_steps = 0usize;
    let exactly_retained_descent = loop {
        let mut limits = GeneratedResidualAffineWhenBadLimits::default();
        limits.max_retained_bytes = descent_retained_limit;
        let candidate = compile_generated_descent_fixture(&fixture, limits).unwrap();
        let next = generated_descent_stats(&candidate).retained_byte_envelope();
        if next == descent_retained_limit {
            break candidate;
        }
        assert!(next < descent_retained_limit);
        descent_retained_limit = next;
        descent_tightening_steps += 1;
        assert!(
            descent_tightening_steps <= 32,
            "descent retained-byte exact boundary did not reach a fixed point"
        );
    };
    let exact_descent_stats = generated_descent_stats(&exactly_retained_descent);
    assert_eq!(
        exact_descent_stats.retained_byte_envelope(),
        descent_retained_limit
    );
    assert!(exact_descent_stats.retained_bytes() <= descent_retained_limit);
    assert_eq!(
        exact_descent_stats.descent_witnesses_precharged(),
        rhs_count
    );
    assert!(descent_retained_limit > 0);
    let mut retained_one_below = GeneratedResidualAffineWhenBadLimits::default();
    retained_one_below.max_retained_bytes = descent_retained_limit - 1;
    let (resource, requested, limit) = descent_resource_limit(expect_descent_error(
        compile_generated_descent_fixture(&fixture, retained_one_below),
        "one-below descent retained-byte limit",
    ));
    assert!(
        [
            "generated affine WhenBad retained bytes",
            "affine ordering manifest bytes",
            "generated affine WhenBad descent retained bytes",
        ]
        .contains(&resource),
        "unexpected retained-byte resource {resource:?}"
    );
    assert!(requested > limit);
}

#[test]
fn generated_sunset_conditions_follow_authenticated_litered_order_and_rebuild_exactly() {
    let fixture = generated_pending_fixture();
    let limits = GeneratedResidualAffineWhenBadLimits::default();
    let descent = compile_generated_descent_fixture(&fixture, limits).unwrap();
    let GeneratedResidualAffineWhenBadDescentCompilation::Ready(ready) = descent else {
        panic!("generated 001 target must pass descent before condition compilation")
    };
    let remaining = limits
        .max_retained_bytes
        .checked_sub(ready.stats().retained_bytes())
        .unwrap();
    let payload_units_remaining = limits
        .max_payload_comparison_units
        .checked_sub(ready.stats().payload_comparison_units_observed())
        .unwrap();
    let private_payload_bytes =
        preflight_generated_affine_private_payload_comparison(ready.input(), ready.input())
            .unwrap();
    let payload_bytes_remaining = limits
        .max_payload_comparison_bytes
        .checked_sub(private_payload_bytes)
        .unwrap();
    let certificate = compile_generated_residual_affine_when_bad_conditions(
        &fixture.context,
        ready.input(),
        remaining,
        payload_units_remaining,
        payload_bytes_remaining,
    )
    .unwrap();

    let target_guards = ready.input().target_guard_composition().entries().len();
    let relation_guards = ready.input().relation().guarded_nonzero_conditions().len();
    let coefficient_count = ready.input().relation().terms().len();
    assert_eq!(
        certificate.inputs().len(),
        target_guards + relation_guards + coefficient_count
    );
    for (entry_ordinal, input) in certificate.inputs()[..target_guards].iter().enumerate() {
        assert_eq!(
            input.scope(),
            GeneratedResidualAffineConditionScope::InheritedTargetPremise
        );
        assert!(matches!(
            input.source().locator(),
            GeneratedResidualAffineConditionSourceLocator::TargetBranchGuard {
                entry_ordinal: actual,
                ..
            } if actual == entry_ordinal
        ));
    }
    for (guard_ordinal, input) in certificate.inputs()
        [target_guards..target_guards + relation_guards]
        .iter()
        .enumerate()
    {
        assert_eq!(
            input.scope(),
            GeneratedResidualAffineConditionScope::CandidateRequired
        );
        assert!(matches!(
            input.source().locator(),
            GeneratedResidualAffineConditionSourceLocator::RecenteredRelationGuard {
                guard_ordinal: actual,
            } if actual == guard_ordinal
        ));
    }
    let denominator_inputs =
        &certificate.inputs()[target_guards + relation_guards..certificate.inputs().len()];
    assert!(matches!(
        denominator_inputs[0].source().locator(),
        GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
            term: GeneratedResidualAffineConditionRelationTerm::Pivot,
        }
    ));
    for (rhs_ordinal, input) in denominator_inputs[1..].iter().enumerate() {
        assert!(matches!(
            input.source().locator(),
            GeneratedResidualAffineConditionSourceLocator::CoefficientDenominator {
                term: GeneratedResidualAffineConditionRelationTerm::Rhs {
                    rhs_ordinal: actual,
                },
            } if actual == rhs_ordinal
        ));
        assert!(input.source().private_shift().is_some());
    }
    assert!(certificate.inputs().iter().all(|input| !matches!(
        input.class(),
        GeneratedResidualAffineConditionInputClass::IdenticallyZeroCandidate
    )));
    assert!(!certificate.candidate_is_identically_bad());
    assert!(certificate.rows().iter().all(|row| {
        row.source_input_ordinals()
            .iter()
            .all(|&ordinal| ordinal < certificate.inputs().len())
    }));

    let rebuilt = compile_generated_residual_affine_when_bad_conditions(
        &fixture.context,
        ready.input(),
        remaining,
        payload_units_remaining,
        payload_bytes_remaining,
    )
    .unwrap();
    assert_eq!(certificate, rebuilt);
    let debug = format!("{certificate:?}");
    assert!(debug.contains("private_payload") || debug.contains("condition"));
    assert!(!debug.contains(ready.input().private_relation_manifest()));
    assert!(!debug.contains("ParametricRelation"));
}

#[test]
fn generated_condition_aggregate_preflights_have_exact_one_below_edges() {
    let fixture = generated_pending_fixture();
    let limits = GeneratedResidualAffineWhenBadLimits::default();
    let descent = compile_generated_descent_fixture(&fixture, limits).unwrap();
    let GeneratedResidualAffineWhenBadDescentCompilation::Ready(ready) = descent else {
        panic!("generated 001 target must pass descent before condition compilation")
    };
    let retained_remaining = limits
        .max_retained_bytes
        .checked_sub(ready.stats().retained_bytes())
        .unwrap();
    let private_payload_bytes =
        preflight_generated_affine_private_payload_comparison(ready.input(), ready.input())
            .unwrap();
    let payload_bytes_remaining = limits
        .max_payload_comparison_bytes
        .checked_sub(private_payload_bytes)
        .unwrap();
    let total_inputs = ready.input().target_guard_composition().entries().len()
        + ready.input().relation().guarded_nonzero_conditions().len()
        + ready.input().relation().terms().len();
    let payload_units =
        generated_affine_condition_payload_preflight(&fixture.context, ready.input(), total_inputs)
            .unwrap()
            .total_units()
            .unwrap();
    assert!(payload_units > 0);

    let certificate = compile_generated_residual_affine_when_bad_conditions(
        &fixture.context,
        ready.input(),
        retained_remaining,
        payload_units,
        payload_bytes_remaining,
    )
    .unwrap();
    let (resource, requested, limit) = when_bad_resource_limit(expect_when_bad_error(
        compile_generated_residual_affine_when_bad_conditions(
            &fixture.context,
            ready.input(),
            retained_remaining,
            payload_units - 1,
            payload_bytes_remaining,
        ),
        "one-below condition aggregate payload preflight",
    ));
    assert_eq!(
        resource,
        "generated affine WhenBad condition payload comparison units"
    );
    assert_eq!(requested, payload_units);
    assert_eq!(limit + 1, payload_units);

    let stats = certificate.stats();
    let checks = stats.equality_comparisons() + stats.associate_checks();
    let term_pairs = stats.equality_term_units() + stats.associate_term_units();
    let exponent_entries = stats.equality_exponent_entries() + stats.associate_exponent_entries();
    let integer_bits = stats.equality_integer_bits() + stats.associate_integer_bits();
    let mut comparison_exact = GeneratedResidualAffineWhenBadLimits::default();
    comparison_exact.max_associate_checks = checks;
    comparison_exact.max_associate_term_pairs = term_pairs;
    comparison_exact.max_associate_exponent_entries = exponent_entries;
    comparison_exact.max_associate_integer_bits = integer_bits;
    check_generated_affine_condition_comparison_limits(stats, comparison_exact).unwrap();
    for (resource, observed, set_one_below) in [
        ("generated affine WhenBad associate checks", checks, 0usize),
        (
            "generated affine WhenBad associate term pairs",
            term_pairs,
            1usize,
        ),
        (
            "generated affine WhenBad associate exponent entries",
            exponent_entries,
            2usize,
        ),
        (
            "generated affine WhenBad associate integer bits",
            integer_bits,
            3usize,
        ),
    ] {
        assert!(observed > 0, "{resource} fixture must be nonzero");
        let mut one_below = comparison_exact;
        match set_one_below {
            0 => one_below.max_associate_checks = observed - 1,
            1 => one_below.max_associate_term_pairs = observed - 1,
            2 => one_below.max_associate_exponent_entries = observed - 1,
            3 => one_below.max_associate_integer_bits = observed - 1,
            _ => unreachable!(),
        }
        let (actual_resource, requested, limit) = when_bad_resource_limit(
            check_generated_affine_condition_comparison_limits(stats, one_below).unwrap_err(),
        );
        assert_eq!(actual_resource, resource);
        assert_eq!(requested, observed);
        assert_eq!(limit + 1, observed);
    }

    let exact_condition_bytes = stats.context_fingerprint_comparison_bytes();
    assert!(exact_condition_bytes > 0);
    compile_generated_residual_affine_when_bad_conditions(
        &fixture.context,
        ready.input(),
        retained_remaining,
        payload_units,
        exact_condition_bytes,
    )
    .unwrap();
    let (_, requested, limit) = when_bad_resource_limit(expect_when_bad_error(
        compile_generated_residual_affine_when_bad_conditions(
            &fixture.context,
            ready.input(),
            retained_remaining,
            payload_units,
            exact_condition_bytes - 1,
        ),
        "one-below condition comparison bytes",
    ));
    assert!(requested > limit);
}

#[test]
fn generated_outer_retained_bytes_accept_exact_and_reject_one_below() {
    let fixture = generated_pending_fixture();
    let compile = |limits| {
        GeneratedResidualAffineWhenBadCompiler::compile(
            &fixture.family,
            &fixture.context,
            fixture.matcher.clone(),
            fixture.pivot_ordinal,
            fixture.target_case_ordinal,
            limits,
        )
    };
    let baseline = compile(GeneratedResidualAffineWhenBadLimits::default()).unwrap();
    assert!(matches!(
        baseline,
        GeneratedResidualAffineWhenBadCompilation::Certified(_)
    ));
    let mut retained_limit = baseline.stats().retained_bytes();
    assert!(retained_limit > 0);

    // The authenticated ordering manifest contains the configured ceiling,
    // so tightening it can shorten the retained manifest.  Iterate to the
    // exact matcher-bound ownership fixed point before testing one below.
    let mut tightening_steps = 0usize;
    let rebuilt = loop {
        let mut exact = GeneratedResidualAffineWhenBadLimits::default();
        exact.max_retained_bytes = retained_limit;
        let candidate = compile(exact).unwrap();
        let next = candidate.stats().retained_bytes();
        if next == retained_limit {
            break candidate;
        }
        assert!(next < retained_limit);
        retained_limit = next;
        tightening_steps += 1;
        assert!(
            tightening_steps <= 32,
            "outer retained-byte exact boundary did not reach a fixed point"
        );
    };
    assert!(matches!(
        rebuilt,
        GeneratedResidualAffineWhenBadCompilation::Certified(_)
    ));
    assert_eq!(rebuilt.stats().retained_bytes(), retained_limit);

    let mut one_below = GeneratedResidualAffineWhenBadLimits::default();
    one_below.max_retained_bytes = retained_limit - 1;
    let (resource, requested, limit) = when_bad_resource_limit(expect_when_bad_error(
        compile(one_below),
        "outer retained one below",
    ));
    assert!(
        resource.contains("retained bytes") || resource.contains("manifest bytes"),
        "unexpected retained resource {resource:?}"
    );
    assert!(requested > limit);
}

#[test]
fn generated_outer_boundary_limit_is_a_hard_error() {
    let fixture = generated_pending_fixture();
    let mut limits = GeneratedResidualAffineWhenBadLimits::default();
    limits.max_boundary_values = 5;
    let (resource, requested, limit) = when_bad_resource_limit(expect_when_bad_error(
        GeneratedResidualAffineWhenBadCompiler::compile(
            &fixture.family,
            &fixture.context,
            fixture.matcher.clone(),
            fixture.pivot_ordinal,
            fixture.target_case_ordinal,
            limits,
        ),
        "outer boundary hard limit",
    ));
    assert_eq!(resource, "generated affine pullback/gate boundary values");
    assert_eq!((requested, limit), (6, 5));
}

#[test]
fn generated_fixture_outer_when_bad_is_transactional_and_replays_exactly() {
    let fixture = generated_pending_fixture();
    let matching_targets_before = fixture.pending().matching_target_case_ordinals().to_vec();
    let compilation = GeneratedResidualAffineWhenBadCompiler::compile(
        &fixture.family,
        &fixture.context,
        fixture.matcher.clone(),
        fixture.pivot_ordinal,
        fixture.target_case_ordinal,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap();
    compilation
        .replay(&fixture.family, &fixture.context)
        .unwrap();
    assert_eq!(
        fixture.pending().matching_target_case_ordinals(),
        matching_targets_before
    );
    assert_eq!(
        compilation.binding().target_case_ordinal(),
        fixture.target_case_ordinal
    );
    match &compilation {
        GeneratedResidualAffineWhenBadCompilation::Certified(certificate) => {
            assert!(certificate.stats().applicable_leaves() > 0);
            assert_eq!(
                certificate.leaf_classifications().len(),
                certificate.stats().applicable_leaves() + certificate.stats().exceptional_leaves()
            );
        }
        GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(outcome) => {
            assert_eq!(outcome.stats().applicable_leaves(), 0);
        }
        GeneratedResidualAffineWhenBadCompilation::Unsupported(outcome) => {
            panic!(
                "generated fixture passed the independently tested descent phase but outer returned unsupported: {:?}",
                outcome.reason()
            );
        }
    }
    let debug = format!("{compilation:?}");
    assert!(debug.contains("private_payload: \"<redacted>\""));
    assert!(!debug.contains("ParametricRelation"));
    assert!(!debug.contains("boundary_value: "));
}

fn compile_generated_point_query_certificate(
    fixture: &GeneratedPendingFixture,
) -> GeneratedResidualAffineWhenBadCertificate {
    match GeneratedResidualAffineWhenBadCompiler::compile(
        &fixture.family,
        &fixture.context,
        fixture.matcher.clone(),
        fixture.pivot_ordinal,
        fixture.target_case_ordinal,
        GeneratedResidualAffineWhenBadLimits::default(),
    )
    .unwrap()
    {
        GeneratedResidualAffineWhenBadCompilation::Certified(certificate) => certificate,
        other => panic!("generated point-query fixture is not certified: {other:?}"),
    }
}

fn generated_exceptional_kind(
    disposition: AffineWhenBadRelativeLeafDisposition,
) -> Option<GeneratedResidualAffineWhenBadExceptionalKind> {
    match disposition {
        AffineWhenBadRelativeLeafDisposition::Applicable => None,
        AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { condition_ordinal } => {
            Some(GeneratedResidualAffineWhenBadExceptionalKind::Domain { condition_ordinal })
        }
        AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal } => {
            Some(GeneratedResidualAffineWhenBadExceptionalKind::Leak { pullback_ordinal })
        }
    }
}

#[test]
fn exceptional_leaf_source_view_borrows_the_exact_case_and_redacts_predicates() {
    let fixture = generated_pending_fixture();
    let certificate = compile_generated_point_query_certificate(&fixture);
    let (leaf_ordinal, classification, expected_kind) = certificate
        .leaf_classifications()
        .iter()
        .enumerate()
        .find_map(|(leaf_ordinal, classification)| {
            generated_exceptional_kind(classification.disposition())
                .map(|kind| (leaf_ordinal, classification, kind))
        })
        .expect("generated fixture must retain an exceptional leaf");

    let view = certificate
        .exceptional_leaf_source_view(leaf_ordinal, classification.case(), expected_kind)
        .unwrap();
    assert_eq!(view.leaf_ordinal(), leaf_ordinal);
    assert_eq!(view.relative_case().id(), classification.case());
    assert_eq!(view.kind(), expected_kind);
    assert!(std::ptr::eq(
        view.predicates(),
        view.relative_case().predicates(),
    ));

    let debug = format!("{view:?}");
    assert!(debug.contains("private_predicates: \"<redacted>\""));
    assert!(debug.contains(&format!("predicate_count: {}", view.predicates().len())));
    assert!(!debug.contains("polynomial:"));
    assert!(!debug.contains("locus_ordinal:"));
    assert!(!debug.contains("ParametricPolynomial"));
    assert!(!debug.contains("ParametricRelation"));
    assert!(!debug.contains("condition_count"));
    assert!(!debug.contains("pullback_count"));
}

#[test]
fn exceptional_leaf_source_view_rejects_tampered_leaf_case_and_kind() {
    let fixture = generated_pending_fixture();
    let certificate = compile_generated_point_query_certificate(&fixture);
    let (leaf_ordinal, classification, retained_kind) = certificate
        .leaf_classifications()
        .iter()
        .enumerate()
        .find_map(|(leaf_ordinal, classification)| {
            generated_exceptional_kind(classification.disposition())
                .map(|kind| (leaf_ordinal, classification, kind))
        })
        .expect("generated fixture must retain an exceptional leaf");

    let wrong_case = certificate
        .leaf_classifications()
        .iter()
        .map(|classification| classification.case())
        .find(|&case| case != classification.case())
        .expect("generated fixture must retain more than one relative case");
    assert_eq!(
        certificate
            .exceptional_leaf_source_view(leaf_ordinal, wrong_case, retained_kind)
            .unwrap_err(),
        GeneratedResidualAffineWhenBadExceptionalLeafSourceError::ExpectedCaseMismatch {
            expected: wrong_case,
            retained: classification.case(),
        },
    );

    let wrong_variant = match retained_kind {
        GeneratedResidualAffineWhenBadExceptionalKind::Domain { condition_ordinal } => {
            GeneratedResidualAffineWhenBadExceptionalKind::Leak {
                pullback_ordinal: condition_ordinal,
            }
        }
        GeneratedResidualAffineWhenBadExceptionalKind::Leak { pullback_ordinal } => {
            GeneratedResidualAffineWhenBadExceptionalKind::Domain {
                condition_ordinal: pullback_ordinal,
            }
        }
    };
    assert_eq!(
        certificate
            .exceptional_leaf_source_view(leaf_ordinal, classification.case(), wrong_variant,)
            .unwrap_err(),
        GeneratedResidualAffineWhenBadExceptionalLeafSourceError::ExceptionalKindMismatch {
            expected: wrong_variant,
            retained: retained_kind,
        },
    );

    let wrong_source_ordinal = match retained_kind {
        GeneratedResidualAffineWhenBadExceptionalKind::Domain { condition_ordinal } => {
            GeneratedResidualAffineWhenBadExceptionalKind::Domain {
                condition_ordinal: condition_ordinal.checked_add(1).unwrap(),
            }
        }
        GeneratedResidualAffineWhenBadExceptionalKind::Leak { pullback_ordinal } => {
            GeneratedResidualAffineWhenBadExceptionalKind::Leak {
                pullback_ordinal: pullback_ordinal.checked_add(1).unwrap(),
            }
        }
    };
    assert_eq!(
        certificate
            .exceptional_leaf_source_view(
                leaf_ordinal,
                classification.case(),
                wrong_source_ordinal,
            )
            .unwrap_err(),
        GeneratedResidualAffineWhenBadExceptionalLeafSourceError::ExceptionalKindMismatch {
            expected: wrong_source_ordinal,
            retained: retained_kind,
        },
    );

    let (applicable_ordinal, applicable) = certificate
        .leaf_classifications()
        .iter()
        .enumerate()
        .find(|(_, classification)| {
            classification.disposition() == AffineWhenBadRelativeLeafDisposition::Applicable
        })
        .expect("generated fixture must retain an applicable leaf");
    assert_eq!(
        certificate
            .exceptional_leaf_source_view(applicable_ordinal, applicable.case(), retained_kind)
            .unwrap_err(),
        GeneratedResidualAffineWhenBadExceptionalLeafSourceError::LeafNotExceptional {
            leaf_ordinal: applicable_ordinal,
        },
    );

    let available = certificate.leaf_classifications().len();
    assert_eq!(
        certificate
            .exceptional_leaf_source_view(available, classification.case(), retained_kind)
            .unwrap_err(),
        GeneratedResidualAffineWhenBadExceptionalLeafSourceError::LeafOutOfRange {
            leaf_ordinal: available,
            available,
        },
    );
}

fn generated_relative_point_samples(
    certificate: &GeneratedResidualAffineWhenBadCertificate,
    context: &ParametricCoefficientContext,
) -> (Vec<i64>, Vec<i64>) {
    // Discover concrete validation witnesses solely through the independent
    // test-only evaluator of the private retained partition.  The search is
    // deterministic and bounded around both the origin and authenticated key
    // center; no topology-specific power or recurrence coefficient is an
    // input to production logic or to the classifier under test.
    let center = certificate.binding().key_center().values();
    assert_eq!(center.len(), 3, "the concrete 001 fixture changed arity");
    let origins = [[0i64; 3], [center[0], center[1], center[2]]];
    let mut applicable = None;
    let mut exceptional = None;
    for origin in origins {
        for first in -12i64..=12 {
            for second in -12i64..=12 {
                for third in -12i64..=12 {
                    let point = vec![
                        origin[0].checked_add(first).unwrap(),
                        origin[1].checked_add(second).unwrap(),
                        origin[2].checked_add(third).unwrap(),
                    ];
                    let oracle = certificate
                        .independently_classify_relative_point_for_test(context, &point)
                        .unwrap();
                    match oracle.2 {
                        AffineWhenBadRelativeLeafDisposition::Applicable => {
                            applicable.get_or_insert_with(|| (point.clone(), oracle.0));
                        }
                        AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { .. }
                        | AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { .. } => {
                            exceptional.get_or_insert_with(|| (point.clone(), oracle.0));
                        }
                    }
                    if applicable.is_some() && exceptional.is_some() {
                        let (applicable, applicable_leaf) = applicable.unwrap();
                        let (exceptional, exceptional_leaf) = exceptional.unwrap();
                        assert_ne!(applicable_leaf, exceptional_leaf);
                        return (applicable, exceptional);
                    }
                }
            }
        }
    }
    assert!(certificate.stats().exceptional_leaves() > 0);
    panic!("bounded independent search did not find both applicable and exceptional witnesses")
}

fn exact_generated_point_limits(
    stats: GeneratedResidualAffineWhenBadPointStats,
) -> GeneratedResidualAffineWhenBadPointLimits {
    GeneratedResidualAffineWhenBadPointLimits {
        max_context_fingerprint_comparison_bytes: stats.context_fingerprint_comparison_bytes(),
        max_index_entries: stats.index_entries(),
        max_cases: stats.cases(),
        max_classifications: stats.classifications(),
        max_predicates: stats.predicates(),
        max_source_terms: stats.source_terms(),
        max_source_exponent_entries: stats.source_exponent_entries(),
        max_preflight_validation_source_term_scan_bound: stats
            .preflight_validation_source_term_scan_bound(),
        max_preflight_validation_source_exponent_entry_scan_bound: stats
            .preflight_validation_source_exponent_entry_scan_bound(),
        max_output_term_bound: stats.output_term_bound(),
        max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
        max_power_operation_bound: stats.power_operation_bound(),
        max_largest_output_integer_bit_bound: stats.largest_output_integer_bit_bound(),
        max_integer_bit_work_bound: stats.integer_bit_work_bound(),
        max_retained_output_term_bound: stats.retained_output_term_bound(),
        max_retained_output_byte_bound: stats.retained_output_byte_bound(),
    }
}

fn generated_point_resource_limit(
    error: GeneratedResidualAffineWhenBadPointError,
) -> (&'static str, usize, usize) {
    match error {
        GeneratedResidualAffineWhenBadPointError::ResourceLimit {
            resource,
            requested,
            limit,
        } => (resource, requested, limit),
        other => panic!("expected generated affine point resource limit, got {other:?}"),
    }
}

fn exact_concrete_specialization_limits(
    stats: ParametricConcreteSpecializationPreflight,
) -> ParametricConcreteSpecializationLimits {
    ParametricConcreteSpecializationLimits {
        arithmetic: Default::default(),
        max_source_terms: stats.source_terms(),
        max_source_exponent_entries: stats.source_exponent_entries(),
        max_output_term_bound: stats.output_term_bound(),
        max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
        max_power_operation_bound: stats.power_operation_bound(),
        max_integer_bit_work_bound: stats.integer_bit_work_bound(),
        max_normalization_input_term_pair_bound: stats.normalization_input_term_pair_bound(),
        max_key_component_bound: stats.key_component_bound(),
        max_guard_occurrence_bound: stats.guard_occurrence_bound(),
        max_guard_polynomial_retained_byte_bound: stats.guard_polynomial_retained_byte_bound(),
        max_guard_origin_occurrence_bound: stats.guard_origin_occurrence_bound(),
        max_guard_origin_retained_byte_bound: stats.guard_origin_retained_byte_bound(),
        max_normalized_coefficient_term_bound: stats.normalized_coefficient_term_bound(),
        max_normalized_coefficient_retained_byte_bound: stats
            .normalized_coefficient_retained_byte_bound(),
        max_concrete_relation_retained_byte_bound: stats.concrete_relation_retained_byte_bound(),
        max_peak_execution_retained_byte_bound: stats.peak_execution_retained_byte_bound(),
    }
}

fn exact_sealed_application_limits(
    stats: GeneratedResidualAffineSealedApplicationStats,
) -> GeneratedResidualAffineSealedApplicationLimits {
    GeneratedResidualAffineSealedApplicationLimits {
        max_condition_rows: stats.condition_rows(),
        max_condition_source_lookups: stats.condition_source_lookups(),
        max_condition_copy_terms: stats.condition_copy_terms(),
        max_condition_copy_exponent_entries: stats.condition_copy_exponent_entries(),
        max_condition_copy_integer_bits: stats.condition_copy_integer_bits(),
        max_condition_origin_inputs: stats.condition_origin_inputs(),
        max_condition_origin_retained_bytes: stats.condition_origin_retained_bytes(),
        max_temporary_condition_retained_byte_bound: stats
            .temporary_condition_retained_byte_bound(),
        max_temporary_plus_relation_peak_byte_bound: stats
            .temporary_plus_relation_peak_byte_bound(),
        relation: exact_concrete_specialization_limits(stats.relation()),
    }
}

fn sealed_application_resource_limit(
    error: GeneratedResidualAffineWhenBadApplicationError,
) -> (&'static str, usize, usize) {
    match error {
        GeneratedResidualAffineWhenBadApplicationError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | GeneratedResidualAffineWhenBadApplicationError::Relation(
            ParametricRelationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => (resource, requested, limit),
        GeneratedResidualAffineWhenBadApplicationError::Coefficient(error)
        | GeneratedResidualAffineWhenBadApplicationError::Relation(
            ParametricRelationError::Coefficient(error),
        ) => coefficient_resource_limit(error),
        other => panic!("expected sealed application resource limit, got {other:?}"),
    }
}

#[test]
fn sealed_application_preflights_exact_condition_resources_and_nested_relation() {
    let fixture = generated_pending_fixture();
    let certificate = compile_generated_point_query_certificate(&fixture);
    // Concrete powers are only a validation witness for this generated
    // family. No production rule or recurrence depends on this point.
    let point = [-4, -4, 2];
    let classification = certificate
        .classify_relative_point(
            &fixture.context,
            &point,
            GeneratedResidualAffineWhenBadPointLimits::default(),
        )
        .unwrap();
    assert_eq!(
        classification.disposition(),
        AffineWhenBadRelativeLeafDisposition::Applicable,
    );
    let authorization = GeneratedSectorAffineSealedLeafAuthorization::for_test(
        &certificate,
        classification.leaf_ordinal(),
        classification.case(),
    );
    let (_, baseline_stats) = certificate
        .specialize_sealed_applicable_leaf(
            &fixture.context,
            &point,
            &authorization,
            GeneratedResidualAffineSealedApplicationLimits::default(),
        )
        .unwrap();

    for (name, observed) in [
        ("condition rows", baseline_stats.condition_rows()),
        (
            "condition source lookups",
            baseline_stats.condition_source_lookups(),
        ),
        (
            "condition copy terms",
            baseline_stats.condition_copy_terms(),
        ),
        (
            "condition copy exponent entries",
            baseline_stats.condition_copy_exponent_entries(),
        ),
        (
            "condition copy integer bits",
            baseline_stats.condition_copy_integer_bits(),
        ),
        (
            "condition origin inputs",
            baseline_stats.condition_origin_inputs(),
        ),
        (
            "condition origin retained bytes",
            baseline_stats.condition_origin_retained_bytes(),
        ),
        (
            "temporary condition retained-byte bound",
            baseline_stats.temporary_condition_retained_byte_bound(),
        ),
        (
            "temporary condition retained bytes",
            baseline_stats.temporary_condition_retained_bytes(),
        ),
        (
            "temporary plus relation peak byte bound",
            baseline_stats.temporary_plus_relation_peak_byte_bound(),
        ),
        (
            "nested relation source terms",
            baseline_stats.relation().source_terms(),
        ),
        (
            "nested relation peak retained bytes",
            baseline_stats
                .relation()
                .peak_execution_retained_byte_bound(),
        ),
    ] {
        assert!(observed > 0, "{name} unexpectedly remained zero");
    }
    assert!(
        baseline_stats.temporary_condition_retained_bytes()
            <= baseline_stats.temporary_condition_retained_byte_bound()
    );
    assert_eq!(
        baseline_stats.temporary_plus_relation_peak_byte_bound(),
        baseline_stats
            .temporary_condition_retained_byte_bound()
            .checked_add(
                baseline_stats
                    .relation()
                    .peak_execution_retained_byte_bound()
            )
            .unwrap()
    );

    let exact = exact_sealed_application_limits(baseline_stats);
    let (_, exact_stats) = certificate
        .specialize_sealed_applicable_leaf(&fixture.context, &point, &authorization, exact)
        .unwrap();
    assert_eq!(exact_stats, baseline_stats);

    macro_rules! one_below {
        ($field:ident, $observed:expr) => {{
            let observed = $observed;
            assert!(observed > 0);
            let mut one_below = exact;
            one_below.$field = observed - 1;
            let (_, requested, limit) = sealed_application_resource_limit(
                certificate
                    .specialize_sealed_applicable_leaf(
                        &fixture.context,
                        &point,
                        &authorization,
                        one_below,
                    )
                    .unwrap_err(),
            );
            assert!(requested > limit);
        }};
    }
    one_below!(max_condition_rows, baseline_stats.condition_rows());
    one_below!(
        max_condition_source_lookups,
        baseline_stats.condition_source_lookups()
    );
    one_below!(
        max_condition_copy_terms,
        baseline_stats.condition_copy_terms()
    );
    one_below!(
        max_condition_copy_exponent_entries,
        baseline_stats.condition_copy_exponent_entries()
    );
    one_below!(
        max_condition_copy_integer_bits,
        baseline_stats.condition_copy_integer_bits()
    );
    one_below!(
        max_condition_origin_inputs,
        baseline_stats.condition_origin_inputs()
    );
    one_below!(
        max_condition_origin_retained_bytes,
        baseline_stats.condition_origin_retained_bytes()
    );
    one_below!(
        max_temporary_condition_retained_byte_bound,
        baseline_stats.temporary_condition_retained_byte_bound()
    );
    one_below!(
        max_temporary_plus_relation_peak_byte_bound,
        baseline_stats.temporary_plus_relation_peak_byte_bound()
    );

    // A stricter aggregate peak which cannot fit the condition-side
    // prospective payload must fail in the allocation-free prepare phase,
    // before the first temporary condition is materialized.
    let mut early_peak_one_below = exact;
    early_peak_one_below.max_temporary_plus_relation_peak_byte_bound =
        baseline_stats.temporary_condition_retained_byte_bound() - 1;
    let (resource, requested, limit) = sealed_application_resource_limit(
        certificate
            .specialize_sealed_applicable_leaf(
                &fixture.context,
                &point,
                &authorization,
                early_peak_one_below,
            )
            .unwrap_err(),
    );
    assert_eq!(
        resource,
        "sealed affine temporary-plus-relation peak byte bound"
    );
    assert_eq!(
        (requested, limit),
        (
            baseline_stats.temporary_condition_retained_byte_bound(),
            baseline_stats.temporary_condition_retained_byte_bound() - 1,
        )
    );

    let nested_peak = baseline_stats
        .relation()
        .peak_execution_retained_byte_bound();
    let mut nested_one_below = exact;
    nested_one_below
        .relation
        .max_peak_execution_retained_byte_bound = nested_peak - 1;
    let (_, requested, limit) = sealed_application_resource_limit(
        certificate
            .specialize_sealed_applicable_leaf(
                &fixture.context,
                &point,
                &authorization,
                nested_one_below,
            )
            .unwrap_err(),
    );
    assert!(requested > limit);

    let debug = format!("{baseline_stats:?}");
    assert!(debug.contains("<redacted resource census>"));
    assert!(!debug.contains("polynomial"));
    assert!(!debug.contains("ParametricRelation"));
}

#[test]
fn generated_relative_point_classifier_is_exact_and_redacted() {
    let fixture = generated_pending_fixture();
    let certificate = compile_generated_point_query_certificate(&fixture);
    let (applicable_point, exceptional_point) =
        generated_relative_point_samples(&certificate, &fixture.context);

    let applicable = certificate
        .classify_relative_point(
            &fixture.context,
            &applicable_point,
            GeneratedResidualAffineWhenBadPointLimits::default(),
        )
        .unwrap();
    let applicable_oracle = certificate
        .independently_classify_relative_point_for_test(&fixture.context, &applicable_point)
        .unwrap();
    assert_eq!(
        applicable.disposition(),
        AffineWhenBadRelativeLeafDisposition::Applicable,
    );
    assert_eq!(
        (
            applicable.leaf_ordinal(),
            applicable.case(),
            applicable.disposition(),
        ),
        applicable_oracle,
    );
    let retained = &certificate.leaf_classifications()[applicable.leaf_ordinal()];
    assert_eq!(retained.case(), applicable.case());
    assert_eq!(retained.disposition(), applicable.disposition());
    assert_eq!(applicable.stats().matched_cases(), 1);

    let exceptional = certificate
        .classify_relative_point(
            &fixture.context,
            &exceptional_point,
            GeneratedResidualAffineWhenBadPointLimits::default(),
        )
        .unwrap();
    let exceptional_oracle = certificate
        .independently_classify_relative_point_for_test(&fixture.context, &exceptional_point)
        .unwrap();
    assert!(matches!(
        exceptional.disposition(),
        AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { .. }
            | AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { .. }
    ));
    assert_eq!(
        (
            exceptional.leaf_ordinal(),
            exceptional.case(),
            exceptional.disposition(),
        ),
        exceptional_oracle,
    );
    let retained = &certificate.leaf_classifications()[exceptional.leaf_ordinal()];
    assert_eq!(retained.case(), exceptional.case());
    assert_eq!(retained.disposition(), exceptional.disposition());

    let debug = format!("{applicable:?}");
    assert!(!debug.contains("polynomial"));
    assert!(!debug.contains("indices: ["));
    assert!(!debug.contains("ParametricRelation"));
}

#[test]
fn generated_relative_point_classifier_checks_authority_and_exact_limits() {
    let fixture = generated_pending_fixture();
    let certificate = compile_generated_point_query_certificate(&fixture);
    let (point, _) = generated_relative_point_samples(&certificate, &fixture.context);

    let wrong_context = ParametricCoefficientContext::try_new(
        fixture.context.base(),
        "generated-relative-point-wrong-context",
        fixture.context.index_count(),
    )
    .unwrap();
    assert!(matches!(
        certificate.classify_relative_point(
            &wrong_context,
            &point,
            GeneratedResidualAffineWhenBadPointLimits::default(),
        ),
        Err(GeneratedResidualAffineWhenBadPointError::WrongContext)
    ));
    assert!(matches!(
        certificate.classify_relative_point(
            &fixture.context,
            &point[..point.len() - 1],
            GeneratedResidualAffineWhenBadPointLimits::default(),
        ),
        Err(GeneratedResidualAffineWhenBadPointError::WrongArity {
            expected: 3,
            actual: 2,
        })
    ));

    let baseline = certificate
        .classify_relative_point(
            &fixture.context,
            &point,
            GeneratedResidualAffineWhenBadPointLimits::default(),
        )
        .unwrap();
    let stats = baseline.stats();
    assert!(stats.predicates() > 0);
    assert!(stats.source_terms() > 0);
    assert_eq!(
        stats.preflight_validation_source_term_scan_bound(),
        stats.source_terms().checked_mul(8).unwrap(),
        "four validation coefficient scans plus two integer-growth and two capacity scans",
    );
    assert_eq!(
        stats.preflight_validation_source_exponent_entry_scan_bound(),
        stats.source_exponent_entries().checked_mul(10).unwrap(),
        "four validation/canonical-order pairs plus two index-exponent preflight scans",
    );
    assert!(
        [
            stats.context_fingerprint_comparison_bytes(),
            stats.index_entries(),
            stats.cases(),
            stats.classifications(),
            stats.predicates(),
            stats.source_terms(),
            stats.source_exponent_entries(),
            stats.preflight_validation_source_term_scan_bound(),
            stats.preflight_validation_source_exponent_entry_scan_bound(),
            stats.output_term_bound(),
            stats.output_exponent_entry_bound(),
            stats.power_operation_bound(),
            stats.largest_output_integer_bit_bound(),
            stats.integer_bit_work_bound(),
            stats.retained_output_term_bound(),
            stats.retained_output_byte_bound(),
        ]
        .into_iter()
        .all(|observed| observed > 0),
        "the 001 fixture must exercise every point-query budget",
    );
    let exact = exact_generated_point_limits(stats);
    assert_eq!(
        certificate
            .classify_relative_point(&fixture.context, &point, exact)
            .unwrap(),
        baseline,
    );

    let mut tested_nonzero_limits = 0usize;
    macro_rules! assert_one_below {
        ($field:ident, $observed:expr, $resource:literal) => {{
            let observed = $observed;
            if observed > 0 {
                tested_nonzero_limits += 1;
                let mut one_below = exact;
                one_below.$field = observed - 1;
                let (resource, requested, limit) = generated_point_resource_limit(
                    certificate
                        .classify_relative_point(&fixture.context, &point, one_below)
                        .unwrap_err(),
                );
                assert_eq!(resource, $resource);
                assert_eq!((requested, limit), (observed, observed - 1));
            }
        }};
    }
    assert_one_below!(
        max_context_fingerprint_comparison_bytes,
        stats.context_fingerprint_comparison_bytes(),
        "generated affine WhenBad point context fingerprint comparison bytes"
    );
    assert_one_below!(
        max_index_entries,
        stats.index_entries(),
        "generated affine WhenBad point index entries"
    );
    assert_one_below!(
        max_cases,
        stats.cases(),
        "generated affine WhenBad point cases"
    );
    assert_one_below!(
        max_classifications,
        stats.classifications(),
        "generated affine WhenBad point classifications"
    );
    assert_one_below!(
        max_predicates,
        stats.predicates(),
        "generated affine WhenBad point predicates"
    );
    assert_one_below!(
        max_source_terms,
        stats.source_terms(),
        "generated affine WhenBad point specialization source terms"
    );
    assert_one_below!(
        max_source_exponent_entries,
        stats.source_exponent_entries(),
        "generated affine WhenBad point specialization source exponent entries"
    );
    assert_one_below!(
        max_preflight_validation_source_term_scan_bound,
        stats.preflight_validation_source_term_scan_bound(),
        "generated affine WhenBad point preflight/validation source-term scan bound"
    );
    assert_one_below!(
        max_preflight_validation_source_exponent_entry_scan_bound,
        stats.preflight_validation_source_exponent_entry_scan_bound(),
        "generated affine WhenBad point preflight/validation source exponent-entry scan bound"
    );
    assert_one_below!(
        max_output_term_bound,
        stats.output_term_bound(),
        "generated affine WhenBad point specialization output term bound"
    );
    assert_one_below!(
        max_output_exponent_entry_bound,
        stats.output_exponent_entry_bound(),
        "generated affine WhenBad point specialization output exponent-entry bound"
    );
    assert_one_below!(
        max_power_operation_bound,
        stats.power_operation_bound(),
        "generated affine WhenBad point specialization power-operation bound"
    );
    assert_one_below!(
        max_largest_output_integer_bit_bound,
        stats.largest_output_integer_bit_bound(),
        "generated affine WhenBad point specialization largest output integer-bit bound"
    );
    assert_one_below!(
        max_integer_bit_work_bound,
        stats.integer_bit_work_bound(),
        "generated affine WhenBad point specialization integer-bit work bound"
    );
    assert_one_below!(
        max_retained_output_term_bound,
        stats.retained_output_term_bound(),
        "generated affine WhenBad point specialization retained output term bound"
    );
    assert_one_below!(
        max_retained_output_byte_bound,
        stats.retained_output_byte_bound(),
        "generated affine WhenBad point specialization retained output byte bound"
    );
    assert_eq!(tested_nonzero_limits, 16);
}
