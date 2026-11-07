//! Custom owners: `CustomStep<M>` over generated candidate programs (owner: M2-T7c rr-custom).
//!
//! A Custom owner is one owner support of a presentation whose rules SpIRed
//! generated all-rank (no numerator cap): NO (`nostar0`, integer) and the eps
//! kite `t1star05` at `L <= 3`. Its payload is the generated candidate bundle
//! of exactly the raw charts of that support (plan M2 sec.3.4): one chart for
//! an integer owner, all `2^j` charts of the `j` eps axes for an eps owner,
//! whose order program must carry two-sided data on every eps axis.
//!
//! The candidate-bundle format belongs to the application crate. This module
//! reads the bundles it generates with a read-only copy of their structural
//! records, so that the library can build a Custom owner from bytes alone
//! ([`build_custom_step`], used by assembly and the loader through
//! `programs::lane_program`). The bundle's family must be the presentation's
//! family. Nothing goes through the bundle loader's admission: it refuses eps
//! families by design, and the p-only constructor
//! (`CandidateOwnerContext::try_new_p`) admits them.
//!
//! The hard false-knock-out guard ([`check_custom_owner`]) runs at generation
//! (`rustred_app::application::propagator::generate_custom_owner`) and again on
//! every build: `lp_count >= 1` with no residual is refused, an eps owner needs
//! two-sided axes exactly on its eps axes and every raw chart, and the meta must
//! describe the payload.
//!
//! [`CustomStep`] runs one rule of its owner per step through the candidate
//! owner step (`solver::owner_step::apply_prepared_owner`): children of the
//! owner support are Apply keys of the same owner, any other child is a Route
//! key of the presentation (a subsector, resolved by the census), and a
//! residual is the owner's terminal. Coefficients are exact elements of
//! `Q(d)`, the library's context; the Ordered engine carries them under any
//! weight arithmetic (exact or `Z_p(d)`), there is no other path.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use symbolica::poly::PolyVariable;

use crate::algebra::{Coefficient, CoefficientPolynomial, IndexedCoefficientContext};
use crate::family::{IntegralFamilyLimits, IntegralKey};
use crate::persistence::{
    BinaryIoLimits, BinaryProgramKind, CoefficientId, DecodedCoefficientTable, NativeFamilyRecord,
    SectionTag, inspect_program,
};
use crate::reduction::{ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::sector::{Mask, OrderingPolicy, zero};
use crate::solver::owner_step::{AppliedOwnerStep, apply_prepared_owner, owner_ordering};
use crate::solver::{
    AffineCase, AffineIntersection, CandidateOwnerContext, CandidateOwnerInput,
    CandidateOwnerPrograms, CandidateOwnerScope, CandidateReductionError, Case, CoordinateCase,
    DeferredCandidateOwnerDescriptor, DeferredCandidateOwnerLoader, DeferredCandidateOwnerPrograms,
    ExceptionalConditions, FiniteCasePolicy, Integral, IntegralOrder, Power, RuleCandidate,
    RuleDispatchPolicy, SearchStats, SectorRule, SectorSolution, SectorStats, Seed, SeedSource,
    Term,
};

use super::error::PError;
use super::lane::OwnerLane;
use super::library::{CustomOwnerMeta, PTerminal};
use super::presentation::{Presentation, PresentationKind};
use super::step::{PChild, PKeyView, PLaneStep, POrderKey, PStage, PStepContext, PStepOut};
use super::{OwnerClassId, PTerminalId, PresentationId};

/// Bundle schemas a payload may carry: the generated candidate bundle without
/// and with a rule-dispatch table (`rustred_app`'s `CANDIDATE_BUNDLE_SCHEMA`
/// and its dispatch variant). Legacy v1 bundles have no order program.
pub const CUSTOM_PAYLOAD_SCHEMAS: [&str; 2] = [
    "rustred.generated-candidates.binary.v2",
    "rustred.generated-candidates.binary.v3",
];
const DISPATCH_SCHEMA: &str = CUSTOM_PAYLOAD_SCHEMAS[1];
const STATUS: &str = "uncertified-candidates";
/// Unrestricted solver policies: the default numerical depth, or an explicit
/// one, without a numerator cap or finite retention.
const DEFAULT_SOLVER_POLICY: &str = "ordinary-source-port-default-v1";
const DEPTH_POLICY: (&str, &str) = ("ordinary-source-port-numerical-depth-", "-v1");
/// Structural allocation allowance of one payload's records.
const MAX_RECORD_BYTES: usize = 1 << 30;

// ----- structural records of a generated candidate bundle (read-only copy) -----

#[derive(bincode::Decode)]
struct WireSchema {
    schema: String,
}

#[derive(bincode::Decode)]
struct WireProgramV2 {
    schema: String,
    status: String,
    solver_policy: String,
    _family_source: String,
    _input_format: String,
    family_fingerprint: String,
    root_sector: Vec<bool>,
    permutation: Option<Vec<usize>>,
    integral_order: String,
    sectors: Vec<WireSector>,
}

#[derive(bincode::Decode)]
struct WireProgramV3 {
    schema: String,
    status: String,
    solver_policy: String,
    _family_source: String,
    _input_format: String,
    family_fingerprint: String,
    root_sector: Vec<bool>,
    permutation: Option<Vec<usize>>,
    integral_order: String,
    sectors: Vec<WireSector>,
    rule_dispatch: Vec<WireDispatch>,
}

#[derive(bincode::Decode)]
struct WireDispatch {
    sector: usize,
    rule: usize,
    policy: u8,
}

#[derive(bincode::Decode)]
struct WireSector {
    sector: Vec<bool>,
    rules: Vec<WireRule>,
    finite_residuals: Vec<WireIntegral>,
}

#[derive(bincode::Decode)]
struct WireRule {
    case: WireCase,
    target: WireIntegral,
    rhs: Vec<WireTerm>,
    sources: Vec<WireSeed>,
    exclusions: Vec<Vec<u32>>,
}

#[derive(bincode::Decode)]
struct WireCase {
    kind: String,
    fixed_axes: Vec<usize>,
    fixed_values: Vec<i16>,
    equations: Vec<u32>,
}

#[derive(bincode::Decode)]
struct WireIntegral {
    symbolic: Vec<bool>,
    values: Vec<i16>,
}

#[derive(bincode::Decode)]
struct WireTerm {
    integral: WireIntegral,
    coefficient: u32,
}

#[derive(bincode::Decode)]
struct WireSeed {
    basis_row: usize,
    integral: WireIntegral,
    shifts: Vec<i16>,
}

/// The program record of either schema, with the dispatch table of v3.
struct WireProgram {
    schema: String,
    status: String,
    solver_policy: String,
    family_fingerprint: String,
    root_sector: Vec<bool>,
    permutation: Option<Vec<usize>>,
    integral_order: String,
    sectors: Vec<WireSector>,
    rule_dispatch: Vec<WireDispatch>,
}

impl From<WireProgramV2> for WireProgram {
    fn from(wire: WireProgramV2) -> Self {
        let WireProgramV2 {
            schema,
            status,
            solver_policy,
            _family_source: _,
            _input_format: _,
            family_fingerprint,
            root_sector,
            permutation,
            integral_order,
            sectors,
        } = wire;
        Self {
            schema,
            status,
            solver_policy,
            family_fingerprint,
            root_sector,
            permutation,
            integral_order,
            sectors,
            rule_dispatch: Vec::new(),
        }
    }
}

impl From<WireProgramV3> for WireProgram {
    fn from(wire: WireProgramV3) -> Self {
        let WireProgramV3 {
            schema,
            status,
            solver_policy,
            _family_source: _,
            _input_format: _,
            family_fingerprint,
            root_sector,
            permutation,
            integral_order,
            sectors,
            rule_dispatch,
        } = wire;
        Self {
            schema,
            status,
            solver_policy,
            family_fingerprint,
            root_sector,
            permutation,
            integral_order,
            sectors,
            rule_dispatch,
        }
    }
}

fn payload_error(detail: impl fmt::Display) -> PError {
    PError::MalformedLibrary(format!("custom payload: {detail}"))
}

fn decode_record<T: bincode::Decode<()>>(bytes: &[u8], what: &str) -> Result<T, PError> {
    let config = bincode::config::standard().with_limit::<MAX_RECORD_BYTES>();
    let (value, consumed) = bincode::decode_from_slice(bytes, config)
        .map_err(|error| payload_error(format!("{what}: {error}")))?;
    if consumed != bytes.len() {
        return Err(payload_error(format!("trailing bytes after the {what}")));
    }
    Ok(value)
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
    state: &'a [u8],
    atoms: &'a [u8],
    family: NativeFamilyRecord,
    program: WireProgram,
    ordering: OrderingPolicy,
}

fn read_structure(payload: &[u8]) -> Result<Structure<'_>, PError> {
    let envelope = inspect_program(payload, BinaryIoLimits::default()).map_err(payload_error)?;
    let tags: Vec<_> = envelope
        .sections()
        .iter()
        .map(|section| section.tag)
        .collect();
    if envelope.kind() != BinaryProgramKind::Candidates
        || tags
            != [
                SectionTag::SYMBOLICA_STATE,
                SectionTag::COEFFICIENTS,
                SectionTag::FAMILY,
                SectionTag::PROGRAM,
            ]
    {
        return Err(payload_error("not a generated candidate bundle"));
    }
    let section = |tag| envelope.section(tag).expect("checked section");
    let program_bytes = section(SectionTag::PROGRAM);
    let config = bincode::config::standard().with_limit::<MAX_RECORD_BYTES>();
    let (WireSchema { schema }, _) = bincode::decode_from_slice(program_bytes, config)
        .map_err(|error| payload_error(format!("program schema: {error}")))?;
    let program: WireProgram = match schema.as_str() {
        schema if schema == CUSTOM_PAYLOAD_SCHEMAS[0] => {
            decode_record::<WireProgramV2>(program_bytes, "program record")?.into()
        }
        DISPATCH_SCHEMA => decode_record::<WireProgramV3>(program_bytes, "program record")?.into(),
        other => {
            return Err(payload_error(format!(
                "schema {other:?}, expected one of {CUSTOM_PAYLOAD_SCHEMAS:?}"
            )));
        }
    };
    let family: NativeFamilyRecord = decode_record(section(SectionTag::FAMILY), "family record")?;
    let ordering = validate(&program)?;
    if family.arity() != program.root_sector.len() {
        return Err(payload_error("family and root arities differ"));
    }
    Ok(Structure {
        state: section(SectionTag::SYMBOLICA_STATE),
        atoms: section(SectionTag::COEFFICIENTS),
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

fn validate_integral(integral: &WireIntegral, arity: usize) -> Result<(), PError> {
    if integral.symbolic.len() != arity || integral.values.len() != arity {
        return Err(payload_error("an integral of another arity"));
    }
    for (&symbolic, &value) in integral.symbolic.iter().zip(&integral.values) {
        Power::new(symbolic, value).map_err(payload_error)?;
    }
    Ok(())
}

/// Shape checks of the whole program; returns its mathematical order.
fn validate(program: &WireProgram) -> Result<OrderingPolicy, PError> {
    if program.status != STATUS {
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
    if program.rule_dispatch.is_empty() == (program.schema == DISPATCH_SCHEMA) {
        return Err(payload_error("dispatch table and schema disagree"));
    }
    let mut previous = None;
    for entry in &program.rule_dispatch {
        let key = (entry.sector, entry.rule);
        if entry.policy != 1
            || previous.is_some_and(|before| before >= key)
            || program
                .sectors
                .get(entry.sector)
                .is_none_or(|sector| entry.rule >= sector.rules.len())
        {
            return Err(payload_error("an invalid dispatch entry"));
        }
        previous = Some(key);
    }
    let root = mask(&program.root_sector);
    let mut seen = BTreeSet::new();
    for sector in &program.sectors {
        let chart = mask(&sector.sector);
        if sector.sector.len() != arity || !seen.insert(chart) || chart & !root != 0 {
            return Err(payload_error(
                "a chart that is duplicate, of another arity or outside the root",
            ));
        }
        for residual in &sector.finite_residuals {
            validate_integral(residual, arity)?;
            if residual.symbolic.iter().any(|&symbolic| symbolic)
                || residual
                    .values
                    .iter()
                    .zip(&sector.sector)
                    .any(|(&value, &active)| (value > 0) != active)
            {
                return Err(payload_error("a residual that is no key of its chart"));
            }
        }
        for rule in &sector.rules {
            validate_integral(&rule.target, arity)?;
            let case = &rule.case;
            if case.fixed_axes.len() != case.fixed_values.len()
                || case.fixed_axes.iter().any(|&axis| axis >= arity)
                || case.fixed_axes.windows(2).any(|axes| axes[0] >= axes[1])
                || !matches!(case.kind.as_str(), "coordinate" | "affine")
                || (case.kind == "coordinate") != case.equations.is_empty()
            {
                return Err(payload_error("a rule case of invalid shape"));
            }
            for term in &rule.rhs {
                validate_integral(&term.integral, arity)?;
                if term.integral.symbolic != rule.target.symbolic {
                    return Err(payload_error("a rule term with another symbolic layout"));
                }
            }
            for source in &rule.sources {
                validate_integral(&source.integral, arity)?;
                if source.shifts.len() != arity {
                    return Err(payload_error("a seed shift of another arity"));
                }
            }
        }
    }
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
    program: WireProgram,
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

fn decode(presentation: &Presentation, payload: &[u8]) -> Result<Decoded, PError> {
    let Structure {
        state,
        atoms,
        family,
        program,
        ordering,
    } = read_structure(payload)?;
    let arity = presentation.slots.len();
    if program.root_sector.len() != arity {
        return Err(payload_error(format!(
            "arity {}, presentation {} has {arity} slots",
            program.root_sector.len(),
            presentation.id.0
        )));
    }
    let table = DecodedCoefficientTable::import_generated(state, atoms, BinaryIoLimits::default())
        .map_err(payload_error)?;
    let rebuilt = family
        .to_family(
            &table,
            IntegralFamilyLimits::default(),
            BinaryIoLimits::default(),
        )
        .map_err(payload_error)?;
    let fingerprint = presentation.family.fingerprint();
    if rebuilt.fingerprint() != fingerprint || program.family_fingerprint != fingerprint {
        return Err(payload_error(format!(
            "generated for family {}, presentation {} has family {fingerprint}",
            program.family_fingerprint, presentation.id.0
        )));
    }
    let count = table.len();
    let ids = program
        .sectors
        .iter()
        .flat_map(|sector| &sector.rules)
        .flat_map(|rule| {
            let rhs = rule.rhs.iter().map(|term| &term.coefficient);
            let exclusions = rule.exclusions.iter().flatten();
            rule.case.equations.iter().chain(rhs).chain(exclusions)
        });
    for &id in ids {
        if id as usize >= count {
            return Err(payload_error("a coefficient id outside the table"));
        }
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

/// The coefficient variables of an `N`-slot storage: the context's, then
/// temporary padding variables (`capacity-dispatch` only).
fn storage_variables<const N: usize>(context: &IndexedCoefficientContext) -> Vec<PolyVariable> {
    let identity = context.one();
    let declared = identity.raw().get_variables();
    let mut variables = declared.as_ref().clone();
    let mut temporary = 0;
    while variables.len() < declared.len() + N - context.index_count() {
        let variable = PolyVariable::Temporary(temporary);
        temporary += 1;
        if !variables.contains(&variable) {
            variables.push(variable);
        }
    }
    variables
}

impl Decoded {
    fn coefficient(
        &self,
        variables: &[PolyVariable],
        id: u32,
    ) -> Result<Coefficient, CandidateReductionError> {
        let id = CoefficientId::try_from_index(id as usize).map_err(invalid)?;
        let value = self.table.coefficient(id).map_err(invalid)?;
        let physical = value.get_variables();
        if physical.as_slice() == variables {
            return Ok(value.clone());
        }
        if !cfg!(feature = "capacity-dispatch")
            || !variables.starts_with(physical.as_slice())
            || variables[physical.len()..]
                .iter()
                .any(|variable| !matches!(variable, PolyVariable::Temporary(_)))
        {
            return Err(invalid("a coefficient with another variable map"));
        }
        use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
        Ok(Coefficient::from_num_den(
            value
                .numerator
                .rearrange_with_growth(variables)
                .map_err(invalid)?,
            value
                .denominator
                .rearrange_with_growth(variables)
                .map_err(invalid)?,
            &symbolica::domains::integer::Z,
            true,
        ))
    }

    fn polynomial(
        &self,
        variables: &[PolyVariable],
        id: u32,
    ) -> Result<CoefficientPolynomial, CandidateReductionError> {
        let value = self.coefficient(variables, id)?;
        if !value.denominator.is_one() {
            return Err(invalid("an equation that is no polynomial"));
        }
        Ok(value.numerator)
    }

    /// The saved solution of `chart`, in `context`'s variables.
    fn input<const N: usize>(
        &self,
        chart: [bool; N],
        context: &CandidateOwnerContext<N>,
    ) -> Result<CandidateOwnerInput<N>, CandidateReductionError> {
        let program = &self.program;
        let (ordinal, record) = (program.sectors.iter().enumerate())
            .find(|(_, sector)| crate::storage_array(&sector.sector, false) == Some(chart))
            .ok_or_else(|| invalid("no chart of this sector"))?;
        let root: [bool; N] =
            crate::storage_array(&program.root_sector, false).ok_or_else(|| invalid("arity"))?;
        let variables = storage_variables::<N>(context.coefficient_context());
        let indices = context.index_variables();
        let rules = (record.rules.iter().enumerate())
            .map(|(rule_ordinal, rule)| {
                let mut rule = self.rule(rule, &variables, indices, &chart)?;
                let dispatched = program
                    .rule_dispatch
                    .binary_search_by_key(&(ordinal, rule_ordinal), |entry| {
                        (entry.sector, entry.rule)
                    })
                    .is_ok();
                if dispatched {
                    rule.dispatch_policy = RuleDispatchPolicy::AfterBaselinePartitionWholePiece;
                }
                Ok(rule)
            })
            .collect::<Result<Vec<_>, CandidateReductionError>>()?;
        let finite_residuals = (record.finite_residuals.iter())
            .map(integral::<N>)
            .collect::<Result<Vec<_>, _>>()?;
        let order = IntegralOrder::from_persisted_policy(chart, &self.ordering)
            .and_then(|order| {
                order.with_physical_arity(context.coefficient_context().index_count())
            })
            .map_err(invalid)?;
        Ok(CandidateOwnerInput {
            sector: chart,
            saved_root: root,
            ordering: self.ordering.clone(),
            solution: SectorSolution {
                order,
                max_numerator_rank: None,
                finite_case_policy: FiniteCasePolicy::SearchFinite,
                rules,
                finite_residuals,
                stats: SectorStats::default(),
            },
        })
    }

    fn rule<const N: usize>(
        &self,
        record: &WireRule,
        variables: &[PolyVariable],
        indices: &[usize; N],
        chart: &[bool; N],
    ) -> Result<SectorRule<N>, CandidateReductionError> {
        let arity = record.target.values.len();
        let mut fixed = [None; N];
        for (&axis, &value) in record.case.fixed_axes.iter().zip(&record.case.fixed_values) {
            fixed[axis] = Some(value);
        }
        // Storage padding axes are fixed at zero.
        for slot in fixed.iter_mut().skip(arity) {
            *slot = Some(0);
        }
        let face = CoordinateCase::new(fixed).map_err(invalid)?;
        if !face.is_in_sector(chart) {
            return Err(invalid("a rule face outside its chart"));
        }
        let case: Case<N> = if record.case.kind == "coordinate" {
            face.into()
        } else {
            let equations = (record.case.equations.iter())
                .map(|&id| self.polynomial(variables, id))
                .collect::<Result<Vec<_>, _>>()?;
            match AffineCase::from_coordinate(&face, &equations, indices, chart).map_err(invalid)? {
                AffineIntersection::Affine(case) if case.face().fixed() == &fixed => case.into(),
                _ => return Err(invalid("an affine case off its fixed face")),
            }
        };
        let target = integral::<N>(&record.target)?;
        if target != case.integral() {
            return Err(invalid("a rule target that is not its case's"));
        }
        let rhs = (record.rhs.iter())
            .map(|term| {
                Ok(Term {
                    integral: integral::<N>(&term.integral)?,
                    coefficient: self.coefficient(variables, term.coefficient)?,
                })
            })
            .collect::<Result<_, CandidateReductionError>>()?;
        let sources = (record.sources.iter())
            .map(|source| {
                Ok(SeedSource {
                    basis_row: source.basis_row,
                    seed: Seed {
                        integral: integral::<N>(&source.integral)?,
                        shifts: crate::storage_array(&source.shifts, 0)
                            .ok_or_else(|| invalid("seed arity"))?,
                    },
                })
            })
            .collect::<Result<_, CandidateReductionError>>()?;
        let branches = (record.exclusions.iter())
            .map(|branch| {
                branch
                    .iter()
                    .map(|&id| self.polynomial(variables, id))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, CandidateReductionError>>()?;
        Ok(SectorRule {
            candidate: RuleCandidate {
                case,
                target,
                rhs,
                sources,
                stats: SearchStats::default(),
            },
            exceptions: ExceptionalConditions { branches },
            dispatch_policy: RuleDispatchPolicy::default(),
        })
    }
}

fn integral<const N: usize>(record: &WireIntegral) -> Result<Integral<N>, CandidateReductionError> {
    if !crate::fits_storage(record.values.len(), N) {
        return Err(invalid("an integral beyond the storage arity"));
    }
    let mut powers = [Power::default(); N];
    for (axis, power) in powers.iter_mut().take(record.values.len()).enumerate() {
        *power = Power::new(record.symbolic[axis], record.values[axis]).map_err(invalid)?;
    }
    Ok(Integral::new(powers))
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
            AppliedOwnerStep::Applied(terms) => {
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

fn build<const N: usize>(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
) -> Result<Arc<dyn PLaneStep>, PError> {
    let decoded = decode(presentation, payload)?;
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
    Ok(Arc::new(CustomStep::<N> {
        presentation: presentation.id,
        arity,
        forced,
        support,
        charts,
        residuals: residuals.into_iter().collect(),
        programs,
    }))
}

/// The lane step of a Custom owner of `presentation` from its payload and
/// record. Fails closed: an arity outside the compiled runtime registry is
/// [`PError::Unsupported`] (build with `RUSTRED_RUNTIME_ARITIES`); the guard of
/// [`check_custom_owner`]; a payload of another family, schema or order, or
/// one that the record does not describe, is refused.
pub fn build_custom_step(
    presentation: &Presentation,
    payload: &[u8],
    meta: &CustomOwnerMeta,
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
                $($n => build::<$n>(presentation, payload, meta),)*
                storage => Err(PError::Unsupported(format!(
                    "custom owner arity {arity} has no compiled storage (width {storage}); \
                     rebuild with RUSTRED_RUNTIME_ARITIES including {arity}"
                ))),
            }
        };
    }
    with_candidate_runtime_arities!(dispatch)
}
