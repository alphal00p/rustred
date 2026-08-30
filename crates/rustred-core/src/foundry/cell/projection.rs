use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::identity::{
    IdentityConditionSource, IndexShift, ParametricNonZeroCondition, RelationBuilder,
    TranslatedSourceBatch,
};
use crate::sector::{Mask, SectorInteriorDomain, symmetry::Canonicalizer};

use super::{
    FixedIndexRestriction, ResidualProjectionEvidence, ResidualTermDisposition,
    ResidualTermProjection, RuleCellError, RuleCellLimits, SourceViewBatch, SourceViewConstruction,
    SourceViewProvenance,
};

impl SourceViewBatch {
    /// Project every translated source in one complete admitted batch.
    ///
    /// Unlike the ordinal-selection entry point, this admits every row already
    /// owned by `translated` exactly once and in batch order, without retaining
    /// a temporary ordinal vector. The caller still owns completeness of the
    /// upstream translation plan. The resulting evidence is otherwise
    /// identical to [`Self::try_project_residual`].
    pub fn try_project_complete_residual(
        translated: TranslatedSourceBatch,
        context: &IndexedCoefficientContext,
        domain: SectorInteriorDomain,
        fixed: impl IntoIterator<Item = FixedIndexRestriction>,
        canonicalizer: &Canonicalizer,
        zero_sectors: &[Mask],
        limits: RuleCellLimits,
    ) -> Result<Self, RuleCellError> {
        let source_count = translated.len();
        Self::try_project_residual_selection(
            translated,
            SourceSelection::Complete(source_count),
            context,
            domain,
            fixed,
            canonicalizer,
            zero_sectors,
            limits,
        )
    }

    /// Restrict translated sources to one residual face and replay exact
    /// immutable feedback before parametric elimination.
    ///
    /// Fixed coefficient indices are substituted with Symbolica. A column is
    /// removed only when its coefficient becomes identically zero or its
    /// shifted box lies wholly in one caller-supplied proved-zero sector. All
    /// other columns are routed by the minimum exact group element whose
    /// permutation stabilizes the residual base point for every lattice point
    /// in `domain`. The original relations and one disposition per input term
    /// remain owned by the returned batch.
    pub fn try_project_residual(
        translated: TranslatedSourceBatch,
        ordinals: &[usize],
        context: &IndexedCoefficientContext,
        domain: SectorInteriorDomain,
        fixed: impl IntoIterator<Item = FixedIndexRestriction>,
        canonicalizer: &Canonicalizer,
        zero_sectors: &[Mask],
        limits: RuleCellLimits,
    ) -> Result<Self, RuleCellError> {
        Self::try_project_residual_selection(
            translated,
            SourceSelection::Ordinals(ordinals),
            context,
            domain,
            fixed,
            canonicalizer,
            zero_sectors,
            limits,
        )
    }

    fn try_project_residual_selection(
        translated: TranslatedSourceBatch,
        selection: SourceSelection<'_>,
        context: &IndexedCoefficientContext,
        domain: SectorInteriorDomain,
        fixed: impl IntoIterator<Item = FixedIndexRestriction>,
        canonicalizer: &Canonicalizer,
        zero_sectors: &[Mask],
        limits: RuleCellLimits,
    ) -> Result<Self, RuleCellError> {
        if selection.is_empty() {
            return Err(RuleCellError::EmptySourceSelection);
        }
        check_limit("source views", selection.len(), limits.max_source_views)?;
        let arity = context.index_count();
        if domain.arity() != arity {
            return Err(RuleCellError::ProjectionDomainArity {
                expected: arity,
                actual: domain.arity(),
            });
        }
        if canonicalizer.arity() != arity {
            return Err(RuleCellError::ProjectionDomainArity {
                expected: arity,
                actual: canonicalizer.arity(),
            });
        }
        check_limit(
            "projection zero sectors",
            zero_sectors.len(),
            limits.max_projection_zero_sectors,
        )?;
        for (ordinal, zero) in zero_sectors.iter().enumerate() {
            if zero.arity() != arity {
                return Err(RuleCellError::ProjectionZeroSectorArity {
                    ordinal,
                    expected: arity,
                    actual: zero.arity(),
                });
            }
        }

        let fixed = canonical_fixed_restrictions(fixed, &domain, limits)?;
        let fixed_pairs = fixed
            .iter()
            .map(|item| (item.position(), item.value()))
            .collect::<Vec<_>>();
        let stabilizers = residual_stabilizers(canonicalizer, &domain, limits)?;

        let (family_fingerprint, context_fingerprint, sources) = translated.into_foundry_parts();
        if context_fingerprint.as_str() != context.fingerprint() {
            return Err(RuleCellError::ForeignContext);
        }
        let available = sources.len();
        let selected_term_count = selection.try_fold(0usize, |count, ordinal| {
            let source = sources
                .get(ordinal)
                .ok_or(RuleCellError::SourceOrdinalOutOfRange { ordinal, available })?;
            count
                .checked_add(source.terms().len())
                .ok_or(RuleCellError::ResourceCountOverflow {
                    resource: "projected source terms",
                })
        })?;
        check_limit(
            "projected source terms",
            selected_term_count,
            limits.max_projected_source_terms,
        )?;

        let mut slots = try_vec(available, "projected source slots")?;
        slots.extend(sources.into_iter().map(Some));
        let mut relations = try_vec(selection.len(), "projected source relations")?;
        let mut original_relations = try_vec(selection.len(), "original projected sources")?;
        let mut provenance = try_vec(selection.len(), "projected source provenance")?;
        let mut term_projections = try_vec(selection.len(), "source term projections")?;

        for ordinal in selection.iter() {
            let slot = slots
                .get_mut(ordinal)
                .ok_or(RuleCellError::SourceOrdinalOutOfRange { ordinal, available })?;
            let source = slot
                .take()
                .ok_or(RuleCellError::DuplicateSourceOrdinal { ordinal })?;
            let (original, translated_provenance) = source.into_foundry_parts();
            original.validate_context(context)?;
            let mut builder = RelationBuilder::new(
                family_fingerprint.clone(),
                original.row_id().clone(),
                context,
            );

            for condition in original.nonzero_conditions() {
                let polynomial = context.specialize_fixed_polynomial_sealed(
                    condition.polynomial(),
                    &fixed_pairs,
                    limits.indexed_algebra,
                )?;
                let condition = ParametricNonZeroCondition::from_authenticated_with_limits(
                    polynomial,
                    condition.sources().iter().cloned(),
                    limits.relation.identity_conditions,
                )?;
                builder.add_sealed_nonzero_condition(context, condition, limits.relation)?;
            }

            let mut witnesses = try_vec(original.terms().len(), "projected term witnesses")?;
            for (shift, coefficient) in original.terms() {
                let source_shift = try_boxed_i64(shift.values(), "projected source shift")?;
                let (coefficient, denominator_guard) = context.specialize_fixed_indices_sealed(
                    coefficient,
                    &fixed_pairs,
                    limits.indexed_algebra,
                )?;
                if coefficient.is_zero() {
                    witnesses.push(ResidualTermProjection {
                        source_shift,
                        disposition: ResidualTermDisposition::CoefficientZero,
                    });
                    continue;
                }
                if let Some(zero_sector) = zero_sectors
                    .iter()
                    .find(|zero| shifted_box_has_sector(&domain, shift.values(), zero))
                {
                    witnesses.push(ResidualTermProjection {
                        source_shift,
                        disposition: ResidualTermDisposition::ProvedZero {
                            zero_sector: zero_sector.clone(),
                        },
                    });
                    continue;
                }

                let (group_element, projected_shift) =
                    select_projected_shift(canonicalizer, &stabilizers, &domain, shift.values())?;
                let projected_shift_box =
                    try_boxed_i64(&projected_shift, "projected integral shift")?;
                let projected = IndexShift::try_new(projected_shift, arity)?;

                if !denominator_guard.is_nonzero_constant() {
                    let guard = ParametricNonZeroCondition::from_authenticated_with_limits(
                        denominator_guard,
                        [IdentityConditionSource::RelationInputTermDenominator {
                            row: original.row_id().clone(),
                            shift: try_boxed_i64(
                                shift.values(),
                                "projected denominator-guard shift",
                            )?,
                        }],
                        limits.relation.identity_conditions,
                    )?;
                    builder.add_sealed_nonzero_condition(context, guard, limits.relation)?;
                }
                builder.add_sealed_term(context, projected, coefficient, limits.relation)?;
                witnesses.push(ResidualTermProjection {
                    source_shift,
                    disposition: ResidualTermDisposition::Routed {
                        group_element,
                        projected_shift: projected_shift_box,
                    },
                });
            }
            relations.push(builder.finish());
            original_relations.push(original);
            provenance.push(SourceViewProvenance {
                translated: translated_provenance,
                symmetry: None,
            });
            term_projections.push(witnesses.into_boxed_slice());
        }

        Ok(Self {
            family_fingerprint,
            context_fingerprint,
            relations,
            provenance,
            construction: SourceViewConstruction::ResidualProjection(ResidualProjectionEvidence {
                domain,
                fixed: fixed.into_boxed_slice(),
                original_relations,
                terms: term_projections,
                stabilizer_group_elements: stabilizers.into_boxed_slice(),
            }),
        })
    }

    /// Independently replay a retained residual projection at the artifact
    /// installation boundary. Runtime reducers never call this method.
    pub(crate) fn verify_residual_projection(
        &self,
        context: &IndexedCoefficientContext,
        canonicalizer: &Canonicalizer,
        proved_zero_sectors: &[Mask],
        limits: RuleCellLimits,
    ) -> Result<bool, RuleCellError> {
        let SourceViewConstruction::ResidualProjection(evidence) = &self.construction else {
            return Ok(true);
        };
        if self.context_fingerprint.as_str() != context.fingerprint()
            || evidence.original_relations.len() != self.relations.len()
            || evidence.terms.len() != self.relations.len()
        {
            return Ok(false);
        }
        let fixed =
            canonical_fixed_restrictions(evidence.fixed.iter().copied(), &evidence.domain, limits)?;
        if fixed.as_slice() != evidence.fixed.as_ref() {
            return Ok(false);
        }
        let stabilizers = residual_stabilizers(canonicalizer, &evidence.domain, limits)?;
        if stabilizers.as_slice() != evidence.stabilizer_group_elements.as_ref() {
            return Ok(false);
        }
        let fixed_pairs = fixed
            .iter()
            .map(|item| (item.position(), item.value()))
            .collect::<Vec<_>>();

        for ((original, witnesses), projected) in evidence
            .original_relations
            .iter()
            .zip(&evidence.terms)
            .zip(&self.relations)
        {
            original.validate_context(context)?;
            if original.family_fingerprint_owner().as_str() != self.family_fingerprint.as_str()
                || witnesses.len() != original.terms().len()
            {
                return Ok(false);
            }
            let mut builder = RelationBuilder::new(
                self.family_fingerprint.clone(),
                original.row_id().clone(),
                context,
            );
            for condition in original.nonzero_conditions() {
                let polynomial = context.specialize_fixed_polynomial_sealed(
                    condition.polynomial(),
                    &fixed_pairs,
                    limits.indexed_algebra,
                )?;
                let condition = ParametricNonZeroCondition::from_authenticated_with_limits(
                    polynomial,
                    condition.sources().iter().cloned(),
                    limits.relation.identity_conditions,
                )?;
                builder.add_sealed_nonzero_condition(context, condition, limits.relation)?;
            }

            for ((shift, coefficient), witness) in original.terms().iter().zip(witnesses.iter()) {
                if witness.source_shift() != shift.values() {
                    return Ok(false);
                }
                let (coefficient, denominator_guard) = context.specialize_fixed_indices_sealed(
                    coefficient,
                    &fixed_pairs,
                    limits.indexed_algebra,
                )?;
                match witness.disposition() {
                    ResidualTermDisposition::CoefficientZero => {
                        if !coefficient.is_zero() {
                            return Ok(false);
                        }
                        continue;
                    }
                    ResidualTermDisposition::ProvedZero { zero_sector } => {
                        if coefficient.is_zero()
                            || !proved_zero_sectors.contains(zero_sector)
                            || !shifted_box_has_sector(
                                &evidence.domain,
                                shift.values(),
                                zero_sector,
                            )
                        {
                            return Ok(false);
                        }
                        continue;
                    }
                    ResidualTermDisposition::Routed {
                        group_element,
                        projected_shift,
                    } => {
                        if coefficient.is_zero()
                            || proved_zero_sectors.iter().any(|zero| {
                                shifted_box_has_sector(&evidence.domain, shift.values(), zero)
                            })
                        {
                            return Ok(false);
                        }
                        let (expected_group, expected_shift) = select_projected_shift(
                            canonicalizer,
                            &stabilizers,
                            &evidence.domain,
                            shift.values(),
                        )?;
                        if *group_element != expected_group
                            || projected_shift.as_ref() != expected_shift.as_slice()
                        {
                            return Ok(false);
                        }
                        if !denominator_guard.is_nonzero_constant() {
                            let guard = ParametricNonZeroCondition::from_authenticated_with_limits(
                                denominator_guard,
                                [IdentityConditionSource::RelationInputTermDenominator {
                                    row: original.row_id().clone(),
                                    shift: try_boxed_i64(
                                        shift.values(),
                                        "replayed projected denominator-guard shift",
                                    )?,
                                }],
                                limits.relation.identity_conditions,
                            )?;
                            builder.add_sealed_nonzero_condition(
                                context,
                                guard,
                                limits.relation,
                            )?;
                        }
                        builder.add_sealed_term(
                            context,
                            IndexShift::try_new(expected_shift, context.index_count())?,
                            coefficient,
                            limits.relation,
                        )?;
                    }
                }
            }
            if &builder.finish() != projected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Clone, Copy)]
enum SourceSelection<'a> {
    Ordinals(&'a [usize]),
    Complete(usize),
}

impl SourceSelection<'_> {
    fn len(self) -> usize {
        match self {
            Self::Ordinals(ordinals) => ordinals.len(),
            Self::Complete(count) => count,
        }
    }

    fn is_empty(self) -> bool {
        self.len() == 0
    }

    fn iter(self) -> impl Iterator<Item = usize> {
        (0..self.len()).map(move |position| match self {
            Self::Ordinals(ordinals) => ordinals[position],
            Self::Complete(_) => position,
        })
    }

    fn try_fold<T, E>(
        self,
        initial: T,
        mut operation: impl FnMut(T, usize) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut accumulator = initial;
        for ordinal in self.iter() {
            accumulator = operation(accumulator, ordinal)?;
        }
        Ok(accumulator)
    }
}

fn canonical_fixed_restrictions(
    fixed: impl IntoIterator<Item = FixedIndexRestriction>,
    domain: &SectorInteriorDomain,
    limits: RuleCellLimits,
) -> Result<Vec<FixedIndexRestriction>, RuleCellError> {
    let mut canonical: Vec<FixedIndexRestriction> = Vec::new();
    for restriction in fixed {
        let requested =
            canonical
                .len()
                .checked_add(1)
                .ok_or(RuleCellError::ResourceCountOverflow {
                    resource: "projection fixed restrictions",
                })?;
        check_limit(
            "projection fixed restrictions",
            requested,
            limits.max_fixed_restrictions,
        )?;
        canonical
            .try_reserve(1)
            .map_err(|_| RuleCellError::AllocationFailure {
                resource: "projection fixed restrictions",
                requested,
            })?;
        canonical.push(restriction);
    }
    canonical.sort_unstable();
    for window in canonical.windows(2) {
        if window[0].position() == window[1].position() {
            return Err(RuleCellError::DuplicateFixedPosition {
                position: window[0].position(),
            });
        }
    }
    for restriction in &canonical {
        let Some(bounds) = domain.bounds().get(restriction.position()) else {
            return Err(RuleCellError::ProjectionFixedCoordinateNotSingleton {
                position: restriction.position(),
                value: restriction.value(),
            });
        };
        if bounds.lower() != restriction.value() || bounds.upper() != restriction.value() {
            return Err(RuleCellError::ProjectionFixedCoordinateNotSingleton {
                position: restriction.position(),
                value: restriction.value(),
            });
        }
    }
    Ok(canonical)
}

fn residual_stabilizers(
    canonicalizer: &Canonicalizer,
    domain: &SectorInteriorDomain,
    limits: RuleCellLimits,
) -> Result<Vec<usize>, RuleCellError> {
    let mut stabilizers = Vec::new();
    for (group_element, mapping) in canonicalizer.group_elements().enumerate() {
        let preserves_base = mapping.iter().enumerate().all(|(target, &source)| {
            target == source
                || (domain.bounds()[target].lower() == domain.bounds()[target].upper()
                    && domain.bounds()[source].lower() == domain.bounds()[source].upper()
                    && domain.bounds()[target].lower() == domain.bounds()[source].lower())
        });
        if !preserves_base {
            continue;
        }
        let requested =
            stabilizers
                .len()
                .checked_add(1)
                .ok_or(RuleCellError::ResourceCountOverflow {
                    resource: "projection stabilizer routes",
                })?;
        check_limit(
            "projection stabilizer routes",
            requested,
            limits.max_projection_group_routes,
        )?;
        stabilizers
            .try_reserve(1)
            .map_err(|_| RuleCellError::AllocationFailure {
                resource: "projection stabilizer routes",
                requested,
            })?;
        stabilizers.push(group_element);
    }
    if stabilizers.is_empty() {
        Err(RuleCellError::ProjectionHasNoDomainStabilizer)
    } else {
        Ok(stabilizers)
    }
}

fn shifted_box_has_sector(domain: &SectorInteriorDomain, shift: &[i64], sector: &Mask) -> bool {
    domain
        .bounds()
        .iter()
        .zip(shift)
        .zip(sector.active_bits())
        .all(|((&bounds, &component), &active)| {
            let lower = i128::from(bounds.lower()) + i128::from(component);
            let upper = i128::from(bounds.upper()) + i128::from(component);
            if active { lower >= 1 } else { upper <= 0 }
        })
}

fn select_projected_shift(
    canonicalizer: &Canonicalizer,
    stabilizers: &[usize],
    domain: &SectorInteriorDomain,
    source_shift: &[i64],
) -> Result<(usize, Vec<i64>), RuleCellError> {
    // Projection relations live over the formal index field and are never
    // applied directly. Choose a deterministic representable point only to
    // order routes; the subsequently installed rule cell independently
    // proves every retained runtime shift representable on its full domain.
    // Prefer zero whenever the face interval contains it, avoiding spurious
    // overflow at an i64 endpoint for source columns later eliminated by
    // Symbolica.
    let anchor = domain
        .bounds()
        .iter()
        .map(|bounds| {
            if bounds.contains(0) {
                0
            } else {
                bounds.lower()
            }
        })
        .collect::<Vec<_>>();
    let group = canonicalizer.group_elements().collect::<Vec<_>>();
    let mut selected = None;
    for &group_element in stabilizers {
        let mapping = group
            .get(group_element)
            .ok_or(RuleCellError::ProjectionHasNoDomainStabilizer)?;
        let mut shift = try_vec(source_shift.len(), "candidate projected shift")?;
        let mut powers = try_vec(source_shift.len(), "candidate projected anchor")?;
        for (target, &source) in mapping.iter().enumerate() {
            let component = source_shift[source];
            shift.push(component);
            powers.push(
                anchor[target]
                    .checked_add(component)
                    .ok_or(RuleCellError::IndexOverflow { position: target })?,
            );
        }
        let key = IntegralKey::try_from_preallocated(powers)?;
        let complexity = canonicalizer.ordering().complexity_key(key.powers())?;
        let candidate = (complexity, shift, group_element);
        if selected
            .as_ref()
            .is_none_or(|current: &(_, Vec<i64>, usize)| candidate < *current)
        {
            selected = Some(candidate);
        }
    }
    selected
        .map(|(_, shift, group_element)| (group_element, shift))
        .ok_or(RuleCellError::ProjectionHasNoDomainStabilizer)
}

fn try_boxed_i64(values: &[i64], resource: &'static str) -> Result<Box<[i64]>, RuleCellError> {
    let mut output = try_vec(values.len(), resource)?;
    output.extend_from_slice(values);
    Ok(output.into_boxed_slice())
}

fn try_vec<T>(len: usize, resource: &'static str) -> Result<Vec<T>, RuleCellError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| RuleCellError::AllocationFailure {
            resource,
            requested: len,
        })?;
    Ok(output)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), RuleCellError> {
    if requested > limit {
        Err(RuleCellError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
