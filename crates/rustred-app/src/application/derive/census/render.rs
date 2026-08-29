use crate::application::MAX_OUTPUT_BYTES;
use crate::application::error::AppError;
use crate::application::model::MetadataValue;

use super::super::model::{ConditionOutputV1, DeriveOutputV1};

/// Reject an output whose serialized representation could cross the wire
/// limit before asking TOML to allocate its result buffer. TOML basic-string
/// escaping expands one UTF-8 input byte by at most six ASCII bytes. The
/// deliberately loose per-node allowance covers keys, punctuation, decimal
/// ordinals, array/table headers, and whitespace independently of payload
/// length. An exact post-render check enforces the advertised wire bound.
pub(in crate::application::derive) fn preflight_output_bound(
    output: &DeriveOutputV1,
) -> Result<(), AppError> {
    const NODE_OVERHEAD: usize = 4_096;
    const TOML_ESCAPE_FACTOR: usize = 6;

    let mut bound = NODE_OVERHEAD;
    let mut add_node = |strings: &[&str], integer_values: usize| -> Result<(), AppError> {
        bound = bound
            .checked_add(NODE_OVERHEAD)
            .and_then(|value| value.checked_add(integer_values.checked_mul(32)?))
            .ok_or_else(output_bound_overflow)?;
        for value in strings {
            let escaped = value
                .len()
                .checked_mul(TOML_ESCAPE_FACTOR)
                .ok_or_else(output_bound_overflow)?;
            bound = bound
                .checked_add(escaped)
                .ok_or_else(output_bound_overflow)?;
        }
        if bound > MAX_OUTPUT_BYTES {
            return Err(output_bound_error(bound));
        }
        Ok(())
    };

    add_node(
        &[
            output.schema,
            output.status,
            output.equation_convention,
            output.relation_selection,
            output.target_disposition,
            output.producer.name,
            output.producer.rustred_version,
            output.producer.symbolica_version,
            output.producer.expression_format,
            output.provenance.requested_input_format,
            output.provenance.detected_input_form,
            &output.provenance.input_schema,
            &output.provenance.parameter_source,
            &output.provenance.canonical_integral,
            &output.family.name,
            &output.family.fingerprint,
            &output.family.parametric_context_fingerprint,
            &output.family.dimension,
            output.target.disposition,
        ],
        8,
    )?;
    for parameter in &output.provenance.input_parameters {
        add_node(&[parameter], 0)?;
    }
    for (key, value) in &output.provenance.metadata {
        match value {
            MetadataValue::String(value) => add_node(&[key, value], 0)?,
            MetadataValue::StringArray(values) => {
                add_node(&[key], 0)?;
                for value in values {
                    add_node(&[value], 0)?;
                }
            }
        }
    }
    for value in output
        .family
        .parameters
        .iter()
        .chain(&output.family.loop_momenta)
        .chain(&output.family.external_momenta)
        .chain(&output.family.index_symbols)
    {
        add_node(&[value], 0)?;
    }
    if let Some(numerator) = &output.target.numerator {
        add_node(&[numerator], 0)?;
    }
    if let Some(powers) = &output.target.powers {
        add_node(&[], powers.len())?;
    }
    for coordinate in &output.coordinates {
        add_node(&[coordinate.kind, &coordinate.left, &coordinate.right], 1)?;
    }
    for denominator in &output.denominators {
        add_node(
            &[
                &denominator.id,
                &denominator.source_expression,
                &denominator.normalized_expression,
                &denominator.power_shift,
                &denominator.constant,
            ],
            1,
        )?;
        for coefficient in &denominator.coefficients {
            add_node(&[&coefficient.value], 1)?;
        }
    }
    for gram in &output.external_gram {
        add_node(&[&gram.left, &gram.right, &gram.value], 2)?;
    }
    for condition in &output.domain_conditions {
        add_condition_bound(condition, &mut add_node)?;
    }
    for relation in &output.relations {
        add_node(
            &[
                &relation.stable_id,
                relation.id.kind,
                relation.id.label.as_deref().unwrap_or(""),
            ],
            6,
        )?;
        for term in &relation.terms {
            add_node(&[&term.coefficient], term.shift.len())?;
        }
        for condition in &relation.nonzero_conditions {
            add_condition_bound(condition, &mut add_node)?;
        }
    }
    Ok(())
}

fn add_condition_bound(
    condition: &ConditionOutputV1,
    add_node: &mut impl FnMut(&[&str], usize) -> Result<(), AppError>,
) -> Result<(), AppError> {
    add_node(&[&condition.expression], 0)?;
    for source in &condition.sources {
        add_node(&[source], 0)?;
    }
    Ok(())
}

fn output_bound_overflow() -> AppError {
    AppError::output_limit("TOML output-size accounting overflowed usize".to_owned())
}

fn output_bound_error(bound: usize) -> AppError {
    AppError::output_limit(format!(
        "TOML output has a conservative {bound}-byte render bound, exceeding the {MAX_OUTPUT_BYTES}-byte application limit"
    ))
}
