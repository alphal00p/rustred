//! Public end-to-end checks for complete residual affine branch-guard
//! composition.  Loop count appears only in the concrete sunset oracle; the
//! production certificate is topology independent.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, GuardOrigin, IntegralFamily, IntegralOrderingPolicy,
    ParametricArithmeticLimits, ParametricCoefficientContext, ParametricIbpGenerator,
    ResidualAffineBranchGuardCompositionCertificate, ResidualAffineBranchGuardCompositionClass,
    ResidualAffineBranchGuardCompositionError, ResidualAffineBranchGuardCompositionLimits,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemLimits,
    ResidualAffineBranchSystemOutcome, ResidualProductLocusBooleanCoverCertificate,
    ResidualProductLocusBooleanCoverCompiler, ResidualProductLocusBooleanCoverLimits,
    ResidualProductLocusBooleanNodeOutcome, SectorMask,
};
use symbolica::prelude::Integer;

fn sunset(name: &str) -> IntegralFamily {
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

fn generated_cover(
    bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<ResidualProductLocusBooleanCoverCertificate>,
) {
    let family = sunset(&format!("branch-guard-composition-sunset-{bits}"));
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
    let cover = Arc::new(
        ResidualProductLocusBooleanCoverCompiler::compile(
            &family,
            &context,
            queue,
            0,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap(),
    );
    (family, context, cover)
}

fn guarded_branches(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
) -> Vec<Arc<ResidualAffineBranchSystemCertificate>> {
    cover
        .nodes()
        .iter()
        .filter(|node| {
            matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            )
        })
        .map(|node| {
            Arc::new(
                ResidualAffineBranchSystemCertificate::compile(
                    family,
                    context,
                    cover.clone(),
                    node.ordinal(),
                    ResidualAffineBranchSystemLimits::default(),
                )
                .unwrap(),
            )
        })
        .filter(|branch| {
            matches!(
                branch.outcome(),
                ResidualAffineBranchSystemOutcome::GuardedAffineMap
            )
        })
        .collect()
}

fn assert_exact_origin(
    certificate: &ResidualAffineBranchGuardCompositionCertificate,
    entry_ordinal: usize,
) {
    let entry = &certificate.entries()[entry_ordinal];
    let Some(condition) = entry.class().condition() else {
        return;
    };
    assert_eq!(condition.origins().len(), 1);
    assert_eq!(
        condition.origins().iter().next(),
        Some(&GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
            source_case: certificate.source_cover().source_case().value(),
            source_work_item_ordinal: certificate.source_cover().source_work_item_ordinal(),
            ready_terminal_ordinal: certificate.source_branch().ready_terminal_ordinal(),
            structural_locus_ordinal: entry.structural_locus_ordinal(),
        })
    );
}

fn assert_direct_g_of_f_samples(
    context: &ParametricCoefficientContext,
    certificate: &ResidualAffineBranchGuardCompositionCertificate,
) {
    let branch = certificate.source_branch();
    let map = branch.affine_map().unwrap();
    let coverage = certificate
        .source_cover()
        .source_queue()
        .discovery()
        .coverage();
    let arithmetic = ParametricArithmeticLimits::default();
    for seed in [-3i64, 0, 5] {
        let mut residual = vec![97; map.ambient_arity()];
        for (free_ordinal, &position) in map.free_positions().iter().enumerate() {
            residual[position] = seed + i64::try_from(free_ordinal).unwrap();
        }
        let ambient: Vec<_> = (0..map.ambient_arity())
            .map(|row| {
                let mut value = map.constant(row).unwrap().clone();
                for &position in map.free_positions() {
                    value += map.linear_coefficient(row, position).unwrap()
                        * Integer::from(residual[position]);
                }
                value.to_i64().expect("small affine sample fits i64")
            })
            .collect();
        for entry in certificate.entries() {
            let source = coverage
                .structural_locus(entry.structural_locus_ordinal())
                .unwrap();
            assert_eq!(
                context
                    .specialize_polynomial(source, &ambient, arithmetic)
                    .unwrap(),
                context
                    .specialize_polynomial(entry.mapped_polynomial(), &residual, arithmetic)
                    .unwrap(),
                "G(F(t)) sample failed for structural locus {}",
                entry.structural_locus_ordinal()
            );
        }
    }
}

#[test]
fn generated_sunset_all_ready_branches_compose_every_guard_and_replay() {
    let mut class_counts = [0usize; 4];
    let mut equal_mapped_pairs = 0usize;
    let mut equal_condition_pairs = 0usize;
    let mut nonleading_free_maps = 0usize;
    let mut multiple_free_dependences = 0usize;
    for bits in ["011", "101", "110", "111"] {
        let (family, context, cover) = generated_cover(bits);
        let branches = guarded_branches(&family, &context, &cover);
        assert!(!branches.is_empty(), "sector {bits}");
        for branch in branches {
            let certificate = ResidualAffineBranchGuardCompositionCertificate::compile(
                &family,
                &context,
                cover.clone(),
                branch.clone(),
                ResidualAffineBranchGuardCompositionLimits::default(),
            )
            .unwrap();
            assert!(Arc::ptr_eq(certificate.source_cover(), &cover));
            assert!(Arc::ptr_eq(certificate.source_branch(), &branch));
            assert_eq!(
                certificate
                    .entries()
                    .iter()
                    .map(|entry| entry.structural_locus_ordinal())
                    .collect::<Vec<_>>(),
                branch.nonzero_guard_locus_ordinals()
            );
            for (entry_ordinal, entry) in certificate.entries().iter().enumerate() {
                assert_exact_origin(&certificate, entry_ordinal);
                match entry.class() {
                    ResidualAffineBranchGuardCompositionClass::Contradiction => {
                        class_counts[0] += 1
                    }
                    ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant => {
                        class_counts[1] += 1
                    }
                    ResidualAffineBranchGuardCompositionClass::BaseAssumption(_) => {
                        class_counts[2] += 1
                    }
                    ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(_) => {
                        class_counts[3] += 1
                    }
                }
                let first_index = context.base().parameter_names().len();
                if entry
                    .mapped_polynomial()
                    .raw()
                    .exponents_iter()
                    .any(|exponents| {
                        branch
                            .affine_map()
                            .unwrap()
                            .free_positions()
                            .iter()
                            .filter(|&&position| exponents[first_index + position] != 0)
                            .count()
                            >= 2
                    })
                {
                    multiple_free_dependences += 1;
                }
            }
            for left in 0..certificate.entries().len() {
                for right in left + 1..certificate.entries().len() {
                    if certificate.entries()[left].mapped_polynomial()
                        == certificate.entries()[right].mapped_polynomial()
                    {
                        equal_mapped_pairs += 1;
                        if certificate.entries()[left].class().condition().is_some()
                            && certificate.entries()[right].class().condition().is_some()
                        {
                            equal_condition_pairs += 1;
                        }
                    }
                }
            }
            let free_positions = branch.affine_map().unwrap().free_positions();
            if free_positions
                .iter()
                .enumerate()
                .any(|(ordinal, &position)| ordinal != position)
            {
                nonleading_free_maps += 1;
            }
            assert_direct_g_of_f_samples(&context, &certificate);
            certificate.replay(&family, &context).unwrap();
        }
    }
    assert_eq!(
        class_counts[0], 0,
        "canonical sunset branches are consistent"
    );
    assert!(class_counts[1] > 0, "integer constants are discharged");
    assert!(class_counts[2] > 0, "base assumptions survive");
    assert!(class_counts[3] > 0, "free-index guards survive");
    assert!(equal_mapped_pairs > 0, "equal images are not merged");
    assert_eq!(
        equal_condition_pairs, 0,
        "sunset's equal images happen to be discharged constants; the synthetic unit test covers distinct origins"
    );
    assert!(nonleading_free_maps > 0);
    assert_eq!(
        multiple_free_dependences, 0,
        "the synthetic source-neutral unit test covers a three-free-coordinate image"
    );
}

#[test]
fn fresh_arc_and_independent_top_level_replay_boundaries_are_explicit() {
    let (family, context, cover) = generated_cover("111");
    let branch = guarded_branches(&family, &context, &cover)
        .into_iter()
        .find(|branch| !branch.nonzero_guard_locus_ordinals().is_empty())
        .expect("sunset has a guarded branch with a nonzero manifest");
    let certificate = ResidualAffineBranchGuardCompositionCertificate::compile(
        &family,
        &context,
        cover.clone(),
        branch.clone(),
        ResidualAffineBranchGuardCompositionLimits::default(),
    )
    .unwrap();

    let equal_cover = Arc::new((*cover).clone());
    assert!(matches!(
        ResidualAffineBranchGuardCompositionCertificate::compile(
            &family,
            &context,
            equal_cover.clone(),
            branch.clone(),
            ResidualAffineBranchGuardCompositionLimits::default(),
        ),
        Err(ResidualAffineBranchGuardCompositionError::BranchSourceCoverAllocationMismatch)
    ));
    certificate
        .replay_with_sources(&family, &context, equal_cover, branch.clone())
        .unwrap();

    let equal_branch = Arc::new((*branch).clone());
    certificate
        .replay_with_sources(&family, &context, cover.clone(), equal_branch)
        .unwrap();

    let wrong_family = sunset("branch-guard-composition-wrong-family");
    assert!(matches!(
        certificate.replay(&wrong_family, &context),
        Err(ResidualAffineBranchGuardCompositionError::WrongFamily)
    ));
    let wrong_context = ParametricCoefficientContext::try_new(
        context.base(),
        "branch-guard-composition-wrong-context",
        context.index_count(),
    )
    .unwrap();
    assert!(matches!(
        certificate.replay(&family, &wrong_context),
        Err(ResidualAffineBranchGuardCompositionError::WrongContext)
    ));
}

fn is_resource_limit(error: &ResidualAffineBranchGuardCompositionError) -> bool {
    matches!(
        error,
        ResidualAffineBranchGuardCompositionError::ResourceLimit { .. }
            | ResidualAffineBranchGuardCompositionError::Composition(
                rustred::ResidualUnitAffineCompositionError::ResourceLimit { .. }
            )
            | ResidualAffineBranchGuardCompositionError::Coefficient(
                rustred::ParametricCoefficientError::ResourceLimit { .. }
            )
    )
}

#[test]
fn every_aggregate_limit_accepts_the_exact_census_and_rejects_one_below() {
    let (family, context, cover) = generated_cover("111");
    let branch = guarded_branches(&family, &context, &cover)
        .into_iter()
        .find(|branch| !branch.nonzero_guard_locus_ordinals().is_empty())
        .expect("sunset has a guarded branch with a nonzero manifest");
    let baseline = ResidualAffineBranchGuardCompositionCertificate::compile(
        &family,
        &context,
        cover.clone(),
        branch.clone(),
        ResidualAffineBranchGuardCompositionLimits::default(),
    )
    .unwrap();
    let stats = baseline.stats();
    let mut exact = ResidualAffineBranchGuardCompositionLimits::default();
    exact.max_family_fingerprint_bytes = stats.family_fingerprint_bytes();
    exact.max_context_fingerprint_bytes = stats.context_fingerprint_bytes();
    exact.max_scope_fingerprint_comparison_bytes = stats.scope_fingerprint_comparison_bytes();
    exact.max_guards = stats.guards();
    exact.max_structural_locus_lookups = stats.structural_locus_lookups();
    exact.max_total_source_terms = stats.total_source_terms();
    exact.max_total_source_exponent_entries = stats.total_source_exponent_entries();
    exact.max_total_source_integer_bits = stats.total_source_integer_bits();
    exact.max_total_expanded_contributions = stats.total_expanded_contributions();
    exact.max_total_output_term_bound = stats.total_output_term_bound();
    exact.max_total_output_terms = stats.total_output_terms();
    exact.max_total_output_exponent_entry_bound = stats.total_output_exponent_entry_bound();
    exact.max_total_output_exponent_entries = stats.total_output_exponent_entries();
    exact.max_total_power_calls = stats.total_power_calls();
    exact.max_total_native_power_heap_pairs = stats.total_native_power_heap_pairs();
    exact.max_total_multiplication_term_pairs = stats.total_multiplication_term_pairs();
    exact.max_total_addition_term_visits = stats.total_addition_term_visits();
    exact.max_total_native_integer_bit_work = stats.total_native_integer_bit_work();
    exact.max_total_integer_bit_work = stats.total_integer_bit_work();
    exact.max_retained_entries = stats.retained_entries();
    exact.max_retained_polynomial_terms = stats.retained_polynomial_terms();
    exact.max_retained_polynomial_exponent_entries = stats.retained_polynomial_exponent_entries();
    exact.max_retained_polynomial_integer_bits = stats.retained_polynomial_integer_bits();
    exact.max_retained_conditions = stats.retained_conditions();
    exact.max_retained_origins = stats.retained_origins();
    exact.max_retained_origin_bytes = stats.retained_origin_bytes();
    exact.max_payload_comparison_units = stats.payload_comparison_units();
    exact.max_payload_comparison_bytes = stats.payload_comparison_bytes();
    exact.max_payload_comparison_integer_bits = stats.payload_comparison_integer_bits();

    ResidualAffineBranchGuardCompositionCertificate::compile(
        &family,
        &context,
        cover.clone(),
        branch.clone(),
        exact,
    )
    .unwrap();

    type Setter = fn(&mut ResidualAffineBranchGuardCompositionLimits, usize);
    let cases: &[(&str, usize, Setter)] = &[
        (
            "family fingerprint bytes",
            stats.family_fingerprint_bytes(),
            |limits, value| limits.max_family_fingerprint_bytes = value,
        ),
        (
            "context fingerprint bytes",
            stats.context_fingerprint_bytes(),
            |limits, value| limits.max_context_fingerprint_bytes = value,
        ),
        (
            "scope comparison bytes",
            stats.scope_fingerprint_comparison_bytes(),
            |limits, value| limits.max_scope_fingerprint_comparison_bytes = value,
        ),
        ("guards", stats.guards(), |limits, value| {
            limits.max_guards = value
        }),
        (
            "lookups",
            stats.structural_locus_lookups(),
            |limits, value| limits.max_structural_locus_lookups = value,
        ),
        (
            "source terms",
            stats.total_source_terms(),
            |limits, value| limits.max_total_source_terms = value,
        ),
        (
            "source exponents",
            stats.total_source_exponent_entries(),
            |limits, value| limits.max_total_source_exponent_entries = value,
        ),
        (
            "source bits",
            stats.total_source_integer_bits(),
            |limits, value| limits.max_total_source_integer_bits = value,
        ),
        (
            "expanded contributions",
            stats.total_expanded_contributions(),
            |limits, value| limits.max_total_expanded_contributions = value,
        ),
        (
            "output term bound",
            stats.total_output_term_bound(),
            |limits, value| limits.max_total_output_term_bound = value,
        ),
        (
            "output terms",
            stats.total_output_terms(),
            |limits, value| limits.max_total_output_terms = value,
        ),
        (
            "output exponent bound",
            stats.total_output_exponent_entry_bound(),
            |limits, value| limits.max_total_output_exponent_entry_bound = value,
        ),
        (
            "output exponents",
            stats.total_output_exponent_entries(),
            |limits, value| limits.max_total_output_exponent_entries = value,
        ),
        ("power calls", stats.total_power_calls(), |limits, value| {
            limits.max_total_power_calls = value
        }),
        (
            "native heap pairs",
            stats.total_native_power_heap_pairs(),
            |limits, value| limits.max_total_native_power_heap_pairs = value,
        ),
        (
            "multiplication pairs",
            stats.total_multiplication_term_pairs(),
            |limits, value| limits.max_total_multiplication_term_pairs = value,
        ),
        (
            "addition visits",
            stats.total_addition_term_visits(),
            |limits, value| limits.max_total_addition_term_visits = value,
        ),
        (
            "native integer work",
            stats.total_native_integer_bit_work(),
            |limits, value| limits.max_total_native_integer_bit_work = value,
        ),
        (
            "integer work",
            stats.total_integer_bit_work(),
            |limits, value| limits.max_total_integer_bit_work = value,
        ),
        (
            "retained entries",
            stats.retained_entries(),
            |limits, value| limits.max_retained_entries = value,
        ),
        (
            "retained terms",
            stats.retained_polynomial_terms(),
            |limits, value| limits.max_retained_polynomial_terms = value,
        ),
        (
            "retained exponents",
            stats.retained_polynomial_exponent_entries(),
            |limits, value| limits.max_retained_polynomial_exponent_entries = value,
        ),
        (
            "retained integer bits",
            stats.retained_polynomial_integer_bits(),
            |limits, value| limits.max_retained_polynomial_integer_bits = value,
        ),
        (
            "retained conditions",
            stats.retained_conditions(),
            |limits, value| limits.max_retained_conditions = value,
        ),
        (
            "retained origins",
            stats.retained_origins(),
            |limits, value| limits.max_retained_origins = value,
        ),
        (
            "retained origin bytes",
            stats.retained_origin_bytes(),
            |limits, value| limits.max_retained_origin_bytes = value,
        ),
        (
            "comparison units",
            stats.payload_comparison_units(),
            |limits, value| limits.max_payload_comparison_units = value,
        ),
        (
            "comparison bytes",
            stats.payload_comparison_bytes(),
            |limits, value| limits.max_payload_comparison_bytes = value,
        ),
        (
            "comparison integer bits",
            stats.payload_comparison_integer_bits(),
            |limits, value| limits.max_payload_comparison_integer_bits = value,
        ),
    ];
    for &(label, exact_value, setter) in cases {
        if exact_value == 0 {
            continue;
        }
        let mut one_below = exact;
        setter(&mut one_below, exact_value - 1);
        let error = ResidualAffineBranchGuardCompositionCertificate::compile(
            &family,
            &context,
            cover.clone(),
            branch.clone(),
            one_below,
        )
        .expect_err(label);
        assert!(is_resource_limit(&error), "{label}: {error:?}");
    }
}
