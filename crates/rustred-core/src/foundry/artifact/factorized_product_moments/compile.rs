//! Structural compilation of one authenticated closed-block product chart.

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey, ScalarProductCoordinate};
use crate::sector::{InteriorBounds, SectorInteriorDomain};

use super::super::CommonMassHomogeneityProof;
use super::super::factorized_numerator_lift::{
    FactorizedNumeratorLiftLimits, compile_factorized_numerator_lift,
};
use super::super::{ClosedArtifact, FactorizationRule};
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;
use super::model::{
    CorrelatedMomentBranch, CorrelatedProductBlock, DependencyRootPreimage,
    FactorizedProductMomentProgram, ProductApplicationDomain, ProductBlockLayout,
    ProductMomentVariable, ProductNumeratorBranch, SingletonProductBlock,
};
use super::resources::{admit_compiled_embedding_key_payload, constant_integer_magnitude_bits};

/// Rebuild every process-local product program from already authenticated
/// factorization recipes. No algebra payload is copied into persistence.
pub(crate) fn compile_factorized_product_moment_programs(
    family: &IntegralFamily,
    dependencies: &[Box<ClosedArtifact>],
    factorization_rules: &[FactorizationRule],
) -> Result<Vec<Option<FactorizedProductMomentProgram>>, FactorizedProductMomentError> {
    let mut programs = Vec::new();
    programs
        .try_reserve_exact(factorization_rules.len())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "factorized product programs",
            requested: factorization_rules.len(),
        })?;
    for ordinal in 0..factorization_rules.len() {
        let program = match compile_factorized_product_moment_program(
            family,
            dependencies,
            factorization_rules,
            ordinal,
            FactorizedProductMomentLimits::default(),
        ) {
            Ok(program) => Some(program),
            Err(error) if error.is_unsupported_program_shape() => None,
            Err(error) => return Err(error),
        };
        programs.push(program);
    }
    Ok(programs)
}

pub(crate) fn compile_factorized_product_moment_program(
    family: &IntegralFamily,
    dependencies: &[Box<ClosedArtifact>],
    factorization_rules: &[FactorizationRule],
    factorization_ordinal: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<FactorizedProductMomentProgram, FactorizedProductMomentError> {
    admit_limit(
        "product chart arity",
        family.denominator_count(),
        limits.max_arity,
    )?;
    validate_parent_power_shifts(family, limits)?;
    let rule = factorization_rules.get(factorization_ordinal).ok_or(
        FactorizedProductMomentError::MissingFactorizationRule {
            ordinal: factorization_ordinal,
        },
    )?;
    let compiled = compile_factorized_numerator_lift(
        family,
        rule,
        FactorizedNumeratorLiftLimits {
            max_arity: limits.max_arity,
            ..FactorizedNumeratorLiftLimits::default()
        },
    )?;
    let routing = compiled.routing();
    if routing.family_fingerprint() != family.fingerprint() {
        return Err(FactorizedProductMomentError::WrongFamily);
    }

    let loop_count = family.loop_count();
    let row_signs = prove_signed_block_equivalence(
        rule.loop_basis().row_major(),
        routing.signed_loop_basis(),
        loop_count,
    )?;

    let active = rule.application_domain().sector().active_bits();
    if active.len() != family.denominator_count() {
        return Err(FactorizedProductMomentError::IncompleteFactorCover);
    }
    let mut seen_parent = fallible_filled(
        family.denominator_count(),
        false,
        "product parent occupancy",
    )?;
    let mut seen_vector = fallible_filled(loop_count, false, "product loop occupancy")?;
    let mut active_parent_positions = Vec::new();
    active_parent_positions
        .try_reserve_exact(active.iter().filter(|&&entry| entry).count())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "product active-parent positions",
            requested: active.iter().filter(|&&entry| entry).count(),
        })?;
    let mut singletons = Vec::new();
    singletons
        .try_reserve_exact(rule.factors().len())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "product singleton blocks",
            requested: rule.factors().len(),
        })?;
    let mut correlated = None;
    let mut correlated_count = 0_usize;
    for (factor_ordinal, factor) in rule.factors().iter().enumerate() {
        let dependency_ordinal = factor.dependency_ordinal();
        let dependency = dependencies.get(dependency_ordinal).ok_or(
            FactorizedProductMomentError::MissingDependency {
                ordinal: dependency_ordinal,
            },
        )?;
        if factor.parent_positions().len() != dependency.arity() {
            return Err(FactorizedProductMomentError::UnsupportedFactorShape {
                factor: factor_ordinal,
                detail: "parent positions do not match the dependency arity",
            });
        }
        if factor.transformed_loop_positions().len() != dependency.family().loop_count() {
            return Err(FactorizedProductMomentError::UnsupportedFactorShape {
                factor: factor_ordinal,
                detail: "transformed loop rows do not match the dependency loop count",
            });
        }
        let active_power_start = active_parent_positions.len();
        for &parent in factor.parent_positions() {
            if parent >= active.len() || !active[parent] || seen_parent[parent] {
                return Err(FactorizedProductMomentError::IncompleteFactorCover);
            }
            seen_parent[parent] = true;
            active_parent_positions.push(parent);
        }
        for &vector in factor.transformed_loop_positions() {
            if vector >= loop_count || seen_vector[vector] {
                return Err(FactorizedProductMomentError::IncompleteFactorCover);
            }
            seen_vector[vector] = true;
        }

        if dependency.family().loop_count() == 1 && dependency.arity() == 1 {
            if dependency.masters().len() != 1 {
                return Err(FactorizedProductMomentError::DependencyMasterCount {
                    ordinal: dependency_ordinal,
                    count: dependency.masters().len(),
                });
            }
            validate_tadpole_dependency(
                family.coefficient_context(),
                family.dimension(),
                dependency,
                dependency_ordinal,
                limits,
            )?;
            singletons.push(SingletonProductBlock {
                dependency_ordinal,
                parent_position: factor.parent_positions()[0],
                transformed_vector: factor.transformed_loop_positions()[0],
                active_power_ordinal: active_power_start,
            });
        } else {
            correlated_count = correlated_count.checked_add(1).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product correlated blocks",
                },
            )?;
            admit_correlated_factor_count(correlated_count)?;
            validate_correlated_dependency(
                family.coefficient_context(),
                family.dimension(),
                dependency,
                dependency_ordinal,
                limits,
            )?;
            let vector_signs = clone_selected_i64_box(
                &row_signs,
                factor.transformed_loop_positions(),
                "product correlated row signs",
            )?;
            let moment_branches =
                compile_correlated_moment_branches(dependency, &vector_signs, limits)?;
            correlated = Some(CorrelatedProductBlock {
                dependency_ordinal,
                parent_positions: clone_usize_box(
                    factor.parent_positions(),
                    "product correlated parent positions",
                )?,
                transformed_vectors: clone_usize_box(
                    factor.transformed_loop_positions(),
                    "product correlated loop positions",
                )?,
                vector_signs,
                active_power_start,
                moment_branches,
            });
        }
    }
    if seen_vector.iter().any(|&seen| !seen)
        || active
            .iter()
            .enumerate()
            .any(|(position, &is_active)| is_active != seen_parent[position])
    {
        return Err(FactorizedProductMomentError::IncompleteFactorCover);
    }
    singletons.sort_unstable_by_key(|block| block.transformed_vector);
    let layout = if let Some(correlated) = correlated {
        if singletons.is_empty() {
            return Err(FactorizedProductMomentError::UnsupportedSingletonFactorCount { count: 0 });
        }
        ProductBlockLayout::OneCorrelated {
            correlated,
            singletons_by_vector: singletons.into_boxed_slice(),
        }
    } else {
        if singletons.len() != loop_count {
            return Err(FactorizedProductMomentError::UnsupportedFactorCount {
                expected: loop_count,
                actual: singletons.len(),
            });
        }
        ProductBlockLayout::AllSingleton {
            singletons_by_vector: singletons.into_boxed_slice(),
        }
    };

    let edges = complete_cross_edges(loop_count)?;
    let variable_count = loop_count.checked_add(edges.len()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product polynomial variables",
        },
    )?;
    admit_limit(
        "product polynomial variables",
        variable_count,
        limits.max_polynomial_variables,
    )?;
    let coordinate_positions = coordinate_positions(family, &edges)?;
    validate_routed_affine_coefficients(
        family.coefficient_context(),
        routing.transformed_denominators(),
        limits,
    )?;
    validate_block_denominators(
        dependencies,
        family.coefficient_context(),
        routing.transformed_denominators(),
        &coordinate_positions,
        &edges,
        &layout,
        limits,
    )?;

    let expected_embeddings = rule.factors().iter().try_fold(1_usize, |count, factor| {
        count
            .checked_mul(dependencies[factor.dependency_ordinal()].masters().len())
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product master embeddings",
            })
    })?;
    if expected_embeddings == 0 || rule.master_embeddings().len() != expected_embeddings {
        return Err(FactorizedProductMomentError::InvalidMasterEmbedding);
    }
    if let [embedding] = rule.master_embeddings() {
        admit_compiled_embedding_key_payload(2, family.denominator_count(), limits)?;
        let _ = IntegralKey::try_new(embedding.raw_parent_master().powers().iter().copied())?;
        let _ = IntegralKey::try_new(embedding.parent_terminal().powers().iter().copied())?;
    }

    let numerator_branches = compile_numerator_branches(
        family,
        routing.transformed_denominators(),
        &coordinate_positions,
        &edges,
    )?;
    let application_domain = compile_product_application_domain(
        rule,
        dependencies,
        &layout,
        &edges,
        &numerator_branches,
    )?;

    Ok(FactorizedProductMomentProgram {
        family_fingerprint: family.fingerprint_owner(),
        routing: compiled,
        layout,
        active_parent_positions: active_parent_positions.into_boxed_slice(),
        edges,
        numerator_branches,
        normalization: rule.normalization().clone(),
        application_domain,
    })
}

fn validate_parent_power_shifts(
    family: &IntegralFamily,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    for (position, shift) in family.power_shifts().iter().enumerate() {
        family
            .coefficient_context()
            .validate_with_limits(shift, limits.exact_algebra)?;
        if !shift.is_zero() {
            return Err(FactorizedProductMomentError::UnsupportedParentPowerShift { position });
        }
    }
    Ok(())
}

fn validate_routed_affine_coefficients(
    context: &CoefficientContext,
    forms: &[super::super::factorized_numerator_lift::RoutedAffineDenominator],
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    for (denominator, form) in forms.iter().enumerate() {
        for coefficient in std::iter::once(form.constant()).chain(form.scalar_coefficients()) {
            context.validate_with_limits(coefficient, limits.exact_algebra)?;
            if !coefficient.is_constant() {
                return Err(FactorizedProductMomentError::NonconstantAffineCoefficient {
                    denominator,
                });
            }
            if constant_integer_magnitude_bits(coefficient).is_none() {
                return Err(FactorizedProductMomentError::NonintegerAffineCoefficient {
                    denominator,
                });
            }
        }
    }
    Ok(())
}

fn validate_tadpole_dependency(
    parent_context: &CoefficientContext,
    parent_dimension: &Coefficient,
    dependency: &super::super::ClosedArtifact,
    ordinal: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let family = dependency.family();
    let semantic_shape = dependency.common_mass_homogeneity()
        == Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
        && family.loop_count() == 1
        && family.external_count() == 0
        && family.denominator_count() == 1
        && family.coordinates() == [ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }]
        && parent_context.has_same_variable_map(family.coefficient_context())
        && family.power_shifts().len() == 1
        && family.power_shifts()[0].is_zero();
    if !semantic_shape {
        return Err(FactorizedProductMomentError::UnsupportedDependencySemantic { ordinal });
    }
    if !coefficients_equal(parent_context, parent_dimension, family.dimension(), limits)? {
        return Err(FactorizedProductMomentError::DependencyDimensionMismatch { ordinal });
    }
    let denominator = &family.denominators()[0];
    if denominator.coefficients().len() != 1
        || !coefficients_equal(
            family.coefficient_context(),
            denominator.constant(),
            &family.coefficient_context().integer(-1),
            limits,
        )?
        || !coefficients_equal(
            family.coefficient_context(),
            &denominator.coefficients()[0],
            &family.coefficient_context().one(),
            limits,
        )?
        || dependency
            .masters()
            .first()
            .is_none_or(|master| master.powers() != [1])
    {
        return Err(FactorizedProductMomentError::UnsupportedDependencySemantic { ordinal });
    }
    Ok(())
}

fn validate_correlated_dependency(
    parent_context: &CoefficientContext,
    parent_dimension: &Coefficient,
    dependency: &super::super::ClosedArtifact,
    ordinal: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let family = dependency.family();
    let loops = family.loop_count();
    let scalar_products = loops
        .checked_mul(loops.checked_add(1).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated dependency scalar products",
            },
        )?)
        .and_then(|value| value.checked_div(2))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "correlated dependency scalar products",
        })?;
    let semantic_shape = loops >= 2
        && family.external_count() == 0
        && dependency.arity() == scalar_products
        && family.denominator_count() == scalar_products
        && family.coordinates().len() == scalar_products
        && family
            .coordinates()
            .iter()
            .all(|coordinate| matches!(coordinate, ScalarProductCoordinate::LoopLoop { .. }))
        && parent_context.has_same_variable_map(family.coefficient_context())
        && family.power_shifts().len() == scalar_products
        && family.power_shifts().iter().all(Coefficient::is_zero)
        && dependency.common_mass_homogeneity()
            == Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
        && !dependency.masters().is_empty();
    if !semantic_shape {
        return Err(FactorizedProductMomentError::UnsupportedDependencySemantic { ordinal });
    }
    if !coefficients_equal(parent_context, parent_dimension, family.dimension(), limits)? {
        return Err(FactorizedProductMomentError::DependencyDimensionMismatch { ordinal });
    }
    for coordinate in 0..scalar_products {
        let expansion = family.scalar_product_expansion(coordinate)?;
        for coefficient in
            std::iter::once(expansion.constant()).chain(expansion.denominator_coefficients())
        {
            family
                .coefficient_context()
                .validate_with_limits(coefficient, limits.exact_algebra)?;
            if super::resources::constant_rational_magnitude_bits(coefficient).is_none() {
                return Err(
                    FactorizedProductMomentError::UnsupportedDependencySemantic { ordinal },
                );
            }
        }
    }
    Ok(())
}

fn compile_correlated_moment_branches(
    dependency: &ClosedArtifact,
    vector_signs: &[i64],
    limits: FactorizedProductMomentLimits,
) -> Result<Box<[Box<[CorrelatedMomentBranch]>]>, FactorizedProductMomentError> {
    let family = dependency.family();
    if vector_signs.len() != family.loop_count() {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "the correlated moment sign gauge has a foreign width",
        });
    }
    let context = family.coefficient_context();
    let mut rows = Vec::new();
    rows.try_reserve_exact(family.coordinates().len())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "correlated moment branch rows",
            requested: family.coordinates().len(),
        })?;
    for (coordinate, product) in family.coordinates().iter().enumerate() {
        let ScalarProductCoordinate::LoopLoop { left, right } = *product else {
            return Err(FactorizedProductMomentError::UnsupportedCoordinate { coordinate });
        };
        let sign = vector_signs[left].checked_mul(vector_signs[right]).ok_or(
            FactorizedProductMomentError::Invariant {
                detail: "a correlated moment sign overflowed i64",
            },
        )?;
        if sign != 1 && sign != -1 {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a correlated moment sign is not unit",
            });
        }
        let expansion = family.scalar_product_expansion(coordinate)?;
        let width = usize::from(!expansion.constant().is_zero())
            .checked_add(
                expansion
                    .denominator_coefficients()
                    .iter()
                    .filter(|coefficient| !coefficient.is_zero())
                    .count(),
            )
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "correlated moment branch width",
            })?;
        let mut branches = Vec::new();
        branches.try_reserve_exact(width).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "correlated moment branches",
                requested: width,
            }
        })?;
        let signed =
            |coefficient: &Coefficient| -> Result<Coefficient, FactorizedProductMomentError> {
                if sign == 1 {
                    Ok(coefficient.clone())
                } else {
                    Ok(context.try_neg(coefficient, limits.exact_algebra)?)
                }
            };
        if !expansion.constant().is_zero() {
            branches.push(CorrelatedMomentBranch {
                coefficient: signed(expansion.constant())?,
                denominator: None,
            });
        }
        for (denominator, coefficient) in expansion.denominator_coefficients().iter().enumerate() {
            if !coefficient.is_zero() {
                branches.push(CorrelatedMomentBranch {
                    coefficient: signed(coefficient)?,
                    denominator: Some(denominator),
                });
            }
        }
        if branches.is_empty() {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a correlated scalar product compiled to zero",
            });
        }
        rows.push(branches.into_boxed_slice());
    }
    Ok(rows.into_boxed_slice())
}

fn prove_signed_block_equivalence(
    original: &[i64],
    signed: &[i64],
    dimension: usize,
) -> Result<Box<[i64]>, FactorizedProductMomentError> {
    let expected = dimension.checked_mul(dimension).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product signed-basis entries",
        },
    )?;
    if original.len() != expected || signed.len() != expected {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "the compiled and installed loop bases have incompatible dimensions",
        });
    }
    let mut signs = Vec::new();
    signs.try_reserve_exact(dimension).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product signed-basis row signs",
            requested: dimension,
        }
    })?;
    for row in 0..dimension {
        let range = row * dimension..(row + 1) * dimension;
        let original_row = &original[range.clone()];
        let signed_row = &signed[range];
        let same = original_row == signed_row;
        let opposite = original_row
            .iter()
            .zip(signed_row)
            .all(|(&left, &right)| left.checked_neg().is_some_and(|negated| negated == right));
        if !same && !opposite {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a compiled singleton block is not the installed row up to sign",
            });
        }
        signs.push(if same { 1 } else { -1 });
    }
    Ok(signs.into_boxed_slice())
}

fn complete_cross_edges(
    loop_count: usize,
) -> Result<Box<[(usize, usize)]>, FactorizedProductMomentError> {
    let count = loop_count
        .checked_mul(loop_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product cross coordinates",
        })?;
    let mut edges = Vec::new();
    edges.try_reserve_exact(count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product cross coordinates",
            requested: count,
        }
    })?;
    for left in 0..loop_count {
        for right in left + 1..loop_count {
            edges.push((left, right));
        }
    }
    Ok(edges.into_boxed_slice())
}

struct CoordinatePositions {
    radial: Box<[usize]>,
    cross: Box<[usize]>,
}

fn compile_numerator_branches(
    family: &IntegralFamily,
    forms: &[super::super::factorized_numerator_lift::RoutedAffineDenominator],
    positions: &CoordinatePositions,
    edges: &[(usize, usize)],
) -> Result<Box<[Box<[ProductNumeratorBranch]>]>, FactorizedProductMomentError> {
    if forms.len() != family.denominator_count()
        || positions.radial.len() != family.loop_count()
        || positions.cross.len() != edges.len()
    {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "the routed numerator table has a foreign shape",
        });
    }
    let mut coordinate_variables = Vec::new();
    coordinate_variables
        .try_reserve_exact(family.coordinates().len())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "factorized product coordinate variables",
            requested: family.coordinates().len(),
        })?;
    coordinate_variables.resize(family.coordinates().len(), None);
    for (vector, &coordinate) in positions.radial.iter().enumerate() {
        let slot = coordinate_variables.get_mut(coordinate).ok_or(
            FactorizedProductMomentError::Invariant {
                detail: "a radial coordinate position lies outside the family basis",
            },
        )?;
        *slot = Some(ProductMomentVariable::Radial(vector));
    }
    for (edge, &coordinate) in positions.cross.iter().enumerate() {
        let slot = coordinate_variables.get_mut(coordinate).ok_or(
            FactorizedProductMomentError::Invariant {
                detail: "a cross coordinate position lies outside the family basis",
            },
        )?;
        *slot = Some(ProductMomentVariable::Cross(edge));
    }
    if coordinate_variables.iter().any(Option::is_none) {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "the factorized product coordinate map is incomplete",
        });
    }

    let mut rows = Vec::new();
    rows.try_reserve_exact(forms.len()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "factorized product numerator branch rows",
            requested: forms.len(),
        }
    })?;
    for form in forms {
        if form.scalar_coefficients().len() != coordinate_variables.len() {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a routed numerator row has a foreign coordinate width",
            });
        }
        let width = usize::from(!form.constant().is_zero())
            .checked_add(
                form.scalar_coefficients()
                    .iter()
                    .filter(|coefficient| !coefficient.is_zero())
                    .count(),
            )
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "factorized product numerator branch width",
            })?;
        let mut branches = Vec::new();
        branches.try_reserve_exact(width).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "factorized product numerator branches",
                requested: width,
            }
        })?;
        if !form.constant().is_zero() {
            branches.push(ProductNumeratorBranch {
                coefficient: form.constant().clone(),
                variable: ProductMomentVariable::Constant,
            });
        }
        for (coordinate, coefficient) in form.scalar_coefficients().iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            branches.push(ProductNumeratorBranch {
                coefficient: coefficient.clone(),
                variable: coordinate_variables[coordinate].ok_or(
                    FactorizedProductMomentError::Invariant {
                        detail: "a routed numerator branch has no moment variable",
                    },
                )?,
            });
        }
        if branches.is_empty() {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a routed parent denominator is the zero polynomial",
            });
        }
        rows.push(branches.into_boxed_slice());
    }
    Ok(rows.into_boxed_slice())
}

/// Compile the exact preimage of every nested dependency's certified root.
///
/// The admitted product shapes carry a deliberately small cold proof: every
/// maximum downward shift is attained by a direct radial/correlated branch,
/// while every angularly generated moment can be charged injectively to one
/// such direct-capable source occurrence.  This makes each maximum a sparse
/// 0/1 support form in the inactive parent ranks; retaining those coupled
/// forms is essential because their intersection is not rectangular.
fn compile_product_application_domain(
    rule: &FactorizationRule,
    dependencies: &[Box<ClosedArtifact>],
    layout: &ProductBlockLayout,
    edges: &[(usize, usize)],
    numerator_branches: &[Box<[ProductNumeratorBranch]>],
) -> Result<ProductApplicationDomain, FactorizedProductMomentError> {
    let sector = rule.application_domain().sector();
    if numerator_branches.len() != sector.arity() {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "product numerator rows do not match the factorized sector",
        });
    }
    for (source, (&is_active, branches)) in sector
        .active_bits()
        .iter()
        .zip(numerator_branches)
        .enumerate()
    {
        if !is_active
            && !branches
                .iter()
                .any(|branch| branch.variable == ProductMomentVariable::Constant)
        {
            return Err(FactorizedProductMomentError::UnsupportedShiftCertificate {
                detail: "an inactive routed denominator has no constant branch proving zero minimum shift",
            });
        }
        if branches.is_empty() {
            return Err(FactorizedProductMomentError::Invariant {
                detail: "a product numerator source has no affine branch",
            });
        }
        let _ = source;
    }

    let mut hull_bounds = Vec::new();
    hull_bounds.try_reserve_exact(sector.arity()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product application hull bounds",
            requested: sector.arity(),
        }
    })?;
    hull_bounds.extend(sector.active_bits().iter().map(|&active| {
        if active {
            InteriorBounds::new(1, i64::MAX)
        } else {
            InteriorBounds::new(i64::MIN, 0)
        }
    }));
    let row_capacity = match layout {
        ProductBlockLayout::AllSingleton {
            singletons_by_vector,
        } => singletons_by_vector.len(),
        ProductBlockLayout::OneCorrelated {
            correlated,
            singletons_by_vector,
        } => correlated
            .parent_positions
            .len()
            .checked_add(singletons_by_vector.len())
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product dependency-root preimages",
            })?,
    };
    let mut preimages = Vec::new();
    preimages.try_reserve_exact(row_capacity).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "product dependency-root preimages",
            requested: row_capacity,
        }
    })?;

    match layout {
        ProductBlockLayout::AllSingleton {
            singletons_by_vector,
        } => {
            for singleton in singletons_by_vector {
                append_singleton_preimage(
                    singleton,
                    dependencies,
                    sector.active_bits(),
                    edges,
                    numerator_branches,
                    &mut hull_bounds,
                    &mut preimages,
                )?;
            }
        }
        ProductBlockLayout::OneCorrelated {
            correlated,
            singletons_by_vector,
        } => {
            if singletons_by_vector.len() != 1 {
                return Err(FactorizedProductMomentError::UnsupportedShiftCertificate {
                    detail: "the exact correlated angular-shift certificate currently requires one singleton block",
                });
            }
            append_correlated_preimages(
                correlated,
                &singletons_by_vector[0],
                dependencies,
                sector.active_bits(),
                edges,
                numerator_branches,
                &mut hull_bounds,
                &mut preimages,
            )?;
            append_singleton_preimage(
                &singletons_by_vector[0],
                dependencies,
                sector.active_bits(),
                edges,
                numerator_branches,
                &mut hull_bounds,
                &mut preimages,
            )?;
        }
    }
    if preimages.len() != row_capacity {
        return Err(FactorizedProductMomentError::Invariant {
            detail: "the product dependency-root certificate lost a denominator row",
        });
    }
    let hull = SectorInteriorDomain::try_new(sector.clone(), hull_bounds)?;
    Ok(ProductApplicationDomain::new(
        hull,
        preimages.into_boxed_slice(),
    ))
}

fn append_singleton_preimage(
    singleton: &SingletonProductBlock,
    dependencies: &[Box<ClosedArtifact>],
    active: &[bool],
    edges: &[(usize, usize)],
    numerator_branches: &[Box<[ProductNumeratorBranch]>],
    hull_bounds: &mut [InteriorBounds],
    output: &mut Vec<DependencyRootPreimage>,
) -> Result<(), FactorizedProductMomentError> {
    let dependency = dependencies.get(singleton.dependency_ordinal).ok_or(
        FactorizedProductMomentError::MissingDependency {
            ordinal: singleton.dependency_ordinal,
        },
    )?;
    let dependency_bounds = *dependency.supported_root_power_bounds().first().ok_or(
        FactorizedProductMomentError::UnsupportedShiftCertificate {
            detail: "a singleton dependency has no certified root coordinate",
        },
    )?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(active.iter().filter(|&&entry| !entry).count())
        .map_err(|_| FactorizedProductMomentError::AllocationFailure {
            resource: "singleton dependency shift sources",
            requested: active.iter().filter(|&&entry| !entry).count(),
        })?;
    for (source, (&is_active, branches)) in active.iter().zip(numerator_branches).enumerate() {
        if is_active {
            continue;
        }
        let direct = branches.iter().any(|branch| {
            branch.variable == ProductMomentVariable::Radial(singleton.transformed_vector)
        });
        let angular_incidence = branches.iter().any(|branch| match branch.variable {
            ProductMomentVariable::Cross(edge) => edges.get(edge).is_some_and(|&(left, right)| {
                left == singleton.transformed_vector || right == singleton.transformed_vector
            }),
            _ => false,
        });
        if angular_incidence && !direct {
            return Err(FactorizedProductMomentError::UnsupportedShiftCertificate {
                detail: "an angular singleton incidence has no direct radial branch to bound it",
            });
        }
        if direct {
            sources.push(source);
        }
    }
    tighten_parent_upper(
        hull_bounds,
        singleton.parent_position,
        dependency_bounds.upper(),
    )?;
    output.push(DependencyRootPreimage::new(
        singleton.parent_position,
        dependency_bounds,
        sources.into_boxed_slice(),
    ));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_correlated_preimages(
    correlated: &CorrelatedProductBlock,
    singleton: &SingletonProductBlock,
    dependencies: &[Box<ClosedArtifact>],
    active: &[bool],
    edges: &[(usize, usize)],
    numerator_branches: &[Box<[ProductNumeratorBranch]>],
    hull_bounds: &mut [InteriorBounds],
    output: &mut Vec<DependencyRootPreimage>,
) -> Result<(), FactorizedProductMomentError> {
    let dependency = dependencies.get(correlated.dependency_ordinal).ok_or(
        FactorizedProductMomentError::MissingDependency {
            ordinal: correlated.dependency_ordinal,
        },
    )?;
    let arity = dependency.arity();
    if correlated.parent_positions.len() != arity
        || dependency.supported_root_power_bounds().len() != arity
    {
        return Err(FactorizedProductMomentError::UnsupportedShiftCertificate {
            detail: "a correlated dependency root does not match its installed parent positions",
        });
    }
    let inactive_count = active.iter().filter(|&&entry| !entry).count();
    let mut direct = Vec::new();
    direct.try_reserve_exact(arity).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "correlated direct shift supports",
            requested: arity,
        }
    })?;
    for _ in 0..arity {
        direct.push(fallible_filled(
            active.len(),
            false,
            "correlated direct shift support row",
        )?);
    }
    let mut half_edges = Vec::new();
    half_edges.try_reserve_exact(inactive_count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource: "correlated angular half-edges",
            requested: inactive_count,
        }
    })?;
    for (source, (&is_active, branches)) in active.iter().zip(numerator_branches).enumerate() {
        if is_active {
            continue;
        }
        for branch in branches {
            if let Some(coordinate) = correlated_coordinate_for_variable(
                branch.variable,
                correlated,
                dependency.family(),
                edges,
            )? {
                for denominator in 0..arity {
                    if correlated.moment_branches[coordinate]
                        .iter()
                        .any(|branch| branch.denominator == Some(denominator))
                    {
                        direct[denominator][source] = true;
                    }
                }
            }
            if let ProductMomentVariable::Cross(edge) = branch.variable {
                let &(left, right) =
                    edges
                        .get(edge)
                        .ok_or(FactorizedProductMomentError::Invariant {
                            detail: "a numerator branch references an absent product edge",
                        })?;
                let correlated_vector = if left == singleton.transformed_vector {
                    Some(right)
                } else if right == singleton.transformed_vector {
                    Some(left)
                } else {
                    None
                };
                if let Some(vector) = correlated_vector {
                    let local = correlated
                        .transformed_vectors
                        .iter()
                        .position(|&candidate| candidate == vector)
                        .ok_or(FactorizedProductMomentError::UnsupportedShiftCertificate {
                            detail: "an angular half-edge leaves the correlated-plus-singleton block cover",
                        })?;
                    half_edges.push((source, local));
                }
            }
        }
    }

    // Same-endpoint contractions create a correlated radial moment.  Its
    // shift must be chargeable to the source occurrence that supplied the
    // half-edge.
    for &(source, local) in &half_edges {
        let coordinate = dependency
            .family()
            .coordinates()
            .iter()
            .position(|coordinate| {
                *coordinate
                    == ScalarProductCoordinate::LoopLoop {
                        left: local,
                        right: local,
                    }
            })
            .ok_or(FactorizedProductMomentError::UnsupportedShiftCertificate {
                detail: "a correlated radial coordinate is absent",
            })?;
        for denominator in 0..arity {
            let reaches = correlated.moment_branches[coordinate]
                .iter()
                .any(|branch| branch.denominator == Some(denominator));
            if reaches && !direct[denominator][source] {
                return Err(FactorizedProductMomentError::UnsupportedShiftCertificate {
                    detail: "an angular radial shift cannot be charged to a direct-capable source",
                });
            }
        }
    }
    // Mixed-endpoint contractions create one correlated cross moment while
    // consuming one occurrence at each endpoint. At least one endpoint must
    // be direct-capable, giving an injective charge for every such shift.
    for &(left_source, left_local) in &half_edges {
        for &(right_source, right_local) in &half_edges {
            if left_local == right_local {
                continue;
            }
            let (left, right) = if left_local < right_local {
                (left_local, right_local)
            } else {
                (right_local, left_local)
            };
            let coordinate = dependency
                .family()
                .coordinates()
                .iter()
                .position(|coordinate| {
                    *coordinate == ScalarProductCoordinate::LoopLoop { left, right }
                })
                .ok_or(FactorizedProductMomentError::UnsupportedShiftCertificate {
                    detail: "a correlated cross coordinate is absent",
                })?;
            for denominator in 0..arity {
                let reaches = correlated.moment_branches[coordinate]
                    .iter()
                    .any(|branch| branch.denominator == Some(denominator));
                if reaches
                    && !direct[denominator][left_source]
                    && !direct[denominator][right_source]
                {
                    return Err(FactorizedProductMomentError::UnsupportedShiftCertificate {
                        detail: "an angular cross shift cannot be charged to a direct-capable endpoint",
                    });
                }
            }
        }
    }

    for denominator in 0..arity {
        let mut sources = Vec::new();
        sources.try_reserve_exact(inactive_count).map_err(|_| {
            FactorizedProductMomentError::AllocationFailure {
                resource: "correlated dependency shift sources",
                requested: inactive_count,
            }
        })?;
        sources.extend(
            direct[denominator]
                .iter()
                .enumerate()
                .filter_map(|(source, &supported)| supported.then_some(source)),
        );
        let parent = correlated.parent_positions[denominator];
        let bounds = dependency.supported_root_power_bounds()[denominator];
        tighten_parent_upper(hull_bounds, parent, bounds.upper())?;
        output.push(DependencyRootPreimage::new(
            parent,
            bounds,
            sources.into_boxed_slice(),
        ));
    }
    Ok(())
}

fn correlated_coordinate_for_variable(
    variable: ProductMomentVariable,
    correlated: &CorrelatedProductBlock,
    dependency: &IntegralFamily,
    edges: &[(usize, usize)],
) -> Result<Option<usize>, FactorizedProductMomentError> {
    let local_pair = match variable {
        ProductMomentVariable::Constant => return Ok(None),
        ProductMomentVariable::Radial(vector) => correlated
            .transformed_vectors
            .iter()
            .position(|&candidate| candidate == vector)
            .map(|local| (local, local)),
        ProductMomentVariable::Cross(edge) => {
            let &(left, right) =
                edges
                    .get(edge)
                    .ok_or(FactorizedProductMomentError::Invariant {
                        detail: "a numerator branch references an absent product edge",
                    })?;
            let left = correlated
                .transformed_vectors
                .iter()
                .position(|&candidate| candidate == left);
            let right = correlated
                .transformed_vectors
                .iter()
                .position(|&candidate| candidate == right);
            left.zip(right)
                .map(|(left, right)| (left.min(right), left.max(right)))
        }
    };
    Ok(local_pair.and_then(|(left, right)| {
        dependency
            .coordinates()
            .iter()
            .position(|coordinate| *coordinate == ScalarProductCoordinate::LoopLoop { left, right })
    }))
}

fn tighten_parent_upper(
    hull_bounds: &mut [InteriorBounds],
    parent: usize,
    dependency_upper: i64,
) -> Result<(), FactorizedProductMomentError> {
    let bounds = hull_bounds
        .get_mut(parent)
        .ok_or(FactorizedProductMomentError::Invariant {
            detail: "a dependency base parent position is out of range",
        })?;
    let upper = bounds.upper().min(dependency_upper);
    if upper < bounds.lower() {
        return Err(FactorizedProductMomentError::UnsupportedShiftCertificate {
            detail: "a dependency certified root has no positive-power factorized preimage",
        });
    }
    *bounds = InteriorBounds::new(bounds.lower(), upper);
    Ok(())
}

fn coordinate_positions(
    family: &IntegralFamily,
    edges: &[(usize, usize)],
) -> Result<CoordinatePositions, FactorizedProductMomentError> {
    let loop_count = family.loop_count();
    let mut radial = fallible_filled(
        loop_count,
        usize::MAX,
        "product radial coordinate positions",
    )?;
    let mut cross = fallible_filled(
        edges.len(),
        usize::MAX,
        "product cross coordinate positions",
    )?;
    for (coordinate, product) in family.coordinates().iter().enumerate() {
        let ScalarProductCoordinate::LoopLoop { left, right } = *product else {
            return Err(FactorizedProductMomentError::UnsupportedCoordinate { coordinate });
        };
        if left >= loop_count || right >= loop_count || left > right {
            return Err(FactorizedProductMomentError::UnsupportedCoordinate { coordinate });
        }
        let destination = if left == right {
            &mut radial[left]
        } else {
            let edge = edges
                .iter()
                .position(|&candidate| candidate == (left, right))
                .ok_or(FactorizedProductMomentError::UnsupportedCoordinate { coordinate })?;
            &mut cross[edge]
        };
        if *destination != usize::MAX {
            return Err(
                FactorizedProductMomentError::DuplicateScalarProductCoordinate { left, right },
            );
        }
        *destination = coordinate;
    }
    for (vector, &position) in radial.iter().enumerate() {
        if position == usize::MAX {
            return Err(
                FactorizedProductMomentError::MissingScalarProductCoordinate {
                    left: vector,
                    right: vector,
                },
            );
        }
    }
    for (edge, &position) in cross.iter().enumerate() {
        if position == usize::MAX {
            return Err(
                FactorizedProductMomentError::MissingScalarProductCoordinate {
                    left: edges[edge].0,
                    right: edges[edge].1,
                },
            );
        }
    }
    Ok(CoordinatePositions {
        radial: radial.into_boxed_slice(),
        cross: cross.into_boxed_slice(),
    })
}

fn validate_block_denominators(
    dependencies: &[Box<ClosedArtifact>],
    context: &CoefficientContext,
    forms: &[super::super::factorized_numerator_lift::RoutedAffineDenominator],
    positions: &CoordinatePositions,
    edges: &[(usize, usize)],
    layout: &ProductBlockLayout,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let validate_singletons = |singletons: &[SingletonProductBlock]| {
        for block in singletons {
            validate_dependency_denominators(
                context,
                forms,
                positions,
                edges,
                std::slice::from_ref(&block.parent_position),
                std::slice::from_ref(&block.transformed_vector),
                &[1],
                &dependencies[block.dependency_ordinal],
                limits,
            )?;
        }
        Ok::<(), FactorizedProductMomentError>(())
    };
    match layout {
        ProductBlockLayout::AllSingleton {
            singletons_by_vector,
        } => validate_singletons(singletons_by_vector),
        ProductBlockLayout::OneCorrelated {
            correlated,
            singletons_by_vector,
        } => {
            validate_dependency_denominators(
                context,
                forms,
                positions,
                edges,
                &correlated.parent_positions,
                &correlated.transformed_vectors,
                &correlated.vector_signs,
                &dependencies[correlated.dependency_ordinal],
                limits,
            )?;
            validate_singletons(singletons_by_vector)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_dependency_denominators(
    context: &CoefficientContext,
    forms: &[super::super::factorized_numerator_lift::RoutedAffineDenominator],
    positions: &CoordinatePositions,
    edges: &[(usize, usize)],
    parent_positions: &[usize],
    transformed_vectors: &[usize],
    vector_signs: &[i64],
    dependency: &super::super::ClosedArtifact,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let family = dependency.family();
    if parent_positions.len() != family.denominator_count()
        || transformed_vectors.len() != family.loop_count()
        || vector_signs.len() != transformed_vectors.len()
    {
        return Err(
            FactorizedProductMomentError::UnsupportedDependencySemantic {
                ordinal: usize::MAX,
            },
        );
    }
    for (local_denominator, &parent) in parent_positions.iter().enumerate() {
        let form = forms
            .get(parent)
            .ok_or(FactorizedProductMomentError::Invariant {
                detail: "a factor parent is absent from the routed denominator table",
            })?;
        let expected = &family.denominators()[local_denominator];
        if !coefficients_equal(context, form.constant(), expected.constant(), limits)? {
            return Err(FactorizedProductMomentError::NonUnitRadialDenominator {
                vector: transformed_vectors[0],
                parent_position: parent,
            });
        }
        for global_coordinate in 0..form.scalar_coefficients().len() {
            let mut expected_coefficient = None;
            for (local_coordinate, coordinate) in family.coordinates().iter().enumerate() {
                let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
                    return Err(
                        FactorizedProductMomentError::UnsupportedDependencySemantic {
                            ordinal: usize::MAX,
                        },
                    );
                };
                let global_position = coordinate_position(
                    transformed_vectors[left],
                    transformed_vectors[right],
                    positions,
                    edges,
                )?;
                if global_position == global_coordinate {
                    expected_coefficient = Some((
                        &expected.coefficients()[local_coordinate],
                        vector_signs[left].checked_mul(vector_signs[right]).ok_or(
                            FactorizedProductMomentError::Invariant {
                                detail: "a signed block coefficient overflowed i64",
                            },
                        )?,
                    ));
                    break;
                }
            }
            let matches = if let Some((expected_coefficient, sign)) = expected_coefficient {
                let signed_expected = if sign == 1 {
                    expected_coefficient.clone()
                } else if sign == -1 {
                    context.try_neg(expected_coefficient, limits.exact_algebra)?
                } else {
                    return Err(FactorizedProductMomentError::Invariant {
                        detail: "a routed block row sign is not unit",
                    });
                };
                coefficients_equal(
                    context,
                    &form.scalar_coefficients()[global_coordinate],
                    &signed_expected,
                    limits,
                )?
            } else {
                form.scalar_coefficients()[global_coordinate].is_zero()
            };
            if !matches {
                return Err(FactorizedProductMomentError::NonUnitRadialDenominator {
                    vector: transformed_vectors[0],
                    parent_position: parent,
                });
            }
        }
    }
    Ok(())
}

fn coordinate_position(
    left: usize,
    right: usize,
    positions: &CoordinatePositions,
    edges: &[(usize, usize)],
) -> Result<usize, FactorizedProductMomentError> {
    if left == right {
        return positions.radial.get(left).copied().ok_or(
            FactorizedProductMomentError::Invariant {
                detail: "a block radial coordinate lies outside the parent basis",
            },
        );
    }
    let pair = (left.min(right), left.max(right));
    edges
        .iter()
        .position(|&edge| edge == pair)
        .and_then(|edge| positions.cross.get(edge).copied())
        .ok_or(FactorizedProductMomentError::Invariant {
            detail: "a block cross coordinate is absent from the parent basis",
        })
}

fn coefficients_equal(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
    limits: FactorizedProductMomentLimits,
) -> Result<bool, FactorizedProductMomentError> {
    if left == right {
        Ok(true)
    } else {
        Ok(context
            .try_sub(left, right, limits.exact_algebra)?
            .is_zero())
    }
}

fn fallible_filled<T: Clone>(
    count: usize,
    value: T,
    resource: &'static str,
) -> Result<Vec<T>, FactorizedProductMomentError> {
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: count,
        }
    })?;
    output.resize(count, value);
    Ok(output)
}

fn clone_usize_box(
    values: &[usize],
    resource: &'static str,
) -> Result<Box<[usize]>, FactorizedProductMomentError> {
    let mut output = Vec::new();
    output.try_reserve_exact(values.len()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: values.len(),
        }
    })?;
    output.extend_from_slice(values);
    Ok(output.into_boxed_slice())
}

fn clone_selected_i64_box(
    values: &[i64],
    positions: &[usize],
    resource: &'static str,
) -> Result<Box<[i64]>, FactorizedProductMomentError> {
    let mut output = Vec::new();
    output.try_reserve_exact(positions.len()).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested: positions.len(),
        }
    })?;
    for &position in positions {
        output.push(
            *values
                .get(position)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "a block references an absent signed-basis row",
                })?,
        );
    }
    Ok(output.into_boxed_slice())
}

pub(super) fn admit_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FactorizedProductMomentError> {
    if requested > limit {
        Err(FactorizedProductMomentError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn admit_correlated_factor_count(count: usize) -> Result<(), FactorizedProductMomentError> {
    if count > 1 {
        Err(FactorizedProductMomentError::UnsupportedCorrelatedFactorCount { count })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::algebra::CoefficientContext;
    use crate::family::{AffineDenominator, IntegralFamily};
    use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;

    use super::{
        FactorizedProductMomentError, FactorizedProductMomentLimits, admit_correlated_factor_count,
        validate_parent_power_shifts, validate_tadpole_dependency,
    };

    #[test]
    fn semantic_admission_rejects_parent_dimension_mismatch() {
        let dependency = derive_one_loop_unit_mass_tadpole().unwrap();
        let context = dependency.family().coefficient_context();
        assert_eq!(
            validate_tadpole_dependency(
                context,
                &context.integer(4),
                &dependency,
                7,
                FactorizedProductMomentLimits::default(),
            ),
            Err(FactorizedProductMomentError::DependencyDimensionMismatch { ordinal: 7 })
        );
    }

    #[test]
    fn semantic_admission_rejects_nonzero_parent_power_shift() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let family = IntegralFamily::new(
            "factorized-product-shift-rejection-fixture",
            vec!["q".to_owned()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.one()],
        )
        .unwrap();
        assert_eq!(
            validate_parent_power_shifts(&family, FactorizedProductMomentLimits::default()),
            Err(FactorizedProductMomentError::UnsupportedParentPowerShift { position: 0 })
        );
    }

    #[test]
    fn semantic_admission_rejects_multiple_correlated_blocks() {
        assert_eq!(admit_correlated_factor_count(1), Ok(()));
        assert_eq!(
            admit_correlated_factor_count(2),
            Err(FactorizedProductMomentError::UnsupportedCorrelatedFactorCount { count: 2 })
        );
    }
}
