//! Saved, unrecentered seed geometry as a bounded ordinary-source nomination.
//! No preconditioned basis reconstruction, frame replay or proof is performed.

use std::collections::BTreeSet;
use std::io::Write;

use rustred::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use serde::Serialize;

use super::super::{CandidateBundleLimits, codec, model::RuleRecord};
use super::{
    CandidateCaseInspection, CandidateFixedAxisInspection, CandidateIntegralInspection,
    LimitedWriter, integral_view,
};
use crate::AppError;

/// A single saved coordinate rule and explicit nomination/generation resources.
/// These limits do not grant any source, descent, dispatch or closure authority.
#[derive(Clone, Debug)]
pub struct CandidateSourceSupportOptions {
    pub sector: Vec<bool>,
    pub rule_ordinal: usize,
    /// Optional common source translation nominated by the caller. Saved
    /// canonical targets do NOT reveal the original winning pivot translation.
    /// This is never a recovered or authenticated recentering; fixed axes must
    /// be zero. None reports the stored seeds without recentering.
    pub source_recenter_nomination: Option<Vec<i64>>,
    pub max_retained_seeds: usize,
    pub max_unique_offsets: usize,
    pub max_original_rows: usize,
    pub max_nominated_pairs: usize,
    /// Retained inventory count checked after native completion. The prepared
    /// row wrapper exposes no term count; source_generation bounds native row
    /// arithmetic, and the caller must supervise scratch time/memory.
    pub max_generated_terms: usize,
    /// Like max_generated_terms, this is not a pre-allocation scratch bound.
    pub max_generated_conditions: usize,
    pub max_output_bytes: usize,
    pub source_generation: ParametricIbpConfig,
}

/// Factored nomination: every named ordinary row at every physical offset.
/// Saved basis ordinals and weights are deliberately absent. In particular,
/// this does not certify that an incumbent's zero-sector-quotiented identity
/// belongs to the full original-source span, or that its guards can be reused.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateSourceSupportInspection {
    pub schema: &'static str,
    pub family_fingerprint: String,
    pub integral_order: String,
    pub priority_slots: Option<Vec<usize>>,
    pub root_sector: Vec<bool>,
    pub sector: Vec<bool>,
    pub rule_ordinal: usize,
    pub case: CandidateCaseInspection,
    pub target: CandidateIntegralInspection,
    /// Unknown by default; an explicit value is only a caller nomination.
    pub source_recenter_nomination: Option<Vec<i64>>,
    pub source_recenter_status: &'static str,
    pub source_offset_frame: &'static str,
    /// Payload-local IDs retained only as saved applicability metadata.
    pub excluded_all_zero_conjunctions: Vec<Vec<u32>>,
    pub guard_metadata_authority: &'static str,
    pub retained_seed_count: usize,
    pub unique_offset_count: usize,
    /// Lexicographically ordered stored-seed displacements, optionally shifted
    /// by the explicit caller nominee. Without a recovered raw winning pivot,
    /// these need not contain the saved canonical rule's source span. Numeric
    /// target coordinates are specialized only after translation.
    pub source_offsets: Vec<Vec<i64>>,
    /// Complete native ordinary generation order, not saved basis-row IDs.
    pub ordinary_source_rows: Vec<String>,
    pub nominated_pair_count: usize,
    pub generated_term_count: usize,
    pub generated_condition_count: usize,
    pub nomination_semantics: &'static str,
    pub original_inventory_completed: bool,
    pub saved_basis_ordinals_validated: bool,
    pub source_replay_claim: bool,
    pub dispatch_claim: bool,
    pub closure_claim: bool,
    #[serde(skip)]
    max_output_bytes: usize,
}

fn bound(value: usize, maximum: usize, what: &str) -> Result<(), AppError> {
    if maximum == 0 || value > maximum {
        return Err(AppError::limit(format!(
            "source-support {what} allowance exceeded or zero"
        )));
    }
    Ok(())
}

fn offsets(
    rule: &RuleRecord,
    arity: usize,
    options: &CandidateSourceSupportOptions,
) -> Result<Vec<Vec<i64>>, AppError> {
    codec::rules::validate_rule(rule, arity)?;
    if rule.case.kind != "coordinate" {
        return Err(AppError::input(
            "source-support nomination currently requires a coordinate case; affine cases are unsupported",
        ));
    }
    bound(
        rule.sources.len(),
        options.max_retained_seeds,
        "retained seed",
    )?;
    bound(0, options.max_unique_offsets, "unique offset")?;
    if rule.sources.is_empty() {
        return Err(AppError::input("saved rule has no retained seed support"));
    }
    let fixed = rule
        .case
        .fixed_axes
        .iter()
        .copied()
        .zip(rule.case.fixed_values.iter().copied())
        .collect::<std::collections::BTreeMap<_, _>>();
    for axis in 0..arity {
        match fixed.get(&axis) {
            Some(&value) if !rule.target.symbolic[axis] && rule.target.values[axis] == value => {}
            None if rule.target.symbolic[axis] && rule.target.values[axis] == 0 => {}
            _ => {
                return Err(AppError::input(
                    "saved target symbolic/fixed layout differs from its coordinate case",
                ));
            }
        }
    }
    if let Some(recenter) = &options.source_recenter_nomination {
        if recenter.len() != arity
            || recenter
                .iter()
                .zip(&rule.target.symbolic)
                .any(|(&value, &symbolic)| !symbolic && value != 0)
        {
            return Err(AppError::input(
                "source recenter nomination requires exact arity and zero fixed-axis entries",
            ));
        }
    }
    let mut unique = BTreeSet::new();
    for seed in &rule.sources {
        if seed.integral.symbolic != rule.target.symbolic {
            return Err(AppError::input(
                "saved seed symbolic pattern differs from target",
            ));
        }
        let mut offset = Vec::with_capacity(arity);
        for axis in 0..arity {
            let value = if seed.integral.symbolic[axis] {
                if seed.integral.values[axis] != seed.shifts[axis] {
                    return Err(AppError::input(
                        "saved symbolic seed coefficient shift differs from physical integral",
                    ));
                }
                i64::from(seed.shifts[axis]).checked_add(
                    options
                        .source_recenter_nomination
                        .as_ref()
                        .map_or(0, |s| s[axis]),
                )
            } else {
                if seed.shifts[axis] != 0 {
                    return Err(AppError::input(
                        "saved numeric seed has a nonzero coefficient shift",
                    ));
                }
                i64::from(seed.integral.values[axis])
                    .checked_sub(i64::from(rule.target.values[axis]))
            }
            .ok_or_else(|| AppError::limit("physical source offset overflow"))?;
            offset.push(value);
        }
        if !unique.contains(&offset) {
            bound(
                unique
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| AppError::limit("source offset count overflow"))?,
                options.max_unique_offsets,
                "unique offset",
            )?;
            unique.insert(offset);
        }
    }
    Ok(unique.into_iter().collect())
}

/// Decode one saved rule, validate seed coordinate transport and complete native
/// ordinary generation. The Cartesian product in the report is a nomination
/// only; it contains neither a selected linear combination nor proof authority.
pub fn inspect_generated_candidate_source_support(
    bytes: &[u8],
    limits: CandidateBundleLimits,
    options: CandidateSourceSupportOptions,
) -> Result<CandidateSourceSupportInspection, AppError> {
    for (cap, name) in [
        (options.max_retained_seeds, "retained seed"),
        (options.max_unique_offsets, "unique offset"),
        (options.max_original_rows, "original row"),
        (options.max_nominated_pairs, "nominated pair"),
        (options.max_generated_terms, "generated term"),
        (options.max_generated_conditions, "generated condition"),
        (options.max_output_bytes, "output byte"),
    ] {
        bound(0, cap, name)?;
    }
    if options.max_output_bytes > crate::MAX_OUTPUT_BYTES {
        return Err(AppError::input(
            "source-support output allowance exceeds MAX_OUTPUT_BYTES",
        ));
    }
    let bundle = codec::read(bytes, limits)?;
    let arity = bundle.root_sector.len();
    let sector = bundle
        .sectors
        .iter()
        .find(|s| s.sector == options.sector)
        .ok_or_else(|| AppError::input("source-support sector is absent"))?;
    let rule = sector
        .rules
        .get(options.rule_ordinal)
        .ok_or_else(|| AppError::input("source-support saved rule ordinal is absent"))?;
    let source_offsets = offsets(rule, arity, &options)?;
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .map_err(codec::binary_error)?;
    if family.fingerprint() != bundle.family_fingerprint || family.denominator_count() != arity {
        return Err(AppError::input("source-support family binding differs"));
    }
    let generator = ParametricIbpGenerator::try_new_with_config(&family, options.source_generation)
        .map_err(|e| AppError::input(e.to_string()))?;
    let prepared = generator
        .prepare_ordinary_ibp()
        .map_err(|e| AppError::input(e.to_string()))?;
    bound(prepared.len(), options.max_original_rows, "original row")?;
    let nominated_pair_count = source_offsets
        .len()
        .checked_mul(prepared.len())
        .ok_or_else(|| AppError::limit("nominated source pair count overflow"))?;
    bound(
        nominated_pair_count,
        options.max_nominated_pairs,
        "nominated pair",
    )?;
    // IbpSourceRow intentionally exposes no relation/count accessor before
    // the native completion barrier. Keep that barrier, then check aggregate
    // retained counts; generation config and outer supervision bound scratch.
    let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    let completed = prepared
        .complete(rows)
        .map_err(|e| AppError::input(e.to_string()))?;
    let mut ordinary_source_rows = Vec::new();
    let mut generated_term_count = 0usize;
    let mut generated_condition_count = 0usize;
    for row in completed.into_relations() {
        generated_term_count = generated_term_count
            .checked_add(row.terms().len())
            .ok_or_else(|| AppError::limit("generated term count overflow"))?;
        generated_condition_count = generated_condition_count
            .checked_add(row.nonzero_conditions().len())
            .ok_or_else(|| AppError::limit("generated condition count overflow"))?;
        bound(
            generated_term_count,
            options.max_generated_terms,
            "generated term",
        )?;
        bound(
            generated_condition_count,
            options.max_generated_conditions,
            "generated condition",
        )?;
        ordinary_source_rows.push(row.row_id().stable_string());
    }
    let report = CandidateSourceSupportInspection {
        schema: "rustred.candidate-source-support.json.v2",
        family_fingerprint: bundle.family_fingerprint.clone(),
        integral_order: bundle.integral_order.clone(),
        priority_slots: bundle.permutation.clone(),
        root_sector: bundle.root_sector.clone(),
        sector: options.sector,
        rule_ordinal: options.rule_ordinal,
        case: CandidateCaseInspection {
            kind: rule.case.kind.clone(),
            fixed: rule
                .case
                .fixed_axes
                .iter()
                .zip(&rule.case.fixed_values)
                .map(|(&axis, &value)| CandidateFixedAxisInspection { axis, value })
                .collect(),
            affine_zero_equations: rule.case.equations.clone(),
        },
        target: integral_view(&rule.target),
        source_recenter_status: if options.source_recenter_nomination.is_some() {
            "caller_nomination_unverified"
        } else {
            "unknown"
        },
        source_offset_frame: if options.source_recenter_nomination.is_some() {
            "caller_recentered_stored_seeds"
        } else {
            "unrecentered_stored_seeds"
        },
        source_recenter_nomination: options.source_recenter_nomination,
        excluded_all_zero_conjunctions: rule.exclusions.clone(),
        guard_metadata_authority: "saved payload-local exclusion IDs only; no guard, pivot or source-condition reuse",
        retained_seed_count: rule.sources.len(),
        unique_offset_count: source_offsets.len(),
        source_offsets,
        ordinary_source_rows,
        nominated_pair_count,
        generated_term_count,
        generated_condition_count,
        nomination_semantics: "every completed native ordinary RowId at each reported stored-seed displacement; the saved canonical target does not determine the raw winning pivot's recentering; no incumbent-span membership is asserted; full sources must be regenerated and independently proved before use",
        original_inventory_completed: true,
        saved_basis_ordinals_validated: false,
        source_replay_claim: false,
        dispatch_claim: false,
        closure_claim: false,
        max_output_bytes: options.max_output_bytes,
    };
    report.write_json(std::io::sink())?;
    Ok(report)
}

impl CandidateSourceSupportInspection {
    pub fn write_json(&self, writer: impl Write) -> Result<(), AppError> {
        let mut writer = LimitedWriter {
            writer,
            bytes: 0,
            limit: self.max_output_bytes,
            exceeded: false,
        };
        serde_json::to_writer_pretty(&mut writer, self).map_err(|error| {
            if writer.exceeded {
                AppError::limit("source-support JSON exceeds output budget")
            } else {
                AppError::serialization(error.to_string())
            }
        })
    }

    pub fn to_json(&self) -> Result<String, AppError> {
        let mut bytes = Vec::new();
        self.write_json(&mut bytes)?;
        String::from_utf8(bytes).map_err(|e| AppError::serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests;
