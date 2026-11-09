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
//! build as a lane step with every chart prepared. A chart whose search fails (for example on an
//! exceptional conjunction SpIRed cannot intersect) fails the owner with the
//! chart mask and the solver's complete message; the owner is not retried
//! another way (plan M2 sec.10, contingency C3 is the coordinator's).
//!
//! A [`GenerationBudget`] bounds the whole call (LP count, solve,
//! preparation) in wall time and process RSS (plan M3 D-M3-4, M3-I1): it is
//! installed as a Symbolica resource budget, so the solver's own resource
//! checkpoints stop at it, and it is checked again at the boundaries of the
//! call. Exceeding it fails closed with [`PError::BudgetExceeded`]. A
//! re-generation may take the LP count from its record
//! ([`CustomOwnerOptions::lp_given`]) instead of counting again.
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

use rustred::propagator::custom::{
    CustomChartSummary, build_custom_step_prepared, check_custom_owner, custom_owner_charts,
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
#[derive(Clone, Debug, PartialEq)]
pub struct CustomOwnerOptions {
    /// The owner support, a mask of line slots holding every eps axis; `None`
    /// takes [`custom_owner_support`] (the sector of an induced presentation,
    /// every line otherwise).
    pub support: Option<u64>,
    /// Generation workers: charts are solved side by side.
    pub n_cores: usize,
    /// Time and RSS ceilings of the whole call; none by default.
    pub budget: GenerationBudget,
    /// The LP count of the support as RECORDED by an earlier generation of
    /// this owner (M3-T15 counts each owner once): the count is not taken
    /// again and the report says so ([`CustomOwnerReport::lp_given`]). For
    /// re-generation only, never a first count; the false-knock-out guard
    /// still checks it against the residuals.
    pub lp_given: Option<u32>,
}

impl Default for CustomOwnerOptions {
    fn default() -> Self {
        Self {
            support: None,
            n_cores: 1,
            budget: GenerationBudget::default(),
            lp_given: None,
        }
    }
}

/// Ceilings of one generation call; `None` imposes none.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GenerationBudget {
    /// Wall seconds from the start of the call (finite, positive).
    pub max_seconds: Option<f64>,
    /// Resident set of the whole process, in bytes (positive). Sampled from
    /// `/proc/self/status` at the solver's resource checkpoints (at most one
    /// read per [`RSS_SAMPLE_INTERVAL`]) and at the boundaries of the call.
    pub max_rss_bytes: Option<u64>,
}

/// The shortest time between two reads of the process RSS.
pub const RSS_SAMPLE_INTERVAL: Duration = Duration::from_millis(50);

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
    /// The LP count was the recorded one of [`CustomOwnerOptions::lp_given`],
    /// not counted by this call (`lp_seconds` is then 0).
    pub lp_given: bool,
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

/// The process RSS of `/proc/self/status` (`VmRSS`), in bytes.
fn process_rss_bytes() -> Result<u64, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("/proc/self/status: {error}"))?;
    let kib = (status.lines())
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.trim().strip_suffix("kB"))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .ok_or_else(|| "no VmRSS in /proc/self/status".to_owned())?;
    kib.checked_mul(1024)
        .ok_or_else(|| "VmRSS overflows u64 bytes".to_owned())
}

/// The process RSS, read at most once per [`RSS_SAMPLE_INTERVAL`], with the
/// largest value read above the ceiling (sticky).
struct RssSampler {
    started: Instant,
    limit: u64,
    /// Nanoseconds after `started` of the last read, `u64::MAX` before any.
    read_at: AtomicU64,
    bytes: AtomicU64,
    over: AtomicU64,
}

impl RssSampler {
    fn sample(&self) -> Result<u64, String> {
        let now = u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX - 1);
        let read_at = self.read_at.load(Ordering::Acquire);
        let interval = RSS_SAMPLE_INTERVAL.as_nanos() as u64;
        if read_at != u64::MAX && now.saturating_sub(read_at) < interval {
            return Ok(self.bytes.load(Ordering::Acquire));
        }
        let bytes = process_rss_bytes()?;
        self.bytes.store(bytes, Ordering::Release);
        self.read_at.store(now, Ordering::Release);
        if bytes > self.limit {
            self.over.fetch_max(bytes, Ordering::AcqRel);
        }
        Ok(bytes)
    }
}

/// A [`GenerationBudget`] in force: the Symbolica resource budget of the call
/// and what it observed.
struct Budget {
    resources: ResourceBudget,
    rss: Option<Arc<RssSampler>>,
}

fn over_budget(resource: &'static str, detail: String) -> PError {
    PError::BudgetExceeded { resource, detail }
}

impl Budget {
    /// `None` for a budget without a ceiling: the call runs as without one.
    fn new(budget: &GenerationBudget) -> Result<Option<Self>, PError> {
        let max_elapsed = match budget.max_seconds {
            None => None,
            Some(seconds) => Some(
                Duration::try_from_secs_f64(seconds)
                    .ok()
                    .filter(|limit| !limit.is_zero())
                    .ok_or_else(|| {
                        PError::Unsupported(format!(
                            "generation time budget {seconds} s: a finite positive number of seconds"
                        ))
                    })?,
            ),
        };
        if budget.max_rss_bytes == Some(0) {
            return Err(PError::Unsupported(
                "generation RSS budget 0 bytes: a positive number of bytes".to_owned(),
            ));
        }
        if max_elapsed.is_none() && budget.max_rss_bytes.is_none() {
            return Ok(None);
        }
        let limits = ResourceLimits {
            max_process_rss_bytes: budget.max_rss_bytes,
            max_elapsed,
        };
        let (resources, rss) = match budget.max_rss_bytes {
            Some(limit) => {
                let sampler = Arc::new(RssSampler {
                    started: Instant::now(),
                    limit,
                    read_at: AtomicU64::new(u64::MAX),
                    bytes: AtomicU64::new(0),
                    over: AtomicU64::new(0),
                });
                let monitor = Arc::clone(&sampler);
                let resources =
                    ResourceBudget::with_memory_monitor(limits, move || monitor.sample());
                (resources, Some(sampler))
            }
            None => (ResourceBudget::new(limits), None),
        };
        Ok(Some(Self { resources, rss }))
    }

    /// The budget at a boundary of the call.
    fn check(&self) -> Result<(), PError> {
        if let Some(rss) = &self.rss {
            let over = rss.over.load(Ordering::Acquire);
            if over > 0 {
                return Err(over_budget(
                    "rss",
                    format!(
                        "process RSS reached {over} bytes, the budget is {} bytes",
                        rss.limit
                    ),
                ));
            }
        }
        self.resources.checkpoint().map_err(|error| match error {
            ResourceLimitError::Elapsed { .. } => over_budget("time", error.to_string()),
            ResourceLimitError::Memory { .. } => over_budget("rss", error.to_string()),
            ResourceLimitError::MonitorUnavailable { .. } => {
                PError::Unsupported(format!("generation budget: {error}"))
            }
        })
    }

    /// A failed call under this budget: [`PError::BudgetExceeded`] when the
    /// budget is spent now, was seen exceeded, or the failure is a Symbolica
    /// resource refusal (a failed chart keeps only its message); otherwise the
    /// failure itself.
    fn attribute(&self, error: PError) -> PError {
        if matches!(error, PError::BudgetExceeded { .. }) {
            return error;
        }
        if let Err(spent) = self.check() {
            return spent;
        }
        let text = error.to_string();
        for (marker, resource) in [
            ("exceeds RAM budget", "rss"),
            ("reached the configured time budget", "time"),
        ] {
            if text.contains(marker) {
                return over_budget(resource, text);
            }
        }
        error
    }
}

/// [`generate_custom_owner`] with what it measured.
pub fn generate_custom_owner_with_report(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    match Budget::new(&options.budget)? {
        None => generate(presentation, options, None),
        Some(budget) => (budget.resources)
            .run(|| generate(presentation, options, Some(&budget)))
            .map_err(|error| budget.attribute(error)),
    }
}

fn generate(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
    budget: Option<&Budget>,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    let check = || budget.map_or(Ok(()), Budget::check);
    check()?;
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
    let lp_count = match options.lp_given {
        Some(recorded) => recorded,
        None => custom_owner_lp_count(presentation, support)?,
    };
    let lp_seconds = started.elapsed().as_secs_f64();
    check()?;

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
    check()?;
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
    // Every chart prepared once: a rule that does not restore through the
    // core's reader fails generation, not the owner's first step.
    build_custom_step_prepared(presentation, &bytes, &meta)?;
    check()?;
    let report = CustomOwnerReport {
        support,
        lp_count,
        lp_given: options.lp_given.is_some(),
        rules: summary.rules(),
        residual_keys: summary.residual_keys(),
        ordering: summary.ordering.clone(),
        charts: summary.charts,
        payload_bytes: bytes.len(),
        lp_seconds: if options.lp_given.is_some() {
            0.0
        } else {
            lp_seconds
        },
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
