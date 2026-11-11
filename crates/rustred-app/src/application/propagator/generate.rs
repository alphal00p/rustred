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
//! chart mask and the solver's complete message; by default the owner is not
//! retried another way (plan M2 sec.10, contingency C3 is the coordinator's).
//!
//! The opt-in L2 fall-through ([`CustomOwnerOptions::fallthrough`], plan M3
//! D-M3-4, M3-T13; AD sec.4.3) retries such an owner, and one over its budget,
//! on other orders of the same presentation: first on permuted slot orders
//! (the order program's support and coordinate priorities, lines among lines
//! and ISPs among ISPs, one deterministic seed each), then, for an owner with
//! eps axes, on the default order with the numerator degree before the eps
//! excess ([`FallthroughOrder::NumeratorBeforeEps`]; right after the default
//! order when that one was over budget), then under the default slot order
//! with alternative source orders ([`FALLTHROUGH_SOURCE_ORDERS`]).
//! The first attempt that closes wins; each attempt is a complete generation
//! under the guard and the budget, the LP count is taken once. Only a failed
//! chart or an exceeded budget is retried: a guard refusal, a failed LP count
//! or a payload that does not prepare fails at once. The payload of a winning
//! slot order records it in its order program (`integral_order`, with the
//! priorities); the report lists every attempt with its seed or source order
//! and refusal ([`CustomOwnerReport::fallthrough`]). It is the whole of the
//! fall-through in M3: no chart re-solve, no deeper search.
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
    CandidateDiscoveryStrategy, CandidateExactBackend, CandidateGenerationSession,
    CandidateRowFeature, CandidateRowPriority, CandidateSourcePriority, FamilyCandidatesRequest,
    FamilyCloseProgress, SectorFailurePolicy,
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
    /// Exact materialization backend for all generation attempts. Sparse by
    /// default; discovery, mathematical order and owner guards are unchanged.
    pub exact_backend: CandidateExactBackend,
    /// Time and RSS ceilings of the whole call; none by default.
    pub budget: GenerationBudget,
    /// The LP count of the support as RECORDED by an earlier generation of
    /// this owner (M3-T15 counts each owner once): the count is not taken
    /// again and the report says so ([`CustomOwnerReport::lp_given`]). For
    /// re-generation only, never a first count; the false-knock-out guard
    /// still checks it against the residuals.
    pub lp_given: Option<u32>,
    /// The opt-in L2 fall-through of a refused or over-budget owner (see the
    /// module documentation); `None` (the default) generates once.
    pub fallthrough: Option<CustomOwnerFallthrough>,
    /// The generation recipe of the first (default-order) attempt (M5-R0
    /// WP3): an integral order program in place of the default SpIRed uncut
    /// order, for example numerator degree before total excess, or a global
    /// numerator-degree row before the support (`pre_support_degree_rows`,
    /// under which no right-hand-side term of a rule can raise the numerator
    /// rank). Only for an owner without eps axes. `None` keeps the default.
    pub integral_order: Option<rustred::order::CompiledOrder>,
    /// The discovery strategy of the first attempt (source row priorities and
    /// the optional bounded rule portfolio with its quality keys, for example
    /// the fewest numerator-raising shifts); `None` keeps the request default
    /// (first valid).
    pub discovery: Option<CandidateDiscoveryStrategy>,
}

impl Default for CustomOwnerOptions {
    fn default() -> Self {
        Self {
            support: None,
            n_cores: 1,
            exact_backend: CandidateExactBackend::default(),
            budget: GenerationBudget::default(),
            lp_given: None,
            fallthrough: None,
            integral_order: None,
            discovery: None,
        }
    }
}

/// The L2 fall-through: the orders tried after a first attempt under the
/// default order fails on a chart or over budget (plan M3 D-M3-4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomOwnerFallthrough {
    /// One permuted slot order per seed, in this order; at least
    /// [`MIN_FALLTHROUGH_ORDERS`].
    pub seeds: Vec<u64>,
    /// How many of [`FALLTHROUGH_SOURCE_ORDERS`] are tried after the slot
    /// orders, in that order: from [`MIN_FALLTHROUGH_ORDERS`] to all.
    pub source_orders: usize,
}

/// The fewest permuted slot orders, and the fewest source orders, of a
/// fall-through (plan M3 D-M3-4; AD sec.4.3).
pub const MIN_FALLTHROUGH_ORDERS: usize = 3;

/// The alternative source orders of AD sec.4.3 (the portfolio alternatives
/// of the closure study, `study/patches/pint_anatomy.rs:75-80`): row features
/// in priority order, `asc`ending or `desc`ending, shift weights all one.
pub const FALLTHROUGH_SOURCE_ORDERS: [&str; 6] = [
    "terms asc, coefficient-monomials asc",
    "negative-shifts asc, terms asc",
    "positive-shifts asc, terms asc",
    "terms desc",
    "absolute-shifts desc, coefficient-monomials asc",
    "coefficient-monomials desc",
];

impl Default for CustomOwnerFallthrough {
    /// Seeds 1, 2, 3 and the first three source orders.
    fn default() -> Self {
        Self {
            seeds: vec![1, 2, 3],
            source_orders: MIN_FALLTHROUGH_ORDERS,
        }
    }
}

/// The order of one generation attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FallthroughOrder {
    /// The default order: the first attempt.
    Default,
    /// The default order program with these support and coordinate
    /// priorities (slots in comparison order), drawn from `seed`.
    Slots { seed: u64, priority: Vec<usize> },
    /// For an owner with eps axes: the default order program with its
    /// degree rows total excess, numerator degree, eps excess (the numerator
    /// row before the eps row; `eps_third` of the KO table dumps,
    /// `rules_study/s5-tables/FORMAT.md`). On the 4L eps top chart of
    /// T082p3 that order closed in 28 s where the default one had not
    /// closed after 5.5 h (Astra, coordinator note 2026-11-09 12:29Z).
    NumeratorBeforeEps,
    /// The default slot order with source order `index` of
    /// [`FALLTHROUGH_SOURCE_ORDERS`].
    Source { index: usize },
}

impl std::fmt::Display for FallthroughOrder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(formatter, "the default order"),
            Self::Slots { seed, priority } => {
                let priority: Vec<String> = priority.iter().map(usize::to_string).collect();
                write!(
                    formatter,
                    "slot order of seed {seed} (priority {})",
                    priority.join(",")
                )
            }
            Self::NumeratorBeforeEps => {
                write!(formatter, "numerator degree before eps excess (eps_third)")
            }
            Self::Source { index } => write!(
                formatter,
                "source order {index} ({})",
                FALLTHROUGH_SOURCE_ORDERS.get(*index).unwrap_or(&"unknown")
            ),
        }
    }
}

/// One attempt of a fall-through.
#[derive(Clone, Debug, PartialEq)]
pub struct FallthroughAttempt {
    pub order: FallthroughOrder,
    pub seconds: f64,
    /// Why it failed (its complete error), `None` for the winning attempt.
    pub refusal: Option<String>,
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
    /// Under [`CustomOwnerOptions::fallthrough`]: every attempt in order, the
    /// last one the winner (the first alone when it closed).
    pub fallthrough: Option<Vec<FallthroughAttempt>>,
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
    generate_custom_owner_observed(presentation, options, None)
}

/// Generate once through the ordinary owner path, observing native progress
/// before bounded session retention. Callbacks may overlap on chart workers;
/// diagnostics do not change the generated payload or grant rule authority.
pub fn generate_custom_owner_with_report_and_progress(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
    progress: impl Fn(&FamilyCloseProgress) + Send + Sync,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    generate_custom_owner_observed(presentation, options, Some(&progress))
}

type OwnerProgress<'a> = Option<&'a (dyn Fn(&FamilyCloseProgress) + Send + Sync)>;

fn generate_custom_owner_observed(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
    progress: OwnerProgress<'_>,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    match &options.fallthrough {
        None => attempt(
            presentation,
            options,
            &FallthroughOrder::Default,
            &mut AttemptTrace::default(),
            progress,
        ),
        Some(fallthrough) => {
            generate_with_fallthrough(presentation, options, fallthrough, progress)
        }
    }
}

/// One attempt of the fall-through by hand: the owner generated once on
/// `order` (one of [`fallthrough_orders`]), without a fall-through.
pub fn generate_custom_owner_on_order(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
    order: &FallthroughOrder,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    let options = CustomOwnerOptions {
        fallthrough: None,
        ..options.clone()
    };
    attempt(
        presentation,
        &options,
        order,
        &mut AttemptTrace::default(),
        None,
    )
}

/// One generation under the budget of `options` and `order`.
fn attempt(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
    order: &FallthroughOrder,
    trace: &mut AttemptTrace,
    progress: OwnerProgress<'_>,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    match Budget::new(&options.budget)? {
        None => generate(presentation, options, None, order, trace, progress),
        Some(budget) => (budget.resources)
            .run(|| generate(presentation, options, Some(&budget), order, trace, progress))
            .map_err(|error| budget.attribute(error)),
    }
}

/// What a failed attempt got: its LP count with the seconds it took, and
/// whether the solve refused a chart.
#[derive(Default)]
struct AttemptTrace {
    lp: Option<(u32, f64)>,
    refused_charts: bool,
}

/// The orders of `fallthrough` on `presentation`, the default order first,
/// then the slot orders, [`FallthroughOrder::NumeratorBeforeEps`] for an
/// owner with eps axes, the source orders; a seed repeating an order already
/// listed is dropped. When the default order runs over its budget, the
/// fall-through tries [`FallthroughOrder::NumeratorBeforeEps`] next
/// ([`fallthrough_retries`]).
pub fn fallthrough_orders(
    presentation: &Presentation,
    fallthrough: &CustomOwnerFallthrough,
) -> Result<Vec<FallthroughOrder>, PError> {
    let sources = fallthrough.source_orders;
    if !(MIN_FALLTHROUGH_ORDERS..=FALLTHROUGH_SOURCE_ORDERS.len()).contains(&sources) {
        return Err(PError::Unsupported(format!(
            "fall-through with {sources} source orders: from {MIN_FALLTHROUGH_ORDERS} to {}",
            FALLTHROUGH_SOURCE_ORDERS.len()
        )));
    }
    let arity = presentation.slots.len();
    let mut priorities = vec![(0..arity).collect::<Vec<_>>()];
    let mut orders = vec![FallthroughOrder::Default];
    for &seed in &fallthrough.seeds {
        let priority = slot_priority(presentation, seed);
        if !priorities.contains(&priority) {
            priorities.push(priority.clone());
            orders.push(FallthroughOrder::Slots { seed, priority });
        }
    }
    if orders.len() <= MIN_FALLTHROUGH_ORDERS {
        return Err(PError::Unsupported(format!(
            "fall-through seeds {:?} give {} slot orders of presentation {} other than the default, at least {MIN_FALLTHROUGH_ORDERS} are needed",
            fallthrough.seeds,
            orders.len() - 1,
            presentation.id.0
        )));
    }
    if presentation.forced_axes() != 0 {
        orders.push(FallthroughOrder::NumeratorBeforeEps);
    }
    orders.extend((0..sources).map(|index| FallthroughOrder::Source { index }));
    Ok(orders)
}

/// The orders a fall-through tries after its default order failed, given
/// `orders` of [`fallthrough_orders`]: as listed, except that
/// [`FallthroughOrder::NumeratorBeforeEps`] comes first when the default
/// order ran `over_budget` (the order that closed a 4L eps top chart the
/// default order had not closed in hours; on a refused chart the slot orders
/// come first, as they closed T075c).
pub fn fallthrough_retries(
    orders: &[FallthroughOrder],
    over_budget: bool,
) -> Vec<FallthroughOrder> {
    let mut retries: Vec<FallthroughOrder> = (orders.iter())
        .filter(|order| **order != FallthroughOrder::Default)
        .cloned()
        .collect();
    let eps = (retries.iter()).position(|order| *order == FallthroughOrder::NumeratorBeforeEps);
    if let (true, Some(eps)) = (over_budget, eps) {
        let eps = retries.remove(eps);
        retries.insert(0, eps);
    }
    retries
}

/// The support and coordinate priorities of `seed`: the line slots shuffled,
/// then the ISP slots shuffled (Fisher-Yates on SplitMix64), so that lines
/// still compare before ISPs, as in a permuted presentation.
fn slot_priority(presentation: &Presentation, seed: u64) -> Vec<usize> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let (mut lines, mut isps): (Vec<usize>, Vec<usize>) = (0..presentation.slots.len())
        .partition(|&slot| matches!(presentation.slots[slot], Slot::Line(_)));
    for group in [&mut lines, &mut isps] {
        for i in (1..group.len()).rev() {
            let j = (next() % (i as u64 + 1)) as usize;
            group.swap(i, j);
        }
    }
    lines.into_iter().chain(isps).collect()
}

/// The default order program of `presentation` (with its eps axes
/// two-sided) under the support and coordinate priorities `priority`.
fn slot_order(
    presentation: &Presentation,
    priority: &[usize],
) -> Result<rustred::order::CompiledOrder, PError> {
    use rustred::order::{CompiledOrder, Limits, OrderDescriptor};
    let arity = presentation.slots.len();
    let forced = presentation.forced_axes();
    let mut descriptor = if forced == 0 {
        OrderDescriptor::spired_uncut(arity)
    } else {
        OrderDescriptor::spired_uncut_two_sided(bits(forced, arity))
    };
    descriptor.support_priority = priority.to_vec();
    descriptor.coordinate_priority = priority.to_vec();
    CompiledOrder::compile(descriptor, Limits::default()).map_err(|error| {
        PError::Unsupported(format!("fall-through slot order {priority:?}: {error}"))
    })
}

/// The default two-sided order program of `presentation` with the numerator
/// row before the eps row ([`FallthroughOrder::NumeratorBeforeEps`]).
fn numerator_before_eps(
    presentation: &Presentation,
) -> Result<rustred::order::CompiledOrder, PError> {
    use rustred::order::{CompiledOrder, Limits, OrderDescriptor};
    let forced = presentation.forced_axes();
    if forced == 0 {
        return Err(PError::Unsupported(format!(
            "fall-through order numerator before eps: presentation {} has no eps axis",
            presentation.id.0
        )));
    }
    let mut descriptor =
        OrderDescriptor::spired_uncut_two_sided(bits(forced, presentation.slots.len()));
    // Rows total excess, eps excess, numerator degree become total excess,
    // numerator degree, eps excess.
    descriptor.degree_rows.swap(1, 2);
    CompiledOrder::compile(descriptor, Limits::default()).map_err(|error| {
        PError::Unsupported(format!("fall-through order numerator before eps: {error}"))
    })
}

/// Source order `index` of [`FALLTHROUGH_SOURCE_ORDERS`] on `arity` slots.
fn source_order(index: usize, arity: usize) -> CandidateDiscoveryStrategy {
    use CandidateRowFeature::{
        AbsoluteShifts, CoefficientMonomials, NegativeShifts, PositiveShifts, Terms,
    };
    let row = |feature, descending| CandidateRowPriority {
        feature,
        descending,
    };
    let ones = || vec![1; arity];
    let priorities = match index {
        0 => vec![row(Terms, false), row(CoefficientMonomials, false)],
        1 => vec![
            row(NegativeShifts { weights: ones() }, false),
            row(Terms, false),
        ],
        2 => vec![
            row(PositiveShifts { weights: ones() }, false),
            row(Terms, false),
        ],
        3 => vec![row(Terms, true)],
        4 => vec![
            row(AbsoluteShifts { weights: ones() }, true),
            row(CoefficientMonomials, false),
        ],
        _ => vec![row(CoefficientMonomials, true)],
    };
    CandidateDiscoveryStrategy {
        rows: CandidateSourcePriority::Features { priorities },
        ..CandidateDiscoveryStrategy::default()
    }
}

/// The L2 fall-through (module documentation): the orders of `fallthrough`
/// in turn, the first that closes wins. The LP count of the first attempt
/// serves every later one.
fn generate_with_fallthrough(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
    fallthrough: &CustomOwnerFallthrough,
    progress: OwnerProgress<'_>,
) -> Result<(CustomOwnerPayload, CustomOwnerReport), PError> {
    let started = Instant::now();
    let mut orders = fallthrough_orders(presentation, fallthrough)?;
    let mut options = CustomOwnerOptions {
        fallthrough: None,
        ..options.clone()
    };
    let given = options.lp_given.is_some();
    let mut counted = None;
    let mut attempts = Vec::with_capacity(orders.len());
    let mut next = 0;
    while let Some(order) = orders.get(next).cloned() {
        next += 1;
        let mut trace = AttemptTrace::default();
        let begun = Instant::now();
        let outcome = attempt(presentation, &options, &order, &mut trace, progress);
        let seconds = begun.elapsed().as_secs_f64();
        match outcome {
            Ok((payload, mut report)) => {
                attempts.push(FallthroughAttempt {
                    order,
                    seconds,
                    refusal: None,
                });
                if let Some(lp_seconds) = counted {
                    report.lp_given = given;
                    report.lp_seconds = lp_seconds;
                }
                report.total_seconds = started.elapsed().as_secs_f64();
                report.fallthrough = Some(attempts);
                return Ok((payload, report));
            }
            Err(error) => {
                let over_budget = matches!(error, PError::BudgetExceeded { .. });
                let Some((count, lp_seconds)) = trace.lp else {
                    return Err(error);
                };
                if !(trace.refused_charts || over_budget) {
                    return Err(error);
                }
                if options.lp_given.is_none() {
                    options.lp_given = Some(count);
                    counted = Some(lp_seconds);
                }
                if order == FallthroughOrder::Default {
                    let retries = fallthrough_retries(&orders, over_budget);
                    orders.truncate(next);
                    orders.extend(retries);
                }
                attempts.push(FallthroughAttempt {
                    order,
                    seconds,
                    refusal: Some(error.to_string()),
                });
            }
        }
    }
    let tried: Vec<String> = (attempts.iter().enumerate())
        .map(|(index, attempt)| {
            format!(
                "attempt {index}, {}, {:.1} s: {}",
                attempt.order,
                attempt.seconds,
                attempt.refusal.as_deref().unwrap_or("closed")
            )
        })
        .collect();
    Err(PError::Generation(format!(
        "custom owner of presentation {}: refused on all {} orders of the L2 fall-through (no chart re-solve, no deeper search): {}",
        presentation.id.0,
        attempts.len(),
        tried.join(" | ")
    )))
}

fn generate(
    presentation: &Presentation,
    options: &CustomOwnerOptions,
    budget: Option<&Budget>,
    order: &FallthroughOrder,
    trace: &mut AttemptTrace,
    progress: OwnerProgress<'_>,
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
    trace.lp = Some((lp_count, lp_seconds));
    check()?;

    let mut request = FamilyCandidatesRequest::new("custom owner");
    request.exact_backend = options.exact_backend;
    match order {
        FallthroughOrder::Default => {
            if let Some(program) = &options.integral_order {
                if forced != 0 {
                    return Err(PError::Unsupported(format!(
                        "custom owner of presentation {}: an integral order recipe is only for an owner without eps axes",
                        presentation.id.0
                    )));
                }
                request.integral_order = Some(program.clone());
            }
            request.discovery_strategy = options.discovery.clone();
        }
        FallthroughOrder::Slots { priority, .. } => {
            request.integral_order = Some(slot_order(presentation, priority)?);
        }
        FallthroughOrder::NumeratorBeforeEps => {
            request.integral_order = Some(numerator_before_eps(presentation)?);
        }
        FallthroughOrder::Source { index } => {
            request.discovery_strategy = Some(source_order(*index, arity));
        }
    }
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
        match progress {
            Some(progress) => job.run_with_progress(progress),
            None => job.run(),
        }
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
        trace.refused_charts = true;
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
        fallthrough: None,
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
    /// The node's normalised, canonically labelled graph.
    pub graph: rustred::propagator::topology::PTopo,
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
                graph: node.topo.clone(),
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
