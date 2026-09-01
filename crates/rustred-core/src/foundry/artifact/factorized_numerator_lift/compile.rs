//! Symbolica-backed compilation and exact replay of affine routing.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::algebra::matrix::multiply_coefficient_matrices;
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{
    AffineDenominator, IntegralFamily, IntegralFamilyError, ScalarProductCoordinate,
    congruence_symbolic_matrix, invert_symbolic_matrix, symbolica_matrix_limits,
};
use crate::foundry::artifact::FactorizationRule;
use crate::sector::{InteriorBounds, SectorInteriorDomain};

use super::error::FactorizedNumeratorLiftError;
use super::limits::FactorizedNumeratorLiftLimits;
use super::model::{
    CanonicalDenominatorRelation, CompiledFactorizationRouting, FactorizedNumeratorLiftAction,
    FactorizedNumeratorLiftCompilation, FactorizedNumeratorLiftUnsupportedReason,
    RoutedAffineDenominator,
};

struct GaugeCandidate {
    routing: CompiledFactorizationRouting,
    unit_image_count: usize,
}

/// Compile one authenticated factorization rule into its exact affine-routing
/// disposition.  All determinant, inverse, congruence, relation construction,
/// and relation replay algebra is delegated to Symbolica's public matrix API
/// through RustRed's checked adapters.
pub(crate) fn compile_factorized_numerator_lift(
    family: &IntegralFamily,
    rule: &FactorizationRule,
    limits: FactorizedNumeratorLiftLimits,
) -> Result<FactorizedNumeratorLiftCompilation, FactorizedNumeratorLiftError> {
    match rule.installed_family_fingerprint() {
        None => {
            return Err(FactorizedNumeratorLiftError::UnauthenticatedFactorizationRule);
        }
        Some(fingerprint) if fingerprint != family.fingerprint() => {
            return Err(FactorizedNumeratorLiftError::WrongFactorizationFamily);
        }
        Some(_) => {}
    }
    let arity = family.denominator_count();
    admit_limit(
        "factorized numerator routing arity",
        arity,
        limits.max_arity,
    )?;
    if family.external_count() != 0 {
        return Err(
            FactorizedNumeratorLiftError::UnsupportedExternalKinematics {
                external_count: family.external_count(),
            },
        );
    }
    if rule.application_domain().arity() != arity {
        return Err(FactorizedNumeratorLiftError::WrongRuleArity {
            expected: arity,
            actual: rule.application_domain().arity(),
        });
    }

    let loop_count = family.loop_count();
    let expected_entries = loop_count.checked_mul(loop_count).ok_or(
        FactorizedNumeratorLiftError::ResourceCountOverflow {
            resource: "factorization loop-basis entries",
        },
    )?;
    let loop_basis = rule.loop_basis();
    if loop_basis.dimension() != loop_count || loop_basis.row_major().len() != expected_entries {
        return Err(FactorizedNumeratorLiftError::MalformedLoopBasis {
            expected_dimension: loop_count,
            actual_dimension: loop_basis.dimension(),
            expected_entries,
            actual_entries: loop_basis.row_major().len(),
        });
    }
    let shift = u32::try_from(loop_count).map_err(|_| {
        FactorizedNumeratorLiftError::ResourceCountOverflow {
            resource: "factorized numerator row-sign gauges",
        }
    })?;
    let gauge_count =
        1_usize
            .checked_shl(shift)
            .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow {
                resource: "factorized numerator row-sign gauges",
            })?;
    admit_limit(
        "factorized numerator row-sign gauges",
        gauge_count,
        limits.max_sign_gauges,
    )?;

    let application_domain = complete_factorized_sector(rule)?;
    let mut best = None;
    for gauge in 0..gauge_count {
        let signed = signed_loop_basis(loop_basis.row_major(), loop_count, gauge)?;
        let candidate = compile_gauge(family, application_domain.clone(), signed)?;
        if best.as_ref().is_none_or(|current: &GaugeCandidate| {
            compare_candidates(&candidate, current) == Ordering::Less
        }) {
            best = Some(candidate);
        }
    }
    let routing = best
        .ok_or(FactorizedNumeratorLiftError::Invariant {
            detail: "a nonempty row-sign portfolio produced no routing candidate",
        })?
        .routing;

    verify_unit_image_injection(&routing)?;
    let missing_count = routing
        .unit_images
        .iter()
        .filter(|image| image.is_none())
        .count();
    let affine_source = routing.unit_images.iter().position(Option::is_none);
    match (missing_count, affine_source) {
        (0, None) => Ok(FactorizedNumeratorLiftCompilation::NoAffineLiftRequired(
            routing,
        )),
        (1, Some(source)) if routing.application_domain.sector().active_bits()[source] => {
            Ok(FactorizedNumeratorLiftCompilation::Unsupported {
                routing,
                reason: FactorizedNumeratorLiftUnsupportedReason::AffineSourceIsActive { source },
            })
        }
        (1, Some(source)) => {
            let relation = &routing.relations[source];
            let branch_width = usize::from(!relation.constant.is_zero())
                .checked_add(
                    relation
                        .denominator_coefficients
                        .iter()
                        .filter(|coefficient| !coefficient.is_zero())
                        .count(),
                )
                .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow {
                    resource: "factorized numerator recurrence branches",
                })?;
            admit_limit(
                "factorized numerator recurrence branches",
                branch_width,
                limits.max_recurrence_branches,
            )?;
            if branch_width == 0 {
                return Err(FactorizedNumeratorLiftError::Invariant {
                    detail: "an affine source relation has no nonzero terms",
                });
            }
            Ok(FactorizedNumeratorLiftCompilation::Action(
                FactorizedNumeratorLiftAction {
                    routing,
                    affine_source: source,
                    branch_width,
                },
            ))
        }
        (count, Some(_)) if count > 1 => Ok(FactorizedNumeratorLiftCompilation::Unsupported {
            routing,
            reason: FactorizedNumeratorLiftUnsupportedReason::MultipleAffineSourceRows { count },
        }),
        _ => Err(FactorizedNumeratorLiftError::Invariant {
            detail: "the affine-source census is internally inconsistent",
        }),
    }
}

fn complete_factorized_sector(
    rule: &FactorizationRule,
) -> Result<SectorInteriorDomain, FactorizedNumeratorLiftError> {
    let sector = rule.application_domain().sector().clone();
    Ok(SectorInteriorDomain::try_new(
        sector.clone(),
        sector.active_bits().iter().map(|&active| {
            if active {
                InteriorBounds::new(1, i64::MAX)
            } else {
                InteriorBounds::new(i64::MIN, 0)
            }
        }),
    )?)
}

fn compare_candidates(left: &GaugeCandidate, right: &GaugeCandidate) -> Ordering {
    candidate_disposition_rank(left)
        .cmp(&candidate_disposition_rank(right))
        .then_with(|| right.unit_image_count.cmp(&left.unit_image_count))
        .then_with(|| {
            left.routing
                .signed_loop_basis
                .cmp(&right.routing.signed_loop_basis)
        })
}

/// Prefer an immediately executable portfolio disposition before applying the
/// unit-count and lexicographic gauge tie-breakers: pure routing, then exactly
/// one inactive affine source, then a typed unsupported routing.
fn candidate_disposition_rank(candidate: &GaugeCandidate) -> u8 {
    let mut missing = candidate
        .routing
        .unit_images
        .iter()
        .enumerate()
        .filter_map(|(source, image)| image.is_none().then_some(source));
    match (missing.next(), missing.next()) {
        (None, None) => 0,
        (Some(source), None)
            if !candidate.routing.application_domain.sector().active_bits()[source] =>
        {
            1
        }
        _ => 2,
    }
}

fn compile_gauge(
    family: &IntegralFamily,
    application_domain: SectorInteriorDomain,
    signed_loop_basis: Box<[i64]>,
) -> Result<GaugeCandidate, FactorizedNumeratorLiftError> {
    let context = family.coefficient_context();
    let basis = coefficient_matrix(context, &signed_loop_basis, family.loop_count())?;
    let (inverse, determinant) =
        invert_symbolic_matrix(context, &basis, family.construction_limits())?;
    if determinant != context.one() && determinant != context.integer(-1) {
        return Err(FactorizedNumeratorLiftError::NonUnimodularLoopBasis);
    }
    let inverse_transpose = transpose_square(&inverse)?;
    let transformed_denominators =
        transform_denominators(family, &inverse_transpose)?.into_boxed_slice();
    let relations = relations_in_canonical_denominators(family, &transformed_denominators)?;
    replay_relations(family, &transformed_denominators, &relations)?;
    let mut unit_images = Vec::new();
    try_reserve(
        &mut unit_images,
        relations.len(),
        "factorized numerator unit images",
    )?;
    for relation in &relations {
        unit_images.push(unit_image(context, relation, family)?);
    }
    let unit_images = unit_images.into_boxed_slice();
    let unit_image_count = unit_images.iter().filter(|image| image.is_some()).count();
    Ok(GaugeCandidate {
        routing: CompiledFactorizationRouting {
            identity: Arc::new(()),
            family_fingerprint: family.fingerprint_owner(),
            application_domain,
            signed_loop_basis,
            loop_basis_determinant: determinant,
            transformed_denominators,
            relations,
            unit_images,
        },
        unit_image_count,
    })
}

fn signed_loop_basis(
    base: &[i64],
    dimension: usize,
    gauge: usize,
) -> Result<Box<[i64]>, FactorizedNumeratorLiftError> {
    let mut signed = Vec::new();
    try_reserve(&mut signed, base.len(), "signed loop-basis entries")?;
    for (entry, &value) in base.iter().enumerate() {
        let row = entry / dimension;
        let value = if gauge & (1 << row) == 0 {
            value
        } else {
            value
                .checked_neg()
                .ok_or(FactorizedNumeratorLiftError::LoopBasisEntryOverflow { entry })?
        };
        signed.push(value);
    }
    Ok(signed.into_boxed_slice())
}

fn coefficient_matrix(
    context: &CoefficientContext,
    entries: &[i64],
    dimension: usize,
) -> Result<Vec<Vec<Coefficient>>, FactorizedNumeratorLiftError> {
    let mut matrix = Vec::new();
    try_reserve(&mut matrix, dimension, "loop-basis matrix rows")?;
    for row in entries.chunks_exact(dimension) {
        let mut output = Vec::new();
        try_reserve(&mut output, dimension, "loop-basis matrix entries")?;
        output.extend(row.iter().map(|&entry| context.integer(entry)));
        matrix.push(output);
    }
    Ok(matrix)
}

fn transpose_square(
    matrix: &[Vec<Coefficient>],
) -> Result<Vec<Vec<Coefficient>>, FactorizedNumeratorLiftError> {
    let dimension = matrix.len();
    if matrix.iter().any(|row| row.len() != dimension) {
        return Err(FactorizedNumeratorLiftError::Invariant {
            detail: "Symbolica's verified inverse is not square",
        });
    }
    let mut transpose = Vec::new();
    try_reserve(&mut transpose, dimension, "inverse-transpose rows")?;
    for column in 0..dimension {
        let mut row = Vec::new();
        try_reserve(&mut row, dimension, "inverse-transpose entries")?;
        row.extend(matrix.iter().map(|input| input[column].clone()));
        transpose.push(row);
    }
    Ok(transpose)
}

fn transform_denominators(
    family: &IntegralFamily,
    inverse_transpose: &[Vec<Coefficient>],
) -> Result<Vec<RoutedAffineDenominator>, FactorizedNumeratorLiftError> {
    let mut forms = Vec::new();
    try_reserve(
        &mut forms,
        family.denominator_count(),
        "transformed affine denominators",
    )?;
    for denominator in family.denominators() {
        let quadratic = denominator_quadratic(family, denominator)?;
        let transformed = congruence_symbolic_matrix(
            family.coefficient_context(),
            inverse_transpose,
            &quadratic,
            family.construction_limits(),
        )?;
        forms.push(RoutedAffineDenominator {
            constant: denominator.constant().clone(),
            scalar_coefficients: scalar_coefficients_from_quadratic(family, &transformed)?,
        });
    }
    Ok(forms)
}

fn denominator_quadratic(
    family: &IntegralFamily,
    denominator: &AffineDenominator,
) -> Result<Vec<Vec<Coefficient>>, FactorizedNumeratorLiftError> {
    let context = family.coefficient_context();
    let loops = family.loop_count();
    let mut matrix = zero_matrix(context, loops, loops, "denominator quadratic entries")?;
    for (coordinate, coefficient) in family.coordinates().iter().zip(denominator.coefficients()) {
        match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                matrix[left][right] = coefficient.clone();
            }
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let half = context
                    .try_div(
                        coefficient,
                        &context.integer(2),
                        family.construction_limits().exact_algebra,
                    )
                    .map_err(IntegralFamilyError::from)?;
                matrix[left][right] = half.clone();
                matrix[right][left] = half;
            }
            ScalarProductCoordinate::LoopExternal { .. } => {
                return Err(
                    FactorizedNumeratorLiftError::UnsupportedExternalKinematics {
                        external_count: family.external_count(),
                    },
                );
            }
        }
    }
    Ok(matrix)
}

fn scalar_coefficients_from_quadratic(
    family: &IntegralFamily,
    quadratic: &[Vec<Coefficient>],
) -> Result<Box<[Coefficient]>, FactorizedNumeratorLiftError> {
    let context = family.coefficient_context();
    let mut coefficients = Vec::new();
    try_reserve(
        &mut coefficients,
        family.coordinates().len(),
        "transformed scalar coefficients",
    )?;
    for coordinate in family.coordinates() {
        let coefficient = match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                quadratic[left][right].clone()
            }
            ScalarProductCoordinate::LoopLoop { left, right } => context
                .try_mul(
                    &context.integer(2),
                    &quadratic[left][right],
                    family.construction_limits().exact_algebra,
                )
                .map_err(IntegralFamilyError::from)?,
            ScalarProductCoordinate::LoopExternal { .. } => {
                return Err(
                    FactorizedNumeratorLiftError::UnsupportedExternalKinematics {
                        external_count: family.external_count(),
                    },
                );
            }
        };
        coefficients.push(coefficient);
    }
    Ok(coefficients.into_boxed_slice())
}

fn relations_in_canonical_denominators(
    family: &IntegralFamily,
    forms: &[RoutedAffineDenominator],
) -> Result<Box<[CanonicalDenominatorRelation]>, FactorizedNumeratorLiftError> {
    let context = family.coefficient_context();
    let arity = family.denominator_count();
    let affine_size =
        arity
            .checked_add(1)
            .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow {
                resource: "affine denominator matrix dimension",
            })?;
    let mut constants = Vec::new();
    try_reserve(&mut constants, arity, "denominator constant rows")?;
    for denominator in family.denominators() {
        let mut row = Vec::new();
        try_reserve(&mut row, 1, "denominator constant entries")?;
        row.push(denominator.constant().clone());
        constants.push(row);
    }
    let matrix_limits = symbolica_matrix_limits(family.construction_limits());
    let (inverse_times_constants, _) =
        multiply_coefficient_matrices(context, family.inverse_basis(), &constants, matrix_limits)?;

    let mut affine_inverse =
        zero_matrix(context, affine_size, affine_size, "affine inverse entries")?;
    affine_inverse[0][0] = context.one();
    for row in 0..arity {
        affine_inverse[row + 1][0] = context
            .try_neg(
                &inverse_times_constants[row][0],
                family.construction_limits().exact_algebra,
            )
            .map_err(IntegralFamilyError::from)?;
        for column in 0..arity {
            affine_inverse[row + 1][column + 1] = family.inverse_basis()[row][column].clone();
        }
    }

    let mut form_rows = Vec::new();
    try_reserve(&mut form_rows, forms.len(), "affine form rows")?;
    for form in forms {
        let mut row = Vec::new();
        try_reserve(&mut row, affine_size, "affine form entries")?;
        row.push(form.constant.clone());
        row.extend(form.scalar_coefficients.iter().cloned());
        form_rows.push(row);
    }
    let (relation_rows, _) =
        multiply_coefficient_matrices(context, &form_rows, &affine_inverse, matrix_limits)?;
    let mut relations = Vec::new();
    try_reserve(
        &mut relations,
        relation_rows.len(),
        "affine denominator relations",
    )?;
    for mut row in relation_rows {
        if row.len() != affine_size {
            return Err(FactorizedNumeratorLiftError::Invariant {
                detail: "Symbolica returned an affine relation with the wrong arity",
            });
        }
        let denominator_coefficients = row.split_off(1).into_boxed_slice();
        let constant = row.pop().ok_or(FactorizedNumeratorLiftError::Invariant {
            detail: "Symbolica returned an empty affine relation",
        })?;
        relations.push(CanonicalDenominatorRelation {
            constant,
            denominator_coefficients,
        });
    }
    Ok(relations.into_boxed_slice())
}

fn replay_relations(
    family: &IntegralFamily,
    forms: &[RoutedAffineDenominator],
    relations: &[CanonicalDenominatorRelation],
) -> Result<(), FactorizedNumeratorLiftError> {
    let context = family.coefficient_context();
    let arity = family.denominator_count();
    let affine_size =
        arity
            .checked_add(1)
            .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow {
                resource: "affine replay matrix dimension",
            })?;
    let mut relation_rows = Vec::new();
    try_reserve(
        &mut relation_rows,
        relations.len(),
        "affine replay relation rows",
    )?;
    for relation in relations {
        let mut row = Vec::new();
        try_reserve(&mut row, affine_size, "affine replay relation entries")?;
        row.push(relation.constant.clone());
        row.extend(relation.denominator_coefficients.iter().cloned());
        relation_rows.push(row);
    }

    // [1,D_1,...,D_K]^T = B [1,S_1,...,S_K]^T.
    let mut canonical_affine = zero_matrix(
        context,
        affine_size,
        affine_size,
        "canonical affine basis entries",
    )?;
    canonical_affine[0][0] = context.one();
    for (row, denominator) in family.denominators().iter().enumerate() {
        canonical_affine[row + 1][0] = denominator.constant().clone();
        for (column, coefficient) in denominator.coefficients().iter().enumerate() {
            canonical_affine[row + 1][column + 1] = coefficient.clone();
        }
    }
    let (replayed, _) = multiply_coefficient_matrices(
        context,
        &relation_rows,
        &canonical_affine,
        symbolica_matrix_limits(family.construction_limits()),
    )?;
    for (denominator, (actual, expected)) in replayed.iter().zip(forms).enumerate() {
        for (component, (actual, expected)) in actual
            .iter()
            .zip(std::iter::once(&expected.constant).chain(expected.scalar_coefficients.iter()))
            .enumerate()
        {
            if !coefficients_equal(family, actual, expected)? {
                return Err(FactorizedNumeratorLiftError::RelationReplayFailure {
                    denominator,
                    component,
                });
            }
        }
    }
    Ok(())
}

fn unit_image(
    context: &CoefficientContext,
    relation: &CanonicalDenominatorRelation,
    family: &IntegralFamily,
) -> Result<Option<usize>, FactorizedNumeratorLiftError> {
    if !relation.constant.is_zero() {
        return Ok(None);
    }
    let mut image = None;
    for (position, coefficient) in relation.denominator_coefficients.iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        if !coefficients_equal(family, coefficient, &context.one())? || image.is_some() {
            return Ok(None);
        }
        image = Some(position);
    }
    Ok(image)
}

fn verify_unit_image_injection(
    routing: &CompiledFactorizationRouting,
) -> Result<(), FactorizedNumeratorLiftError> {
    let arity = routing.unit_images.len();
    let mut seen = Vec::new();
    try_reserve(
        &mut seen,
        arity,
        "factorized numerator unit-image occupancy",
    )?;
    seen.resize(arity, false);
    for image in routing.unit_images.iter().flatten() {
        let occupied = seen
            .get_mut(*image)
            .ok_or(FactorizedNumeratorLiftError::Invariant {
                detail: "a canonical unit image is outside the family arity",
            })?;
        if *occupied {
            return Err(FactorizedNumeratorLiftError::UnitImageCollision { image: *image });
        }
        *occupied = true;
    }
    Ok(())
}

fn coefficients_equal(
    family: &IntegralFamily,
    left: &Coefficient,
    right: &Coefficient,
) -> Result<bool, FactorizedNumeratorLiftError> {
    if left == right {
        Ok(true)
    } else {
        Ok(family
            .coefficient_context()
            .try_sub(left, right, family.construction_limits().exact_algebra)
            .map_err(IntegralFamilyError::from)?
            .is_zero())
    }
}

fn zero_matrix(
    context: &CoefficientContext,
    rows: usize,
    columns: usize,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, FactorizedNumeratorLiftError> {
    let entries = rows
        .checked_mul(columns)
        .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow { resource })?;
    let mut matrix = Vec::new();
    try_reserve(&mut matrix, rows, resource)?;
    for _ in 0..rows {
        let mut row = Vec::new();
        try_reserve(&mut row, columns, resource)?;
        row.resize_with(columns, || context.zero());
        matrix.push(row);
    }
    debug_assert_eq!(matrix.iter().map(Vec::len).sum::<usize>(), entries);
    Ok(matrix)
}

fn admit_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FactorizedNumeratorLiftError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(FactorizedNumeratorLiftError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn try_reserve<T>(
    output: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), FactorizedNumeratorLiftError> {
    let requested = output
        .len()
        .checked_add(additional)
        .ok_or(FactorizedNumeratorLiftError::ResourceCountOverflow { resource })?;
    output.try_reserve_exact(additional).map_err(|_| {
        FactorizedNumeratorLiftError::AllocationFailure {
            resource,
            requested,
        }
    })
}
