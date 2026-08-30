use crate::family::{CoefficientLocation, IntegralKey};
use crate::foundry::parametric::{ParametricGuardOrigin, ParametricRule};
use crate::identity::{IdentityConditionSource, RowId};

use super::super::error::ArtifactPersistenceError;
use super::binary::{Reader, Writer, try_vec};
use super::coefficient::{encode_indexed_coefficient, encode_indexed_polynomial};

fn copy_string(value: &str, resource: &'static str) -> Result<String, ArtifactPersistenceError> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|_| {
        ArtifactPersistenceError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    output.push_str(value);
    Ok(output)
}

pub(super) fn decode_owned_string(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<String, ArtifactPersistenceError> {
    copy_string(reader.string(field)?, field)
}

pub(super) fn encode_i64_slice(
    writer: &mut Writer,
    values: &[i64],
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(values.len(), "integer vector length")?;
    for &value in values {
        writer.i64(value)?;
    }
    Ok(())
}

pub(super) fn decode_i64_vec(
    reader: &mut Reader<'_>,
    resource: &'static str,
) -> Result<Vec<i64>, ArtifactPersistenceError> {
    let len = reader.count(resource)?;
    if len > reader.limits().max_index_arity {
        return Err(ArtifactPersistenceError::ResourceLimit {
            resource,
            requested: len,
            limit: reader.limits().max_index_arity,
        });
    }
    let mut values = try_vec(len, resource)?;
    for _ in 0..len {
        values.push(reader.i64()?);
    }
    Ok(values)
}

pub(super) fn encode_bool_slice(
    writer: &mut Writer,
    values: &[bool],
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(values.len(), "boolean vector length")?;
    for &value in values {
        writer.u8(u8::from(value))?;
    }
    Ok(())
}

pub(super) fn decode_bool_vec(
    reader: &mut Reader<'_>,
    resource: &'static str,
) -> Result<Vec<bool>, ArtifactPersistenceError> {
    let len = reader.count(resource)?;
    if len > reader.limits().max_index_arity {
        return Err(ArtifactPersistenceError::ResourceLimit {
            resource,
            requested: len,
            limit: reader.limits().max_index_arity,
        });
    }
    let mut values = try_vec(len, resource)?;
    for _ in 0..len {
        values.push(match reader.u8()? {
            0 => false,
            1 => true,
            _ => {
                return Err(ArtifactPersistenceError::SemanticMismatch {
                    field: "boolean encoding",
                });
            }
        });
    }
    Ok(values)
}

pub(super) fn encode_integral_key(
    writer: &mut Writer,
    key: &IntegralKey,
) -> Result<(), ArtifactPersistenceError> {
    encode_i64_slice(writer, key.powers())
}

pub(super) fn encode_row_id(
    writer: &mut Writer,
    row: &RowId,
) -> Result<(), ArtifactPersistenceError> {
    match row {
        RowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } => {
            writer.u8(0)?;
            writer.usize(*contraction_momentum, "row contraction momentum")?;
            writer.usize(*differentiated_loop, "row differentiated loop")
        }
        RowId::LorentzInvariance {
            first_external,
            second_external,
        } => {
            writer.u8(1)?;
            writer.usize(*first_external, "row first external")?;
            writer.usize(*second_external, "row second external")
        }
        RowId::Derived { label } => {
            writer.u8(2)?;
            writer.string(label, "derived row label")
        }
    }
}

fn encode_coefficient_location(
    writer: &mut Writer,
    location: &CoefficientLocation,
) -> Result<(), ArtifactPersistenceError> {
    match location {
        CoefficientLocation::Dimension => writer.u8(0),
        CoefficientLocation::DenominatorConstant { denominator } => {
            writer.u8(1)?;
            writer.usize(*denominator, "denominator location")
        }
        CoefficientLocation::DenominatorCoefficient {
            denominator,
            coordinate,
        } => {
            writer.u8(2)?;
            writer.usize(*denominator, "denominator location")?;
            writer.usize(*coordinate, "coordinate location")
        }
        CoefficientLocation::ExternalGram { row, column } => {
            writer.u8(3)?;
            writer.usize(*row, "Gram row location")?;
            writer.usize(*column, "Gram column location")
        }
        CoefficientLocation::PowerShift { denominator } => {
            writer.u8(4)?;
            writer.usize(*denominator, "power-shift location")
        }
        CoefficientLocation::BasisDeterminantNumerator => writer.u8(5),
    }
}

pub(super) fn encode_condition_source(
    writer: &mut Writer,
    source: &IdentityConditionSource,
) -> Result<(), ArtifactPersistenceError> {
    match source {
        IdentityConditionSource::FamilyInputCoefficientDenominator { location } => {
            writer.u8(0)?;
            encode_coefficient_location(writer, location)
        }
        IdentityConditionSource::FamilyBasisDeterminantNumerator => writer.u8(1),
        IdentityConditionSource::RelationConditionAttached { row } => {
            writer.u8(2)?;
            encode_row_id(writer, row)
        }
        IdentityConditionSource::RelationInputTermDenominator { row, shift } => {
            writer.u8(3)?;
            encode_row_id(writer, row)?;
            encode_i64_slice(writer, shift)
        }
        IdentityConditionSource::RelationCollectedTermDenominator { row, shift } => {
            writer.u8(4)?;
            encode_row_id(writer, row)?;
            encode_i64_slice(writer, shift)
        }
        IdentityConditionSource::RelationScaleFactorDenominator {
            target_row,
            source_row,
        } => {
            writer.u8(5)?;
            encode_row_id(writer, target_row)?;
            encode_row_id(writer, source_row)
        }
        IdentityConditionSource::RelationTranslation {
            source_row,
            target_row,
            offset,
        } => {
            writer.u8(6)?;
            encode_row_id(writer, source_row)?;
            encode_row_id(writer, target_row)?;
            encode_i64_slice(writer, offset)
        }
        IdentityConditionSource::IndexTranslation { offset } => {
            writer.u8(7)?;
            encode_i64_slice(writer, offset)
        }
    }
}

fn encode_parametric_guard_origin(
    writer: &mut Writer,
    origin: &ParametricGuardOrigin,
) -> Result<(), ArtifactPersistenceError> {
    match origin {
        ParametricGuardOrigin::SourceCondition {
            source_ordinal,
            row_id,
            condition_ordinal,
            condition_sources,
        } => {
            writer.u8(0)?;
            writer.usize(*source_ordinal, "guard source ordinal")?;
            encode_row_id(writer, row_id)?;
            writer.usize(*condition_ordinal, "guard condition ordinal")?;
            writer.usize(condition_sources.len(), "guard condition sources")?;
            for source in condition_sources {
                encode_condition_source(writer, source)?;
            }
            Ok(())
        }
        ParametricGuardOrigin::SourceCoefficientDenominator {
            source_ordinal,
            row_id,
            shift,
        } => {
            writer.u8(1)?;
            writer.usize(*source_ordinal, "guard source ordinal")?;
            encode_row_id(writer, row_id)?;
            encode_i64_slice(writer, shift.values())
        }
        ParametricGuardOrigin::ReducerPivotNumerator {
            source_ordinal,
            row_id,
            pivot_column,
            pivot_shift,
        } => {
            writer.u8(2)?;
            writer.usize(*source_ordinal, "guard source ordinal")?;
            encode_row_id(writer, row_id)?;
            writer.usize(*pivot_column, "guard pivot column")?;
            encode_i64_slice(writer, pivot_shift.values())
        }
        ParametricGuardOrigin::ReducerPivotDenominator {
            source_ordinal,
            row_id,
            pivot_column,
            pivot_shift,
        } => {
            writer.u8(3)?;
            writer.usize(*source_ordinal, "guard source ordinal")?;
            encode_row_id(writer, row_id)?;
            writer.usize(*pivot_column, "guard pivot column")?;
            encode_i64_slice(writer, pivot_shift.values())
        }
        ParametricGuardOrigin::RuleCoefficientDenominator { shift } => {
            writer.u8(4)?;
            encode_i64_slice(writer, shift.values())
        }
        ParametricGuardOrigin::SourceCombinationDenominator {
            source_ordinal,
            row_id,
        } => {
            writer.u8(5)?;
            writer.usize(*source_ordinal, "guard source ordinal")?;
            encode_row_id(writer, row_id)
        }
    }
}

/// Encode every semantic and replay-bearing field of one derived rule. The
/// loader compares these bytes to a freshly authenticated derivation from the
/// retained source rows.
pub(super) fn encode_rule_snapshot(
    rule: &ParametricRule,
    parent: &Writer,
) -> Result<Vec<u8>, ArtifactPersistenceError> {
    let mut writer = parent.child();
    writer.string(rule.family_fingerprint(), "rule family fingerprint")?;
    writer.string(rule.context_fingerprint(), "rule context fingerprint")?;
    encode_bool_slice(&mut writer, rule.sector().active_bits())?;
    writer.usize(rule.domain().bounds().len(), "rule domain bounds")?;
    for bounds in rule.domain().bounds() {
        writer.i64(bounds.lower())?;
        writer.i64(bounds.upper())?;
    }
    writer.string(rule.ordering().stable_id(), "rule ordering identifier")?;
    encode_i64_slice(&mut writer, rule.pivot().values())?;
    writer.usize(rule.right_hand_side().len(), "rule RHS terms")?;
    for term in rule.right_hand_side() {
        encode_i64_slice(&mut writer, term.shift().values())?;
        encode_indexed_coefficient(&mut writer, term.coefficient())?;
        writer.u8(u8::from(term.descent().verify()))?;
    }
    writer.usize(
        rule.elimination_pivot_guards().len(),
        "parametric pivot guards",
    )?;
    for guard in rule.elimination_pivot_guards() {
        writer.usize(guard.source_ordinal(), "parametric pivot source ordinal")?;
        encode_row_id(&mut writer, guard.row_id())?;
        writer.usize(guard.pivot_column(), "parametric pivot column")?;
        encode_i64_slice(&mut writer, guard.pivot_shift().values())?;
        encode_indexed_coefficient(&mut writer, guard.coefficient())?;
        encode_indexed_polynomial(&mut writer, guard.nonzero_polynomial())?;
    }
    writer.usize(rule.nonzero_guards().len(), "parametric nonzero guards")?;
    for guard in rule.nonzero_guards() {
        encode_indexed_polynomial(&mut writer, guard.polynomial())?;
        writer.usize(guard.origins().len(), "parametric guard origins")?;
        for origin in guard.origins() {
            encode_parametric_guard_origin(&mut writer, origin)?;
        }
    }
    writer.usize(
        rule.source_combination().len(),
        "parametric source combination",
    )?;
    for contribution in rule.source_combination() {
        writer.usize(contribution.source_ordinal(), "parametric source ordinal")?;
        encode_row_id(&mut writer, contribution.row_id())?;
        encode_indexed_coefficient(&mut writer, contribution.coefficient())?;
    }
    let replay = rule.replay();
    writer.usize(replay.source_rows_used(), "parametric replay source rows")?;
    writer.usize(
        replay.shift_columns_checked(),
        "parametric replay shift columns",
    )?;
    writer.usize(replay.exact_operations(), "parametric replay operations")?;
    let concrete = rule.concrete_replay();
    encode_integral_key(&mut writer, concrete.anchor())?;
    writer.usize(
        concrete.source_contributions_checked(),
        "concrete replay source contributions",
    )?;
    writer.usize(
        concrete.source_terms_checked(),
        "concrete replay source terms",
    )?;
    writer.usize(
        concrete.right_hand_side_terms_checked(),
        "concrete replay RHS terms",
    )?;
    writer.usize(
        concrete.integral_keys_checked(),
        "concrete replay integral keys",
    )?;
    writer.usize(concrete.nonzero_guards_checked(), "concrete replay guards")?;
    writer.usize(
        concrete.exact_operations(),
        "concrete replay exact operations",
    )?;
    writer.usize(
        concrete.peak_retained_coefficient_terms(),
        "concrete replay peak retained coefficient terms",
    )?;
    match rule.sector_monotone_admission() {
        None => writer.u8(0)?,
        Some(admission) => {
            writer.u8(1)?;
            encode_bool_slice(&mut writer, admission.parent_sector().active_bits())?;
            writer.usize(
                admission.domain().bounds().len(),
                "sector-monotone domain bounds",
            )?;
            for bounds in admission.domain().bounds() {
                writer.i64(bounds.lower())?;
                writer.i64(bounds.upper())?;
            }
            encode_i64_slice(&mut writer, admission.pivot().values())?;
            writer.usize(
                admission.dependencies().len(),
                "sector-monotone dependencies",
            )?;
            for dependency in admission.dependencies() {
                writer.usize(
                    dependency.right_hand_side_ordinal(),
                    "sector-monotone RHS ordinal",
                )?;
                encode_i64_slice(&mut writer, dependency.pivot_shift().values())?;
                encode_i64_slice(&mut writer, dependency.shift().values())?;
                let descent = dependency.descent();
                writer.string(
                    descent.policy().stable_id(),
                    "sector-monotone ordering identifier",
                )?;
                writer.usize(descent.thresholds().len(), "sector-monotone thresholds")?;
                for threshold in descent.thresholds() {
                    writer.usize(threshold.position(), "pinch threshold position")?;
                    writer.i64(threshold.pinched_upper())?;
                    match threshold.same_sector_lower() {
                        None => writer.u8(0)?,
                        Some(lower) => {
                            writer.u8(1)?;
                            writer.i64(lower)?;
                        }
                    }
                }
                writer.u8(u8::from(descent.same_sector_descent().is_some()))?;
                writer.u8(u8::from(descent.verify()))?;
                writer.u8(u8::from(dependency.verify()))?;
            }
            writer.u8(u8::from(admission.verify()))?;
        }
    }
    Ok(writer.finish())
}
