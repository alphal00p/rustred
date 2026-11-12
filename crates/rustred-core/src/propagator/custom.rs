//! Custom owners: `CustomStep<M>` over generated candidate programs (owner: M2-T7c rr-custom).
//!
//! A Custom owner is one owner support of a presentation whose rules SpIRed
//! generated all-rank (no numerator cap): NO (`nostar0`, integer) and the eps
//! kite `t1star05` at `L <= 3`. Its payload is the generated candidate bundle
//! of exactly the raw charts of that support (plan M2 sec.3.4): one chart for
//! an integer owner, all `2^j` charts of the `j` eps axes for an eps owner,
//! whose order program must carry two-sided data on every eps axis.
//!
//! The application crate generates candidate bundles; this module reads them
//! with the bundle's one decoder ([`super::bundle_wire`], shared with the
//! application's codec), so that the library can build a Custom owner from
//! bytes alone ([`build_custom_step`], used by assembly and the loader through
//! `programs::lane_program`). The bundle's family must be the presentation's
//! family. Nothing goes through the bundle loader's admission: it refuses eps
//! families by design, and the p-only constructor
//! (`CandidateOwnerContext::try_new_p`) admits them.
//!
//! The hard false-knock-out guard ([`check_custom_owner`]) runs at generation
//! (`rustred_app::application::propagator::generate_custom_owner`) and again on
//! every build: `lp_count >= 1` with no residual is refused, an eps owner needs
//! two-sided axes exactly on its eps axes and every raw chart, and the meta must
//! describe the payload. The residuals of an eps owner sit at `n = 1` on every
//! eps axis (plan M2 sec.3.4, Forcer's master position).
//!
//! [`CustomStep`] runs one rule of its owner per step through the candidate
//! owner step (`solver::owner_step::apply_prepared_owner`): children of the
//! owner support are Apply keys of the same owner, any other child is a Route
//! key of the presentation (a subsector, resolved by the census), and a
//! residual is the owner's terminal. Coefficients are exact elements of
//! `Q(d)`, the library's context; the Ordered engine carries them under any
//! weight arithmetic (exact or `Z_p(d)`), there is no other path. Charts are
//! prepared on first use ([`build_custom_step`]) or all at build
//! ([`build_custom_step_prepared`], used at generation).
//!
//! [`custom_payload_rules`] lists a payload's rules with their coefficients as
//! text, and [`custom_payload_dump`] writes them as one canonical text that
//! does not depend on the process symbol table (unlike the payload bytes).
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::{IntegralFamilyLimits, IntegralKey};
use crate::persistence::{
    BinaryIoLimits, CoefficientId, DecodedCoefficientTable, NativeFamilyRecord, ProgramEnvelope,
    SectionTag,
};
use crate::reduction::{ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::sector::{Mask, OrderingPolicy, zero};
use crate::solver::owner_step::{AppliedOwnerStep, apply_prepared_owner, owner_ordering};
use crate::solver::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
    CandidateReductionError, DeferredCandidateOwnerDescriptor, DeferredCandidateOwnerLoader,
    DeferredCandidateOwnerPrograms, FiniteCasePolicy,
};

use super::bundle_wire::{
    BundleWireError, CANDIDATE_BUNDLE_SCHEMA, CANDIDATE_BUNDLE_STATUS,
    DISPATCH_CANDIDATE_BUNDLE_SCHEMA, IntegralRecord, ProgramRecord, Restore, RuleRecord,
    StoredProgram, candidate_sections, decode_family_record, decode_program_record,
    import_coefficients, storage_variables, validate_dispatch, validate_sectors,
};
use super::error::PError;
use super::lane::OwnerLane;
use super::library::{CustomOwnerMeta, PTerminal};
use super::presentation::{Presentation, PresentationKind};
use super::step::{PChild, PKeyView, PLaneStep, POrderKey, PStage, PStepContext, PStepOut};
use super::{OwnerClassId, PTerminalId, PresentationId};

/// Bundle schemas a payload may carry: the generated candidate bundle without
/// and with a rule-dispatch table, as [`super::bundle_wire`] defines them (the
/// application's codec writes them). Legacy v1 bundles have no order program.
pub const CUSTOM_PAYLOAD_SCHEMAS: [&str; 2] =
    [CANDIDATE_BUNDLE_SCHEMA, DISPATCH_CANDIDATE_BUNDLE_SCHEMA];
/// First line of [`custom_payload_dump`]; bump it when the dump's text changes.
pub const CUSTOM_PAYLOAD_DUMP_VERSION: &str = "rustred.p-custom-payload-dump.v1";
/// Unrestricted solver policies: the default numerical depth, or an explicit
/// one, without a numerator cap or finite retention.
const DEFAULT_SOLVER_POLICY: &str = "ordinary-source-port-default-v1";
const DEPTH_POLICY: (&str, &str) = ("ordinary-source-port-numerical-depth-", "-v1");
/// Structural allocation allowance of one payload's records.
const MAX_RECORD_BYTES: usize = 1 << 30;

fn payload_error(detail: impl fmt::Display) -> PError {
    PError::MalformedLibrary(format!("custom payload: {detail}"))
}

/// A refused payload record; a resource refusal stays an engine resource error.
fn wire_error(error: BundleWireError) -> PError {
    match error.resource_limit_error() {
        Some(resource) => PError::Engine(resource.clone().into()),
        None => payload_error(error),
    }
}

fn mask(bits: &[bool]) -> u64 {
    bits.iter()
        .enumerate()
        .filter(|&(_, &active)| active)
        .fold(0, |mask, (slot, _)| mask | 1 << slot)
}

fn bits(mask: u64, arity: usize) -> Vec<bool> {
    (0..arity).map(|slot| mask >> slot & 1 == 1).collect()
}

/// The structure of a payload, checked before any coefficient is read.
struct Structure<'a> {
    envelope: ProgramEnvelope<'a>,
    family: NativeFamilyRecord,
    program: ProgramRecord,
    ordering: OrderingPolicy,
}

fn read_structure(payload: &[u8]) -> Result<Structure<'_>, PError> {
    let envelope = candidate_sections(payload, BinaryIoLimits::default()).map_err(wire_error)?;
    let section = |tag| envelope.section(tag).expect("checked section");
    let program = match decode_program_record(section(SectionTag::PROGRAM), MAX_RECORD_BYTES)
        .map_err(wire_error)?
    {
        StoredProgram::Current(program) => program,
        StoredProgram::Legacy(program) => {
            return Err(payload_error(format!(
                "schema {:?}, expected one of {CUSTOM_PAYLOAD_SCHEMAS:?}",
                program.schema
            )));
        }
    };
    let family =
        decode_family_record(section(SectionTag::FAMILY), MAX_RECORD_BYTES).map_err(wire_error)?;
    let ordering = validate(&program)?;
    if family.arity() != program.root_sector.len() {
        return Err(payload_error("family and root arities differ"));
    }
    Ok(Structure {
        envelope,
        family,
        program,
        ordering,
    })
}

/// Unrestricted search: no numerator cap, no finite retention.
fn unrestricted(policy: &str) -> bool {
    let (prefix, suffix) = DEPTH_POLICY;
    policy == DEFAULT_SOLVER_POLICY
        || policy
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .is_some_and(|depth| !depth.is_empty() && depth.bytes().all(|b| b.is_ascii_digit()))
}

/// The Custom owner's checks of a program (all-rank, an order, at least one
/// chart) on top of the shape checks every bundle reader applies; returns its
/// mathematical order.
fn validate(program: &ProgramRecord) -> Result<OrderingPolicy, PError> {
    if program.status != CANDIDATE_BUNDLE_STATUS {
        return Err(payload_error(format!("status {:?}", program.status)));
    }
    if !unrestricted(&program.solver_policy) {
        return Err(PError::Unsupported(format!(
            "custom payload: solver policy {:?} is not all-rank",
            program.solver_policy
        )));
    }
    if program.permutation.is_some() {
        return Err(payload_error("a legacy permutation instead of an order"));
    }
    let arity = program.root_sector.len();
    if arity == 0 || arity > 64 {
        return Err(payload_error(format!("arity {arity}")));
    }
    let ordering =
        OrderingPolicy::try_from_stable_id(&program.integral_order).map_err(payload_error)?;
    match ordering.program() {
        Some(order) if order.arity() != arity => {
            return Err(payload_error("the order program has another arity"));
        }
        None if ordering != OrderingPolicy::SpiredUncutV1 => {
            return Err(payload_error(
                "an order that is neither SpIRed nor a program",
            ));
        }
        _ => {}
    }
    // An imported table's payload descends in the table's own slot order.
    super::table_import::enforce_slot_order(
        &program.family_source,
        &program.integral_order,
        arity,
    )?;
    validate_dispatch(program).map_err(wire_error)?;
    validate_sectors(program, |_| Ok::<_, BundleWireError>(())).map_err(wire_error)?;
    if program.sectors.is_empty() {
        return Err(payload_error("no chart"));
    }
    Ok(ordering)
}

// ----- summary and meta -----

/// One solved raw chart of a payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomChartSummary {
    /// Slots of positive power.
    pub chart: u64,
    pub rules: usize,
    pub residuals: Vec<Vec<i64>>,
}

/// What a payload holds, read from its structure alone (no coefficient).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomPayloadSummary {
    pub schema: String,
    pub family_fingerprint: String,
    pub arity: usize,
    /// The owner support: the generation root.
    pub support: u64,
    /// Two-sided axes of the order program (none without a program).
    pub two_sided_axes: u64,
    /// Stable id of the generation order, the runtime descent order.
    pub ordering: String,
    pub charts: Vec<CustomChartSummary>,
}

impl CustomPayloadSummary {
    pub fn rules(&self) -> usize {
        self.charts.iter().map(|chart| chart.rules).sum()
    }

    /// Every residual key, chart by chart.
    pub fn residual_keys(&self) -> Vec<Vec<i64>> {
        let residuals = self.charts.iter().flat_map(|chart| &chart.residuals);
        residuals.cloned().collect()
    }

    /// The owner record of this payload under the LP count `lp_count`.
    pub fn meta(&self, lp_count: u32) -> Result<CustomOwnerMeta, PError> {
        let residuals = u32::try_from(self.residual_keys().len())
            .map_err(|_| payload_error("too many residuals"))?;
        let mut charts: Vec<u64> = self.charts.iter().map(|chart| chart.chart).collect();
        charts.sort_unstable();
        Ok(CustomOwnerMeta {
            lp_count,
            residuals,
            two_sided_axes: self.two_sided_axes,
            charts,
        })
    }
}

/// Read the structure of a payload: charts, rule counts, residual keys, order.
/// Nothing is imported into Symbolica.
pub fn inspect_custom_payload(payload: &[u8]) -> Result<CustomPayloadSummary, PError> {
    let structure = read_structure(payload)?;
    let program = &structure.program;
    let arity = program.root_sector.len();
    let two_sided_axes = structure
        .ordering
        .program()
        .map_or(0, |order| mask(order.two_sided()));
    Ok(CustomPayloadSummary {
        schema: program.schema.clone(),
        family_fingerprint: program.family_fingerprint.clone(),
        arity,
        support: mask(&program.root_sector),
        two_sided_axes,
        ordering: program.integral_order.clone(),
        charts: program
            .sectors
            .iter()
            .map(|sector| CustomChartSummary {
                chart: mask(&sector.sector),
                rules: sector.rules.len(),
                residuals: sector
                    .finite_residuals
                    .iter()
                    .map(|residual| residual.values.iter().map(|&v| i64::from(v)).collect())
                    .collect(),
            })
            .collect(),
    })
}

/// The owner support of a Custom presentation: the sector of an induced
/// presentation, every line otherwise.
pub fn custom_owner_support(presentation: &Presentation) -> u64 {
    match presentation.kind {
        PresentationKind::Induced { mask, .. } => mask,
        _ => presentation
            .lines()
            .fold(0, |mask, (slot, _)| mask | 1 << slot),
    }
}

/// Every raw chart of `support`: its slots outside `forced` active, each
/// forced axis either active (`n >= 1`) or not (`n <= 0`); ascending.
pub fn custom_owner_charts(support: u64, forced: u64) -> Vec<u64> {
    let free = support & !forced;
    let axes: Vec<u32> = (0..64).filter(|&slot| forced >> slot & 1 == 1).collect();
    let mut charts: Vec<u64> = (0..1_u64 << axes.len())
        .map(|bits| {
            axes.iter()
                .enumerate()
                .filter(|&(bit, _)| bits >> bit & 1 == 1)
                .fold(free, |chart, (_, &axis)| chart | 1 << axis)
        })
        .collect();
    charts.sort_unstable();
    charts
}

/// The hard false-knock-out guard and the chart rule of plan M2 sec.3.4 on an
/// owner record of `presentation`:
/// - `lp_count >= 1` with no residual is a false knock-out
///   ([`PError::Generation`]);
/// - an eps owner without two-sided axes is [`PError::Unsupported`]; its
///   two-sided axes must be exactly its eps (forced) axes, and an integer
///   owner has none;
/// - the charts are every raw chart of the owner support, and lie on the
///   presentation's lines.
pub fn check_custom_owner(
    presentation: &Presentation,
    meta: &CustomOwnerMeta,
) -> Result<(), PError> {
    if meta.lp_count >= 1 && meta.residuals == 0 {
        return Err(PError::Generation(format!(
            "false knock-out guard: LP={}, residuals=0",
            meta.lp_count
        )));
    }
    let forced = presentation.forced_axes();
    if forced != 0 && meta.two_sided_axes == 0 {
        return Err(PError::Unsupported(
            "eps custom owner requires two-sided axes".to_owned(),
        ));
    }
    if meta.two_sided_axes != forced {
        return Err(PError::Generation(format!(
            "two-sided axes {:#x} differ from the eps axes {forced:#x} of presentation {}",
            meta.two_sided_axes, presentation.id.0
        )));
    }
    let lines = presentation
        .lines()
        .fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
    let support = meta
        .charts
        .iter()
        .fold(forced, |support, chart| support | chart);
    if meta.charts.is_empty() || support & !lines != 0 {
        return Err(PError::Generation(format!(
            "custom owner charts {:x?} on the lines {lines:#x} of presentation {}",
            meta.charts, presentation.id.0
        )));
    }
    let expected = custom_owner_charts(support, forced);
    if meta.charts != expected {
        return Err(PError::Generation(format!(
            "custom owner charts {:x?}, every raw chart of support {support:#x} is {expected:x?}",
            meta.charts
        )));
    }
    Ok(())
}

// ----- the decoded program -----

/// A payload bound to its presentation's family, with its coefficients.
struct Decoded {
    program: ProgramRecord,
    ordering: OrderingPolicy,
    table: DecodedCoefficientTable,
}

impl fmt::Debug for Decoded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Decoded")
            .field("charts", &self.program.sectors.len())
            .field("ordering", &self.program.integral_order)
            .field("coefficients", &self.table.len())
            .finish()
    }
}

/// Decode a payload with its coefficients, bound to `presentation`'s family
/// and arity; without a presentation, to the family the payload records.
fn decode(presentation: Option<&Presentation>, payload: &[u8]) -> Result<Decoded, PError> {
    let Structure {
        envelope,
        family,
        program,
        ordering,
    } = read_structure(payload)?;
    if let Some(presentation) = presentation {
        let arity = presentation.slots.len();
        if program.root_sector.len() != arity {
            return Err(payload_error(format!(
                "arity {}, presentation {} has {arity} slots",
                program.root_sector.len(),
                presentation.id.0
            )));
        }
    }
    let table =
        import_coefficients(&envelope, &program, BinaryIoLimits::default()).map_err(wire_error)?;
    let rebuilt = family
        .to_family(
            &table,
            IntegralFamilyLimits::default(),
            BinaryIoLimits::default(),
        )
        .map_err(payload_error)?;
    let fingerprint = presentation.map_or_else(
        || program.family_fingerprint.clone(),
        |presentation| presentation.family.fingerprint().to_owned(),
    );
    if rebuilt.fingerprint() != fingerprint || program.family_fingerprint != fingerprint {
        return Err(payload_error(format!(
            "generated for family {}, the family is {fingerprint}{}",
            program.family_fingerprint,
            presentation.map_or_else(String::new, |presentation| format!(
                " (presentation {})",
                presentation.id.0
            ))
        )));
    }
    Ok(Decoded {
        program,
        ordering,
        table,
    })
}

fn invalid(detail: impl fmt::Display) -> CandidateReductionError {
    CandidateReductionError::InvalidInput(format!("custom payload: {detail}"))
}

/// A chart that does not restore; a resource refusal stays one.
fn restore_error(error: BundleWireError) -> CandidateReductionError {
    match error.resource_limit_error() {
        Some(resource) => ReductionError::from(resource.clone()).into(),
        None => invalid(error),
    }
}

impl Decoded {
    /// The saved solution of `chart`, in `context`'s variables.
    fn input<const N: usize>(
        &self,
        chart: [bool; N],
        context: &CandidateOwnerContext<N>,
    ) -> Result<CandidateOwnerInput<N>, CandidateReductionError> {
        let program = &self.program;
        let ordinal = (program.sectors.iter())
            .position(|sector| crate::storage_array(&sector.sector, false) == Some(chart))
            .ok_or_else(|| invalid("no chart of this sector"))?;
        let root: [bool; N] =
            crate::storage_array(&program.root_sector, false).ok_or_else(|| invalid("arity"))?;
        let coefficients = context.coefficient_context();
        let variables = storage_variables::<N>(coefficients);
        let restore = Restore {
            table: &self.table,
            variables: &variables,
            indices: context.index_variables(),
        };
        let (_, solution) = restore
            .sector(
                program,
                ordinal,
                &self.ordering,
                coefficients.index_count(),
                None,
                FiniteCasePolicy::SearchFinite,
            )
            .map_err(restore_error)?;
        Ok(CandidateOwnerInput {
            sector: chart,
            saved_root: root,
            ordering: self.ordering.clone(),
            solution,
        })
    }
}

// ----- the rules as data and the canonical dump -----

/// One slot of a payload rule's integral.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomPower {
    /// The rule's free index of this slot plus the offset.
    Symbolic(i64),
    /// A fixed power.
    Fixed(i64),
}

/// One generated rule of a payload, as stored: `target = Sum rhs` on the
/// rule's case, coefficients as text (rational functions of `d` and the free
/// indices `n<slot>`, Symbolica's canonical printing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomRule {
    /// The raw chart of the rule (slots of positive power).
    pub chart: u64,
    /// Position of the rule in its chart: the application order.
    pub ordinal: usize,
    /// An affine case (its face plus polynomial equations) or a coordinate face.
    pub affine: bool,
    /// The face: fixed axes and their values, ascending by axis.
    pub fixed: Vec<(usize, i64)>,
    /// The equations of an affine case.
    pub equations: Vec<String>,
    pub target: Vec<CustomPower>,
    pub rhs: Vec<(Vec<CustomPower>, String)>,
    /// Exceptional branches, each a list of polynomial conditions.
    pub exclusions: Vec<Vec<String>>,
    /// Dispatched after the baseline partition (a schema v3 dispatch entry).
    pub dispatched: bool,
    /// Provenance: the seed rows (IBP row x seed) the rule was solved from.
    pub sources: usize,
}

fn custom_powers(record: &IntegralRecord) -> Vec<CustomPower> {
    (record.symbolic.iter().zip(&record.values))
        .map(|(&symbolic, &value)| {
            if symbolic {
                CustomPower::Symbolic(i64::from(value))
            } else {
                CustomPower::Fixed(i64::from(value))
            }
        })
        .collect()
}

/// `(n0+1, 1, n2, 0)`: `n<slot>` is the free index of that slot, the name of
/// its variable in the coefficients (`n0` is slot 0).
fn powers_text(powers: &[CustomPower]) -> String {
    let slots: Vec<String> = (powers.iter().enumerate())
        .map(|(slot, power)| match *power {
            CustomPower::Symbolic(0) => format!("n{slot}"),
            CustomPower::Symbolic(offset) => format!("n{slot}{offset:+}"),
            CustomPower::Fixed(value) => value.to_string(),
        })
        .collect();
    format!("({})", slots.join(", "))
}

impl Decoded {
    fn text(&self, id: u32) -> Result<String, PError> {
        let id = CoefficientId::try_from_index(id as usize).map_err(payload_error)?;
        Ok(self
            .table
            .coefficient(id)
            .map_err(payload_error)?
            .to_string())
    }
}

/// Every rule of a payload, chart by chart in the payload's order, with its
/// coefficients. The payload is bound to the family it records.
pub fn custom_payload_rules(payload: &[u8]) -> Result<Vec<CustomRule>, PError> {
    let decoded = decode(None, payload)?;
    let program = &decoded.program;
    let texts = |ids: &[u32]| -> Result<Vec<String>, PError> {
        ids.iter().map(|&id| decoded.text(id)).collect()
    };
    let mut rules = Vec::new();
    for (sector_ordinal, sector) in program.sectors.iter().enumerate() {
        for (ordinal, rule) in sector.rules.iter().enumerate() {
            let case = &rule.case;
            let rhs = (rule.rhs.iter())
                .map(|term| {
                    Ok((
                        custom_powers(&term.integral),
                        decoded.text(term.coefficient)?,
                    ))
                })
                .collect::<Result<_, PError>>()?;
            rules.push(CustomRule {
                chart: mask(&sector.sector),
                ordinal,
                affine: case.kind == "affine",
                fixed: (case.fixed_axes.iter().zip(&case.fixed_values))
                    .map(|(&axis, &value)| (axis, i64::from(value)))
                    .collect(),
                equations: texts(&case.equations)?,
                target: custom_powers(&rule.target),
                rhs,
                exclusions: (rule.exclusions.iter())
                    .map(|branch| texts(branch))
                    .collect::<Result<_, _>>()?,
                dispatched: program
                    .rule_dispatch
                    .binary_search_by_key(&(sector_ordinal, ordinal), |entry| {
                        (entry.sector, entry.rule)
                    })
                    .is_ok(),
                sources: rule.sources.len(),
            });
        }
    }
    Ok(rules)
}

/// The decoded coefficient table of a payload and, per rule (chart by chart in
/// the payload's order), the coefficient ids of its RHS terms. Measurement
/// access for coefficient-representation studies (coef-form); no restore.
#[derive(Debug)]
pub struct CustomCoefficientTable {
    pub coefficients: Vec<Coefficient>,
    pub rules: Vec<Vec<u32>>,
    pub symbolic_slots: usize,
}

/// [`CustomCoefficientTable`] of `payload`, bound to the family it records.
pub fn custom_payload_coefficient_table(payload: &[u8]) -> Result<CustomCoefficientTable, PError> {
    let decoded = decode(None, payload)?;
    let mut coefficients = Vec::with_capacity(decoded.table.len());
    for index in 0..decoded.table.len() {
        let id = CoefficientId::try_from_index(index).map_err(payload_error)?;
        coefficients.push(
            decoded
                .table
                .coefficient(id)
                .map_err(payload_error)?
                .clone(),
        );
    }
    let rules = (decoded.program.sectors.iter())
        .flat_map(|sector| sector.rules.iter())
        .map(|rule| rule.rhs.iter().map(|term| term.coefficient).collect())
        .collect();
    Ok(CustomCoefficientTable {
        coefficients,
        rules,
        symbolic_slots: decoded.program.root_sector.len(),
    })
}

/// A payload as one canonical text ([`CUSTOM_PAYLOAD_DUMP_VERSION`] first):
/// its structure, every chart with its residuals and every rule with its
/// coefficients. Equal payloads give equal dumps whatever the process symbol
/// table was at generation (the payload bytes embed it), so the dump serves
/// test guards and library comparisons.
pub fn custom_payload_dump(payload: &[u8]) -> Result<String, PError> {
    let summary = inspect_custom_payload(payload)?;
    let rules = custom_payload_rules(payload)?;
    let mut dump = String::new();
    let write = |dump: &mut String, line: fmt::Arguments<'_>| {
        dump.write_fmt(line).expect("writing to a String");
        dump.push('\n');
    };
    write(&mut dump, format_args!("{CUSTOM_PAYLOAD_DUMP_VERSION}"));
    write(&mut dump, format_args!("schema {}", summary.schema));
    write(
        &mut dump,
        format_args!("family {}", summary.family_fingerprint),
    );
    write(
        &mut dump,
        format_args!(
            "arity {}, support {:#x}, two-sided {:#x}",
            summary.arity, summary.support, summary.two_sided_axes
        ),
    );
    write(&mut dump, format_args!("order {}", summary.ordering));
    for chart in &summary.charts {
        write(
            &mut dump,
            format_args!(
                "chart {:#x}: {} rules, residuals {:?}",
                chart.chart, chart.rules, chart.residuals
            ),
        );
    }
    for rule in &rules {
        write(
            &mut dump,
            format_args!(
                "rule {:#x}/{}: {} {:?}{}, target {}",
                rule.chart,
                rule.ordinal,
                if rule.affine { "affine" } else { "coordinate" },
                rule.fixed,
                if rule.dispatched { ", dispatched" } else { "" },
                powers_text(&rule.target)
            ),
        );
        for equation in &rule.equations {
            write(&mut dump, format_args!("  equation {equation}"));
        }
        for (powers, coefficient) in &rule.rhs {
            write(
                &mut dump,
                format_args!("  + ({coefficient}) {}", powers_text(powers)),
            );
        }
        for branch in &rule.exclusions {
            write(&mut dump, format_args!("  exclusion {}", branch.join(", ")));
        }
    }
    Ok(dump)
}

/// Prepares one chart of a payload on first use.
#[derive(Debug)]
struct PayloadLoader {
    decoded: Arc<Decoded>,
}

impl<const N: usize> DeferredCandidateOwnerLoader<N> for PayloadLoader {
    fn prepare(
        &self,
        sector: [bool; N],
        context: Arc<CandidateOwnerContext<N>>,
    ) -> Result<CandidateOwnerPrograms<N>, CandidateReductionError> {
        let input = self.decoded.input(sector, &context)?;
        CandidateOwnerPrograms::try_new(context, [input])
    }
}

// ----- the lane step -----

/// A Custom owner's program: the generated rules of every raw chart of its
/// support, prepared chart by chart on first use. `N` is the storage arity of
/// the owner presentation (its slot count without `capacity-dispatch`).
pub struct CustomStep<const N: usize> {
    presentation: PresentationId,
    arity: usize,
    forced: u64,
    support: u64,
    charts: Vec<u64>,
    residuals: BTreeSet<IntegralKey>,
    programs: DeferredCandidateOwnerPrograms<N>,
}

impl<const N: usize> fmt::Debug for CustomStep<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomStep")
            .field("presentation", &self.presentation.0)
            .field("arity", &self.arity)
            .field("support", &format_args!("{:#x}", self.support))
            .field("charts", &self.charts)
            .field("residuals", &self.residuals)
            .finish_non_exhaustive()
    }
}

impl<const N: usize> CustomStep<N> {
    /// The owner support: the generation root, with every eps axis.
    pub fn support(&self) -> u64 {
        self.support
    }

    /// The raw charts, ascending.
    pub fn charts(&self) -> &[u64] {
        &self.charts
    }

    /// The residual keys: the owner's terminals.
    pub fn residuals(&self) -> &BTreeSet<IntegralKey> {
        &self.residuals
    }

    fn chart(&self, powers: &[i64]) -> [bool; N] {
        std::array::from_fn(|slot| powers.get(slot).copied().unwrap_or(0) > 0)
    }

    fn support_of(&self, powers: &[i64]) -> u64 {
        powers
            .iter()
            .enumerate()
            .filter(|&(_, &power)| power > 0)
            .fold(self.forced, |mask, (slot, _)| mask | 1 << slot)
    }

    /// `powers` is a key of this owner: the presentation's arity, the owner support.
    fn check_key(&self, presentation: PresentationId, powers: &[i64]) -> Result<(), PError> {
        if presentation != self.presentation
            || powers.len() != self.arity
            || self.support_of(powers) != self.support
        {
            return Err(PError::Contract(format!(
                "custom owner on presentation {}, support {:#x}: key {powers:?} of presentation {}",
                self.presentation.0, self.support, presentation.0
            )));
        }
        Ok(())
    }

    fn terminal(&self, cx: &PStepContext<'_>, key: &IntegralKey) -> Result<PTerminalId, PError> {
        let class = cx.owner.spec.class;
        cx.owner
            .spec
            .terminals
            .iter()
            .copied()
            .find(|terminal| {
                matches!(
                    cx.library.terminals().get(terminal.0 as usize),
                    Some(PTerminal::Residual { owner, key: residual }) if *owner == class && residual == key
                )
            })
            .ok_or_else(|| {
                PError::MalformedLibrary(format!(
                    "the residual {:?} of custom owner {:016x} is no terminal of the library",
                    key.powers(),
                    class.0
                ))
            })
    }

    /// Prepare every chart now (otherwise each is prepared on its first step),
    /// through the step itself at the chart's corner (every slot of the chart
    /// at power 1, the others at 0): the rules restore through the solver's
    /// admission, the prepared residuals equal the payload's, and the corner is
    /// a residual or covered by a rule.
    fn prepare_charts(&self) -> Result<(), PError> {
        for &chart in &self.charts {
            let corner: Vec<i64> = (0..self.arity)
                .map(|slot| i64::from(chart >> slot & 1 == 1))
                .collect();
            let failed = |detail: String| {
                PError::MalformedLibrary(format!(
                    "custom owner on presentation {}: chart {chart:#x} at {corner:?}: {detail}",
                    self.presentation.0
                ))
            };
            let integral = IntegralKey::try_new(corner.iter().copied())
                .map_err(|error| failed(error.to_string()))?;
            let applied = apply_prepared_owner(
                &self.programs,
                &self.chart(&corner),
                &integral,
                &mut Vec::new(),
                &mut ReductionRequest::default(),
                &mut ReductionStatistics::default(),
                &ReductionLimits::default(),
            );
            match applied {
                Ok(AppliedOwnerStep::Terminal | AppliedOwnerStep::Applied(..)) => {}
                Ok(AppliedOwnerStep::Uncovered) => return Err(failed("uncovered".to_owned())),
                Err(CandidateReductionError::Application(error)) => {
                    return Err(PError::Engine(error));
                }
                Err(error) => return Err(failed(error.to_string())),
            }
        }
        Ok(())
    }
}

/// Plan M2 sec.3.4: the residuals of an eps owner sit at `n = 1` on every eps
/// axis (`forced`), Forcer's master position `1 + j eps` and the least excess
/// of the two-sided order.
fn check_residual_positions(forced: u64, residuals: &[IntegralKey]) -> Result<(), PError> {
    for residual in residuals {
        let off = (residual.powers().iter().enumerate())
            .any(|(slot, &power)| forced >> slot & 1 == 1 && power != 1);
        if off {
            return Err(PError::Generation(format!(
                "eps custom owner residual {:?} is off n = 1 on the eps axes {forced:#x}",
                residual.powers()
            )));
        }
    }
    Ok(())
}

fn step_error(class: OwnerClassId, error: CandidateReductionError) -> PError {
    match error {
        CandidateReductionError::Application(error) => PError::Engine(error),
        CandidateReductionError::Uncovered { target } => PError::Uncovered { class, key: target },
        other => PError::MalformedLibrary(format!("custom owner {:016x}: {other}", class.0)),
    }
}

impl<const N: usize> PLaneStep for CustomStep<N> {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    /// One rule application, evaluated under the request's limits
    /// (`cx.limits`, M3-I1; with `ReductionLimits::default()` exactly as
    /// before). Preparing a chart on its first step still runs under the
    /// owner context's build-time `ReductionLimits::default()`, so a prepared
    /// chart does not depend on which request prepared it.
    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        self.check_key(key.presentation, key.powers)?;
        let class = cx.owner.spec.class;
        let integral = IntegralKey::try_new(key.powers.iter().copied())
            .map_err(|error| PError::Contract(error.to_string()))?;
        let chart = self.chart(key.powers);
        let mut conditions = Vec::new();
        let mut request = ReductionRequest::default();
        let mut work = ReductionStatistics::default();
        let applied = apply_prepared_owner(
            &self.programs,
            &chart,
            &integral,
            &mut conditions,
            &mut request,
            &mut work,
            cx.limits,
        )
        .map_err(|error| step_error(class, error))?;
        let mut out = PStepOut {
            conditions,
            ..PStepOut::default()
        };
        match applied {
            AppliedOwnerStep::Terminal => {
                let terminal = self.terminal(cx, &integral)?;
                let one = cx.library.coefficient_context().one();
                out.terminals.push((terminal, key.tag.clone(), one));
            }
            AppliedOwnerStep::Uncovered => {
                return Err(PError::Uncovered {
                    class,
                    key: integral,
                });
            }
            AppliedOwnerStep::Applied(terms, position) => {
                record_rule_hit(mask(&chart), position);
                for (child, coefficient) in terms {
                    let powers = child.powers();
                    if powers.len() != self.arity {
                        return Err(PError::MalformedLibrary(format!(
                            "custom owner {:016x}: a child {powers:?} of another arity",
                            class.0
                        )));
                    }
                    let stage = if self.support_of(powers) == self.support {
                        PStage::Apply
                    } else {
                        PStage::Route
                    };
                    out.children.push(PChild {
                        stage,
                        presentation: self.presentation,
                        powers: powers.to_vec(),
                        tag: key.tag.clone(),
                        coefficient,
                    });
                }
            }
        }
        Ok(out)
    }

    /// The order the rules were generated in: the runtime descent order of
    /// their own descent checks (two-sided axes ordered by the injective `e(n)`).
    fn order_key(&self, cx: &PStepContext<'_>, powers: &[i64]) -> Result<POrderKey, PError> {
        self.check_key(cx.owner.spec.presentation, powers)?;
        let ordering = owner_ordering(&self.programs, &self.chart(powers)).ok_or_else(|| {
            PError::Contract(format!("no chart of the custom owner holds {powers:?}"))
        })?;
        let key = ordering
            .complexity_key(powers)
            .map_err(|error| PError::Contract(format!("custom owner order: {error}")))?;
        Ok(POrderKey::Complexity(key))
    }
}

thread_local! {
    /// The rule the last Custom step of this thread applied, `(chart, position
    /// in the chart's rule list)`: the p path census reads and clears it right
    /// after the step, on the same thread (M5 artefact v2, spec item 8).
    static LAST_RULE: std::cell::Cell<Option<(u64, u32)>> = const { std::cell::Cell::new(None) };
}

fn record_rule_hit(chart: u64, position: usize) {
    let position = u32::try_from(position).unwrap_or(u32::MAX);
    LAST_RULE.with(|cell| cell.set(Some((chart, position))));
}

/// The rule the last Custom step of this thread applied, cleared: `(chart,
/// position in the chart's rule list)`. `None` after any other step.
pub(crate) fn take_rule_hit() -> Option<(u64, u32)> {
    LAST_RULE.with(std::cell::Cell::take)
}

/// Zero proofs of the downset of `support` (raw sectors; eps axes are always
/// effective in the analyzer).
fn zero_certificates(
    presentation: &Presentation,
    support: u64,
) -> Result<Vec<zero::Certificate>, PError> {
    let family = &presentation.family;
    let analyzer = zero::Analyzer::try_unrestricted(family)
        .map_err(|error| PError::Unsupported(format!("custom owner zero analysis: {error}")))?;
    let arity = presentation.slots.len();
    let mut proofs = Vec::new();
    let mut subset = support;
    loop {
        let sector = Mask::try_new(bits(subset, arity))
            .map_err(|error| PError::Contract(error.to_string()))?;
        match analyzer
            .analyze(&sector)
            .map_err(|error| PError::Unsupported(format!("custom owner zero analysis: {error}")))?
        {
            zero::Decision::ProvedZero(proof) => proofs.push(proof),
            zero::Decision::Inconclusive(_) => {}
            zero::Decision::Excluded(_) => {
                return Err(PError::Unsupported(format!(
                    "custom owner zero analysis excluded the sector {subset:#x}"
                )));
            }
        }
        if subset == 0 {
            break;
        }
        subset = (subset - 1) & support;
    }
    Ok(proofs)
}

/// The explicit application order of a Custom owner's rules (p artefact v2,
/// notes/artifact-ordering.SPEC.md item 1): per chart (raw sector mask), the
/// payload's rule indices in the order the owner's FirstValid dispatch tries
/// them. Every chart of the payload must be listed, each with a permutation of
/// its rules; anything else is refused. Two rules with the same case are two
/// entries (rules are never keyed by case).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PRuleOrder {
    pub charts: BTreeMap<u64, Vec<u32>>,
}

/// Put the rules of every chart of `program` in the order `order` gives, and
/// move the whole-piece dispatch entries with their rules.
fn apply_rule_order(program: &mut ProgramRecord, order: &PRuleOrder) -> Result<(), PError> {
    let held: BTreeSet<u64> = program.sectors.iter().map(|s| mask(&s.sector)).collect();
    let listed: BTreeSet<u64> = order.charts.keys().copied().collect();
    if held != listed {
        return Err(PError::MalformedLibrary(format!(
            "rule order: charts {listed:x?}, the payload has {held:x?}"
        )));
    }
    let mut moved = Vec::with_capacity(program.rule_dispatch.len());
    for (sector_index, sector) in program.sectors.iter_mut().enumerate() {
        let chart = mask(&sector.sector);
        let permutation = &order.charts[&chart];
        let count = sector.rules.len();
        let mut seen = vec![false; count];
        for &index in permutation {
            match seen.get_mut(index as usize) {
                Some(slot) if !*slot => *slot = true,
                _ => {
                    return Err(PError::MalformedLibrary(format!(
                        "rule order of chart {chart:#x}: {permutation:?} is no permutation of {count} rules"
                    )));
                }
            }
        }
        if permutation.len() != count {
            return Err(PError::MalformedLibrary(format!(
                "rule order of chart {chart:#x}: {} entries for {count} rules",
                permutation.len()
            )));
        }
        let mut position_of = vec![0_usize; count];
        for (position, &index) in permutation.iter().enumerate() {
            position_of[index as usize] = position;
        }
        let rules = std::mem::take(&mut sector.rules);
        let mut slots: Vec<Option<RuleRecord>> = rules.into_iter().map(Some).collect();
        sector.rules = permutation
            .iter()
            .map(|&index| slots[index as usize].take().expect("a permutation"))
            .collect();
        for entry in program
            .rule_dispatch
            .iter()
            .filter(|entry| entry.sector == sector_index)
        {
            let mut entry = entry.clone();
            entry.rule = *position_of.get(entry.rule).ok_or_else(|| {
                payload_error(format!("a dispatch entry names rule {}", entry.rule))
            })?;
            moved.push(entry);
        }
    }
    if moved.len() != program.rule_dispatch.len() {
        return Err(payload_error("a dispatch entry names an unknown chart"));
    }
    moved.sort_by_key(|entry| (entry.sector, entry.rule));
    program.rule_dispatch = moved;
    Ok(())
}

fn build<const N: usize>(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
    prepare: bool,
    order: Option<&PRuleOrder>,
) -> Result<CustomStep<N>, PError> {
    let mut decoded = decode(Some(presentation), payload)?;
    if let Some(order) = order {
        apply_rule_order(&mut decoded.program, order)?;
    }
    let program = &decoded.program;
    let arity = presentation.slots.len();
    let support = mask(&program.root_sector);
    let forced = presentation.forced_axes();
    let two_sided_axes = decoded
        .ordering
        .program()
        .map_or(0, |order| mask(order.two_sided()));
    if forced != 0 && two_sided_axes == 0 {
        return Err(PError::Unsupported(
            "eps custom owner requires two-sided axes".to_owned(),
        ));
    }
    let mut charts: Vec<u64> = program
        .sectors
        .iter()
        .map(|sector| mask(&sector.sector))
        .collect();
    charts.sort_unstable();
    let residuals: Vec<IntegralKey> = (program.sectors.iter())
        .flat_map(|sector| &sector.finite_residuals)
        .map(|residual| IntegralKey::try_new(residual.values.iter().map(|&v| i64::from(v))))
        .collect::<Result<_, _>>()
        .map_err(payload_error)?;
    let described = u32::try_from(residuals.len()).is_ok_and(|count| count == meta.residuals)
        && two_sided_axes == meta.two_sided_axes
        && charts == meta.charts
        && support
            == meta
                .charts
                .iter()
                .fold(forced, |support, chart| support | chart);
    if !described {
        return Err(PError::MalformedLibrary(format!(
            "custom payload and meta disagree: payload support {support:#x}, charts {charts:x?}, \
             two-sided {two_sided_axes:#x}, {} residuals; meta {meta:?}",
            residuals.len()
        )));
    }
    check_residual_positions(forced, &residuals)?;
    let proofs = zero_certificates(presentation, support)?;
    let scope = CandidateOwnerScope {
        max_numerator_rank: None,
        finite_case_policy: FiniteCasePolicy::SearchFinite,
    };
    let candidate =
        |error: CandidateReductionError| PError::Unsupported(format!("custom owner: {error}"));
    let context = Arc::new(
        CandidateOwnerContext::<N>::try_new_p(
            Arc::clone(&presentation.family),
            scope,
            proofs,
            ReductionLimits::default(),
        )
        .map_err(candidate)?,
    );
    let root: [bool; N] = crate::storage_array(&program.root_sector, false)
        .ok_or_else(|| payload_error("root arity"))?;
    let mut by_chart: BTreeMap<u64, BTreeSet<IntegralKey>> = charts
        .iter()
        .map(|&chart| (chart, BTreeSet::new()))
        .collect();
    for residual in &residuals {
        let chart = mask(
            &residual
                .powers()
                .iter()
                .map(|&power| power > 0)
                .collect::<Vec<_>>(),
        );
        by_chart
            .get_mut(&chart)
            .ok_or_else(|| payload_error("a residual on no chart"))?
            .insert(residual.clone());
    }
    let descriptors = by_chart.into_iter().map(|(chart, terminals)| {
        let sector: [bool; N] = std::array::from_fn(|slot| slot < arity && chart >> slot & 1 == 1);
        DeferredCandidateOwnerDescriptor {
            sector,
            saved_root: root,
            ordering: decoded.ordering.clone(),
            terminals,
        }
    });
    let descriptors: Vec<_> = descriptors.collect();
    let programs = DeferredCandidateOwnerPrograms::try_new(
        context,
        descriptors,
        Arc::new(PayloadLoader {
            decoded: Arc::new(decoded),
        }),
    )
    .map_err(candidate)?;
    let step = CustomStep::<N> {
        presentation: presentation.id,
        arity,
        forced,
        support,
        charts,
        residuals: residuals.into_iter().collect(),
        programs,
    };
    if prepare {
        step.prepare_charts()?;
    }
    Ok(step)
}

/// The lane step of a Custom owner of `presentation` from its payload and
/// record; each chart is prepared on its first step. Fails closed: an arity
/// outside the compiled runtime registry is [`PError::Unsupported`] (build with
/// `RUSTRED_RUNTIME_ARITIES`); the guard of [`check_custom_owner`]; residuals
/// of an eps owner off `n = 1`; a payload of another family, schema or order,
/// or one that the record does not describe, is refused.
pub fn build_custom_step(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
) -> Result<Arc<dyn PLaneStep>, PError> {
    build_custom_step_with(presentation, payload, meta, false, None)
}

/// [`build_custom_step`] whose FirstValid dispatch tries every chart's rules
/// in the explicit order `order` (p artefact v2) instead of the payload's.
pub fn build_custom_step_ordered(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
    order: Option<&PRuleOrder>,
) -> Result<Arc<dyn PLaneStep>, PError> {
    build_custom_step_with(presentation, payload, meta, false, order)
}

/// [`build_custom_step`] with every chart prepared now: a rule that does not
/// restore through the solver's admission fails here, not at its first step
/// ([`PError::MalformedLibrary`]). Generation builds this way.
pub fn build_custom_step_prepared(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
) -> Result<Arc<dyn PLaneStep>, PError> {
    build_custom_step_with(presentation, payload, meta, true, None)
}

fn build_custom_step_with(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
    prepare: bool,
    order: Option<&PRuleOrder>,
) -> Result<Arc<dyn PLaneStep>, PError> {
    let arity = presentation.slots.len();
    if !crate::compiled_runtime_arities().contains(&arity) {
        return Err(PError::Unsupported(format!(
            "custom owner arity {arity} is not compiled into this RustRed (registered: {:?}); \
             rebuild with RUSTRED_RUNTIME_ARITIES including {arity} (feature runtime-arity-selection)",
            crate::compiled_runtime_arities()
        )));
    }
    check_custom_owner(presentation, meta)?;
    macro_rules! dispatch {
        ($($n:literal),*) => {
            match crate::campaign_storage_arity(arity) {
                $($n => build::<$n>(presentation, payload, meta, prepare, order)
                    .map(|step| Arc::new(step) as Arc<dyn PLaneStep>),)*
                storage => Err(PError::Unsupported(format!(
                    "custom owner arity {arity} has no compiled storage (width {storage}); \
                     rebuild with RUSTRED_RUNTIME_ARITIES including {arity}"
                ))),
            }
        };
    }
    with_candidate_runtime_arities!(dispatch)
}

/// What one rule application of a Custom fallback program does to a key
/// (M5 no6): the key is the program's residual, or the rule's children with the
/// nonzero conditions the step met.
#[derive(Debug)]
pub enum PFallbackApplied {
    Residual,
    Children {
        children: Vec<(Vec<i64>, Coefficient)>,
        conditions: Vec<Coefficient>,
    },
}

/// A Custom owner's program used as the FALLBACK of another owner of the same
/// node (M5 no6: the Scheme owner of no6 closes the keys its Forcer rules do
/// not take with the shipped Custom rules). The program is bound to
/// `presentation` (the presentation the payload was generated on, or one with
/// the same family and slots); it never names terminals: the residual is
/// reported as [`PFallbackApplied::Residual`] and the calling owner maps it.
pub trait PCustomFallback: Send + Sync + fmt::Debug {
    /// The owner support of the payload (its generation root).
    fn support(&self) -> u64;
    /// The payload's residual keys.
    fn residual_keys(&self) -> Vec<IntegralKey>;
    /// One rule application at `powers` (a key of the support).
    fn apply(
        &self,
        class: OwnerClassId,
        powers: &[i64],
        limits: &ReductionLimits,
    ) -> Result<PFallbackApplied, PError>;
    /// The order the rules were generated in (as [`PLaneStep::order_key`]).
    fn order(&self, powers: &[i64]) -> Result<POrderKey, PError>;
}

impl<const N: usize> PCustomFallback for CustomStep<N> {
    fn support(&self) -> u64 {
        self.support
    }

    fn residual_keys(&self) -> Vec<IntegralKey> {
        self.residuals.iter().cloned().collect()
    }

    fn apply(
        &self,
        class: OwnerClassId,
        powers: &[i64],
        limits: &ReductionLimits,
    ) -> Result<PFallbackApplied, PError> {
        self.check_key(self.presentation, powers)?;
        let integral = IntegralKey::try_new(powers.iter().copied())
            .map_err(|error| PError::Contract(error.to_string()))?;
        let mut conditions = Vec::new();
        let applied = apply_prepared_owner(
            &self.programs,
            &self.chart(powers),
            &integral,
            &mut conditions,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
            limits,
        )
        .map_err(|error| step_error(class, error))?;
        match applied {
            AppliedOwnerStep::Terminal => Ok(PFallbackApplied::Residual),
            AppliedOwnerStep::Uncovered => Err(PError::Uncovered {
                class,
                key: integral,
            }),
            AppliedOwnerStep::Applied(terms, _) => {
                let mut children = Vec::with_capacity(terms.len());
                for (child, coefficient) in terms {
                    if child.powers().len() != self.arity {
                        return Err(PError::MalformedLibrary(format!(
                            "custom fallback of {:016x}: a child {:?} of another arity",
                            class.0,
                            child.powers()
                        )));
                    }
                    children.push((child.powers().to_vec(), coefficient));
                }
                Ok(PFallbackApplied::Children {
                    children,
                    conditions,
                })
            }
        }
    }

    fn order(&self, powers: &[i64]) -> Result<POrderKey, PError> {
        self.check_key(self.presentation, powers)?;
        let ordering = owner_ordering(&self.programs, &self.chart(powers)).ok_or_else(|| {
            PError::Contract(format!("no chart of the custom fallback holds {powers:?}"))
        })?;
        let key = ordering
            .complexity_key(powers)
            .map_err(|error| PError::Contract(format!("custom fallback order: {error}")))?;
        Ok(POrderKey::Complexity(key))
    }
}

/// The Custom program of `payload` as a fallback ([`PCustomFallback`]) on
/// `presentation`, with the explicit rule order `order` if given: the same
/// checks as [`build_custom_step_ordered`] (guard, arity registry, payload
/// bound to the presentation's family and described by `meta`).
pub fn build_custom_fallback(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
    order: Option<&PRuleOrder>,
) -> Result<Arc<dyn PCustomFallback>, PError> {
    let arity = presentation.slots.len();
    if !crate::compiled_runtime_arities().contains(&arity) {
        return Err(PError::Unsupported(format!(
            "custom fallback arity {arity} is not compiled into this RustRed (registered: {:?})",
            crate::compiled_runtime_arities()
        )));
    }
    check_custom_owner(presentation, meta)?;
    macro_rules! dispatch {
        ($($n:literal),*) => {
            match crate::campaign_storage_arity(arity) {
                $($n => build::<$n>(presentation, payload, meta, false, order)
                    .map(|step| Arc::new(step) as Arc<dyn PCustomFallback>),)*
                storage => Err(PError::Unsupported(format!(
                    "custom fallback arity {arity} has no compiled storage (width {storage})"
                ))),
            }
        };
    }
    with_candidate_runtime_arities!(dispatch)
}

// ----- import-time verification and replay (p5l-converter) -----

/// One rule application of a Custom owner, without a library around it.
enum Applied {
    Terminal,
    Children(Vec<(Vec<i64>, Coefficient)>),
}

/// The owner program of one payload, for checks outside a library.
trait CustomOwnerOps {
    fn support(&self) -> u64;
    fn arity(&self) -> usize;
    fn forced(&self) -> u64;
    fn is_residual(&self, powers: &[i64]) -> bool;
    fn apply_powers(&self, powers: &[i64]) -> Result<Applied, PError>;
    fn order_of(&self, powers: &[i64]) -> Result<crate::sector::ComplexityKey, PError>;
}

impl<const N: usize> CustomOwnerOps for CustomStep<N> {
    fn support(&self) -> u64 {
        self.support
    }

    fn arity(&self) -> usize {
        self.arity
    }

    fn forced(&self) -> u64 {
        self.forced
    }

    fn is_residual(&self, powers: &[i64]) -> bool {
        IntegralKey::try_new(powers.iter().copied()).is_ok_and(|key| self.residuals.contains(&key))
    }

    /// The step of [`PLaneStep::step`] on `powers`, which must be a key of
    /// the owner support.
    fn apply_powers(&self, powers: &[i64]) -> Result<Applied, PError> {
        if powers.len() != self.arity || self.support_of(powers) != self.support {
            return Err(PError::Contract(format!(
                "custom owner support {:#x}: key {powers:?}",
                self.support
            )));
        }
        let integral = IntegralKey::try_new(powers.iter().copied())
            .map_err(|error| PError::Contract(error.to_string()))?;
        let mut conditions = Vec::new();
        let applied = apply_prepared_owner(
            &self.programs,
            &self.chart(powers),
            &integral,
            &mut conditions,
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
            &ReductionLimits::default(),
        )
        .map_err(|error| step_error(OwnerClassId(0), error))?;
        match applied {
            AppliedOwnerStep::Terminal => Ok(Applied::Terminal),
            AppliedOwnerStep::Uncovered => Err(PError::Uncovered {
                class: OwnerClassId(0),
                key: integral,
            }),
            AppliedOwnerStep::Applied(terms, _) => Ok(Applied::Children(
                terms
                    .into_iter()
                    .map(|(child, coefficient)| (child.powers().to_vec(), coefficient))
                    .collect(),
            )),
        }
    }

    fn order_of(&self, powers: &[i64]) -> Result<crate::sector::ComplexityKey, PError> {
        let ordering = owner_ordering(&self.programs, &self.chart(powers)).ok_or_else(|| {
            PError::Contract(format!("no chart of the custom owner holds {powers:?}"))
        })?;
        ordering
            .complexity_key(powers)
            .map_err(|error| PError::Contract(format!("custom owner order: {error}")))
    }
}

fn custom_owner_ops(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
) -> Result<Box<dyn CustomOwnerOps>, PError> {
    let arity = presentation.slots.len();
    if !crate::compiled_runtime_arities().contains(&arity) {
        return Err(PError::Unsupported(format!(
            "custom owner arity {arity} is not compiled into this RustRed (registered: {:?}); \
             rebuild with RUSTRED_RUNTIME_ARITIES including {arity} (feature runtime-arity-selection)",
            crate::compiled_runtime_arities()
        )));
    }
    check_custom_owner(presentation, meta)?;
    macro_rules! dispatch {
        ($($n:literal),*) => {
            match crate::campaign_storage_arity(arity) {
                $($n => build::<$n>(presentation, payload, meta, true, None)
                    .map(|step| Box::new(step) as Box<dyn CustomOwnerOps>),)*
                storage => Err(PError::Unsupported(format!(
                    "custom owner arity {arity} has no compiled storage (width {storage})"
                ))),
            }
        };
    }
    with_candidate_runtime_arities!(dispatch)
}

/// What [`verify_custom_payload`] checked on one payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomPayloadCheck {
    pub charts: usize,
    pub rules: usize,
    /// The order is support-primary (else rank-gated or two-sided).
    pub support_primary: bool,
    pub witness_keys: usize,
    /// Apply children strictly below their key in the owner order.
    pub same_sector_children: usize,
    /// Route children on strict subsectors of the owner support.
    pub subsector_children: usize,
    /// Witness keys that are residuals of the owner.
    pub terminals: usize,
}

/// The load-time checks of a Custom owner (`descent::check` items 1, 2 and 7)
/// on one payload and its presentation, without a library: the hard guard
/// and chart rule ([`check_custom_owner`]); every chart prepared through the
/// solver's admission; an order that is support-primary or rank-gated for an
/// integer owner and two-sided exactly on the eps axes of an eps owner; and
/// the descent witness box ([`super::descent::custom_witness_keys`]): every
/// child of a step is an Apply key of the support strictly lower in the owner
/// order or a key of a strict subsector, every terminal a residual.
pub fn verify_custom_payload(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
) -> Result<CustomPayloadCheck, PError> {
    let summary = inspect_custom_payload(payload)?;
    let ops = custom_owner_ops(presentation, payload, meta)?;
    let (support, forced, arity) = (ops.support(), ops.forced(), ops.arity());
    let fail = |detail: String| {
        PError::MalformedLibrary(format!(
            "custom payload on presentation {}: {detail}",
            presentation.id.0
        ))
    };
    let policy = OrderingPolicy::try_from_stable_id(&summary.ordering)
        .map_err(|error| fail(format!("order {}: {error}", summary.ordering)))?;
    let program_axes = policy
        .program()
        .map_or(0, |program| mask(program.two_sided()));
    if program_axes != forced || summary.two_sided_axes != forced {
        return Err(fail(format!(
            "the order is two-sided on {program_axes:#x}, the eps axes are {forced:#x}"
        )));
    }
    let support_primary = policy.is_support_primary();
    if forced == 0 && !support_primary && !super::descent::is_rank_gated(&policy, support) {
        return Err(fail(format!(
            "the order {} is neither support-primary nor rank-gated",
            summary.ordering
        )));
    }
    let mut check = CustomPayloadCheck {
        charts: summary.charts.len(),
        rules: summary.rules(),
        support_primary,
        ..CustomPayloadCheck::default()
    };
    let support_of = |powers: &[i64]| {
        (powers.iter().enumerate())
            .filter(|&(_, &power)| power > 0)
            .fold(forced, |mask, (slot, _)| mask | 1 << slot)
    };
    for key in super::descent::custom_witness_keys(arity, support, forced) {
        let order = ops.order_of(&key)?;
        match ops.apply_powers(&key)? {
            Applied::Terminal => {
                if !ops.is_residual(&key) {
                    return Err(fail(format!("the terminal {key:?} is no residual")));
                }
                check.terminals += 1;
            }
            Applied::Children(children) => {
                for (child, _) in children {
                    let child_support = support_of(&child);
                    if child_support == support {
                        if ops.order_of(&child)? >= order {
                            return Err(fail(format!(
                                "key {key:?}: the child {child:?} does not descend"
                            )));
                        }
                        check.same_sector_children += 1;
                    } else if child_support & !support == 0 {
                        check.subsector_children += 1;
                    } else {
                        return Err(fail(format!(
                            "key {key:?}: the child {child:?} leaves the support"
                        )));
                    }
                }
            }
        }
        check.witness_keys += 1;
    }
    Ok(check)
}

/// One seed reduced by the owner's rules alone ([`replay_custom_payload`]).
#[derive(Clone, Debug)]
pub struct CustomReplay {
    pub seed: Vec<i64>,
    /// Coefficients of the owner's residuals, exact in `Q(d)`.
    pub residuals: Vec<(Vec<i64>, Coefficient)>,
    /// Keys of strict subsectors of the owner support (Route keys in a
    /// library), with their coefficients.
    pub boundary: Vec<(Vec<i64>, Coefficient)>,
    /// Rule applications.
    pub steps: usize,
}

/// Each seed (a key of the owner support) reduced with the owner's program,
/// exactly as its library steps do (`apply_prepared_owner` on the prepared
/// charts, keys in descending owner order, each key expanded once with the
/// sum of its weights): the result is the seed in terms of the owner's
/// residuals plus keys of strict subsectors, which a library would route on.
/// A child that does not descend fails.
pub fn replay_custom_payload(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
    seeds: &[Vec<i64>],
) -> Result<Vec<CustomReplay>, PError> {
    let ops = custom_owner_ops(presentation, payload, meta)?;
    let (support, forced) = (ops.support(), ops.forced());
    let support_of = |powers: &[i64]| {
        (powers.iter().enumerate())
            .filter(|&(_, &power)| power > 0)
            .fold(forced, |mask, (slot, _)| mask | 1 << slot)
    };
    let context = presentation.family.coefficient_context();
    let add =
        |map: &mut BTreeMap<Vec<i64>, Coefficient>, key: Vec<i64>, value: Coefficient| match map
            .get_mut(&key)
        {
            Some(slot) => *slot = &*slot + &value,
            None => {
                map.insert(key, value);
            }
        };
    let mut replays = Vec::with_capacity(seeds.len());
    for seed in seeds {
        if support_of(seed) != support {
            return Err(PError::Contract(format!(
                "seed {seed:?} is not on the owner support {support:#x}"
            )));
        }
        let mut queue: BTreeMap<(crate::sector::ComplexityKey, Vec<i64>), Coefficient> =
            BTreeMap::new();
        queue.insert((ops.order_of(seed)?, seed.clone()), context.one());
        let (mut residuals, mut boundary) = (BTreeMap::new(), BTreeMap::new());
        let mut steps = 0;
        while let Some(((order, key), weight)) = queue.pop_last() {
            if weight.is_zero() {
                continue;
            }
            steps += 1;
            match ops.apply_powers(&key)? {
                Applied::Terminal => add(&mut residuals, key, weight),
                Applied::Children(children) => {
                    for (child, coefficient) in children {
                        let value = &weight * &coefficient;
                        if support_of(&child) == support {
                            let child_order = ops.order_of(&child)?;
                            if child_order >= order {
                                return Err(PError::MalformedLibrary(format!(
                                    "replay of {seed:?}: the child {child:?} of {key:?} does not descend"
                                )));
                            }
                            match queue.get_mut(&(child_order.clone(), child.clone())) {
                                Some(slot) => *slot = &*slot + &value,
                                None => {
                                    queue.insert((child_order, child), value);
                                }
                            }
                        } else {
                            add(&mut boundary, child, value);
                        }
                    }
                }
            }
        }
        let nonzero = |map: BTreeMap<Vec<i64>, Coefficient>| -> Vec<(Vec<i64>, Coefficient)> {
            map.into_iter()
                .filter(|(_, value)| !value.is_zero())
                .collect()
        };
        replays.push(CustomReplay {
            seed: seed.clone(),
            residuals: nonzero(residuals),
            boundary: nonzero(boundary),
            steps,
        });
    }
    Ok(replays)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(powers: &[&[i64]]) -> Vec<IntegralKey> {
        (powers.iter())
            .map(|powers| IntegralKey::try_new(powers.iter().copied()).unwrap())
            .collect()
    }

    /// Plan M2 sec.3.4: an eps owner's residuals sit at `n = 1` on every eps
    /// axis; an integer owner (no forced axis) is not constrained.
    #[test]
    fn eps_residuals_sit_at_n_equal_one() {
        check_residual_positions(0b1, &keys(&[&[1, 1, 1, 1, 1]])).unwrap();
        check_residual_positions(0b101, &keys(&[&[1, 2, 1, 0]])).unwrap();
        check_residual_positions(0, &keys(&[&[2, 1, 0], &[1, 1, -1]])).unwrap();
        for (forced, powers) in [
            (0b1, [0_i64, 1, 1, 1, 1]),
            (0b1, [2, 1, 1, 1, 1]),
            (0b100, [1, 1, -1, 1, 1]),
        ] {
            assert_eq!(
                check_residual_positions(forced, &keys(&[&powers])),
                Err(PError::Generation(format!(
                    "eps custom owner residual {powers:?} is off n = 1 on the eps axes {forced:#x}"
                )))
            );
        }
    }
}
