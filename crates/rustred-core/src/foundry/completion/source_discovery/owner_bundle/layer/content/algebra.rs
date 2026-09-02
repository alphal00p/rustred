use crate::algebra::{CoefficientPolynomial, IndexedCoefficient, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, GuardPredicateAuthority,
    ImmutableOwnerKind, ImmutableOwnerWitness, ProperSubsectorOwner, StratumRegistryError,
};
use crate::identity::{
    IdentityConditionSource, IndexShift, IntegralShift, ParametricNonZeroCondition,
    ParametricRelation, TranslatedSource, TranslatedSourceProvenance,
};
use crate::sector::{
    ComplexityComponent, Mask, SectorInteriorDomain, SectorMonotoneDomain,
    SectorMonotoneShiftDescentWitness, ShiftComplexityKey, ShiftStrictDescentWitness,
};

use super::encoder::BoundedContentHasher;

pub(super) fn append_mask(
    output: &mut BoundedContentHasher,
    mask: &Mask,
) -> Result<(), StratumRegistryError> {
    output.count(mask.arity())?;
    for &active in mask.active_bits() {
        output.boolean(active)?;
    }
    Ok(())
}

pub(super) fn append_i64_slice(
    output: &mut BoundedContentHasher,
    values: &[i64],
) -> Result<(), StratumRegistryError> {
    output.count(values.len())?;
    for &value in values {
        output.i64(value)?;
    }
    Ok(())
}

pub(super) fn append_u64_slice(
    output: &mut BoundedContentHasher,
    values: &[u64],
) -> Result<(), StratumRegistryError> {
    output.count(values.len())?;
    for &value in values {
        output.u64(value)?;
    }
    Ok(())
}

pub(super) fn append_index_shift(
    output: &mut BoundedContentHasher,
    shift: &IndexShift,
) -> Result<(), StratumRegistryError> {
    append_i64_slice(output, shift.values())
}

pub(super) fn append_integral_shift(
    output: &mut BoundedContentHasher,
    shift: &IntegralShift,
) -> Result<(), StratumRegistryError> {
    append_i64_slice(output, shift.values())
}

pub(super) fn append_integral_key(
    output: &mut BoundedContentHasher,
    key: &IntegralKey,
) -> Result<(), StratumRegistryError> {
    append_i64_slice(output, key.powers())
}

pub(super) fn append_interior_domain(
    output: &mut BoundedContentHasher,
    domain: &SectorInteriorDomain,
) -> Result<(), StratumRegistryError> {
    append_mask(output, domain.sector())?;
    output.count(domain.bounds().len())?;
    for bounds in domain.bounds() {
        output.i64(bounds.lower())?;
        output.i64(bounds.upper())?;
    }
    Ok(())
}

pub(super) fn append_monotone_domain(
    output: &mut BoundedContentHasher,
    domain: &SectorMonotoneDomain,
) -> Result<(), StratumRegistryError> {
    append_mask(output, domain.sector())?;
    output.count(domain.bounds().len())?;
    for bounds in domain.bounds() {
        output.i64(bounds.lower())?;
        output.i64(bounds.upper())?;
    }
    Ok(())
}

pub(super) fn append_decorated_stratum(
    output: &mut BoundedContentHasher,
    stratum: &DecoratedStratum,
) -> Result<(), StratumRegistryError> {
    output.text(stratum.family_fingerprint())?;
    output.text(stratum.context_fingerprint())?;
    append_monotone_domain(output, stratum.domain())?;
    output.count(stratum.guards().len())?;
    for guard in stratum.guards() {
        append_guard_branch_identity(output, guard)?;
    }
    output.text(stratum.id().as_str())
}

pub(super) fn append_guard_branch_identity(
    output: &mut BoundedContentHasher,
    guard: &GuardBranchIdentity,
) -> Result<(), StratumRegistryError> {
    output.text(guard.predicate())?;
    output.tag(match guard.authority() {
        GuardPredicateAuthority::BoundExternalProof => 0,
        GuardPredicateAuthority::IndexedPolynomial => 1,
    })?;
    output.tag(match guard.branch() {
        GuardBranch::Zero => 0,
        GuardBranch::NonZero => 1,
    })
}

pub(super) fn append_polynomial(
    output: &mut BoundedContentHasher,
    polynomial: &IndexedPolynomial,
) -> Result<(), StratumRegistryError> {
    append_raw_polynomial(output, polynomial.raw())
}

fn append_raw_polynomial(
    output: &mut BoundedContentHasher,
    polynomial: &CoefficientPolynomial,
) -> Result<(), StratumRegistryError> {
    output.usize(polynomial.nvars())?;
    output.usize(polynomial.nterms())?;
    output.count(polynomial.coefficients.len())?;
    for (coefficient, exponents) in polynomial
        .coefficients
        .iter()
        .zip(polynomial.exponents_iter())
    {
        output.text(&coefficient.to_string())?;
        output.count(exponents.len())?;
        for &exponent in exponents {
            output.u16(exponent)?;
        }
    }
    Ok(())
}

pub(super) fn append_coefficient(
    output: &mut BoundedContentHasher,
    coefficient: &IndexedCoefficient,
) -> Result<(), StratumRegistryError> {
    append_raw_polynomial(output, &coefficient.raw().numerator)?;
    append_raw_polynomial(output, &coefficient.raw().denominator)
}

pub(super) fn append_identity_condition_source(
    output: &mut BoundedContentHasher,
    source: &IdentityConditionSource,
) -> Result<(), StratumRegistryError> {
    // `stable_string` is versioned, exhaustive over the enum, and already
    // canonicalizes every row, coefficient location, and coordinate vector.
    output.text(&source.stable_string())
}

pub(super) fn append_nonzero_condition(
    output: &mut BoundedContentHasher,
    condition: &ParametricNonZeroCondition,
) -> Result<(), StratumRegistryError> {
    append_polynomial(output, condition.polynomial())?;
    output.count(condition.sources().len())?;
    for source in condition.sources() {
        append_identity_condition_source(output, source)?;
    }
    Ok(())
}

pub(super) fn append_translated_provenance(
    output: &mut BoundedContentHasher,
    provenance: &TranslatedSourceProvenance,
) -> Result<(), StratumRegistryError> {
    output.usize(provenance.source_ordinal())?;
    output.text(&provenance.source_row().stable_string())?;
    append_integral_shift(output, provenance.offset())
}

pub(super) fn append_parametric_relation(
    output: &mut BoundedContentHasher,
    relation: &ParametricRelation,
) -> Result<(), StratumRegistryError> {
    output.text(&relation.row_id().stable_string())?;
    output.count(relation.terms().len())?;
    for (shift, coefficient) in relation.terms() {
        append_index_shift(output, shift)?;
        append_coefficient(output, coefficient)?;
    }
    output.count(relation.nonzero_conditions().len())?;
    for condition in relation.nonzero_conditions() {
        append_nonzero_condition(output, condition)?;
    }
    Ok(())
}

pub(super) fn append_translated_source(
    output: &mut BoundedContentHasher,
    source: &TranslatedSource,
) -> Result<(), StratumRegistryError> {
    append_translated_provenance(output, source.provenance())?;
    output.text(&source.row_id().stable_string())?;
    output.count(source.terms().len())?;
    for (shift, coefficient) in source.terms() {
        append_index_shift(output, shift)?;
        append_coefficient(output, coefficient)?;
    }
    output.count(source.nonzero_conditions().len())?;
    for condition in source.nonzero_conditions() {
        append_nonzero_condition(output, condition)?;
    }
    Ok(())
}

pub(super) fn append_complexity_component(
    output: &mut BoundedContentHasher,
    component: ComplexityComponent,
) -> Result<(), StratumRegistryError> {
    match component {
        ComplexityComponent::Arity => output.tag(0),
        ComplexityComponent::PropagatorCount => output.tag(1),
        ComplexityComponent::SectorBit { position } => {
            output.tag(2)?;
            output.usize(position)
        }
        ComplexityComponent::CornerDistance => output.tag(3),
        ComplexityComponent::DotPower => output.tag(4),
        ComplexityComponent::NumeratorPower => output.tag(5),
        ComplexityComponent::IndexExcess { position } => {
            output.tag(6)?;
            output.usize(position)
        }
    }
}

fn append_shift_complexity_key(
    output: &mut BoundedContentHasher,
    key: &ShiftComplexityKey,
) -> Result<(), StratumRegistryError> {
    output.text(&key.policy().stable_id())?;
    output.usize(key.arity())?;
    append_mask(output, key.sector())?;
    output.i128(key.corner_distance_offset())?;
    output.i128(key.dot_offset())?;
    output.i128(key.numerator_offset())?;
    output.count(key.index_excess_offsets().len())?;
    for &offset in key.index_excess_offsets() {
        output.i128(offset)?;
    }
    Ok(())
}

pub(super) fn append_shift_descent(
    output: &mut BoundedContentHasher,
    witness: &ShiftStrictDescentWitness,
) -> Result<(), StratumRegistryError> {
    output.text(&witness.policy().stable_id())?;
    append_interior_domain(output, witness.domain())?;
    append_shift_complexity_key(output, witness.source())?;
    append_shift_complexity_key(output, witness.target())?;
    append_complexity_component(output, witness.decisive_component())
}

pub(super) fn append_monotone_descent(
    output: &mut BoundedContentHasher,
    witness: &SectorMonotoneShiftDescentWitness,
) -> Result<(), StratumRegistryError> {
    output.text(&witness.policy().stable_id())?;
    append_monotone_domain(output, witness.domain())?;
    append_shift_complexity_key(output, witness.pivot())?;
    append_shift_complexity_key(output, witness.target())?;
    match witness.same_sector_descent() {
        Some(descent) => {
            output.tag(1)?;
            append_shift_descent(output, descent)?;
        }
        None => output.tag(0)?,
    }
    output.count(witness.thresholds().len())?;
    for threshold in witness.thresholds() {
        output.usize(threshold.position())?;
        output.i64(threshold.pinched_upper())?;
        match threshold.same_sector_lower() {
            Some(lower) => {
                output.tag(1)?;
                output.i64(lower)?;
            }
            None => output.tag(0)?,
        }
    }
    Ok(())
}

fn append_owner_witness(
    output: &mut BoundedContentHasher,
    witness: ImmutableOwnerWitness,
) -> Result<(), StratumRegistryError> {
    output.usize(witness.owner_ordinal())?;
    output.usize(witness.route_ordinal())?;
    output.tag(match witness.kind() {
        ImmutableOwnerKind::ZeroSector => 0,
        ImmutableOwnerKind::Factorization => 1,
        ImmutableOwnerKind::Master => 2,
        ImmutableOwnerKind::SolvedRewriteSector => 3,
    })
}

#[cfg(test)]
mod tests {
    use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, Mask, OrderingPolicy};

    use super::super::encoder::BoundedContentHasher;
    use super::append_shift_complexity_key;

    fn encoded_shift_key(policy: OrderingPolicy) -> Box<[u8]> {
        let key = policy
            .shift_complexity_key(&Mask::try_new([true; 6]).unwrap(), &[1, 0, 0, 0, 0, 0])
            .unwrap();
        let mut output = BoundedContentHasher::exact(4_096, "ordering semantic test");
        append_shift_complexity_key(&mut output, &key).unwrap();
        output.finish_exact()
    }

    #[test]
    fn canonical_owner_encoding_commits_the_full_coordinate_priority_identity() {
        let priority = CoordinatePriority::try_new(
            6,
            &[5, 3, 4, 2, 0, 1],
            CoordinatePriorityLimits::default(),
        )
        .unwrap();
        let custom = OrderingPolicy::try_with_coordinate_priority(&priority).unwrap();
        let first = encoded_shift_key(custom);
        let second = encoded_shift_key(custom);
        let natural = encoded_shift_key(OrderingPolicy::RustRedUnshiftedV1);
        assert_eq!(first, second);
        assert_ne!(first, natural);
        assert!(
            first
                .windows(custom.stable_id().len())
                .any(|window| { window == custom.stable_id().as_str().as_bytes() })
        );
    }
}

pub(super) fn append_proper_subsector_owner(
    output: &mut BoundedContentHasher,
    owner: ProperSubsectorOwner,
) -> Result<(), StratumRegistryError> {
    output.usize(owner.cell_ordinal())?;
    append_owner_witness(output, owner.owner())
}
