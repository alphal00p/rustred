//! End-to-end checks for generated rows bound to complete residual-affine
//! branches.
//!
//! The equal-mass sunset is only a concrete validation fixture.  Every source
//! relation comes from the topology-generic generated row-span certificate;
//! no recurrence or reduction rule is embedded here.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedResidualAffineBranchBoundRelationCompilation,
    GeneratedResidualAffineBranchBoundRelationCompiler,
    GeneratedResidualAffineBranchBoundRelationError,
    GeneratedResidualAffineBranchBoundRelationLimits,
    GeneratedResidualAffineBranchConcreteSpecializationLimits,
    GeneratedResidualAffineBranchEmptyReason, GeneratedResidualAffineBranchUnavailableReason,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
    GeneratedSymbolicRowSpanCertificate, IndexShift, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator, ParametricRelation,
    ResidualAffineBranchGuardCompositionCertificate, ResidualAffineBranchGuardCompositionLimits,
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
    let family = sunset(&format!("generated-residual-affine-bound-sunset-{bits}"));
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

fn composed_guards(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
    branch: &Arc<ResidualAffineBranchSystemCertificate>,
) -> Arc<ResidualAffineBranchGuardCompositionCertificate> {
    Arc::new(
        ResidualAffineBranchGuardCompositionCertificate::compile(
            family,
            context,
            cover.clone(),
            branch.clone(),
            ResidualAffineBranchGuardCompositionLimits::default(),
        )
        .unwrap(),
    )
}

fn row_span(
    cover: &ResidualProductLocusBooleanCoverCertificate,
) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
    cover.source_queue().discovery().row_span_arc()
}

fn polynomial_multiset(relation: &rustred::ConcreteRelation) -> Vec<String> {
    let mut polynomials = relation
        .nonzero_conditions()
        .iter()
        .map(|polynomial| polynomial.raw().to_string())
        .collect::<Vec<_>>();
    polynomials.sort();
    polynomials
}

fn affine_ambient_point(
    branch: &ResidualAffineBranchSystemCertificate,
    free_values: &[i64],
) -> Vec<i64> {
    let map = branch
        .affine_map()
        .expect("guarded branch has an affine map");
    assert_eq!(free_values.len(), map.free_positions().len());
    (0..map.ambient_arity())
        .map(|row| {
            let mut value = map.constant(row).unwrap().clone();
            for (free_ordinal, &free_position) in map.free_positions().iter().enumerate() {
                value += map.linear_coefficient(row, free_position).unwrap()
                    * Integer::from(free_values[free_ordinal]);
            }
            value.to_i64().expect("small affine sample fits i64")
        })
        .collect()
}

fn translated_ambient_point(ambient: &[i64], displacement: [i64; 3]) -> Option<Vec<i64>> {
    ambient
        .iter()
        .zip(displacement)
        .map(|(&value, delta)| value.checked_add(delta))
        .collect()
}

fn every_concrete_label_fits(source: &ParametricRelation, point: &[i64]) -> bool {
    source.terms().keys().all(|shift| {
        point
            .iter()
            .zip(shift.values())
            .all(|(&value, &offset)| value.checked_add(offset).is_some())
    })
}

fn centered_free_points(free_count: usize, radius: i64) -> Vec<Vec<i64>> {
    assert!(radius >= 0);
    let mut coordinates = Vec::new();
    coordinates.push(0);
    for magnitude in 1..=radius {
        coordinates.push(magnitude);
        coordinates.push(-magnitude);
    }
    let exponent = u32::try_from(free_count).expect("sunset free-coordinate count fits u32");
    let point_count = coordinates
        .len()
        .checked_pow(exponent)
        .expect("bounded sunset Cartesian sample count fits usize");
    let mut points = Vec::new();
    points
        .try_reserve_exact(point_count)
        .expect("bounded sunset Cartesian samples fit memory");
    for mut ordinal in 0..point_count {
        let mut point = Vec::new();
        point
            .try_reserve_exact(free_count)
            .expect("bounded sunset free point fits memory");
        for _ in 0..free_count {
            point.push(coordinates[ordinal % coordinates.len()]);
            ordinal /= coordinates.len();
        }
        points.push(point);
    }
    points
}

macro_rules! assert_exact_source_arcs {
    ($certificate:expr, $span:expr, $branch:expr, $guards:expr, $row:expr, $translation:expr) => {{
        assert!(Arc::ptr_eq($certificate.row_span(), $span));
        assert!(Arc::ptr_eq($certificate.branch(), $branch));
        assert!(Arc::ptr_eq($certificate.branch_guards(), $guards));
        assert_eq!($certificate.source_row_ordinal(), $row);
        assert_eq!($certificate.translation(), $translation);
    }};
}

#[test]
fn every_generated_sunset_branch_bound_outcome_retains_exact_sources_and_replays() {
    let mut retained = 0usize;
    let mut empty = 0usize;
    let mut unavailable = 0usize;
    let mut compilations = 0usize;

    for bits in ["011", "101", "110", "111"] {
        let (family, context, cover) = generated_cover(bits);
        let branches = guarded_branches(&family, &context, &cover);
        assert!(!branches.is_empty(), "sector {bits}");
        let span = row_span(&cover);
        assert!(!span.rows().is_empty());

        for branch in branches {
            let guards = composed_guards(&family, &context, &cover, &branch);
            for source_row_ordinal in 0..span.rows().len() {
                let translation = IndexShift::try_new([0, 0, 0], 3).unwrap();
                let compilation = GeneratedResidualAffineBranchBoundRelationCompiler::compile(
                    &family,
                    &context,
                    source_row_ordinal,
                    translation.clone(),
                    branch.clone(),
                    guards.clone(),
                    GeneratedResidualAffineBranchBoundRelationLimits::default(),
                )
                .unwrap();
                compilations += 1;

                match compilation {
                    GeneratedResidualAffineBranchBoundRelationCompilation::Retained(bound) => {
                        retained += 1;
                        assert_exact_source_arcs!(
                            bound,
                            span,
                            &branch,
                            &guards,
                            source_row_ordinal,
                            &translation
                        );
                        bound.replay(&family, &context).unwrap();
                    }
                    GeneratedResidualAffineBranchBoundRelationCompilation::EmptyBranch(
                        certificate,
                    ) => {
                        empty += 1;
                        assert_exact_source_arcs!(
                            certificate,
                            span,
                            &branch,
                            &guards,
                            source_row_ordinal,
                            &translation
                        );
                        match certificate.reason() {
                            GeneratedResidualAffineBranchEmptyReason::NonzeroGuardContradiction {
                                entry_ordinal,
                                structural_locus_ordinal,
                            } => {
                                assert_eq!(
                                    guards.entries()[*entry_ordinal].structural_locus_ordinal(),
                                    *structural_locus_ordinal
                                );
                            }
                        }
                        certificate.replay(&family, &context).unwrap();
                    }
                    GeneratedResidualAffineBranchBoundRelationCompilation::UnavailableRow(
                        certificate,
                    ) => {
                        unavailable += 1;
                        assert_exact_source_arcs!(
                            certificate,
                            span,
                            &branch,
                            &guards,
                            source_row_ordinal,
                            &translation
                        );
                        let source = &span.rows()[source_row_ordinal];
                        match certificate.reason() {
                            GeneratedResidualAffineBranchUnavailableReason::TranslatedSourceGuardComposesToZero {
                                guard_ordinal,
                            } => {
                                assert!(*guard_ordinal < source.nonzero_conditions().len());
                            }
                            GeneratedResidualAffineBranchUnavailableReason::TranslatedSourceTermDenominatorComposesToZero {
                                term_ordinal,
                                translated_shift,
                            } => {
                                assert!(*term_ordinal < source.terms().len());
                                assert_eq!(
                                    translated_shift.values().len(),
                                    context.index_count()
                                );
                            }
                        }
                        certificate.replay(&family, &context).unwrap();
                    }
                }
            }
        }
    }

    assert!(
        retained > 0,
        "generated sunset rows must survive on some branch"
    );
    assert!(compilations > 0);
    assert_eq!(retained + empty + unavailable, compilations);
}

#[test]
fn public_pretranslation_shape_limits_accept_exact_and_reject_one_below() {
    let (family, context, cover) = generated_cover("011");
    let span = row_span(&cover);
    let branch = guarded_branches(&family, &context, &cover)
        .into_iter()
        .next()
        .expect("sunset sector 011 has a guarded affine branch");
    let guards = composed_guards(&family, &context, &cover, &branch);
    let source_row_ordinal = 0usize;
    let source = &span.rows()[source_row_ordinal];
    let source_terms = source.terms().len();
    let translated_guard_upper_bound = source
        .guarded_nonzero_conditions()
        .len()
        .checked_add(source_terms)
        .expect("small generated sunset guard bound fits usize");
    let polynomial_composition_upper_bound = source_terms
        .checked_mul(2)
        .and_then(|terms| terms.checked_add(translated_guard_upper_bound))
        .expect("small generated sunset composition bound fits usize");
    assert!(source_terms > 0);
    assert!(translated_guard_upper_bound > 0);
    assert!(polynomial_composition_upper_bound > 0);

    let compile = |limits| {
        GeneratedResidualAffineBranchBoundRelationCompiler::compile(
            &family,
            &context,
            source_row_ordinal,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            branch.clone(),
            guards.clone(),
            limits,
        )
    };

    let mut exact = GeneratedResidualAffineBranchBoundRelationLimits::default();
    exact.max_translated_terms = source_terms;
    assert!(compile(exact).is_ok());
    let mut one_below = GeneratedResidualAffineBranchBoundRelationLimits::default();
    one_below.max_translated_terms = source_terms - 1;
    assert!(matches!(
        compile(one_below),
        Err(
            GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
                resource: "translated terms",
                requested,
                limit,
            }
        ) if requested == source_terms && limit == source_terms - 1
    ));

    let mut exact = GeneratedResidualAffineBranchBoundRelationLimits::default();
    exact.max_translated_guards = translated_guard_upper_bound;
    assert!(compile(exact).is_ok());
    let mut one_below = GeneratedResidualAffineBranchBoundRelationLimits::default();
    one_below.max_translated_guards = translated_guard_upper_bound - 1;
    assert!(matches!(
        compile(one_below),
        Err(
            GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
                resource: "translated guards",
                requested,
                limit,
            }
        ) if requested == translated_guard_upper_bound
            && limit == translated_guard_upper_bound - 1
    ));

    let mut exact = GeneratedResidualAffineBranchBoundRelationLimits::default();
    exact.max_polynomial_compositions = polynomial_composition_upper_bound;
    assert!(compile(exact).is_ok());
    let mut one_below = GeneratedResidualAffineBranchBoundRelationLimits::default();
    one_below.max_polynomial_compositions = polynomial_composition_upper_bound - 1;
    assert!(matches!(
        compile(one_below),
        Err(
            GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
                resource: "polynomial compositions",
                requested,
                limit,
            }
        ) if requested == polynomial_composition_upper_bound
            && limit == polynomial_composition_upper_bound - 1
    ));
}

#[test]
fn nonzero_translation_matches_direct_generated_row_specialization_at_free_points() {
    let (family, context, cover) = generated_cover("011");
    let span = row_span(&cover).clone();
    let displacement = [1i64, -1, 2];
    let translation = IndexShift::try_new(displacement, 3).unwrap();

    let mut retained = None;
    let mut branch_count = 0usize;
    let mut branches_with_three_points = 0usize;
    let mut retained_rows = 0usize;
    let mut empty_rows = 0usize;
    let mut unavailable_rows = 0usize;
    for branch in guarded_branches(&family, &context, &cover) {
        branch_count += 1;
        let Some(map) = branch.affine_map() else {
            continue;
        };
        if map.free_positions().is_empty() {
            continue;
        }
        let free_count = map.free_positions().len();
        let mut applicable_points = Vec::new();
        for free_values in centered_free_points(free_count, 12) {
            let ambient = affine_ambient_point(&branch, &free_values);
            if branch
                .guarded_affine_map_applies_at_original_indices(&context, &ambient)
                .unwrap()
                && translated_ambient_point(&ambient, displacement).is_some()
            {
                applicable_points.push(free_values);
                if applicable_points.len() == 64 {
                    break;
                }
            }
        }
        if applicable_points.len() < 3 {
            continue;
        }
        branches_with_three_points += 1;
        let guards = composed_guards(&family, &context, &cover, &branch);
        for source_row_ordinal in 0..span.rows().len() {
            let source = &span.rows()[source_row_ordinal];
            let safe_points = applicable_points
                .iter()
                .filter(|free_values| {
                    let ambient = affine_ambient_point(&branch, free_values);
                    translated_ambient_point(&ambient, displacement)
                        .is_some_and(|point| every_concrete_label_fits(source, &point))
                })
                .take(3)
                .cloned()
                .collect::<Vec<_>>();
            if safe_points.len() != 3 {
                continue;
            }
            let compilation = GeneratedResidualAffineBranchBoundRelationCompiler::compile(
                &family,
                &context,
                source_row_ordinal,
                translation.clone(),
                branch.clone(),
                guards.clone(),
                GeneratedResidualAffineBranchBoundRelationLimits::default(),
            )
            .unwrap();
            match compilation {
                GeneratedResidualAffineBranchBoundRelationCompilation::Retained(bound) => {
                    retained_rows += 1;
                    retained = Some((source_row_ordinal, branch, guards, bound, safe_points));
                    break;
                }
                GeneratedResidualAffineBranchBoundRelationCompilation::EmptyBranch(_) => {
                    empty_rows += 1;
                }
                GeneratedResidualAffineBranchBoundRelationCompilation::UnavailableRow(_) => {
                    unavailable_rows += 1;
                }
            }
        }
        if retained.is_some() {
            break;
        }
    }

    let (source_row_ordinal, branch, guards, bound, applicable_points) = retained.unwrap_or_else(|| {
        panic!(
            "a generated sunset row survives a nonzero translation; branches={branch_count}, branches_with_three_points={branches_with_three_points}, retained_rows={retained_rows}, empty_rows={empty_rows}, unavailable_rows={unavailable_rows}"
        )
    });
    assert_exact_source_arcs!(
        bound,
        &span,
        &branch,
        &guards,
        source_row_ordinal,
        &translation
    );
    bound.replay(&family, &context).unwrap();
    let source: &ParametricRelation = &span.rows()[source_row_ordinal];
    let concrete_limits = GeneratedResidualAffineBranchConcreteSpecializationLimits::default();
    let mut samples = 0usize;

    for free_values in applicable_points {
        let ambient = affine_ambient_point(&branch, &free_values);
        let direct_point = translated_ambient_point(&ambient, displacement)
            .expect("the concrete probe preflighted every translated coordinate");
        let mapped = bound
            .specialize_at_free_values(&context, &free_values, concrete_limits)
            .unwrap();
        let direct = source
            .specialize(&context, &direct_point, concrete_limits.arithmetic)
            .unwrap();
        assert_eq!(mapped.family_fingerprint(), direct.family_fingerprint());
        assert_eq!(
            mapped.terms(),
            direct.terms(),
            "term mismatch at free point {free_values:?}"
        );
        // Restricting a row to a complete Boolean branch deliberately adds
        // the branch's common nonzero conditions.  The translated source
        // domain must be preserved exactly; any additional concrete guards
        // can only come from those already-authenticated branch conditions.
        let mut mapped_guards = polynomial_multiset(&mapped);
        for direct_guard in polynomial_multiset(&direct) {
            let position = mapped_guards
                .iter()
                .position(|mapped_guard| mapped_guard == &direct_guard)
                .unwrap_or_else(|| {
                    panic!(
                        "translated source guard {direct_guard} was lost at free point {free_values:?}"
                    )
                });
            mapped_guards.remove(position);
        }
        let branch_condition_count = guards
            .entries()
            .iter()
            .filter(|entry| entry.class().condition().is_some())
            .count();
        assert!(mapped_guards.len() <= branch_condition_count);
        samples += 1;
        if samples == 3 {
            break;
        }
    }
    assert_eq!(samples, 3, "expected three applicable affine samples");
}

#[test]
fn invalid_generated_source_ordinal_translation_arity_and_branch_arc_are_rejected() {
    let (family, context, cover) = generated_cover("111");
    let span = row_span(&cover);
    let branch = guarded_branches(&family, &context, &cover)
        .into_iter()
        .next()
        .expect("sunset has a guarded affine branch");
    let guards = composed_guards(&family, &context, &cover, &branch);
    let limits = GeneratedResidualAffineBranchBoundRelationLimits::default();

    let ordinal = span.rows().len();
    assert!(matches!(
        GeneratedResidualAffineBranchBoundRelationCompiler::compile(
            &family,
            &context,
            ordinal,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            branch.clone(),
            guards.clone(),
            limits,
        ),
        Err(GeneratedResidualAffineBranchBoundRelationError::SourceRowOrdinalOutOfRange {
            ordinal: rejected,
            rows,
        }) if rejected == ordinal && rows == ordinal
    ));

    assert!(matches!(
        GeneratedResidualAffineBranchBoundRelationCompiler::compile(
            &family,
            &context,
            0,
            IndexShift::try_new([0, 0], 2).unwrap(),
            branch.clone(),
            guards.clone(),
            limits,
        ),
        Err(
            GeneratedResidualAffineBranchBoundRelationError::WrongArity {
                expected: 3,
                actual: 2,
            }
        )
    ));

    // A payload-equal clone is still the wrong fresh source allocation for
    // the already-authenticated guard certificate.
    let mismatched_branch = Arc::new((*branch).clone());
    assert!(!Arc::ptr_eq(&mismatched_branch, &branch));
    assert!(matches!(
        GeneratedResidualAffineBranchBoundRelationCompiler::compile(
            &family,
            &context,
            0,
            IndexShift::try_new([0, 0, 0], 3).unwrap(),
            mismatched_branch,
            guards,
            limits,
        ),
        Err(
            GeneratedResidualAffineBranchBoundRelationError::BranchGuardSourceBranchAllocationMismatch
        )
    ));
}

#[test]
fn retained_term_and_byte_publication_limits_accept_exact_and_reject_one_below() {
    let (family, context, cover) = generated_cover("011");
    let span = row_span(&cover).clone();
    let translation = IndexShift::try_new([0, 0, 0], 3).unwrap();
    let mut retained_fixture = None;

    'branches: for branch in guarded_branches(&family, &context, &cover) {
        let guards = composed_guards(&family, &context, &cover, &branch);
        for source_row_ordinal in 0..span.rows().len() {
            let compilation = GeneratedResidualAffineBranchBoundRelationCompiler::compile(
                &family,
                &context,
                source_row_ordinal,
                translation.clone(),
                branch.clone(),
                guards.clone(),
                GeneratedResidualAffineBranchBoundRelationLimits::default(),
            )
            .unwrap();
            if let GeneratedResidualAffineBranchBoundRelationCompilation::Retained(bound) =
                compilation
            {
                retained_fixture = Some((
                    source_row_ordinal,
                    branch,
                    guards,
                    bound.stats().retained_terms(),
                    bound.stats().retained_bytes(),
                ));
                break 'branches;
            }
        }
    }

    let (source_row_ordinal, branch, guards, retained_terms, retained_bytes) = retained_fixture
        .expect("sector 011 exposes at least one retained generated branch-bound row");
    assert!(retained_terms > 0);
    assert!(retained_bytes > 0);
    let compile = |limits| {
        GeneratedResidualAffineBranchBoundRelationCompiler::compile(
            &family,
            &context,
            source_row_ordinal,
            translation.clone(),
            branch.clone(),
            guards.clone(),
            limits,
        )
    };

    let mut exact = GeneratedResidualAffineBranchBoundRelationLimits::default();
    exact.max_retained_terms = retained_terms;
    assert!(compile(exact).is_ok());
    let mut one_below = GeneratedResidualAffineBranchBoundRelationLimits::default();
    one_below.max_retained_terms = retained_terms - 1;
    assert!(matches!(
        compile(one_below),
        Err(
            GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
                resource: "retained terms",
                limit,
                ..
            }
        ) if limit == retained_terms - 1
    ));

    let mut exact = GeneratedResidualAffineBranchBoundRelationLimits::default();
    exact.max_retained_bytes = retained_bytes;
    assert!(compile(exact).is_ok());
    let mut one_below = GeneratedResidualAffineBranchBoundRelationLimits::default();
    one_below.max_retained_bytes = retained_bytes - 1;
    assert!(matches!(
        compile(one_below),
        Err(
            GeneratedResidualAffineBranchBoundRelationError::ResourceLimit {
                resource: "retained bytes",
                limit,
                ..
            }
        ) if limit == retained_bytes - 1
    ));
}
