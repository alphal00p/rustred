//! Custom-owner generation with the false-knock-out guard (owner: M2-T7c rr-custom).
//!
//! [`generate_custom_owner`] solves every raw chart of one owner support of a
//! Custom presentation with SpIRed through the ordinary candidate generation
//! (`CandidateGenerationSession::from_family`, the native-family entry of
//! `family_candidates_from_family_controlled`): all-rank (no numerator cap,
//! finite cases searched), the request's default discovery and rule
//! selection, the presentation's other slots as nonpositive indices, and for an
//! eps presentation the opt-in two-sided eps axes with all `2^j` raw charts
//! selected (plan M2 sec.3.4). The payload is the generated candidate bundle.
//!
//! Before the solve the owner support's Lee-Pomeransky count is taken with its
//! shifts (`MasterCounter::try_new_p_shifted`; no verdict is a typed error,
//! never a count of 0). After it the hard guard of
//! [`rustred::propagator::custom::check_custom_owner`] refuses `LP >= 1` with no
//! residual, missing two-sided data and missing charts, and the payload must
//! build as a lane step. A chart whose search fails (for example on an
//! exceptional conjunction SpIRed cannot intersect) fails the owner with the
//! chart mask and the solver's complete message; the owner is not retried
//! another way (plan M2 sec.10, contingency C3 is the coordinator's).
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use rustred::propagator::custom::{
    CustomChartSummary, build_custom_step, check_custom_owner, custom_owner_charts,
    custom_owner_support, inspect_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::load_p_families;
use rustred::propagator::lane::OwnerLane;
use rustred::propagator::library::CustomOwnerMeta;
use rustred::propagator::present::{present_node, top_topology};
use rustred::propagator::presentation::{Presentation, Slot};
use rustred::propagator::topology::{ChildMode, reduction_graph};
use rustred::propagator::{NodeIndex, PresentationId};
use rustred::sector::{Mask, MasterCount, MasterCountOptions, MasterCounter};

use crate::application::{
    CandidateGenerationSession, FamilyCandidatesRequest, SectorFailurePolicy,
};

/// How a Custom owner is generated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomOwnerOptions {
    /// The owner support, a mask of line slots holding every eps axis; `None`
    /// takes [`custom_owner_support`] (the sector of an induced presentation,
    /// every line otherwise).
    pub support: Option<u64>,
    /// Generation workers: charts are solved side by side.
    pub n_cores: usize,
}

impl Default for CustomOwnerOptions {
    fn default() -> Self {
        Self {
            support: None,
            n_cores: 1,
        }
    }
}

/// A generated Custom owner: its candidate bundle and its record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomOwnerPayload {
    pub bytes: Arc<[u8]>,
    pub meta: CustomOwnerMeta,
}

/// What one generation measured, for reports and cost tables.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomOwnerReport {
    pub support: u64,
    pub lp_count: u32,
    pub charts: Vec<CustomChartSummary>,
    pub rules: usize,
    pub residual_keys: Vec<Vec<i64>>,
    /// Stable id of the generation order.
    pub ordering: String,
    pub payload_bytes: usize,
    /// Wall time of the LP count, of the generation and of the whole call.
    pub lp_seconds: f64,
    pub generation_seconds: f64,
    pub total_seconds: f64,
    /// The generation report (`rustred.family-candidates-output.toml.v1`).
    pub generation_report: String,
}

/// Generate the Custom owner of `presentation`; see the module documentation.
pub fn generate_custom_owner(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
) -> Result<CustomOwnerPayload, PError> {
    generate_custom_owner_with_report(presentation, options).map(|(payload, _)| payload)
}

fn line_slots(presentation: &Presentation) -> u64 {
    (presentation.slots.iter().enumerate())
        .filter(|(_, slot)| matches!(slot, Slot::Line(_)))
        .fold(0, |mask, (slot, _)| mask | 1 << slot)
}

fn bits(mask: u64, arity: usize) -> Vec<bool> {
    (0..arity).map(|slot| mask >> slot & 1 == 1).collect()
}

fn mask_text(mask: u64, arity: usize) -> String {
    (0..arity)
        .map(|slot| if mask >> slot & 1 == 1 { '1' } else { '0' })
        .collect()
}

/// The LP count of `support` with shifts; no verdict is an error, never 0.
pub fn custom_owner_lp_count(presentation: &Presentation, support: u64) -> Result<u32, PError> {
    let family = &presentation.family;
    let failed = |detail: String| {
        PError::Generation(format!(
            "LP count of support {} of presentation {}: {detail}",
            mask_text(support, presentation.slots.len()),
            presentation.id.0
        ))
    };
    let mut counter = MasterCounter::try_new_p_shifted(family, MasterCountOptions::default())
        .map_err(|error| failed(error.to_string()))?;
    let sector = Mask::try_new(bits(support, presentation.slots.len()))
        .map_err(|error| failed(error.to_string()))?;
    match counter
        .count(&sector)
        .map_err(|error| failed(error.to_string()))?
    {
        MasterCount::Zero => Ok(0),
        MasterCount::Counted(count) => u32::try_from(count).map_err(|_| failed(format!("{count}"))),
        verdict @ MasterCount::NoVerdict { .. } => Err(failed(format!("no verdict, {verdict:?}"))),
    }
}

/// The failed charts of a generation report under `SectorFailurePolicy::Report`,
/// each with its complete message.
fn failed_charts(report: &str) -> Result<Vec<String>, PError> {
    let report: toml::Value = toml::from_str(report)
        .map_err(|error| PError::Generation(format!("generation report: {error}")))?;
    let Some(failed) = report.get("failed_sectors") else {
        return Ok(Vec::new());
    };
    let entries = failed
        .as_array()
        .ok_or_else(|| PError::Generation("generation report: failed_sectors".to_owned()))?;
    Ok(entries
        .iter()
        .map(|entry| {
            let chart: String = (entry.get("sector").and_then(toml::Value::as_array))
                .map(|bits| {
                    bits.iter()
                        .map(|bit| {
                            if bit.as_bool() == Some(true) {
                                '1'
                            } else {
                                '0'
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let text = |field: &str| {
                entry
                    .get(field)
                    .and_then(toml::Value::as_str)
                    .unwrap_or("?")
                    .to_owned()
            };
            format!("chart {chart}: {}: {}", text("kind"), text("error"))
        })
        .collect())
}

/// [`generate_custom_owner`] with what it measured.
pub fn generate_custom_owner_with_report(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    let started = Instant::now();
    let arity = presentation.slots.len();
    let lines = line_slots(presentation);
    let forced = presentation.forced_axes();
    let support = options
        .support
        .unwrap_or_else(|| custom_owner_support(presentation));
    if support == 0 || support & !lines != 0 || support & forced != forced {
        return Err(PError::Unsupported(format!(
            "custom owner support {} of presentation {}: a nonempty set of lines {} with every eps axis {}",
            mask_text(support, arity),
            presentation.id.0,
            mask_text(lines, arity),
            mask_text(forced, arity)
        )));
    }
    if options.n_cores == 0 {
        return Err(PError::Unsupported(
            "custom owner generation needs at least one worker".to_owned(),
        ));
    }
    let charts = custom_owner_charts(support, forced);
    // Before the solve, so that a refused count cannot lose a long run.
    let lp_count = custom_owner_lp_count(presentation, support)?;
    let lp_seconds = started.elapsed().as_secs_f64();

    let mut request = FamilyCandidatesRequest::new("custom owner");
    request.n_cores = options.n_cores;
    request.nonpositive_indices = (0..arity)
        .filter(|&slot| support >> slot & 1 == 0)
        .collect();
    request.selected_sectors = Some(charts.iter().map(|&chart| bits(chart, arity)).collect());
    request.two_sided_eps_axes = forced != 0;
    request.sector_failures = SectorFailurePolicy::Report;
    let generation = Instant::now();
    let generated = (|| {
        let (session, job) = CandidateGenerationSession::from_family(
            Arc::clone(&presentation.family),
            request,
            1024,
        )?;
        job.run();
        session.result()
    })()
    .map_err(|error| {
        PError::Generation(format!(
            "custom owner of presentation {}, support {}: {error}",
            presentation.id.0,
            mask_text(support, arity)
        ))
    })?;
    let generation_seconds = generation.elapsed().as_secs_f64();
    let failed = failed_charts(generated.to_toml())?;
    if !failed.is_empty() {
        return Err(PError::Generation(format!(
            "custom owner of presentation {}, support {}: {}",
            presentation.id.0,
            mask_text(support, arity),
            failed.join("; ")
        )));
    }

    let bytes: Arc<[u8]> = Arc::from(generated.bundle());
    let summary = inspect_custom_payload(&bytes)?;
    let solved: BTreeSet<u64> = summary.charts.iter().map(|chart| chart.chart).collect();
    if summary.support != support || solved != charts.iter().copied().collect() {
        return Err(PError::Generation(format!(
            "custom owner of presentation {}: generated support {} and charts {:x?}, requested {} and {charts:x?}",
            presentation.id.0,
            mask_text(summary.support, arity),
            solved,
            mask_text(support, arity)
        )));
    }
    let meta = summary.meta(lp_count)?;
    check_custom_owner(presentation, &meta)?;
    build_custom_step(presentation, &bytes, &meta)?;
    let report = CustomOwnerReport {
        support,
        lp_count,
        rules: summary.rules(),
        residual_keys: summary.residual_keys(),
        ordering: summary.ordering.clone(),
        charts: summary.charts,
        payload_bytes: bytes.len(),
        lp_seconds,
        generation_seconds,
        total_seconds: started.elapsed().as_secs_f64(),
        generation_report: generated.to_toml().to_owned(),
    };
    Ok((CustomOwnerPayload { bytes, meta }, report))
}

/// One Custom node of a reduction DAG with its presentation, as assembly
/// presents it.
#[derive(Clone, Debug)]
pub struct CustomOwnerNode {
    pub node: NodeIndex,
    pub loops: u8,
    pub eps_lines: u8,
    pub presentation: Presentation,
}

/// The Custom nodes of the reduction DAG (`ChildMode::ForcerTree`) of the
/// families with at most `max_loops` loops: tops are presentations
/// `0..`, node presentations follow in node order, as in `rustred p-census`.
pub fn custom_owner_nodes(
    families_json: &str,
    max_loops: u8,
) -> Result<Vec<CustomOwnerNode>, PError> {
    let (file, presentations) = load_p_families(families_json)?;
    let (entries, tops): (Vec<_>, Vec<_>) = (file.families.iter().zip(presentations))
        .filter(|(entry, _)| entry.loops <= max_loops)
        .unzip();
    let context = (tops.first())
        .ok_or_else(|| PError::Unsupported(format!("no family with at most {max_loops} loops")))?
        .family
        .coefficient_context()
        .clone();
    let roots: Vec<_> = entries.iter().map(|entry| top_topology(entry)).collect();
    let dag = reduction_graph(&roots, ChildMode::ForcerTree);
    let mut next = tops.iter().map(|top| top.id.0 + 1).max().unwrap_or(0);
    let mut nodes = Vec::new();
    for node in &dag.nodes {
        let made = present_node(node, &tops, PresentationId(next), &context)?;
        let Some(presentation) = made.presentation else {
            continue;
        };
        next += 1;
        if made.decision.lane == OwnerLane::Custom {
            nodes.push(CustomOwnerNode {
                node: NodeIndex(node.index),
                loops: node.loops,
                eps_lines: node.eps_lines,
                presentation,
            });
        }
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A failed chart comes back with its mask and the solver's complete
    /// message (coordinator note 2026-11-06, change 2).
    #[test]
    fn failed_charts_keep_the_mask_and_the_whole_message() {
        let long = format!("exact case intersection: {}", "x".repeat(2000));
        let report = format!(
            "schema = \"rustred.family-candidates-output.toml.v1\"\n\n[[failed_sectors]]\n\
             ordinal = 1\nsector = [true, false, true]\nkind = \"unsupported-nonlinear-equalities\"\n\
             error = \"{long}\"\n"
        );
        let failed = failed_charts(&report).unwrap();
        assert_eq!(
            failed,
            [format!(
                "chart 101: unsupported-nonlinear-equalities: {long}"
            )]
        );
        assert!(failed_charts("schema = \"x\"").unwrap().is_empty());
    }
}
