//! Native, dynamically sized diagnostic views of saved candidate programs.
//! Shape inspection performs no search, source generation, zero census, replay
//! or dispatch. The separate opt-in source-support API completes the ordinary
//! inventory solely to name its rows; it does not replay or certify a rule.

mod model;
mod source_support;
#[cfg(test)]
mod tests;

pub use model::*;
pub use source_support::{
    CandidateSourceSupportInspection, CandidateSourceSupportOptions,
    inspect_generated_candidate_source_support,
};

use std::collections::BTreeSet;
use std::io::Write;

use rustred::identity::ParametricIbpGenerator;
use rustred::persistence::CoefficientId;
use symbolica::prelude::AtomCore;

use super::model::{IntegralRecord, RuleRecord};
use super::{CandidateBundleLimits, codec};
use crate::AppError;

/// Decode trusted matching RustRed/Symbolica data into a bounded human-readable
/// view. Native import and family binding are checked, but identities, saved
/// rule-frame applicability and whole-family coverage are NOT established.
/// The canonical strings below are diagnostics, not a new artifact format or
/// a semantic equality/digest implementation. Coefficient IDs are payload-local.
pub fn inspect_generated_candidate_program(
    bytes: &[u8],
    limits: CandidateBundleLimits,
    options: CandidateProgramInspectionOptions,
) -> Result<CandidateProgramInspection, AppError> {
    if options.max_output_bytes == 0 || options.max_output_bytes > crate::MAX_OUTPUT_BYTES {
        return Err(AppError::input(
            "diagnostic output budget must be positive and within MAX_OUTPUT_BYTES",
        ));
    }
    let bundle = codec::read(bytes, limits)?;
    let arity = bundle.root_sector.len();
    let selected = selection(
        options.sectors.as_deref(),
        bundle.sectors.iter().map(|s| s.sector.clone()),
    )?;
    let ordinals = selection(
        options.rule_ordinals.as_deref(),
        bundle
            .sectors
            .iter()
            .filter(|s| selected.as_ref().is_none_or(|set| set.contains(&s.sector)))
            .flat_map(|s| 0..s.rules.len()),
    )?;
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .map_err(codec::binary_error)?;
    if family.fingerprint() != bundle.family_fingerprint || family.denominator_count() != arity {
        return Err(AppError::input(
            "candidate inspection family binding differs",
        ));
    }
    // This constructor creates the same positional indexed context but does
    // not generate a single IBP/source row or prepare any sector census.
    let generator =
        ParametricIbpGenerator::try_new(&family).map_err(|e| AppError::input(e.to_string()))?;
    let mut text_bytes = 0usize;
    let mut bounded = |text: String| -> Result<String, AppError> {
        text_bytes = text_bytes
            .checked_add(text.len())
            .ok_or_else(|| AppError::limit("diagnostic text byte count overflow"))?;
        if text_bytes > options.max_output_bytes {
            return Err(AppError::limit("diagnostic text exceeds output budget"));
        }
        Ok(text)
    };
    let index_symbols = (0..arity)
        .map(|axis| {
            let value = generator
                .context()
                .index(axis)
                .map_err(|e| AppError::input(e.to_string()))?;
            bounded(value.to_expression().to_canonical_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut ids = BTreeSet::new();
    let mut sectors = Vec::new();
    let mut included_rules = 0;
    let mut included_terminals = 0;
    for (ordinal, sector) in bundle.sectors.iter().enumerate() {
        if selected
            .as_ref()
            .is_some_and(|set| !set.contains(&sector.sector))
        {
            continue;
        }
        let mut rules = Vec::new();
        for (rule_ordinal, rule) in sector.rules.iter().enumerate() {
            if ordinals
                .as_ref()
                .is_some_and(|set| !set.contains(&rule_ordinal))
            {
                continue;
            }
            ids.extend(rule.case.equations.iter().copied());
            ids.extend(rule.exclusions.iter().flatten().copied());
            if options.include_rhs_coefficients {
                ids.extend(rule.rhs.iter().map(|term| term.coefficient));
            }
            let mut view = rule_view(rule_ordinal, rule);
            if codec::dispatch::policy(&bundle.records, ordinal, rule_ordinal)
                == rustred::solver::RuleDispatchPolicy::AfterBaselinePartitionWholePiece
            {
                view.dispatch_policy = Some("AfterBaselinePartitionWholePiece");
            }
            rules.push(view);
        }
        included_rules += rules.len();
        included_terminals += sector.finite_residuals.len();
        sectors.push(CandidateSectorInspection {
            ordinal,
            sector: sector.sector.clone(),
            total_rules: sector.rules.len(),
            omitted_rules: sector.rules.len() - rules.len(),
            rules,
            terminals: sector
                .finite_residuals
                .iter()
                .map(|key| key.values.clone())
                .collect(),
        });
    }
    let coefficients = ids
        .into_iter()
        .map(|id| {
            let key = CoefficientId::try_from_index(id as usize).map_err(codec::binary_error)?;
            let value = bundle
                .coefficients
                .coefficient(key)
                .map_err(codec::binary_error)?;
            Ok(CandidateCoefficientInspection {
                id,
                variables: value
                    .numerator
                    .variables()
                    .iter()
                    .map(|variable| bounded(variable.to_atom().to_canonical_string()))
                    .collect::<Result<Vec<_>, _>>()?,
                numerator: bounded(value.numerator.to_expression().to_canonical_string())?,
                denominator: bounded(value.denominator.to_expression().to_canonical_string())?,
                numerator_terms: value.numerator.nterms(),
                denominator_terms: value.denominator.nterms(),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let total_rules = bundle.sectors.iter().map(|s| s.rules.len()).sum::<usize>();
    let total_terminals = bundle
        .sectors
        .iter()
        .map(|s| s.finite_residuals.len())
        .sum::<usize>();
    let report = CandidateProgramInspection {
        schema: "rustred.candidate-program-inspection.json.v1",
        candidate_schema: bundle.schema.clone(),
        family_fingerprint: bundle.family_fingerprint.clone(),
        arity,
        root_sector: bundle.root_sector.clone(),
        integral_order: bundle.integral_order.clone(),
        priority_slots: bundle.permutation.clone(),
        coordinates: "original physical denominator axes, zero-based; priority_slots never reroutes keys",
        index_symbols,
        exclusion_semantics: "inapplicable if ANY branch has ALL its polynomials zero; branches retain saved order",
        applicability_warning: "ordered saved candidates only, not disjoint dispatch partitions; source conditions, denominator poles, earlier rules, terminal and zero handling still govern application",
        coefficient_text_authority: "native Symbolica diagnostic display only; IDs local to this payload; no coefficient equivalence claim",
        source_replay_claim: false,
        closure_claim: false,
        total_sectors: bundle.sectors.len(),
        omitted_sectors: bundle.sectors.len() - sectors.len(),
        total_rules,
        omitted_rules: total_rules - included_rules,
        total_terminals,
        omitted_terminals: total_terminals - included_terminals,
        rhs_coefficient_details_included: options.include_rhs_coefficients,
        sectors,
        coefficients,
        max_output_bytes: options.max_output_bytes,
    };
    // Charge JSON punctuation and structure too, without retaining another
    // full serialized copy. A too-small budget fails, never silently truncates.
    report.write_json(std::io::sink())?;
    Ok(report)
}

fn selection<T: Ord + Clone>(
    requested: Option<&[T]>,
    available: impl Iterator<Item = T>,
) -> Result<Option<BTreeSet<T>>, AppError> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    let wanted: BTreeSet<_> = requested.iter().cloned().collect();
    let available: BTreeSet<_> = available.collect();
    if wanted.is_empty() || wanted.len() != requested.len() || !wanted.is_subset(&available) {
        return Err(AppError::input(
            "inspection filter is empty, duplicated or selects an absent sector/rule ordinal",
        ));
    }
    Ok(Some(wanted))
}

pub(super) fn integral_view(key: &IntegralRecord) -> CandidateIntegralInspection {
    CandidateIntegralInspection {
        symbolic: key.symbolic.clone(),
        values: key.values.clone(),
    }
}

pub(super) fn rule_view(ordinal: usize, rule: &RuleRecord) -> CandidateRuleInspection {
    CandidateRuleInspection {
        dispatch_policy: None,
        ordinal,
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
        rhs: rule
            .rhs
            .iter()
            .enumerate()
            .map(|(ordinal, term)| CandidateTermInspection {
                ordinal,
                integral: integral_view(&term.integral),
                coefficient_id: term.coefficient,
                symbolic_shifts: rule
                    .target
                    .symbolic
                    .iter()
                    .enumerate()
                    .map(|(axis, &symbolic)| {
                        symbolic.then(|| {
                            i32::from(term.integral.values[axis])
                                - i32::from(rule.target.values[axis])
                        })
                    })
                    .collect(),
                fixed_replacements: term
                    .integral
                    .symbolic
                    .iter()
                    .enumerate()
                    .map(|(axis, &symbolic)| (!symbolic).then_some(term.integral.values[axis]))
                    .collect(),
            })
            .collect(),
        excluded_all_zero_conjunctions: rule.exclusions.clone(),
        retained_source_count: rule.sources.len(),
    }
}

impl CandidateProgramInspection {
    /// Stream diagnostic JSON under the caller's byte limit. On I/O failure the
    /// destination may contain a prefix; this is not atomic artifact output.
    pub fn write_json(&self, writer: impl Write) -> Result<(), AppError> {
        let mut writer = LimitedWriter {
            writer,
            bytes: 0,
            limit: self.max_output_bytes,
            exceeded: false,
        };
        serde_json::to_writer_pretty(&mut writer, self).map_err(|error| {
            if writer.exceeded {
                AppError::limit("diagnostic JSON exceeds output budget")
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

struct LimitedWriter<W> {
    writer: W,
    bytes: usize,
    limit: usize,
    exceeded: bool,
}
impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes) {
            self.exceeded = true;
            return Err(std::io::Error::other("diagnostic output budget exceeded"));
        }
        let written = self.writer.write(bytes)?;
        self.bytes += written;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
