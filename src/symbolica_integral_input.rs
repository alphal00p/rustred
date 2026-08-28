//! Strict, topology-neutral parsing of one complete Symbolica `I(...)` input.
//!
//! This module owns syntax only.  Both the raw-expression CLI mode and the
//! hybrid TOML mode compile into [`NormalizedProjectInputV1`]; the explicit
//! TOML adapter constructs the same type through [`NormalizedProjectPartsV1`].
//! Concrete target powers and the numerator are retained as input data but do
//! not participate in universal parametric IBP generation.

use std::collections::BTreeMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::atom::{
    Atom, AtomCore, AtomView, FunctionBuilder, NamespacedSymbol, SymbolBuilder, UserData,
};
use symbolica::coefficient::CoefficientView;
use symbolica::id::{MatchSettings, Pattern};
use symbolica::parser::{Operator, ParseSettings, Token};
use symbolica::prelude::*;
use symbolica::state::Workspace;

use crate::algebra::{Coefficient, CoefficientContext, CoefficientContextError};
use crate::family::{AffineDenominator, IntegralFamily, IntegralFamilyError, IntegralFamilyLimits};
use crate::symbolica_affine_denominator::{
    CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorCompiler,
    SymbolicaAffineDenominatorError, SymbolicaAffineDenominatorLimits,
};

pub const RUSTRED_PROJECT_TOML_V1_SCHEMA: &str = "rustred.project.toml.v1";
pub const RUSTRED_SYMBOLICA_INTEGRAL_V1_SCHEMA: &str = "rustred.symbolica-integral.v1";
pub const RUSTRED_LOWERED_SYMBOLICA_PROJECT_V1_SCHEMA: &str =
    "rustred.lowered-symbolica-project.v1";

const RUSTRED_NAMESPACE_PREFIX: &str = "rustred::";
const DEFAULT_FAMILY_NAME: &str = "symbolica_integral";
// The caller-owned compact Atom and the normalized source clone coexist with
// as many as four field/Gram/canonical payload copies during normalization.
const MAX_COMPACT_SOURCE_ATOM_COPIES: usize = 6;
const MAX_NORMALIZED_FIELD_ATOM_COPIES: usize = 4;
// A generated function/variable/small-number node occupies far less than this
// in Symbolica's packed representation. Keeping a deliberately loose fixed
// envelope lets us preflight canonical scaffolding before constructing it.
const PACKED_ATOM_SCAFFOLD_BYTES_PER_NODE: usize = 64;

const RESERVED_NAMES: &[&str] = &[
    "I",
    "name",
    "loops",
    "externals",
    "parameters",
    "dimension",
    "prop",
    "power_shift",
    "gram",
    "numerator",
    "sp",
    "vec",
    "metric",
    "J",
];

/// Whether the exact base-field variable order was declared or inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterSourceV1 {
    Declared,
    Inferred,
}

impl ParameterSourceV1 {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Inferred => "inferred",
        }
    }
}

/// One ordered denominator expression before affine lowering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedPropagatorV1 {
    id: String,
    expression: Atom,
    target_power: i64,
    power_shift: Atom,
    power_shift_explicit: bool,
}

impl NormalizedPropagatorV1 {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn expression(&self) -> &Atom {
        &self.expression
    }

    pub const fn target_power(&self) -> i64 {
        self.target_power
    }

    pub const fn power_shift(&self) -> &Atom {
        &self.power_shift
    }

    pub const fn power_shift_was_explicit(&self) -> bool {
        self.power_shift_explicit
    }
}

/// Construction input shared by compact and explicit frontends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropagatorInputV1 {
    pub id: String,
    pub expression: Atom,
    pub target_power: i64,
    pub power_shift: Option<Atom>,
}

/// One upper-triangular external Gram entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalGramInputV1 {
    pub left: String,
    pub right: String,
    pub value: Atom,
}

/// Fully typed, but not yet validated, common project input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedProjectPartsV1 {
    pub name: Option<String>,
    /// `None` requests deterministic inference. `Some` is a strict ordered
    /// allowlist. Extra entries are retained as frontend/application metadata,
    /// but only parameters actually discovered in family-defining fields enter
    /// the derived family's coefficient field.
    pub parameters: Option<Vec<String>>,
    pub loop_momenta: Vec<String>,
    pub external_momenta: Vec<String>,
    pub dimension: Atom,
    pub propagators: Vec<PropagatorInputV1>,
    pub external_gram: Vec<ExternalGramInputV1>,
    pub numerator: Option<Atom>,
}

/// Textual propagator accepted by the explicit TOML frontend.
///
/// Expression strings are parsed only by
/// [`SymbolicaIntegralInputCompiler::compile_text_parts`], under the same
/// namespace, resource limits, and panic boundary as compact `I(...)` input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPropagatorInputV1 {
    pub id: String,
    pub expression: String,
    pub target_power: i64,
    pub power_shift: Option<String>,
}

/// Textual upper-triangular external Gram entry for explicit TOML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextExternalGramInputV1 {
    pub left: String,
    pub right: String,
    pub value: String,
}

/// Fully textual explicit-project seam used by the CLI adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextProjectPartsV1 {
    pub name: Option<String>,
    pub parameters: Option<Vec<String>>,
    pub loop_momenta: Vec<String>,
    pub external_momenta: Vec<String>,
    pub dimension: String,
    pub propagators: Vec<TextPropagatorInputV1>,
    pub external_gram: Vec<TextExternalGramInputV1>,
    pub numerator: Option<String>,
}

/// Normalized concrete target retained by `derive` without being processed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedTargetV1 {
    powers: Vec<i64>,
    numerator: Atom,
    numerator_explicit: bool,
}

impl NormalizedTargetV1 {
    pub fn powers(&self) -> &[i64] {
        &self.powers
    }

    pub const fn numerator(&self) -> &Atom {
        &self.numerator
    }

    pub const fn numerator_was_explicit(&self) -> bool {
        self.numerator_explicit
    }

    pub const fn derive_disposition(&self) -> &'static str {
        "not_processed_by_derive"
    }
}

/// Origin of the normalized syntax payload. Metadata is intentionally absent:
/// it belongs to the CLI document and never affects family identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NormalizedProjectSourceV1 {
    Symbolica { source: Atom },
    Explicit,
}

/// Resource policy for exact Symbolica-to-family lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicaProjectLoweringLimits {
    pub affine_denominator: SymbolicaAffineDenominatorLimits,
    pub integral_family: IntegralFamilyLimits,
}

impl Default for SymbolicaProjectLoweringLimits {
    fn default() -> Self {
        Self {
            affine_denominator: SymbolicaAffineDenominatorLimits::default(),
            integral_family: IntegralFamilyLimits::default(),
        }
    }
}

/// One named denominator after checked Symbolica evaluation and affine
/// projection. Source and canonical normalized Atoms are both retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredSymbolicaDenominatorV1 {
    id: String,
    compiled: CompiledSymbolicaAffineDenominator,
}

impl LoweredSymbolicaDenominatorV1 {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn source(&self) -> &Atom {
        self.compiled.source()
    }

    pub const fn normalized_expression(&self) -> &Atom {
        self.compiled.normalized_expression()
    }

    pub const fn compiled(&self) -> &CompiledSymbolicaAffineDenominator {
        &self.compiled
    }

    pub const fn affine_denominator(&self) -> &AffineDenominator {
        self.compiled.affine_denominator()
    }
}

/// Exact topology-neutral family ready for parametric IBP derivation.
#[derive(Clone, Debug)]
pub struct LoweredSymbolicaProjectV1 {
    normalized: NormalizedProjectInputV1,
    dimension: Coefficient,
    denominators: Vec<LoweredSymbolicaDenominatorV1>,
    family: IntegralFamily,
    limits: SymbolicaProjectLoweringLimits,
}

impl LoweredSymbolicaProjectV1 {
    pub const fn schema(&self) -> &'static str {
        RUSTRED_LOWERED_SYMBOLICA_PROJECT_V1_SCHEMA
    }

    pub const fn normalized(&self) -> &NormalizedProjectInputV1 {
        &self.normalized
    }

    pub const fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    pub fn denominators(&self) -> &[LoweredSymbolicaDenominatorV1] {
        &self.denominators
    }

    pub const fn family(&self) -> &IntegralFamily {
        &self.family
    }

    pub const fn limits(&self) -> SymbolicaProjectLoweringLimits {
        self.limits
    }

    pub fn into_parts(
        self,
    ) -> (
        NormalizedProjectInputV1,
        Vec<LoweredSymbolicaDenominatorV1>,
        IntegralFamily,
    ) {
        (self.normalized, self.denominators, self.family)
    }

    pub fn into_family(self) -> IntegralFamily {
        self.family
    }
}

/// Typed failures while lowering normalized syntax to an exact family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicaProjectLoweringError {
    CoefficientContext(CoefficientContextError),
    AffineDenominator(SymbolicaAffineDenominatorError),
    IntegralFamily(IntegralFamilyError),
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic {
        operation: &'static str,
    },
}

impl fmt::Display for SymbolicaProjectLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoefficientContext(error) => {
                write!(formatter, "invalid coefficient context: {error}")
            }
            Self::AffineDenominator(error) => {
                write!(formatter, "Symbolica affine lowering failed: {error}")
            }
            Self::IntegralFamily(error) => {
                write!(formatter, "integral-family authentication failed: {error}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not allocate {requested} units for {resource}"
            ),
            Self::SymbolicaPanic { operation } => {
                write!(formatter, "Symbolica panicked during {operation}")
            }
        }
    }
}

impl std::error::Error for SymbolicaProjectLoweringError {}

impl From<CoefficientContextError> for SymbolicaProjectLoweringError {
    fn from(error: CoefficientContextError) -> Self {
        Self::CoefficientContext(error)
    }
}

impl From<SymbolicaAffineDenominatorError> for SymbolicaProjectLoweringError {
    fn from(error: SymbolicaAffineDenominatorError) -> Self {
        Self::AffineDenominator(error)
    }
}

impl From<IntegralFamilyError> for SymbolicaProjectLoweringError {
    fn from(error: IntegralFamilyError) -> Self {
        Self::IntegralFamily(error)
    }
}

/// One syntax-authenticated project, common to every input frontend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedProjectInputV1 {
    schema: &'static str,
    source: NormalizedProjectSourceV1,
    name: String,
    name_explicit: bool,
    parameter_names: Vec<String>,
    operational_parameter_names: Vec<String>,
    parameter_source: ParameterSourceV1,
    loop_momenta: Vec<String>,
    external_momenta: Vec<String>,
    dimension: Atom,
    propagators: Vec<NormalizedPropagatorV1>,
    external_gram: Vec<Vec<Atom>>,
    target: NormalizedTargetV1,
    canonical: Atom,
    stats: SymbolicaIntegralInputStats,
    limits: SymbolicaIntegralInputLimits,
}

impl NormalizedProjectInputV1 {
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn source(&self) -> &NormalizedProjectSourceV1 {
        &self.source
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn name_was_explicit(&self) -> bool {
        self.name_explicit
    }

    pub fn parameter_names(&self) -> &[String] {
        &self.parameter_names
    }

    /// Canonical family base-field order used by lowering and fingerprints.
    ///
    /// For declared inputs, [`Self::parameter_names`] retains the complete
    /// source allowlist (including numerator-only extras), while this list is
    /// the sorted subset discovered in family-defining fields. Inferred inputs
    /// are already sorted, so both views agree.
    pub fn operational_parameter_names(&self) -> &[String] {
        &self.operational_parameter_names
    }

    pub const fn parameter_source(&self) -> ParameterSourceV1 {
        self.parameter_source
    }

    pub fn loop_momenta(&self) -> &[String] {
        &self.loop_momenta
    }

    pub fn external_momenta(&self) -> &[String] {
        &self.external_momenta
    }

    pub const fn dimension(&self) -> &Atom {
        &self.dimension
    }

    pub fn propagators(&self) -> &[NormalizedPropagatorV1] {
        &self.propagators
    }

    pub fn external_gram(&self) -> &[Vec<Atom>] {
        &self.external_gram
    }

    pub const fn target(&self) -> &NormalizedTargetV1 {
        &self.target
    }

    pub const fn canonical_atom(&self) -> &Atom {
        &self.canonical
    }

    pub fn canonical_string(&self) -> String {
        self.canonical.to_canonical_string()
    }

    /// Lower this normalized, topology-neutral declaration to the exact
    /// [`IntegralFamily`] consumed by parametric IBP generation.
    pub fn lower(
        &self,
        limits: SymbolicaProjectLoweringLimits,
    ) -> Result<LoweredSymbolicaProjectV1, SymbolicaProjectLoweringError> {
        guarded_lowering("normalized project lowering", || {
            lower_normalized_project(self.clone(), limits)
        })
    }

    /// Ownership-preserving variant of [`Self::lower`].
    pub fn into_lowered(
        self,
        limits: SymbolicaProjectLoweringLimits,
    ) -> Result<LoweredSymbolicaProjectV1, SymbolicaProjectLoweringError> {
        guarded_lowering("normalized project lowering", || {
            lower_normalized_project(self, limits)
        })
    }

    pub const fn stats(&self) -> SymbolicaIntegralInputStats {
        self.stats
    }

    pub const fn limits(&self) -> SymbolicaIntegralInputLimits {
        self.limits
    }

    /// Construct the common normalized DTO from an explicit frontend.
    pub fn try_from_parts(
        parts: NormalizedProjectPartsV1,
        limits: SymbolicaIntegralInputLimits,
    ) -> Result<Self, SymbolicaIntegralInputError> {
        guarded_symbolica("explicit input normalization", || {
            let compiler = SymbolicaIntegralInputCompiler::new(limits)?;
            normalize_parts(
                parts,
                NormalizedProjectSourceV1::Explicit,
                false,
                &compiler.syntax,
                SymbolicaIntegralInputStats::default(),
                limits,
            )
        })
    }
}

/// Aggregate parser and normalization limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicaIntegralInputLimits {
    pub max_input_bytes: usize,
    pub max_raw_parser_units: usize,
    pub max_raw_integer_digits: usize,
    pub max_abs_power: u32,
    pub max_preconversion_integer_bits: usize,
    /// Conservative aggregate integer-bit envelope of every packed Atom copy
    /// retained by one normalized project (source, normalized fields, Gram
    /// symmetry copies, and canonical rendering payload).
    pub max_retained_atom_integer_bits: usize,
    /// Conservative aggregate bytes of every packed Atom copy retained by one
    /// normalized project. This is distinct from textual input bytes because
    /// exact arithmetic can produce a much larger packed integer than its
    /// source spelling.
    pub max_retained_atom_bytes: usize,
    pub max_unique_identifiers: usize,
    pub max_atom_nodes: usize,
    pub max_nesting_depth: usize,
    pub max_clauses: usize,
    pub max_clause_arguments: usize,
    pub max_pattern_attempts: usize,
    pub max_pattern_matches: usize,
    pub max_label_bytes: usize,
    pub max_parameters: usize,
    pub max_momenta: usize,
    pub max_propagators: usize,
    pub max_gram_entries: usize,
    pub max_symbol_inspections: usize,
    pub max_canonical_nodes: usize,
}

impl Default for SymbolicaIntegralInputLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 4 * 1024 * 1024,
            max_raw_parser_units: 1_000_000,
            max_raw_integer_digits: 1_000_000,
            max_abs_power: 256,
            max_preconversion_integer_bits: 64_000_000,
            max_retained_atom_integer_bits: 256_000_000,
            max_retained_atom_bytes: 256 * 1024 * 1024,
            max_unique_identifiers: 16_384,
            max_atom_nodes: 250_000,
            max_nesting_depth: 128,
            max_clauses: 16_384,
            max_clause_arguments: 65_536,
            max_pattern_attempts: 150_000,
            max_pattern_matches: 16_384,
            max_label_bytes: 256,
            max_parameters: 4_096,
            max_momenta: 256,
            max_propagators: 16_384,
            max_gram_entries: 16_384,
            max_symbol_inspections: 1_000_000,
            max_canonical_nodes: 500_000,
        }
    }
}

/// Exact work census for one compact syntax compilation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymbolicaIntegralInputStats {
    input_bytes: usize,
    atom_nodes: usize,
    maximum_depth: usize,
    clauses: usize,
    clause_arguments: usize,
    pattern_attempts: usize,
    pattern_matches: usize,
    symbol_inspections: usize,
    inferred_parameters: usize,
    canonical_nodes: usize,
    preconversion_integer_bits: usize,
    retained_atom_integer_bits: usize,
    retained_atom_bytes: usize,
}

impl SymbolicaIntegralInputStats {
    pub const fn input_bytes(self) -> usize {
        self.input_bytes
    }
    pub const fn atom_nodes(self) -> usize {
        self.atom_nodes
    }
    pub const fn maximum_depth(self) -> usize {
        self.maximum_depth
    }
    pub const fn clauses(self) -> usize {
        self.clauses
    }
    pub const fn clause_arguments(self) -> usize {
        self.clause_arguments
    }
    pub const fn pattern_attempts(self) -> usize {
        self.pattern_attempts
    }
    pub const fn pattern_matches(self) -> usize {
        self.pattern_matches
    }
    pub const fn symbol_inspections(self) -> usize {
        self.symbol_inspections
    }
    pub const fn inferred_parameters(self) -> usize {
        self.inferred_parameters
    }
    pub const fn canonical_nodes(self) -> usize {
        self.canonical_nodes
    }
    /// Conservative exact-arithmetic work charged before Token-to-Atom
    /// conversion, aggregated across all explicit text fields.
    pub const fn preconversion_integer_bits(self) -> usize {
        self.preconversion_integer_bits
    }
    /// Conservative integer-bit envelope of all packed Atom copies retained by
    /// the normalized project.
    pub const fn retained_atom_integer_bits(self) -> usize {
        self.retained_atom_integer_bits
    }
    /// Conservative packed-byte envelope of all Atom copies retained by the
    /// normalized project.
    pub const fn retained_atom_bytes(self) -> usize {
        self.retained_atom_bytes
    }
}

/// Typed syntax/normalization failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicaIntegralInputError {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic {
        operation: &'static str,
    },
    Parse(String),
    UnsupportedToken {
        detail: String,
    },
    UnsafeRegisteredSymbol {
        symbol: String,
        reason: &'static str,
    },
    GrammarSymbol {
        name: &'static str,
        detail: String,
    },
    AttributedGrammarHead {
        name: &'static str,
    },
    WrongRoot,
    RootPatternMismatch,
    AmbiguousPattern {
        clause: usize,
    },
    UnknownClause {
        clause: usize,
        expression: Atom,
    },
    WrongClauseArity {
        clause: usize,
        kind: &'static str,
        expected: &'static str,
        actual: usize,
    },
    MissingClause {
        kind: &'static str,
    },
    DuplicateClause {
        kind: &'static str,
    },
    InvalidLabel {
        role: &'static str,
        expression: Atom,
    },
    InvalidLabelText {
        role: &'static str,
        label: String,
    },
    ReservedLabel {
        role: &'static str,
        label: String,
    },
    DuplicateLabel {
        role: &'static str,
        label: String,
    },
    CrossClassLabelCollision {
        label: String,
    },
    NoLoopMomenta,
    WrongPropagatorCount {
        expected: usize,
        actual: usize,
    },
    InvalidTargetPower {
        denominator: String,
        expression: Atom,
    },
    DuplicatePowerShift {
        denominator: String,
    },
    UnknownPowerShift {
        denominator: String,
    },
    UnknownExternalGramMomentum {
        momentum: String,
    },
    DiagonalGramOrientation,
    DuplicateExternalGram {
        left: String,
        right: String,
    },
    MissingExternalGram {
        left: String,
        right: String,
    },
    ForeignScalarSymbol {
        symbol: String,
    },
    ReservedScalarSymbol {
        symbol: String,
    },
    IdentifierUsedAsScalar {
        symbol: String,
    },
    UndeclaredScalarSymbol {
        symbol: String,
    },
    ConflictingParameterOverride,
}

impl fmt::Display for SymbolicaIntegralInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not allocate {requested} units for {resource}"
            ),
            Self::SymbolicaPanic { operation } => {
                write!(formatter, "Symbolica panicked during {operation}")
            }
            Self::Parse(detail) => write!(
                formatter,
                "could not parse Symbolica integral input: {detail}"
            ),
            Self::UnsupportedToken { detail } => {
                write!(formatter, "unsupported Symbolica token: {detail}")
            }
            Self::UnsafeRegisteredSymbol { symbol, reason } => write!(
                formatter,
                "registered Symbolica symbol {symbol} is unsafe for RustRed input: {reason}"
            ),
            Self::GrammarSymbol { name, detail } => write!(
                formatter,
                "could not register grammar head {name}: {detail}"
            ),
            Self::AttributedGrammarHead { name } => write!(
                formatter,
                "grammar head {name} must be an un-attributed plain Symbolica symbol"
            ),
            Self::WrongRoot => formatter.write_str("compact input must have the exact root I(...)"),
            Self::RootPatternMismatch => formatter.write_str(
                "compact I(...) root failed strict whole-expression pattern authentication",
            ),
            Self::AmbiguousPattern { clause } => write!(
                formatter,
                "I clause {clause} matched more than one grammar production"
            ),
            Self::UnknownClause { clause, expression } => {
                write!(formatter, "unknown I clause {clause}: {expression}")
            }
            Self::WrongClauseArity {
                clause,
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "I clause {clause} ({kind}) has {actual} arguments, expected {expected}"
            ),
            Self::MissingClause { kind } => write!(
                formatter,
                "compact I input is missing required {kind}(...) clause"
            ),
            Self::DuplicateClause { kind } => write!(
                formatter,
                "compact I input repeats singleton {kind}(...) clause"
            ),
            Self::InvalidLabel { role, expression } => write!(
                formatter,
                "{role} must be an unqualified Symbolica symbol, found {expression}"
            ),
            Self::InvalidLabelText { role, label } => {
                write!(formatter, "invalid {role} label {label:?}")
            }
            Self::ReservedLabel { role, label } => write!(
                formatter,
                "{role} label {label:?} is reserved by the v1 grammar"
            ),
            Self::DuplicateLabel { role, label } => {
                write!(formatter, "{role} label {label:?} is repeated")
            }
            Self::CrossClassLabelCollision { label } => write!(
                formatter,
                "label {label:?} is reused across incompatible input classes"
            ),
            Self::NoLoopMomenta => {
                formatter.write_str("loops(...) must contain at least one loop momentum")
            }
            Self::WrongPropagatorCount { expected, actual } => write!(
                formatter,
                "complete family needs {expected} propagators, found {actual}"
            ),
            Self::InvalidTargetPower {
                denominator,
                expression,
            } => write!(
                formatter,
                "target power for {denominator} is not an exact i64 integer: {expression}"
            ),
            Self::DuplicatePowerShift { denominator } => {
                write!(formatter, "power_shift for {denominator} is repeated")
            }
            Self::UnknownPowerShift { denominator } => write!(
                formatter,
                "power_shift refers to unknown propagator {denominator}"
            ),
            Self::UnknownExternalGramMomentum { momentum } => write!(
                formatter,
                "Gram clause refers to unknown external momentum {momentum}"
            ),
            Self::DiagonalGramOrientation => {
                formatter.write_str("internal diagonal Gram orientation failure")
            }
            Self::DuplicateExternalGram { left, right } => write!(
                formatter,
                "external Gram entry ({left},{right}) is repeated, including reversed duplicates"
            ),
            Self::MissingExternalGram { left, right } => {
                write!(formatter, "external Gram entry ({left},{right}) is missing")
            }
            Self::ForeignScalarSymbol { symbol } => write!(
                formatter,
                "scalar symbol {symbol} is outside the rustred namespace"
            ),
            Self::ReservedScalarSymbol { symbol } => write!(
                formatter,
                "scalar symbol {symbol} is reserved by the input grammar"
            ),
            Self::IdentifierUsedAsScalar { symbol } => write!(
                formatter,
                "family identifier {symbol} cannot also be used as a scalar parameter"
            ),
            Self::UndeclaredScalarSymbol { symbol } => write!(
                formatter,
                "scalar symbol {symbol} is not present in parameters(...)"
            ),
            Self::ConflictingParameterOverride => {
                formatter.write_str("hybrid TOML parameter override conflicts with parameters(...)")
            }
        }
    }
}

impl std::error::Error for SymbolicaIntegralInputError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClauseKind {
    Name,
    Loops,
    Externals,
    Parameters,
    Dimension,
    Prop,
    PowerShift,
    Gram,
    Numerator,
}

impl ClauseKind {
    const ALL: [Self; 9] = [
        Self::Name,
        Self::Loops,
        Self::Externals,
        Self::Parameters,
        Self::Dimension,
        Self::Prop,
        Self::PowerShift,
        Self::Gram,
        Self::Numerator,
    ];

    const fn head(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Loops => "loops",
            Self::Externals => "externals",
            Self::Parameters => "parameters",
            Self::Dimension => "dimension",
            Self::Prop => "prop",
            Self::PowerShift => "power_shift",
            Self::Gram => "gram",
            Self::Numerator => "numerator",
        }
    }

    const fn pattern(self) -> &'static str {
        match self {
            Self::Name => "name(rrv1x_)",
            Self::Loops => "loops(rrv1x__)",
            Self::Externals => "externals(rrv1x___)",
            Self::Parameters => "parameters(rrv1x___)",
            Self::Dimension => "dimension(rrv1x_)",
            Self::Prop => "prop(rrv1x_,rrv1y_,rrv1z_)",
            Self::PowerShift => "power_shift(rrv1x_,rrv1y_)",
            Self::Gram => "gram(rrv1x_,rrv1y_,rrv1z_)",
            Self::Numerator => "numerator(rrv1x_)",
        }
    }

    const fn expected_arity(self) -> &'static str {
        match self {
            Self::Name | Self::Dimension | Self::Numerator => "exactly 1",
            Self::Loops => "at least 1",
            Self::Externals | Self::Parameters => "zero or more",
            Self::Prop | Self::Gram => "exactly 3",
            Self::PowerShift => "exactly 2",
        }
    }

    fn from_head(head: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.head() == head)
    }
}

struct ClausePattern {
    kind: ClauseKind,
    pattern: Pattern,
}

struct IntegralSyntax {
    root: Symbol,
    heads: BTreeMap<&'static str, Symbol>,
    root_pattern: Pattern,
    clauses: Vec<ClausePattern>,
}

impl IntegralSyntax {
    fn try_new() -> Result<Self, SymbolicaIntegralInputError> {
        let mut heads = BTreeMap::new();
        for &name in RESERVED_NAMES {
            let symbol = plain_grammar_symbol(name)?;
            heads.insert(name, symbol);
        }
        authenticate_pattern_wildcard("rrv1x_", 1)?;
        authenticate_pattern_wildcard("rrv1x__", 2)?;
        authenticate_pattern_wildcard("rrv1x___", 3)?;
        authenticate_pattern_wildcard("rrv1y_", 1)?;
        authenticate_pattern_wildcard("rrv1z_", 1)?;
        authenticate_pattern_wildcard("rrv1args__", 2)?;
        let root = heads["I"];
        let root_pattern = parse_trusted_pattern("I(rrv1args__)")?;
        let mut clauses = Vec::new();
        clauses
            .try_reserve_exact(ClauseKind::ALL.len())
            .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                resource: "clause patterns",
                requested: ClauseKind::ALL.len(),
            })?;
        for kind in ClauseKind::ALL {
            clauses.push(ClausePattern {
                kind,
                pattern: parse_trusted_pattern(kind.pattern())?,
            });
        }
        Ok(Self {
            root,
            heads,
            root_pattern,
            clauses,
        })
    }

    fn head(&self, kind: ClauseKind) -> Symbol {
        self.heads[kind.head()]
    }
}

/// Compiler for the compact raw/hybrid Symbolica grammar.
pub struct SymbolicaIntegralInputCompiler {
    syntax: IntegralSyntax,
    limits: SymbolicaIntegralInputLimits,
}

impl SymbolicaIntegralInputCompiler {
    pub fn new(limits: SymbolicaIntegralInputLimits) -> Result<Self, SymbolicaIntegralInputError> {
        guarded_symbolica("grammar initialization", || {
            Ok(Self {
                syntax: IntegralSyntax::try_new()?,
                limits,
            })
        })
    }

    pub const fn limits(&self) -> SymbolicaIntegralInputLimits {
        self.limits
    }

    /// Parse one explicit-project scalar/tensor expression under this
    /// compiler's byte, node, depth, namespace, and panic policy.
    pub fn parse_expression(&self, source: &str) -> Result<Atom, SymbolicaIntegralInputError> {
        guarded_symbolica("explicit Symbolica expression parsing", || {
            let mut stats = SymbolicaIntegralInputStats::default();
            self.parse_expression_accumulating(source, RawSourceKind::GeneralExpression, &mut stats)
        })
    }

    /// Compile fully explicit textual fields into the same normalized DTO as
    /// compact raw and hybrid inputs. This is the only Symbolica parser seam
    /// needed by the explicit TOML adapter.
    pub fn compile_text_parts(
        &self,
        parts: TextProjectPartsV1,
    ) -> Result<NormalizedProjectInputV1, SymbolicaIntegralInputError> {
        guarded_symbolica("explicit project expression parsing", || {
            let TextProjectPartsV1 {
                name,
                parameters,
                loop_momenta,
                external_momenta,
                dimension,
                propagators,
                external_gram,
                numerator,
            } = parts;
            check_limit(
                "propagators",
                propagators.len(),
                self.limits.max_propagators,
            )?;
            check_limit(
                "external Gram entries",
                external_gram.len(),
                self.limits.max_gram_entries,
            )?;
            let mut stats = SymbolicaIntegralInputStats::default();
            let dimension = self.parse_expression_accumulating(
                &dimension,
                RawSourceKind::BaseCoefficientExpression,
                &mut stats,
            )?;

            let mut parsed_propagators = Vec::new();
            parsed_propagators
                .try_reserve_exact(propagators.len())
                .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                    resource: "explicit propagators",
                    requested: propagators.len(),
                })?;
            for propagator in propagators {
                let expression = self.parse_expression_accumulating(
                    &propagator.expression,
                    RawSourceKind::DenominatorExpression,
                    &mut stats,
                )?;
                let power_shift = match propagator.power_shift {
                    Some(source) => Some(self.parse_expression_accumulating(
                        &source,
                        RawSourceKind::BaseCoefficientExpression,
                        &mut stats,
                    )?),
                    None => None,
                };
                parsed_propagators.push(PropagatorInputV1 {
                    id: propagator.id,
                    expression,
                    target_power: propagator.target_power,
                    power_shift,
                });
            }

            let mut parsed_gram = Vec::new();
            parsed_gram
                .try_reserve_exact(external_gram.len())
                .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                    resource: "explicit external Gram entries",
                    requested: external_gram.len(),
                })?;
            for entry in external_gram {
                let value = self.parse_expression_accumulating(
                    &entry.value,
                    RawSourceKind::BaseCoefficientExpression,
                    &mut stats,
                )?;
                parsed_gram.push(ExternalGramInputV1 {
                    left: entry.left,
                    right: entry.right,
                    value,
                });
            }
            let numerator = match numerator {
                Some(source) => Some(self.parse_expression_accumulating(
                    &source,
                    RawSourceKind::TensorExpression,
                    &mut stats,
                )?),
                None => None,
            };
            normalize_parts(
                NormalizedProjectPartsV1 {
                    name,
                    parameters,
                    loop_momenta,
                    external_momenta,
                    dimension,
                    propagators: parsed_propagators,
                    external_gram: parsed_gram,
                    numerator,
                },
                NormalizedProjectSourceV1::Explicit,
                false,
                &self.syntax,
                stats,
                self.limits,
            )
        })
    }

    pub fn compile_str(
        &self,
        source: &str,
    ) -> Result<NormalizedProjectInputV1, SymbolicaIntegralInputError> {
        self.compile_str_with_parameter_override(source, None)
    }

    /// Hybrid TOML seam: an outer parameter list supplements an omitted
    /// `parameters(...)` clause. If both are present, exact ordered equality
    /// is accepted and any conflict is rejected.
    pub fn compile_str_with_parameter_override(
        &self,
        source: &str,
        parameter_override: Option<Vec<String>>,
    ) -> Result<NormalizedProjectInputV1, SymbolicaIntegralInputError> {
        guarded_symbolica("compact integral parsing", || {
            check_limit(
                "Symbolica integral input bytes",
                source.len(),
                self.limits.max_input_bytes,
            )?;
            let parsed =
                parse_authenticated_source(source, RawSourceKind::CompactIntegral, self.limits)?;
            self.compile_atom_with_parameter_override(
                parsed.atom.as_view(),
                source.len(),
                parsed.preconversion_integer_bits,
                parameter_override,
            )
        })
    }

    pub fn compile(
        &self,
        source: AtomView<'_>,
    ) -> Result<NormalizedProjectInputV1, SymbolicaIntegralInputError> {
        guarded_symbolica("compact integral normalization", || {
            self.compile_atom_with_parameter_override(source, 0, 0, None)
        })
    }

    fn compile_atom_with_parameter_override(
        &self,
        source: AtomView<'_>,
        input_bytes: usize,
        preconversion_integer_bits: usize,
        parameter_override: Option<Vec<String>>,
    ) -> Result<NormalizedProjectInputV1, SymbolicaIntegralInputError> {
        let mut stats = SymbolicaIntegralInputStats {
            input_bytes,
            preconversion_integer_bits,
            ..Default::default()
        };
        let source_census = census_atom_resources(
            source,
            self.limits.max_atom_nodes,
            self.limits.max_nesting_depth,
        )?;
        check_limit(
            "source Atom integer-bit copy envelope",
            checked_mul(
                "source Atom integer-bit copy envelope",
                source_census.integer_bits,
                MAX_COMPACT_SOURCE_ATOM_COPIES,
            )?,
            self.limits.max_retained_atom_integer_bits,
        )?;
        let source_copy_bytes = checked_add(
            "source Atom copy bytes",
            checked_mul(
                "source Atom copy bytes",
                source_census.packed_bytes,
                MAX_COMPACT_SOURCE_ATOM_COPIES,
            )?,
            checked_mul(
                "source Atom copy bytes",
                source_census.nodes,
                PACKED_ATOM_SCAFFOLD_BYTES_PER_NODE,
            )?,
        )?;
        check_limit(
            "source Atom copy bytes",
            source_copy_bytes,
            self.limits.max_retained_atom_bytes,
        )?;
        stats.atom_nodes = source_census.nodes;
        stats.maximum_depth = source_census.maximum_depth;
        stats.retained_atom_integer_bits = source_census.integer_bits;
        stats.retained_atom_bytes = source_copy_bytes;
        authenticate_atom_tree(source, self.limits)?;
        authenticate_whole_pattern(source, &self.syntax.root_pattern, &mut stats, self.limits)?;
        let AtomView::Fun(root) = source else {
            return Err(SymbolicaIntegralInputError::WrongRoot);
        };
        if root.get_symbol() != self.syntax.root || !root.get_symbol().get_attributes().is_empty() {
            return Err(SymbolicaIntegralInputError::WrongRoot);
        }
        check_limit("I clauses", root.get_nargs(), self.limits.max_clauses)?;
        stats.clauses = root.get_nargs();

        let mut name: Option<String> = None;
        let mut loops: Option<Vec<String>> = None;
        let mut externals: Option<Vec<String>> = None;
        let mut parameters: Option<Vec<String>> = None;
        let mut dimension: Option<Atom> = None;
        let mut props = Vec::<PropagatorInputV1>::new();
        let mut shifts = Vec::<(String, Atom)>::new();
        let mut grams = Vec::<ExternalGramInputV1>::new();
        let mut numerator: Option<Atom> = None;

        props
            .try_reserve(root.get_nargs().min(self.limits.max_propagators))
            .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                resource: "propagator clauses",
                requested: root.get_nargs().min(self.limits.max_propagators),
            })?;
        shifts
            .try_reserve(root.get_nargs().min(self.limits.max_propagators))
            .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                resource: "power-shift clauses",
                requested: root.get_nargs().min(self.limits.max_propagators),
            })?;
        grams
            .try_reserve(root.get_nargs().min(self.limits.max_gram_entries))
            .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                resource: "external Gram clauses",
                requested: root.get_nargs().min(self.limits.max_gram_entries),
            })?;

        for (clause_ordinal, clause) in root.iter().enumerate() {
            let kind = self.classify_clause(clause, clause_ordinal, &mut stats)?;
            let AtomView::Fun(function) = clause else {
                return Err(SymbolicaIntegralInputError::UnknownClause {
                    clause: clause_ordinal,
                    expression: clause.to_owned(),
                });
            };
            if function.get_symbol() != self.syntax.head(kind)
                || !function.get_symbol().get_attributes().is_empty()
            {
                return Err(SymbolicaIntegralInputError::UnknownClause {
                    clause: clause_ordinal,
                    expression: clause.to_owned(),
                });
            }
            let nargs = function.get_nargs();
            stats.clause_arguments =
                checked_add("clause arguments", stats.clause_arguments, nargs)?;
            check_limit(
                "clause arguments",
                stats.clause_arguments,
                self.limits.max_clause_arguments,
            )?;
            validate_clause_arity(kind, nargs, clause_ordinal)?;
            let args = collect_atom_views(function.iter(), nargs)?;
            match kind {
                ClauseKind::Name => set_singleton(
                    &mut name,
                    atom_label(args[0], "family name", self.limits)?,
                    "name",
                )?,
                ClauseKind::Loops => {
                    if loops.is_some() {
                        return Err(SymbolicaIntegralInputError::DuplicateClause { kind: "loops" });
                    }
                    loops = Some(collect_labels(&args, "loop momentum", self.limits)?);
                }
                ClauseKind::Externals => {
                    if externals.is_some() {
                        return Err(SymbolicaIntegralInputError::DuplicateClause {
                            kind: "externals",
                        });
                    }
                    externals = Some(collect_labels(&args, "external momentum", self.limits)?);
                }
                ClauseKind::Parameters => {
                    if parameters.is_some() {
                        return Err(SymbolicaIntegralInputError::DuplicateClause {
                            kind: "parameters",
                        });
                    }
                    parameters = Some(collect_labels(&args, "parameter", self.limits)?);
                }
                ClauseKind::Dimension => {
                    set_singleton(&mut dimension, args[0].to_owned(), "dimension")?
                }
                ClauseKind::Prop => {
                    let requested = checked_add("propagators", props.len(), 1)?;
                    check_limit("propagators", requested, self.limits.max_propagators)?;
                    let id = atom_label(args[0], "propagator", self.limits)?;
                    let target_power = atom_i64(args[2]).ok_or_else(|| {
                        SymbolicaIntegralInputError::InvalidTargetPower {
                            denominator: id.clone(),
                            expression: args[2].to_owned(),
                        }
                    })?;
                    props.push(PropagatorInputV1 {
                        id,
                        expression: args[1].to_owned(),
                        target_power,
                        power_shift: None,
                    });
                }
                ClauseKind::PowerShift => {
                    let id = atom_label(args[0], "power-shift propagator", self.limits)?;
                    if shifts.iter().any(|(candidate, _)| candidate == &id) {
                        return Err(SymbolicaIntegralInputError::DuplicatePowerShift {
                            denominator: id,
                        });
                    }
                    shifts.push((id, args[1].to_owned()));
                }
                ClauseKind::Gram => {
                    let requested = checked_add("external Gram entries", grams.len(), 1)?;
                    check_limit(
                        "external Gram entries",
                        requested,
                        self.limits.max_gram_entries,
                    )?;
                    grams.push(ExternalGramInputV1 {
                        left: atom_label(args[0], "Gram momentum", self.limits)?,
                        right: atom_label(args[1], "Gram momentum", self.limits)?,
                        value: args[2].to_owned(),
                    });
                }
                ClauseKind::Numerator => {
                    set_singleton(&mut numerator, args[0].to_owned(), "numerator")?
                }
            }
        }

        let loops = loops.ok_or(SymbolicaIntegralInputError::MissingClause { kind: "loops" })?;
        let externals =
            externals.ok_or(SymbolicaIntegralInputError::MissingClause { kind: "externals" })?;
        let dimension =
            dimension.ok_or(SymbolicaIntegralInputError::MissingClause { kind: "dimension" })?;
        if props.is_empty() {
            return Err(SymbolicaIntegralInputError::MissingClause { kind: "prop" });
        }
        if loops.is_empty() {
            return Err(SymbolicaIntegralInputError::NoLoopMomenta);
        }
        if let Some(override_names) = parameter_override {
            match &parameters {
                Some(internal) if *internal != override_names => {
                    return Err(SymbolicaIntegralInputError::ConflictingParameterOverride);
                }
                Some(_) => {}
                None => parameters = Some(override_names),
            }
        }
        for prop in &mut props {
            if let Some(position) = shifts.iter().position(|(id, _)| id == &prop.id) {
                let (_, shift) = shifts.remove(position);
                prop.power_shift = Some(shift);
            }
        }
        if let Some((denominator, _)) = shifts.into_iter().next() {
            return Err(SymbolicaIntegralInputError::UnknownPowerShift { denominator });
        }

        let parts = NormalizedProjectPartsV1 {
            name,
            parameters,
            loop_momenta: loops,
            external_momenta: externals,
            dimension,
            propagators: props,
            external_gram: grams,
            numerator,
        };
        normalize_parts(
            parts,
            NormalizedProjectSourceV1::Symbolica {
                source: source.to_owned(),
            },
            true,
            &self.syntax,
            stats,
            self.limits,
        )
    }

    fn parse_expression_accumulating(
        &self,
        source: &str,
        kind: RawSourceKind,
        stats: &mut SymbolicaIntegralInputStats,
    ) -> Result<Atom, SymbolicaIntegralInputError> {
        stats.input_bytes = checked_add(
            "explicit Symbolica expression bytes",
            stats.input_bytes,
            source.len(),
        )?;
        check_limit(
            "explicit Symbolica expression bytes",
            stats.input_bytes,
            self.limits.max_input_bytes,
        )?;
        let parsed = parse_authenticated_source(source, kind, self.limits)?;
        stats.preconversion_integer_bits = checked_add(
            "aggregate pre-conversion integer bits",
            stats.preconversion_integer_bits,
            parsed.preconversion_integer_bits,
        )?;
        check_limit(
            "aggregate pre-conversion integer bits",
            stats.preconversion_integer_bits,
            self.limits.max_preconversion_integer_bits,
        )?;
        stats.atom_nodes = checked_add(
            "explicit Symbolica expression nodes",
            stats.atom_nodes,
            parsed.census.nodes,
        )?;
        check_limit(
            "explicit Symbolica expression nodes",
            stats.atom_nodes,
            self.limits.max_atom_nodes,
        )?;
        stats.maximum_depth = stats.maximum_depth.max(parsed.census.maximum_depth);
        stats.retained_atom_integer_bits = checked_add(
            "aggregate explicit Atom integer bits",
            stats.retained_atom_integer_bits,
            parsed.census.integer_bits,
        )?;
        check_limit(
            "aggregate explicit Atom integer bits",
            stats.retained_atom_integer_bits,
            self.limits.max_retained_atom_integer_bits,
        )?;
        stats.retained_atom_bytes = checked_add(
            "aggregate explicit Atom bytes",
            stats.retained_atom_bytes,
            parsed.census.packed_bytes,
        )?;
        check_limit(
            "aggregate explicit Atom bytes",
            stats.retained_atom_bytes,
            self.limits.max_retained_atom_bytes,
        )?;
        Ok(parsed.atom)
    }

    fn classify_clause(
        &self,
        clause: AtomView<'_>,
        ordinal: usize,
        stats: &mut SymbolicaIntegralInputStats,
    ) -> Result<ClauseKind, SymbolicaIntegralInputError> {
        let settings = whole_match_settings();
        let mut found = None;
        for candidate in &self.syntax.clauses {
            stats.pattern_attempts = checked_add("pattern attempts", stats.pattern_attempts, 1)?;
            check_limit(
                "pattern attempts",
                stats.pattern_attempts,
                self.limits.max_pattern_attempts,
            )?;
            let mut matches = clause.pattern_match(&candidate.pattern, None, Some(&settings));
            if matches.next().is_some() {
                stats.pattern_matches = checked_add("pattern matches", stats.pattern_matches, 1)?;
                check_limit(
                    "pattern matches",
                    stats.pattern_matches,
                    self.limits.max_pattern_matches,
                )?;
                if matches.next().is_some() || found.replace(candidate.kind).is_some() {
                    return Err(SymbolicaIntegralInputError::AmbiguousPattern { clause: ordinal });
                }
            }
        }
        found.ok_or_else(|| SymbolicaIntegralInputError::UnknownClause {
            clause: ordinal,
            expression: clause.to_owned(),
        })
    }
}

fn lower_normalized_project(
    normalized: NormalizedProjectInputV1,
    limits: SymbolicaProjectLoweringLimits,
) -> Result<LoweredSymbolicaProjectV1, SymbolicaProjectLoweringError> {
    let coefficients =
        CoefficientContext::try_new(normalized.operational_parameter_names.iter().cloned())?;
    let bootstrap_gram = coefficient_matrix(
        normalized.external_momenta.len(),
        &coefficients,
        "bootstrap external Gram matrix",
    )?;
    let bootstrap = SymbolicaAffineDenominatorCompiler::try_new(
        coefficients.clone(),
        normalized.loop_momenta.clone(),
        normalized.external_momenta.clone(),
        bootstrap_gram,
        limits.affine_denominator,
    )?;
    let dimension = bootstrap.parse_base_coefficient(normalized.dimension.as_view())?;

    let mut external_gram = Vec::<Vec<Coefficient>>::new();
    external_gram
        .try_reserve_exact(normalized.external_gram.len())
        .map_err(|_| SymbolicaProjectLoweringError::AllocationFailure {
            resource: "lowered external Gram rows",
            requested: normalized.external_gram.len(),
        })?;
    for row in &normalized.external_gram {
        let mut lowered_row = Vec::<Coefficient>::new();
        lowered_row.try_reserve_exact(row.len()).map_err(|_| {
            SymbolicaProjectLoweringError::AllocationFailure {
                resource: "lowered external Gram row",
                requested: row.len(),
            }
        })?;
        for value in row {
            lowered_row.push(bootstrap.parse_base_coefficient(value.as_view())?);
        }
        external_gram.push(lowered_row);
    }

    let mut power_shifts = Vec::<Coefficient>::new();
    power_shifts
        .try_reserve_exact(normalized.propagators.len())
        .map_err(|_| SymbolicaProjectLoweringError::AllocationFailure {
            resource: "lowered power shifts",
            requested: normalized.propagators.len(),
        })?;
    for propagator in &normalized.propagators {
        power_shifts.push(bootstrap.parse_base_coefficient(propagator.power_shift.as_view())?);
    }

    // Rebuild with the authenticated physical Gram matrix before evaluating
    // any denominator containing external scalar products.
    let compiler = SymbolicaAffineDenominatorCompiler::try_new(
        coefficients.clone(),
        normalized.loop_momenta.clone(),
        normalized.external_momenta.clone(),
        external_gram.clone(),
        limits.affine_denominator,
    )?;
    let mut denominators = Vec::<LoweredSymbolicaDenominatorV1>::new();
    denominators
        .try_reserve_exact(normalized.propagators.len())
        .map_err(|_| SymbolicaProjectLoweringError::AllocationFailure {
            resource: "compiled Symbolica denominators",
            requested: normalized.propagators.len(),
        })?;
    let mut affine_denominators = Vec::<AffineDenominator>::new();
    affine_denominators
        .try_reserve_exact(normalized.propagators.len())
        .map_err(|_| SymbolicaProjectLoweringError::AllocationFailure {
            resource: "affine denominator rows",
            requested: normalized.propagators.len(),
        })?;
    for propagator in &normalized.propagators {
        let compiled = compiler.compile(propagator.expression.as_view())?;
        affine_denominators.push(compiled.affine_denominator().clone());
        denominators.push(LoweredSymbolicaDenominatorV1 {
            id: propagator.id.clone(),
            compiled,
        });
    }

    let family = IntegralFamily::new_with_limits(
        normalized.name.clone(),
        normalized.loop_momenta.clone(),
        normalized.external_momenta.clone(),
        coefficients,
        dimension.clone(),
        affine_denominators,
        external_gram,
        power_shifts,
        limits.integral_family,
    )?;
    Ok(LoweredSymbolicaProjectV1 {
        normalized,
        dimension,
        denominators,
        family,
        limits,
    })
}

fn coefficient_matrix(
    size: usize,
    coefficients: &CoefficientContext,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, SymbolicaProjectLoweringError> {
    size.checked_mul(size)
        .ok_or(SymbolicaProjectLoweringError::ResourceCountOverflow { resource })?;
    let mut matrix = Vec::new();
    matrix.try_reserve_exact(size).map_err(|_| {
        SymbolicaProjectLoweringError::AllocationFailure {
            resource,
            requested: size,
        }
    })?;
    for _ in 0..size {
        let mut row = Vec::new();
        row.try_reserve_exact(size).map_err(|_| {
            SymbolicaProjectLoweringError::AllocationFailure {
                resource,
                requested: size,
            }
        })?;
        for _ in 0..size {
            row.push(coefficients.zero());
        }
        matrix.push(row);
    }
    Ok(matrix)
}

fn normalize_parts(
    parts: NormalizedProjectPartsV1,
    source: NormalizedProjectSourceV1,
    compact: bool,
    syntax: &IntegralSyntax,
    mut stats: SymbolicaIntegralInputStats,
    limits: SymbolicaIntegralInputLimits,
) -> Result<NormalizedProjectInputV1, SymbolicaIntegralInputError> {
    check_limit(
        "propagators",
        parts.propagators.len(),
        limits.max_propagators,
    )?;
    check_limit(
        "external Gram entries",
        parts.external_gram.len(),
        limits.max_gram_entries,
    )?;
    check_limit("loop momenta", parts.loop_momenta.len(), limits.max_momenta)?;
    check_limit(
        "external momenta",
        parts.external_momenta.len(),
        limits.max_momenta,
    )?;
    if let Some(parameters) = &parts.parameters {
        check_limit("parameters", parameters.len(), limits.max_parameters)?;
    }
    // Count caller-owned or independently parsed field Atoms before Gram
    // symmetry and canonical rendering clone any of their packed payloads.
    let project_census = census_project_parts(&parts, limits)?;
    let canonical_scaffold_base = canonical_scaffold_base(&parts, limits)?;
    let source_census = match &source {
        NormalizedProjectSourceV1::Symbolica { source } => Some(census_atom_resources(
            source.as_view(),
            limits.max_atom_nodes,
            limits.max_nesting_depth,
        )?),
        NormalizedProjectSourceV1::Explicit => None,
    };
    let source_integer_bits = source_census.map_or(0, |census| census.integer_bits);
    let source_packed_bytes = source_census.map_or(0, |census| census.packed_bytes);
    let retained_atom_integer_bits = checked_add(
        "retained project Atom integer bits",
        source_integer_bits,
        checked_mul(
            "retained project Atom integer bits",
            project_census.retained_atom_integer_bits,
            MAX_NORMALIZED_FIELD_ATOM_COPIES,
        )?,
    )?;
    check_limit(
        "retained project Atom integer bits",
        retained_atom_integer_bits,
        limits.max_retained_atom_integer_bits,
    )?;
    let retained_atom_base_bytes = checked_add(
        "retained project Atom bytes",
        source_packed_bytes,
        checked_mul(
            "retained project Atom bytes",
            project_census.retained_atom_bytes,
            MAX_NORMALIZED_FIELD_ATOM_COPIES,
        )?,
    )?;
    check_limit(
        "retained project Atom bytes",
        retained_atom_base_bytes,
        limits.max_retained_atom_bytes,
    )?;
    authenticate_project_parts(&parts, limits)?;
    if stats.atom_nodes == 0 {
        stats.atom_nodes = project_census.atom_nodes;
        stats.maximum_depth = project_census.maximum_depth;
    }
    stats.retained_atom_integer_bits = retained_atom_integer_bits;
    stats.retained_atom_bytes = retained_atom_base_bytes;
    let name_explicit = parts.name.is_some();
    let name = parts.name.unwrap_or_else(|| DEFAULT_FAMILY_NAME.to_owned());
    validate_label_text(&name, "family name", limits)?;
    validate_ordered_labels(
        &parts.loop_momenta,
        "loop momentum",
        limits.max_momenta,
        limits,
    )?;
    validate_ordered_labels(
        &parts.external_momenta,
        "external momentum",
        limits.max_momenta,
        limits,
    )?;
    if parts.loop_momenta.is_empty() {
        return Err(SymbolicaIntegralInputError::NoLoopMomenta);
    }
    let momentum_count = parts
        .loop_momenta
        .len()
        .checked_add(parts.external_momenta.len())
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "momenta",
        })?;
    check_limit("momenta", momentum_count, limits.max_momenta)?;

    let mut momentum_names = Vec::<&str>::new();
    momentum_names
        .try_reserve_exact(momentum_count)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "momentum-name index",
            requested: momentum_count,
        })?;
    for label in parts.loop_momenta.iter().chain(&parts.external_momenta) {
        if momentum_names.iter().any(|candidate| *candidate == label) {
            return Err(SymbolicaIntegralInputError::CrossClassLabelCollision {
                label: label.clone(),
            });
        }
        momentum_names.push(label);
    }
    if momentum_names.iter().any(|candidate| *candidate == name) {
        return Err(SymbolicaIntegralInputError::CrossClassLabelCollision {
            label: name.clone(),
        });
    }

    let scalar_products =
        checked_scalar_product_count(parts.loop_momenta.len(), parts.external_momenta.len())?;
    if parts.propagators.len() != scalar_products {
        return Err(SymbolicaIntegralInputError::WrongPropagatorCount {
            expected: scalar_products,
            actual: parts.propagators.len(),
        });
    }
    let mut denominator_ids = Vec::<&str>::new();
    denominator_ids
        .try_reserve_exact(parts.propagators.len())
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "propagator-name index",
            requested: parts.propagators.len(),
        })?;
    for prop in &parts.propagators {
        validate_label_text(&prop.id, "propagator", limits)?;
        if denominator_ids
            .iter()
            .any(|candidate| *candidate == prop.id)
        {
            return Err(SymbolicaIntegralInputError::DuplicateLabel {
                role: "propagator",
                label: prop.id.clone(),
            });
        }
        if momentum_names.iter().any(|candidate| *candidate == prop.id) || prop.id == name {
            return Err(SymbolicaIntegralInputError::CrossClassLabelCollision {
                label: prop.id.clone(),
            });
        }
        denominator_ids.push(&prop.id);
    }

    let (external_gram, ordered_gram_atoms) =
        build_external_gram(&parts.external_momenta, parts.external_gram, limits)?;
    let scalar_atoms =
        family_scalar_atoms(&parts.dimension, &parts.propagators, &ordered_gram_atoms)?;
    let mut forbidden_identifiers = Vec::<&str>::new();
    let forbidden_count = checked_add("family identifiers", denominator_ids.len(), 1)?;
    forbidden_identifiers
        .try_reserve_exact(forbidden_count)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "family-identifier index",
            requested: forbidden_count,
        })?;
    forbidden_identifiers.extend(denominator_ids.iter().copied());
    forbidden_identifiers.push(&name);
    let discovered = discover_scalar_symbols(
        &scalar_atoms,
        &momentum_names,
        &forbidden_identifiers,
        &mut stats,
        limits,
    )?;
    let (parameter_names, operational_parameter_names, parameter_source) = match parts.parameters {
        Some(parameters) => {
            validate_ordered_labels(&parameters, "parameter", limits.max_parameters, limits)?;
            for parameter in &parameters {
                if momentum_names
                    .iter()
                    .any(|candidate| *candidate == parameter)
                    || forbidden_identifiers
                        .iter()
                        .any(|candidate| *candidate == parameter)
                {
                    return Err(SymbolicaIntegralInputError::CrossClassLabelCollision {
                        label: parameter.clone(),
                    });
                }
            }
            for symbol in &discovered {
                if !parameters.iter().any(|declared| declared == symbol) {
                    return Err(SymbolicaIntegralInputError::UndeclaredScalarSymbol {
                        symbol: symbol.clone(),
                    });
                }
            }
            (parameters, discovered, ParameterSourceV1::Declared)
        }
        None => {
            let parameters = discovered;
            stats.inferred_parameters = parameters.len();
            check_limit(
                "inferred parameters",
                parameters.len(),
                limits.max_parameters,
            )?;
            (parameters.clone(), parameters, ParameterSourceV1::Inferred)
        }
    };
    let mut operational_parameter_names = operational_parameter_names;
    operational_parameter_names.sort_unstable();

    let canonical_scaffold =
        canonical_scaffold_base.with_parameter_count(parameter_names.len(), limits)?;
    let prospective_canonical_nodes = checked_add(
        "canonical nodes",
        project_census.atom_nodes,
        canonical_scaffold.nodes,
    )?;
    check_limit(
        "canonical nodes",
        prospective_canonical_nodes,
        limits.max_canonical_nodes,
    )?;
    let canonical_scaffold_bytes = checked_mul(
        "canonical Atom scaffold bytes",
        canonical_scaffold.retained_scaffold_nodes,
        PACKED_ATOM_SCAFFOLD_BYTES_PER_NODE,
    )?;
    stats.retained_atom_bytes = checked_add(
        "retained project Atom bytes",
        stats.retained_atom_bytes,
        canonical_scaffold_bytes,
    )?;
    check_limit(
        "retained project Atom bytes",
        stats.retained_atom_bytes,
        limits.max_retained_atom_bytes,
    )?;
    stats.retained_atom_integer_bits = checked_add(
        "retained project Atom integer bits",
        stats.retained_atom_integer_bits,
        checked_mul(
            "canonical Atom scaffold integer bits",
            canonical_scaffold.numeric_nodes,
            u64::BITS as usize,
        )?,
    )?;
    check_limit(
        "retained project Atom integer bits",
        stats.retained_atom_integer_bits,
        limits.max_retained_atom_integer_bits,
    )?;

    let mut propagators = Vec::new();
    propagators
        .try_reserve_exact(parts.propagators.len())
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "normalized propagators",
            requested: parts.propagators.len(),
        })?;
    for prop in parts.propagators {
        let explicit = prop.power_shift.is_some();
        propagators.push(NormalizedPropagatorV1 {
            id: prop.id,
            expression: prop.expression,
            target_power: prop.target_power,
            power_shift: prop.power_shift.unwrap_or_else(|| Atom::num(0)),
            power_shift_explicit: explicit,
        });
    }
    let numerator_explicit = parts.numerator.is_some();
    let mut target_powers = Vec::new();
    target_powers
        .try_reserve_exact(propagators.len())
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "target powers",
            requested: propagators.len(),
        })?;
    target_powers.extend(propagators.iter().map(|prop| prop.target_power));
    let target = NormalizedTargetV1 {
        powers: target_powers,
        numerator: parts.numerator.unwrap_or_else(|| Atom::num(1)),
        numerator_explicit,
    };
    let canonical = render_canonical(
        syntax,
        &name,
        &parameter_names,
        &parts.loop_momenta,
        &parts.external_momenta,
        &parts.dimension,
        &propagators,
        &external_gram,
        &target,
        limits,
    )?;
    let (canonical_nodes, _) = census_atom(
        canonical.as_view(),
        limits.max_canonical_nodes,
        limits.max_nesting_depth,
    )?;
    stats.canonical_nodes = canonical_nodes;
    Ok(NormalizedProjectInputV1 {
        schema: if compact {
            RUSTRED_SYMBOLICA_INTEGRAL_V1_SCHEMA
        } else {
            RUSTRED_PROJECT_TOML_V1_SCHEMA
        },
        source,
        name,
        name_explicit,
        parameter_names,
        operational_parameter_names,
        parameter_source,
        loop_momenta: parts.loop_momenta,
        external_momenta: parts.external_momenta,
        dimension: parts.dimension,
        propagators,
        external_gram,
        target,
        canonical,
        stats,
        limits,
    })
}

fn family_scalar_atoms<'a>(
    dimension: &'a Atom,
    props: &'a [PropagatorInputV1],
    gram: &'a [Atom],
) -> Result<Vec<&'a Atom>, SymbolicaIntegralInputError> {
    let prop_slots =
        props
            .len()
            .checked_mul(2)
            .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "family scalar expressions",
            })?;
    let requested = checked_add(
        "family scalar expressions",
        checked_add("family scalar expressions", 1, prop_slots)?,
        gram.len(),
    )?;
    let mut atoms = Vec::new();
    atoms.try_reserve_exact(requested).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "family scalar expressions",
            requested,
        }
    })?;
    atoms.push(dimension);
    for prop in props {
        atoms.push(&prop.expression);
        if let Some(shift) = &prop.power_shift {
            atoms.push(shift);
        }
    }
    atoms.extend(gram);
    Ok(atoms)
}

fn discover_scalar_symbols(
    atoms: &[&Atom],
    momenta: &[&str],
    forbidden_identifiers: &[&str],
    stats: &mut SymbolicaIntegralInputStats,
    limits: SymbolicaIntegralInputLimits,
) -> Result<Vec<String>, SymbolicaIntegralInputError> {
    let mut output = Vec::<String>::new();
    let mut pending = Vec::<AtomView<'_>>::new();
    pending.try_reserve(atoms.len()).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "scalar-symbol traversal",
            requested: atoms.len(),
        }
    })?;
    pending.extend(atoms.iter().map(|atom| atom.as_view()));
    while let Some(atom) = pending.pop() {
        stats.symbol_inspections =
            checked_add("scalar symbol inspections", stats.symbol_inspections, 1)?;
        check_limit(
            "scalar symbol inspections",
            stats.symbol_inspections,
            limits.max_symbol_inspections,
        )?;
        match atom {
            AtomView::Var(variable) => {
                let label = symbol_label(variable.get_symbol(), "scalar parameter", limits)?;
                if RESERVED_NAMES.contains(&label.as_str()) {
                    return Err(SymbolicaIntegralInputError::ReservedScalarSymbol {
                        symbol: label,
                    });
                }
                if momenta.iter().any(|candidate| *candidate == label) {
                    continue;
                }
                if forbidden_identifiers
                    .iter()
                    .any(|candidate| *candidate == label)
                {
                    return Err(SymbolicaIntegralInputError::IdentifierUsedAsScalar {
                        symbol: label,
                    });
                }
                if !output.iter().any(|candidate| candidate == &label) {
                    let requested = checked_add("inferred parameters", output.len(), 1)?;
                    check_limit("inferred parameters", requested, limits.max_parameters)?;
                    output.try_reserve(1).map_err(|_| {
                        SymbolicaIntegralInputError::AllocationFailure {
                            resource: "inferred parameters",
                            requested,
                        }
                    })?;
                    output.push(label);
                }
            }
            AtomView::Fun(function) => append_pending_atoms(&mut pending, function.iter(), limits)?,
            AtomView::Pow(power) => append_pending_atoms(&mut pending, power.iter(), limits)?,
            AtomView::Mul(product) => append_pending_atoms(&mut pending, product.iter(), limits)?,
            AtomView::Add(sum) => append_pending_atoms(&mut pending, sum.iter(), limits)?,
            AtomView::Num(_) => {}
        }
    }
    output.sort_unstable();
    Ok(output)
}

fn append_pending_atoms<'a>(
    pending: &mut Vec<AtomView<'a>>,
    children: impl Iterator<Item = AtomView<'a>>,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    for child in children {
        let requested = checked_add("scalar-symbol traversal stack", pending.len(), 1)?;
        check_limit(
            "scalar-symbol traversal stack",
            requested,
            limits.max_atom_nodes,
        )?;
        pending
            .try_reserve(1)
            .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                resource: "scalar-symbol traversal stack",
                requested,
            })?;
        pending.push(child);
    }
    Ok(())
}

fn build_external_gram(
    external: &[String],
    entries: Vec<ExternalGramInputV1>,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(Vec<Vec<Atom>>, Vec<Atom>), SymbolicaIntegralInputError> {
    let expected = external
        .len()
        .checked_mul(external.len().checked_add(1).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "external Gram entries",
            },
        )?)
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "external Gram entries",
        })?
        / 2;
    check_limit("external Gram entries", expected, limits.max_gram_entries)?;
    check_limit(
        "supplied external Gram entries",
        entries.len(),
        limits.max_gram_entries,
    )?;
    let mut supplied = Vec::<((usize, usize), Atom)>::new();
    supplied.try_reserve_exact(entries.len()).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "supplied external Gram entries",
            requested: entries.len(),
        }
    })?;
    for entry in entries {
        let left = external
            .iter()
            .position(|name| name == &entry.left)
            .ok_or_else(
                || SymbolicaIntegralInputError::UnknownExternalGramMomentum {
                    momentum: entry.left.clone(),
                },
            )?;
        let right = external
            .iter()
            .position(|name| name == &entry.right)
            .ok_or_else(
                || SymbolicaIntegralInputError::UnknownExternalGramMomentum {
                    momentum: entry.right.clone(),
                },
            )?;
        let key = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        if supplied.iter().any(|(candidate, _)| *candidate == key) {
            return Err(SymbolicaIntegralInputError::DuplicateExternalGram {
                left: external[key.0].clone(),
                right: external[key.1].clone(),
            });
        }
        supplied.push((key, entry.value));
    }
    let mut matrix = Vec::<Vec<Atom>>::new();
    matrix.try_reserve_exact(external.len()).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "external Gram matrix rows",
            requested: external.len(),
        }
    })?;
    for _ in external {
        let mut row = Vec::<Atom>::new();
        row.try_reserve_exact(external.len()).map_err(|_| {
            SymbolicaIntegralInputError::AllocationFailure {
                resource: "external Gram matrix row",
                requested: external.len(),
            }
        })?;
        for _ in external {
            row.push(Atom::num(0));
        }
        matrix.push(row);
    }
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(expected).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "ordered external Gram",
            requested: expected,
        }
    })?;
    for left in 0..external.len() {
        for right in left..external.len() {
            let position = supplied
                .iter()
                .position(|(candidate, _)| *candidate == (left, right))
                .ok_or_else(|| SymbolicaIntegralInputError::MissingExternalGram {
                    left: external[left].clone(),
                    right: external[right].clone(),
                })?;
            let (_, value) = supplied.remove(position);
            matrix[left][right] = value.clone();
            matrix[right][left] = value.clone();
            ordered.push(value);
        }
    }
    debug_assert!(supplied.is_empty());
    Ok((matrix, ordered))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalScaffoldBase {
    nodes_without_parameters: usize,
    numeric_nodes: usize,
    extra_retained_numeric_nodes: usize,
    clause_arguments_without_parameters: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalScaffoldCensus {
    nodes: usize,
    numeric_nodes: usize,
    retained_scaffold_nodes: usize,
}

impl CanonicalScaffoldBase {
    fn with_parameter_count(
        self,
        parameters: usize,
        limits: SymbolicaIntegralInputLimits,
    ) -> Result<CanonicalScaffoldCensus, SymbolicaIntegralInputError> {
        let nodes = checked_add(
            "canonical scaffold nodes",
            self.nodes_without_parameters,
            parameters,
        )?;
        let clause_arguments = checked_add(
            "canonical clause arguments",
            self.clause_arguments_without_parameters,
            parameters,
        )?;
        check_limit(
            "canonical clause arguments",
            clause_arguments,
            limits.max_clause_arguments,
        )?;
        Ok(CanonicalScaffoldCensus {
            nodes,
            numeric_nodes: checked_add(
                "retained scaffold numeric nodes",
                self.numeric_nodes,
                self.extra_retained_numeric_nodes,
            )?,
            retained_scaffold_nodes: checked_add(
                "retained scaffold nodes",
                nodes,
                self.extra_retained_numeric_nodes,
            )?,
        })
    }
}

fn canonical_scaffold_base(
    parts: &NormalizedProjectPartsV1,
    limits: SymbolicaIntegralInputLimits,
) -> Result<CanonicalScaffoldBase, SymbolicaIntegralInputError> {
    let propagators = parts.propagators.len();
    let gram = parts
        .external_momenta
        .len()
        .checked_mul(parts.external_momenta.len().checked_add(1).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "canonical scaffold Gram entries",
            },
        )?)
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "canonical scaffold Gram entries",
        })?
        / 2;
    let clause_count = checked_add(
        "canonical clauses",
        checked_add(
            "canonical clauses",
            6,
            checked_mul("canonical clauses", propagators, 2)?,
        )?,
        gram,
    )?;
    check_limit("canonical clauses", clause_count, limits.max_clauses)?;

    let label_nodes = checked_add(
        "canonical label nodes",
        checked_add(
            "canonical label nodes",
            checked_add(
                "canonical label nodes",
                checked_add("canonical label nodes", 1, parts.loop_momenta.len())?,
                parts.external_momenta.len(),
            )?,
            checked_mul("canonical label nodes", propagators, 2)?,
        )?,
        checked_mul("canonical label nodes", gram, 2)?,
    )?;
    let default_shift_nodes = parts
        .propagators
        .iter()
        .filter(|propagator| propagator.power_shift.is_none())
        .count();
    let default_numerator_nodes = usize::from(parts.numerator.is_none());
    let numeric_nodes = checked_add(
        "canonical numeric nodes",
        checked_add("canonical numeric nodes", propagators, default_shift_nodes)?,
        default_numerator_nodes,
    )?;
    let nodes_without_parameters = checked_add(
        "canonical scaffold nodes",
        checked_add(
            "canonical scaffold nodes",
            checked_add("canonical scaffold nodes", 1, clause_count)?,
            label_nodes,
        )?,
        numeric_nodes,
    )?;
    let clause_arguments_without_parameters = checked_add(
        "canonical clause arguments",
        checked_add(
            "canonical clause arguments",
            checked_add(
                "canonical clause arguments",
                checked_add("canonical clause arguments", 3, parts.loop_momenta.len())?,
                parts.external_momenta.len(),
            )?,
            checked_mul("canonical clause arguments", propagators, 5)?,
        )?,
        checked_mul("canonical clause arguments", gram, 3)?,
    )?;
    Ok(CanonicalScaffoldBase {
        nodes_without_parameters,
        numeric_nodes,
        extra_retained_numeric_nodes: checked_add(
            "extra retained default numeric nodes",
            default_shift_nodes,
            default_numerator_nodes,
        )?,
        clause_arguments_without_parameters,
    })
}

#[allow(clippy::too_many_arguments)]
fn render_canonical(
    syntax: &IntegralSyntax,
    name: &str,
    parameters: &[String],
    loops: &[String],
    externals: &[String],
    dimension: &Atom,
    propagators: &[NormalizedPropagatorV1],
    gram: &[Vec<Atom>],
    target: &NormalizedTargetV1,
    limits: SymbolicaIntegralInputLimits,
) -> Result<Atom, SymbolicaIntegralInputError> {
    let mut clauses = Vec::new();
    let gram_count = externals
        .len()
        .checked_mul(externals.len().checked_add(1).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "canonical clauses",
            },
        )?)
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "canonical clauses",
        })?
        / 2;
    let prop_clauses = propagators.len().checked_mul(2).ok_or(
        SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "canonical clauses",
        },
    )?;
    let clause_count = checked_add(
        "canonical clauses",
        checked_add("canonical clauses", 6, prop_clauses)?,
        gram_count,
    )?;
    check_limit("canonical clauses", clause_count, limits.max_clauses)?;
    clauses.try_reserve_exact(clause_count).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "canonical clauses",
            requested: clause_count,
        }
    })?;
    clauses.push(function(
        syntax.head(ClauseKind::Name),
        [label_atom(name, limits)?],
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Loops),
        labels_to_atoms(loops, limits)?,
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Externals),
        labels_to_atoms(externals, limits)?,
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Parameters),
        labels_to_atoms(parameters, limits)?,
    ));
    clauses.push(function(
        syntax.head(ClauseKind::Dimension),
        [dimension.clone()],
    ));
    for prop in propagators {
        clauses.push(function(
            syntax.head(ClauseKind::Prop),
            [
                label_atom(&prop.id, limits)?,
                prop.expression.clone(),
                Atom::num(prop.target_power),
            ],
        ));
    }
    for prop in propagators {
        clauses.push(function(
            syntax.head(ClauseKind::PowerShift),
            [label_atom(&prop.id, limits)?, prop.power_shift.clone()],
        ));
    }
    for left in 0..externals.len() {
        for right in left..externals.len() {
            clauses.push(function(
                syntax.head(ClauseKind::Gram),
                [
                    label_atom(&externals[left], limits)?,
                    label_atom(&externals[right], limits)?,
                    gram[left][right].clone(),
                ],
            ));
        }
    }
    clauses.push(function(
        syntax.head(ClauseKind::Numerator),
        [target.numerator.clone()],
    ));
    Ok(function(syntax.root, clauses))
}

fn function(symbol: Symbol, args: impl IntoIterator<Item = Atom>) -> Atom {
    FunctionBuilder::new(symbol).add_args(args).finish()
}

fn labels_to_atoms(
    labels: &[String],
    limits: SymbolicaIntegralInputLimits,
) -> Result<Vec<Atom>, SymbolicaIntegralInputError> {
    let mut atoms = Vec::new();
    atoms.try_reserve_exact(labels.len()).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "canonical labels",
            requested: labels.len(),
        }
    })?;
    for label in labels {
        atoms.push(label_atom(label, limits)?);
    }
    Ok(atoms)
}

fn label_atom(
    label: &str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<Atom, SymbolicaIntegralInputError> {
    Ok(Atom::var(label_symbol(label, "label", limits)?))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawSourceKind {
    CompactIntegral,
    BaseCoefficientExpression,
    DenominatorExpression,
    TensorExpression,
    GeneralExpression,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpressionHeadPolicy {
    BaseCoefficient,
    Denominator,
    Tensor,
    General,
}

/// Bound raw parser work before Symbolica owns a recursive Token tree.  The
/// Token parser itself is iterative, but rejecting a deeply nested tree only
/// after construction would still recurse while that tree is dropped.
fn preflight_raw_source(
    source: &str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    let mut units = 0usize;
    let mut depth = 0usize;
    let mut maximum_depth = 0usize;
    let mut prefix_operator_depth = 0usize;
    let mut maximum_prefix_operator_depth = 0usize;
    let mut expecting_operand = true;
    let mut has_add_layer = false;
    let mut has_mul_layer = false;
    let mut has_power_layer = false;
    let mut integer_digits = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut lexical_units = 0usize;
    let mut lexical_run = RawLexicalRun::None;
    for character in source.chars() {
        units = checked_add("raw parser units", units, 1)?;
        check_limit("raw parser units", units, limits.max_raw_parser_units)?;

        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            integer_digits = 0;
            continue;
        }
        if character == '"' {
            if lexical_run == RawLexicalRun::Numeric {
                charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
            } else if lexical_run == RawLexicalRun::None {
                charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
            }
            lexical_run = RawLexicalRun::Identifier;
            quoted = true;
            integer_digits = 0;
            continue;
        }

        // Symbolica removes these separators while it is scanning one numeric
        // literal. They must therefore preserve both the numeric run and its
        // cumulative digit count in the pre-Token census.
        if lexical_run == RawLexicalRun::Numeric && matches!(character, '_' | '\u{2009}') {
            continue;
        }

        // In Symbolica mode a backslash is parser whitespace even though Rust
        // does not classify it as Unicode whitespace. Splitting the lexical
        // run here charges the implicit multiplication that the next operand
        // may create.
        if character == '\\' || character.is_whitespace() {
            lexical_run = RawLexicalRun::None;
            integer_digits = 0;
            continue;
        }

        if character.is_ascii_digit() {
            if lexical_run == RawLexicalRun::None {
                if !expecting_operand {
                    has_mul_layer = true;
                }
                charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
                lexical_run = RawLexicalRun::Numeric;
            }
            if lexical_run == RawLexicalRun::Numeric {
                integer_digits = checked_add("raw integer digits", integer_digits, 1)?;
                check_limit(
                    "raw integer digits",
                    integer_digits,
                    limits.max_raw_integer_digits,
                )?;
            } else {
                integer_digits = 0;
            }
            expecting_operand = false;
            prefix_operator_depth = 0;
            continue;
        }

        integer_digits = 0;
        if matches!(
            character,
            '+' | '-' | '*' | '/' | '^' | '(' | ')' | '[' | ']' | ','
        ) {
            let run_before_operator = lexical_run;
            lexical_run = RawLexicalRun::None;
            charge_raw_lexical_units(&mut lexical_units, 1, limits)?;
            match character {
                '(' | '[' => {
                    if !expecting_operand && run_before_operator != RawLexicalRun::Identifier {
                        has_mul_layer = true;
                    }
                    depth = checked_add("raw parser nesting depth", depth, 1)?;
                    maximum_depth = maximum_depth.max(depth);
                    check_limit(
                        "raw parser nesting depth",
                        checked_add("raw parser nesting depth", depth, prefix_operator_depth)?,
                        limits.max_nesting_depth,
                    )?;
                    expecting_operand = true;
                }
                ')' | ']' => {
                    depth = depth.saturating_sub(1);
                    expecting_operand = false;
                    prefix_operator_depth = 0;
                }
                ',' => {
                    expecting_operand = true;
                    prefix_operator_depth = 0;
                }
                '-' | '/' => {
                    if !expecting_operand {
                        if character == '-' {
                            has_add_layer = true;
                        } else {
                            has_mul_layer = true;
                        }
                    }
                    prefix_operator_depth = if expecting_operand {
                        checked_add("raw parser nesting depth", prefix_operator_depth, 1)?
                    } else {
                        1
                    };
                    maximum_prefix_operator_depth =
                        maximum_prefix_operator_depth.max(prefix_operator_depth);
                    check_limit(
                        "raw parser nesting depth",
                        checked_add("raw parser nesting depth", depth, prefix_operator_depth)?,
                        limits.max_nesting_depth,
                    )?;
                    expecting_operand = true;
                }
                '+' => {
                    if !expecting_operand {
                        has_add_layer = true;
                        prefix_operator_depth = 0;
                    }
                    expecting_operand = true;
                }
                '*' => {
                    if !expecting_operand {
                        has_mul_layer = true;
                    }
                    expecting_operand = true;
                    prefix_operator_depth = 0;
                }
                '^' => {
                    if !expecting_operand {
                        has_power_layer = true;
                    }
                    expecting_operand = true;
                    prefix_operator_depth = 0;
                }
                _ => unreachable!(),
            }
            continue;
        }

        if !expecting_operand && lexical_run != RawLexicalRun::Identifier {
            has_mul_layer = true;
        }
        if lexical_run == RawLexicalRun::Numeric {
            // Symbolica may insert an implicit multiplication between a
            // numeric literal and a following identifier.
            charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
        } else if lexical_run == RawLexicalRun::None {
            charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
        }
        lexical_run = RawLexicalRun::Identifier;
        if expecting_operand {
            // Any pending prefix chain has reached its operand. Its maximum
            // depth was charged while the chain was being constructed.
            expecting_operand = false;
            prefix_operator_depth = 0;
        } else {
            integer_digits = 0;
        }
    }
    let binary_layers = usize::from(has_add_layer)
        .checked_add(usize::from(has_mul_layer))
        .and_then(|layers| layers.checked_add(usize::from(has_power_layer)))
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "raw parser nesting depth",
        })?;
    let conservative_depth = checked_add(
        "raw parser nesting depth",
        checked_add(
            "raw parser nesting depth",
            maximum_depth,
            maximum_prefix_operator_depth,
        )?,
        binary_layers,
    )?;
    check_limit(
        "raw parser nesting depth",
        conservative_depth,
        limits.max_nesting_depth,
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawLexicalRun {
    None,
    Numeric,
    Identifier,
}

fn charge_raw_lexical_units(
    total: &mut usize,
    amount: usize,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    *total = checked_add("raw lexical tokens", *total, amount)?;
    check_limit("raw lexical tokens", *total, limits.max_atom_nodes)
}

fn validate_expression_token_tree(
    token: &Token,
    policy: ExpressionHeadPolicy,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    let mut pending = Vec::<&Token>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "position-sensitive Token validation",
            requested: 1,
        })?;
    pending.push(token);
    while let Some(current) = pending.pop() {
        let children: &[Token] = match current {
            Token::Fn(_, _, children) => {
                let Some(Token::ID(raw_head)) = children.first() else {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: "expression function head is not an identifier".to_owned(),
                    });
                };
                let head = rustred_identifier(raw_head.as_str())?;
                let allowed = match policy {
                    ExpressionHeadPolicy::BaseCoefficient => false,
                    ExpressionHeadPolicy::Denominator => head == "sp",
                    ExpressionHeadPolicy::Tensor | ExpressionHeadPolicy::General => {
                        matches!(head, "sp" | "vec" | "metric" | "J")
                    }
                };
                if !allowed {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!(
                            "function head {head:?} is not allowed in a {policy:?} expression"
                        ),
                    });
                }
                let arity = children.len().saturating_sub(1);
                if matches!(head, "sp" | "vec" | "metric") && arity != 2 {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("expression head {head:?} needs exactly 2 arguments"),
                    });
                }
                &children[1..]
            }
            Token::Op(_, _, _, children) => children,
            Token::ID(_) | Token::Number(_, _) => continue,
            other => {
                return Err(SymbolicaIntegralInputError::UnsupportedToken {
                    detail: other.to_string(),
                });
            }
        };
        for child in children {
            let requested = checked_add(
                "position-sensitive Token validation stack",
                pending.len(),
                1,
            )?;
            check_limit(
                "position-sensitive Token validation stack",
                requested,
                limits.max_atom_nodes,
            )?;
            pending
                .try_reserve(1)
                .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                    resource: "position-sensitive Token validation stack",
                    requested,
                })?;
            pending.push(child);
        }
    }
    Ok(())
}

/// Parse without permitting Symbolica to convert partial products into Atoms.
/// Every identifier is then mapped explicitly to an authenticated plain
/// `rustred` symbol before the sole controlled Token-to-Atom conversion.
struct AuthenticatedParsedSource {
    atom: Atom,
    preconversion_integer_bits: usize,
    census: AtomResourceCensus,
}

fn parse_authenticated_source(
    source: &str,
    kind: RawSourceKind,
    limits: SymbolicaIntegralInputLimits,
) -> Result<AuthenticatedParsedSource, SymbolicaIntegralInputError> {
    check_limit(
        "Symbolica source bytes",
        source.len(),
        limits.max_input_bytes,
    )?;
    if source.contains('\u{1b}') {
        return Err(SymbolicaIntegralInputError::UnsupportedToken {
            detail: "ANSI escape sequences are not accepted".to_owned(),
        });
    }
    preflight_raw_source(source, limits)?;
    let token = Token::parse(
        source,
        ParseSettings::symbolica().convert_mul_to_atom(false),
    )
    .map_err(SymbolicaIntegralInputError::Parse)?;
    validate_and_authenticate_token_tree(&token, limits)?;
    match kind {
        RawSourceKind::CompactIntegral => validate_compact_token_grammar(&token, limits)?,
        RawSourceKind::BaseCoefficientExpression => {
            validate_expression_token_tree(&token, ExpressionHeadPolicy::BaseCoefficient, limits)?;
        }
        RawSourceKind::DenominatorExpression => {
            validate_expression_token_tree(&token, ExpressionHeadPolicy::Denominator, limits)?;
        }
        RawSourceKind::TensorExpression => {
            validate_expression_token_tree(&token, ExpressionHeadPolicy::Tensor, limits)?;
        }
        RawSourceKind::GeneralExpression => {
            validate_expression_token_tree(&token, ExpressionHeadPolicy::General, limits)?;
        }
    }
    let preconversion_integer_bits = validate_numeric_preconversion_envelope(&token, limits)?;

    let mut validated_names = BTreeMap::<String, String>::new();
    let mut pending = Vec::<&Token>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "raw identifier traversal",
            requested: 1,
        })?;
    pending.push(&token);
    while let Some(current) = pending.pop() {
        match current {
            Token::ID(raw) => {
                if validated_names.contains_key(raw.as_str()) {
                    continue;
                }
                let logical = rustred_identifier(raw.as_str())?;
                let requested = checked_add("raw Symbolica identifiers", validated_names.len(), 1)?;
                check_limit(
                    "unique raw Symbolica identifiers",
                    requested,
                    limits.max_unique_identifiers,
                )?;
                validated_names.insert(raw.to_string(), logical.to_owned());
            }
            Token::Op(_, _, _, children) | Token::Fn(_, _, children) => {
                for child in children {
                    let requested = checked_add("raw identifier traversal", pending.len(), 1)?;
                    check_limit("raw identifier traversal", requested, limits.max_atom_nodes)?;
                    pending.try_reserve(1).map_err(|_| {
                        SymbolicaIntegralInputError::AllocationFailure {
                            resource: "raw identifier traversal",
                            requested,
                        }
                    })?;
                    pending.push(child);
                }
            }
            Token::Number(_, _) => {}
            other => {
                return Err(SymbolicaIntegralInputError::UnsupportedToken {
                    detail: other.to_string(),
                });
            }
        }
    }

    let mut symbols = BTreeMap::<String, Symbol>::new();
    for (raw, logical) in validated_names {
        symbols.insert(raw, authenticated_plain_symbol(&logical, limits)?);
    }
    let mut atom = Atom::new();
    Workspace::get_local()
        .with(|workspace| authenticated_token_to_atom(&token, workspace, &symbols, &mut atom))?;
    let census = census_atom_resources(
        atom.as_view(),
        limits.max_atom_nodes,
        limits.max_nesting_depth,
    )?;
    check_limit(
        "one parsed Atom integer bits",
        census.integer_bits,
        limits.max_retained_atom_integer_bits,
    )?;
    check_limit(
        "one parsed Atom bytes",
        census.packed_bytes,
        limits.max_retained_atom_bytes,
    )?;
    Ok(AuthenticatedParsedSource {
        atom,
        preconversion_integer_bits,
        census,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NumericBitEnvelope {
    numerator: usize,
    denominator: usize,
}

fn validate_numeric_preconversion_envelope(
    token: &Token,
    limits: SymbolicaIntegralInputLimits,
) -> Result<usize, SymbolicaIntegralInputError> {
    let mut aggregate = 0usize;
    analyze_numeric_token(token, &mut aggregate, limits)?;
    Ok(aggregate)
}

fn analyze_numeric_token(
    token: &Token,
    aggregate: &mut usize,
    limits: SymbolicaIntegralInputLimits,
) -> Result<Option<NumericBitEnvelope>, SymbolicaIntegralInputError> {
    let envelope = match token {
        Token::Number(number, false) => {
            let digits = raw_integer_digits(number).ok_or_else(|| {
                SymbolicaIntegralInputError::UnsupportedToken {
                    detail: format!("non-integral numeric literal {number}"),
                }
            })?;
            let significant = digits.trim_start_matches('0').len().max(1);
            let numerator = significant.checked_mul(4).ok_or(
                SymbolicaIntegralInputError::ResourceCountOverflow {
                    resource: "pre-conversion integer bits",
                },
            )?;
            Some(NumericBitEnvelope {
                numerator,
                denominator: 1,
            })
        }
        Token::ID(_) => None,
        Token::Fn(_, _, children) => {
            for argument in children.iter().skip(1) {
                analyze_numeric_token(argument, aggregate, limits)?;
            }
            None
        }
        Token::Op(_, _, operator, arguments) => match operator {
            Operator::Add | Operator::Mul => {
                let mut combined = None::<NumericBitEnvelope>;
                let mut all_numeric = true;
                for argument in arguments {
                    let child = analyze_numeric_token(argument, aggregate, limits)?;
                    let Some(child) = child else {
                        all_numeric = false;
                        continue;
                    };
                    if all_numeric {
                        combined = Some(match combined {
                            None => child,
                            Some(current) if *operator == Operator::Add => {
                                add_numeric_envelopes(current, child)?
                            }
                            Some(current) => multiply_numeric_envelopes(current, child)?,
                        });
                    }
                }
                if all_numeric { combined } else { None }
            }
            Operator::Neg => analyze_numeric_token(&arguments[0], aggregate, limits)?,
            Operator::Inv => {
                analyze_numeric_token(&arguments[0], aggregate, limits)?.map(|value| {
                    NumericBitEnvelope {
                        numerator: value.denominator,
                        denominator: value.numerator,
                    }
                })
            }
            Operator::Pow => {
                let base = analyze_numeric_token(&arguments[0], aggregate, limits)?;
                analyze_numeric_token(&arguments[1], aggregate, limits)?;
                match base {
                    Some(base) => {
                        let exponent = raw_i64(&arguments[1]).ok_or_else(|| {
                            SymbolicaIntegralInputError::UnsupportedToken {
                                detail: "authenticated power lost its exact integer exponent"
                                    .to_owned(),
                            }
                        })?;
                        let magnitude = usize::try_from(exponent.unsigned_abs()).map_err(|_| {
                            SymbolicaIntegralInputError::ResourceCountOverflow {
                                resource: "pre-conversion power magnitude",
                            }
                        })?;
                        if magnitude == 0 {
                            Some(NumericBitEnvelope {
                                numerator: 1,
                                denominator: 1,
                            })
                        } else {
                            let numerator = base.numerator.checked_mul(magnitude).ok_or(
                                SymbolicaIntegralInputError::ResourceCountOverflow {
                                    resource: "pre-conversion power numerator bits",
                                },
                            )?;
                            let denominator = base.denominator.checked_mul(magnitude).ok_or(
                                SymbolicaIntegralInputError::ResourceCountOverflow {
                                    resource: "pre-conversion power denominator bits",
                                },
                            )?;
                            if exponent < 0 {
                                Some(NumericBitEnvelope {
                                    numerator: denominator,
                                    denominator: numerator,
                                })
                            } else {
                                Some(NumericBitEnvelope {
                                    numerator,
                                    denominator,
                                })
                            }
                        }
                    }
                    None => None,
                }
            }
            Operator::Argument => None,
        },
        other => {
            return Err(SymbolicaIntegralInputError::UnsupportedToken {
                detail: other.to_string(),
            });
        }
    };
    if let Some(envelope) = envelope {
        let retained = envelope.numerator.checked_add(envelope.denominator).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "pre-conversion integer bits",
            },
        )?;
        check_limit(
            "pre-conversion integer bits",
            retained,
            limits.max_preconversion_integer_bits,
        )?;
        *aggregate = aggregate.checked_add(retained).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "aggregate pre-conversion integer bits",
            },
        )?;
        check_limit(
            "aggregate pre-conversion integer bits",
            *aggregate,
            limits.max_preconversion_integer_bits,
        )?;
    }
    Ok(envelope)
}

fn multiply_numeric_envelopes(
    left: NumericBitEnvelope,
    right: NumericBitEnvelope,
) -> Result<NumericBitEnvelope, SymbolicaIntegralInputError> {
    Ok(NumericBitEnvelope {
        numerator: left.numerator.checked_add(right.numerator).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "pre-conversion product numerator bits",
            },
        )?,
        denominator: left.denominator.checked_add(right.denominator).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "pre-conversion product denominator bits",
            },
        )?,
    })
}

fn add_numeric_envelopes(
    left: NumericBitEnvelope,
    right: NumericBitEnvelope,
) -> Result<NumericBitEnvelope, SymbolicaIntegralInputError> {
    let left_cross = left.numerator.checked_add(right.denominator).ok_or(
        SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "pre-conversion sum numerator bits",
        },
    )?;
    let right_cross = right.numerator.checked_add(left.denominator).ok_or(
        SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "pre-conversion sum numerator bits",
        },
    )?;
    Ok(NumericBitEnvelope {
        numerator: left_cross.max(right_cross).checked_add(1).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "pre-conversion sum numerator bits",
            },
        )?,
        denominator: left.denominator.checked_add(right.denominator).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "pre-conversion sum denominator bits",
            },
        )?,
    })
}

/// Convert an already authenticated Token tree with logarithmic symbol lookup.
/// Symbolica's public variable-map converter performs a linear name scan for
/// every identifier, which would reintroduce quadratic hostile-input work.
fn authenticated_token_to_atom(
    token: &Token,
    workspace: &Workspace,
    symbols: &BTreeMap<String, Symbol>,
    out: &mut Atom,
) -> Result<(), SymbolicaIntegralInputError> {
    match token {
        Token::Number(number, false) => {
            let integer = number.parse::<Integer>().map_err(|error| {
                SymbolicaIntegralInputError::Parse(format!(
                    "could not parse authenticated integer {number:?}: {error}"
                ))
            })?;
            out.to_num(integer);
        }
        Token::ID(raw) => {
            let symbol = symbols.get(raw.as_str()).ok_or_else(|| {
                SymbolicaIntegralInputError::Parse(format!(
                    "authenticated symbol map lost identifier {raw:?}"
                ))
            })?;
            out.to_var(*symbol);
        }
        Token::Op(_, _, operator, arguments) => match operator {
            Operator::Mul => {
                let factors = authenticated_token_arguments_to_atoms(
                    arguments,
                    workspace,
                    symbols,
                    "authenticated product factors",
                )?;
                Atom::mul_many(factors).as_view().clone_into(out);
            }
            Operator::Add => {
                let terms = authenticated_token_arguments_to_atoms(
                    arguments,
                    workspace,
                    symbols,
                    "authenticated sum terms",
                )?;
                Atom::add_many(terms).as_view().clone_into(out);
            }
            Operator::Pow => {
                let mut base = workspace.new_atom();
                authenticated_token_to_atom(&arguments[0], workspace, symbols, &mut base)?;
                let mut exponent = workspace.new_atom();
                authenticated_token_to_atom(&arguments[1], workspace, symbols, &mut exponent)?;
                let mut power = workspace.new_atom();
                power.to_pow(base.as_view(), exponent.as_view());
                power.as_view().normalize(workspace, out);
            }
            Operator::Neg => {
                let mut value = workspace.new_atom();
                authenticated_token_to_atom(&arguments[0], workspace, symbols, &mut value)?;
                value.as_view().neg_with_ws_into(workspace, out);
            }
            Operator::Inv => {
                let mut value = workspace.new_atom();
                authenticated_token_to_atom(&arguments[0], workspace, symbols, &mut value)?;
                let minus_one = workspace.new_num(-1);
                let mut power = workspace.new_atom();
                power.to_pow(value.as_view(), minus_one.as_view());
                power.as_view().normalize(workspace, out);
            }
            Operator::Argument => {
                return Err(SymbolicaIntegralInputError::UnsupportedToken {
                    detail: "argument operator reached authenticated conversion".to_owned(),
                });
            }
        },
        Token::Fn(_, _, children) => {
            let Some(Token::ID(raw_head)) = children.first() else {
                return Err(SymbolicaIntegralInputError::UnsupportedToken {
                    detail: "function without an authenticated identifier head".to_owned(),
                });
            };
            let head = symbols.get(raw_head.as_str()).ok_or_else(|| {
                SymbolicaIntegralInputError::Parse(format!(
                    "authenticated symbol map lost function head {raw_head:?}"
                ))
            })?;
            let arguments = authenticated_token_arguments_to_atoms(
                &children[1..],
                workspace,
                symbols,
                "authenticated function arguments",
            )?;
            FunctionBuilder::new(*head)
                .add_args(arguments)
                .finish()
                .as_view()
                .clone_into(out);
        }
        other => {
            return Err(SymbolicaIntegralInputError::UnsupportedToken {
                detail: other.to_string(),
            });
        }
    }
    Ok(())
}

/// Materialize one already-authenticated argument slice for Symbolica's public
/// n-ary builders. `validate_and_authenticate_token_tree` has bounded the
/// entire Token tree before this conversion, and the exact reserve keeps this
/// remaining allocation failure typed.
fn authenticated_token_arguments_to_atoms(
    arguments: &[Token],
    workspace: &Workspace,
    symbols: &BTreeMap<String, Symbol>,
    resource: &'static str,
) -> Result<Vec<Atom>, SymbolicaIntegralInputError> {
    let mut converted = Vec::new();
    converted.try_reserve_exact(arguments.len()).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource,
            requested: arguments.len(),
        }
    })?;
    for argument in arguments {
        let mut child = workspace.new_atom();
        authenticated_token_to_atom(argument, workspace, symbols, &mut child)?;
        converted.push(child.into_inner());
    }
    Ok(converted)
}

fn validate_and_authenticate_token_tree(
    token: &Token,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    let mut pending = Vec::<(&Token, usize)>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "raw Token census",
            requested: 1,
        })?;
    pending.push((token, 0));
    let mut nodes = 0usize;
    while let Some((current, depth)) = pending.pop() {
        nodes = checked_add("raw Token nodes", nodes, 1)?;
        check_limit("raw Token nodes", nodes, limits.max_atom_nodes)?;
        check_limit("raw Token nesting depth", depth, limits.max_nesting_depth)?;
        let children: &[Token] = match current {
            Token::Number(number, imaginary) => {
                let Some(digits) = raw_integer_digits(number) else {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("non-integral numeric literal {number}"),
                    });
                };
                if *imaginary {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("non-integral numeric literal {number}"),
                    });
                }
                check_limit(
                    "raw integer digits",
                    digits.len(),
                    limits.max_raw_integer_digits,
                )?;
                continue;
            }
            Token::ID(raw) => {
                let logical = rustred_identifier(raw.as_str())?;
                validate_identifier_text(logical, limits)?;
                continue;
            }
            Token::Op(more_left, more_right, operator, children) => {
                if *more_left || *more_right {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: "incomplete operator token".to_owned(),
                    });
                }
                let valid_arity = match operator {
                    Operator::Mul | Operator::Add => !children.is_empty(),
                    Operator::Pow => children.len() == 2,
                    Operator::Neg | Operator::Inv => children.len() == 1,
                    Operator::Argument => false,
                };
                if !valid_arity {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("invalid {operator} token arity {}", children.len()),
                    });
                }
                if *operator == Operator::Pow {
                    let exponent = raw_i64(&children[1]).ok_or_else(|| {
                        SymbolicaIntegralInputError::UnsupportedToken {
                            detail: format!(
                                "power exponent must be a syntactic exact signed integer, found {}",
                                children[1]
                            ),
                        }
                    })?;
                    let magnitude = exponent.unsigned_abs();
                    let requested = usize::try_from(magnitude).map_err(|_| {
                        SymbolicaIntegralInputError::ResourceCountOverflow {
                            resource: "raw absolute power",
                        }
                    })?;
                    check_limit(
                        "raw absolute power",
                        requested,
                        limits.max_abs_power as usize,
                    )?;
                }
                children
            }
            Token::Fn(more_right, bracket, children) => {
                if *more_right || *bracket || children.is_empty() {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: "incomplete, bracketed, or headless function token".to_owned(),
                    });
                }
                let Token::ID(raw_head) = &children[0] else {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: "function head is not an identifier".to_owned(),
                    });
                };
                let head = rustred_identifier(raw_head.as_str())?;
                if !RESERVED_NAMES.contains(&head) {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("function head {head:?} is outside the v1 grammar"),
                    });
                }
                children
            }
            Token::SpecialNumber(character) => {
                return Err(SymbolicaIntegralInputError::UnsupportedToken {
                    detail: format!("special number {character}"),
                });
            }
            Token::RationalPolynomial(_)
            | Token::ParsedMul(_)
            | Token::Start
            | Token::OpenParenthesis
            | Token::CloseParenthesis
            | Token::CloseBracket
            | Token::EOF => {
                return Err(SymbolicaIntegralInputError::UnsupportedToken {
                    detail: current.to_string(),
                });
            }
        };
        let child_depth =
            depth
                .checked_add(1)
                .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
                    resource: "raw Token nesting depth",
                })?;
        check_limit(
            "raw Token nesting depth",
            child_depth,
            limits.max_nesting_depth,
        )?;
        for child in children {
            let requested = checked_add("raw Token census stack", pending.len(), 1)?;
            check_limit("raw Token census stack", requested, limits.max_atom_nodes)?;
            pending
                .try_reserve(1)
                .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
                    resource: "raw Token census stack",
                    requested,
                })?;
            pending.push((child, child_depth));
        }
    }
    Ok(())
}

fn validate_compact_token_grammar(
    token: &Token,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    let (root, clauses) = raw_function_parts(token)?;
    if root != "I" {
        return Err(SymbolicaIntegralInputError::WrongRoot);
    }
    if clauses.is_empty() {
        return Err(SymbolicaIntegralInputError::WrongRoot);
    }
    check_limit("I clauses", clauses.len(), limits.max_clauses)?;
    for (ordinal, clause) in clauses.iter().enumerate() {
        let (head, arguments) = raw_function_parts(clause)?;
        let kind = ClauseKind::from_head(head).ok_or_else(|| {
            SymbolicaIntegralInputError::UnsupportedToken {
                detail: format!("unknown I clause {ordinal} head {head:?}"),
            }
        })?;
        validate_clause_arity(kind, arguments.len(), ordinal)?;
        match kind {
            ClauseKind::Name => {
                validate_raw_label(&arguments[0], "family name", limits)?;
            }
            ClauseKind::Loops => {
                for argument in arguments {
                    validate_raw_label(argument, "loop momentum", limits)?;
                }
            }
            ClauseKind::Externals => {
                for argument in arguments {
                    validate_raw_label(argument, "external momentum", limits)?;
                }
            }
            ClauseKind::Parameters => {
                for argument in arguments {
                    validate_raw_label(argument, "parameter", limits)?;
                }
            }
            ClauseKind::Dimension => {
                validate_expression_token_tree(
                    &arguments[0],
                    ExpressionHeadPolicy::BaseCoefficient,
                    limits,
                )?;
            }
            ClauseKind::Numerator => {
                validate_expression_token_tree(
                    &arguments[0],
                    ExpressionHeadPolicy::Tensor,
                    limits,
                )?;
            }
            ClauseKind::Prop => {
                let id = validate_raw_label(&arguments[0], "propagator", limits)?;
                validate_expression_token_tree(
                    &arguments[1],
                    ExpressionHeadPolicy::Denominator,
                    limits,
                )?;
                if raw_i64(&arguments[2]).is_none() {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("target power for {id} is not an exact i64 integer"),
                    });
                }
            }
            ClauseKind::PowerShift => {
                validate_raw_label(&arguments[0], "power-shift propagator", limits)?;
                validate_expression_token_tree(
                    &arguments[1],
                    ExpressionHeadPolicy::BaseCoefficient,
                    limits,
                )?;
            }
            ClauseKind::Gram => {
                validate_raw_label(&arguments[0], "Gram momentum", limits)?;
                validate_raw_label(&arguments[1], "Gram momentum", limits)?;
                validate_expression_token_tree(
                    &arguments[2],
                    ExpressionHeadPolicy::BaseCoefficient,
                    limits,
                )?;
            }
        }
    }
    Ok(())
}

fn raw_function_parts(token: &Token) -> Result<(&str, &[Token]), SymbolicaIntegralInputError> {
    let Token::Fn(false, false, children) = token else {
        return Err(SymbolicaIntegralInputError::WrongRoot);
    };
    let Some(Token::ID(raw_head)) = children.first() else {
        return Err(SymbolicaIntegralInputError::WrongRoot);
    };
    Ok((rustred_identifier(raw_head.as_str())?, &children[1..]))
}

fn validate_raw_label<'a>(
    token: &'a Token,
    role: &'static str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<&'a str, SymbolicaIntegralInputError> {
    let Token::ID(raw) = token else {
        return Err(SymbolicaIntegralInputError::UnsupportedToken {
            detail: format!("{role} is not an identifier"),
        });
    };
    let label = rustred_identifier(raw.as_str())?;
    validate_label_text(label, role, limits)?;
    Ok(label)
}

fn raw_i64(token: &Token) -> Option<i64> {
    match token {
        Token::Number(number, false) => number.parse::<i64>().ok(),
        Token::Op(false, false, Operator::Neg, arguments) if arguments.len() == 1 => {
            let Token::Number(number, false) = &arguments[0] else {
                return None;
            };
            let magnitude = number.parse::<u64>().ok()?;
            if magnitude == (i64::MAX as u64) + 1 {
                Some(i64::MIN)
            } else {
                i64::try_from(magnitude).ok()?.checked_neg()
            }
        }
        _ => None,
    }
}

fn raw_integer_digits(number: &str) -> Option<&str> {
    let digits = number.strip_prefix('-').unwrap_or(number);
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        None
    } else {
        Some(digits)
    }
}

fn rustred_identifier(raw: &str) -> Result<&str, SymbolicaIntegralInputError> {
    let logical = if let Some(label) = raw.strip_prefix("rustred::{}::") {
        label
    } else if let Some(label) = raw.strip_prefix(RUSTRED_NAMESPACE_PREFIX) {
        label
    } else if raw.contains("::") {
        return Err(SymbolicaIntegralInputError::ForeignScalarSymbol {
            symbol: raw.to_owned(),
        });
    } else {
        raw
    };
    if logical.is_empty() || logical.contains("::") || logical.ends_with('_') {
        return Err(SymbolicaIntegralInputError::InvalidLabelText {
            role: "Symbolica identifier",
            label: raw.to_owned(),
        });
    }
    Ok(logical)
}

fn validate_identifier_text(
    identifier: &str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    check_limit("identifier bytes", identifier.len(), limits.max_label_bytes)?;
    if identifier.is_empty() || identifier.contains("::") || identifier.ends_with('_') {
        return Err(SymbolicaIntegralInputError::InvalidLabelText {
            role: "Symbolica identifier",
            label: identifier.to_owned(),
        });
    }
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{identifier}");
    if NamespacedSymbol::try_parse(&qualified).is_none() {
        return Err(SymbolicaIntegralInputError::InvalidLabelText {
            role: "Symbolica identifier",
            label: identifier.to_owned(),
        });
    }
    Ok(())
}

fn authenticated_plain_symbol(
    identifier: &str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<Symbol, SymbolicaIntegralInputError> {
    validate_identifier_text(identifier, limits)?;
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{identifier}");
    let namespaced = NamespacedSymbol::try_parse(&qualified).ok_or_else(|| {
        SymbolicaIntegralInputError::InvalidLabelText {
            role: "Symbolica identifier",
            label: identifier.to_owned(),
        }
    })?;
    let symbol = SymbolBuilder::new(namespaced).build().map_err(|detail| {
        SymbolicaIntegralInputError::GrammarSymbol {
            name: "input symbol",
            detail: detail.to_string(),
        }
    })?;
    authenticate_symbol_properties(symbol, &qualified, 0)?;
    Ok(symbol)
}

fn authenticate_symbol_properties(
    symbol: Symbol,
    qualified: &str,
    wildcard_level: u8,
) -> Result<(), SymbolicaIntegralInputError> {
    let unsafe_symbol = |reason| SymbolicaIntegralInputError::UnsafeRegisteredSymbol {
        symbol: qualified.to_owned(),
        reason,
    };
    if symbol.get_name() != qualified {
        return Err(unsafe_symbol("canonical name mismatch"));
    }
    if symbol.get_wildcard_level() != wildcard_level {
        return Err(unsafe_symbol("unexpected wildcard level"));
    }
    if symbol.has_attributes() {
        return Err(unsafe_symbol("attributes or tags are present"));
    }
    if !symbol.is_exportable() {
        return Err(unsafe_symbol("a custom callback is registered"));
    }
    if !symbol.get_aliases().is_empty() {
        return Err(unsafe_symbol("aliases are registered"));
    }
    if !matches!(symbol.get_data(), UserData::None) {
        return Err(unsafe_symbol("user data is registered"));
    }
    Ok(())
}

fn plain_grammar_symbol(name: &'static str) -> Result<Symbol, SymbolicaIntegralInputError> {
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{name}");
    let namespaced = NamespacedSymbol::try_parse(&qualified).ok_or_else(|| {
        SymbolicaIntegralInputError::GrammarSymbol {
            name,
            detail: "invalid namespaced symbol".to_owned(),
        }
    })?;
    let symbol = SymbolBuilder::new(namespaced).build().map_err(|error| {
        SymbolicaIntegralInputError::GrammarSymbol {
            name,
            detail: error.to_string(),
        }
    })?;
    authenticate_symbol_properties(symbol, &qualified, 0)?;
    Ok(symbol)
}

fn authenticate_pattern_wildcard(
    name: &'static str,
    wildcard_level: u8,
) -> Result<Symbol, SymbolicaIntegralInputError> {
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{name}");
    let namespaced = NamespacedSymbol::try_parse(&qualified).ok_or_else(|| {
        SymbolicaIntegralInputError::GrammarSymbol {
            name: "pattern wildcard",
            detail: format!("invalid wildcard symbol {qualified}"),
        }
    })?;
    let symbol = SymbolBuilder::new(namespaced).build().map_err(|detail| {
        SymbolicaIntegralInputError::GrammarSymbol {
            name: "pattern wildcard",
            detail: detail.to_string(),
        }
    })?;
    authenticate_symbol_properties(symbol, &qualified, wildcard_level)?;
    Ok(symbol)
}

fn label_symbol(
    label: &str,
    role: &'static str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<Symbol, SymbolicaIntegralInputError> {
    validate_label_text(label, role, limits)?;
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{label}");
    let namespaced = NamespacedSymbol::try_parse(&qualified).ok_or_else(|| {
        SymbolicaIntegralInputError::InvalidLabelText {
            role,
            label: label.to_owned(),
        }
    })?;
    let symbol = SymbolBuilder::new(namespaced).build().map_err(|_| {
        SymbolicaIntegralInputError::InvalidLabelText {
            role,
            label: label.to_owned(),
        }
    })?;
    authenticate_symbol_properties(symbol, &qualified, 0)?;
    Ok(symbol)
}

fn parse_trusted_pattern(source: &'static str) -> Result<Pattern, SymbolicaIntegralInputError> {
    try_parse!(source, default_namespace = "rustred")
        .map(|atom| atom.to_pattern())
        .map_err(|error| SymbolicaIntegralInputError::GrammarSymbol {
            name: "pattern",
            detail: error.to_string(),
        })
}

fn whole_match_settings() -> MatchSettings {
    MatchSettings::new()
        .level_range((0, Some(0)))
        .level_is_tree_depth(true)
        .partial(false)
}

fn authenticate_whole_pattern(
    source: AtomView<'_>,
    pattern: &Pattern,
    stats: &mut SymbolicaIntegralInputStats,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    stats.pattern_attempts = checked_add("pattern attempts", stats.pattern_attempts, 1)?;
    check_limit(
        "pattern attempts",
        stats.pattern_attempts,
        limits.max_pattern_attempts,
    )?;
    let settings = whole_match_settings();
    let mut matches = source.pattern_match(pattern, None, Some(&settings));
    if matches.next().is_none() {
        return Err(SymbolicaIntegralInputError::RootPatternMismatch);
    }
    stats.pattern_matches = checked_add("pattern matches", stats.pattern_matches, 1)?;
    check_limit(
        "pattern matches",
        stats.pattern_matches,
        limits.max_pattern_matches,
    )?;
    if matches.next().is_some() {
        return Err(SymbolicaIntegralInputError::RootPatternMismatch);
    }
    Ok(())
}

fn validate_clause_arity(
    kind: ClauseKind,
    actual: usize,
    clause: usize,
) -> Result<(), SymbolicaIntegralInputError> {
    let valid = match kind {
        ClauseKind::Name | ClauseKind::Dimension | ClauseKind::Numerator => actual == 1,
        ClauseKind::Loops => actual >= 1,
        ClauseKind::Externals | ClauseKind::Parameters => true,
        ClauseKind::Prop | ClauseKind::Gram => actual == 3,
        ClauseKind::PowerShift => actual == 2,
    };
    if valid {
        Ok(())
    } else {
        Err(SymbolicaIntegralInputError::WrongClauseArity {
            clause,
            kind: kind.head(),
            expected: kind.expected_arity(),
            actual,
        })
    }
}

fn set_singleton<T>(
    slot: &mut Option<T>,
    value: T,
    kind: &'static str,
) -> Result<(), SymbolicaIntegralInputError> {
    if slot.replace(value).is_some() {
        Err(SymbolicaIntegralInputError::DuplicateClause { kind })
    } else {
        Ok(())
    }
}

fn collect_atom_views<'a>(
    arguments: impl Iterator<Item = AtomView<'a>>,
    count: usize,
) -> Result<Vec<AtomView<'a>>, SymbolicaIntegralInputError> {
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "clause arguments",
            requested: count,
        }
    })?;
    for argument in arguments {
        output.push(argument);
    }
    Ok(output)
}

fn collect_labels(
    args: &[AtomView<'_>],
    role: &'static str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<Vec<String>, SymbolicaIntegralInputError> {
    let mut labels = Vec::new();
    labels.try_reserve_exact(args.len()).map_err(|_| {
        SymbolicaIntegralInputError::AllocationFailure {
            resource: "input labels",
            requested: args.len(),
        }
    })?;
    for &arg in args {
        labels.push(atom_label(arg, role, limits)?);
    }
    Ok(labels)
}

fn atom_label(
    atom: AtomView<'_>,
    role: &'static str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<String, SymbolicaIntegralInputError> {
    let AtomView::Var(variable) = atom else {
        return Err(SymbolicaIntegralInputError::InvalidLabel {
            role,
            expression: atom.to_owned(),
        });
    };
    let label = symbol_label(variable.get_symbol(), role, limits)?;
    validate_label_text(&label, role, limits)?;
    Ok(label)
}

fn symbol_label(
    symbol: Symbol,
    _role: &'static str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<String, SymbolicaIntegralInputError> {
    let qualified = symbol.get_name();
    let Some(label) = qualified.strip_prefix(RUSTRED_NAMESPACE_PREFIX) else {
        return Err(SymbolicaIntegralInputError::ForeignScalarSymbol {
            symbol: qualified.to_owned(),
        });
    };
    if label.contains("::") || label.ends_with('_') {
        return Err(SymbolicaIntegralInputError::ForeignScalarSymbol {
            symbol: qualified.to_owned(),
        });
    }
    check_limit("label bytes", label.len(), limits.max_label_bytes)?;
    Ok(label.to_owned())
}

fn validate_label_text(
    label: &str,
    role: &'static str,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    check_limit("label bytes", label.len(), limits.max_label_bytes)?;
    if label.is_empty() || label.contains("::") || label.ends_with('_') {
        return Err(SymbolicaIntegralInputError::InvalidLabelText {
            role,
            label: label.to_owned(),
        });
    }
    if RESERVED_NAMES.contains(&label) {
        return Err(SymbolicaIntegralInputError::ReservedLabel {
            role,
            label: label.to_owned(),
        });
    }
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{label}");
    if NamespacedSymbol::try_parse(&qualified).is_none() {
        return Err(SymbolicaIntegralInputError::InvalidLabelText {
            role,
            label: label.to_owned(),
        });
    }
    Ok(())
}

fn validate_ordered_labels(
    labels: &[String],
    role: &'static str,
    maximum: usize,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    check_limit(role, labels.len(), maximum)?;
    for (ordinal, label) in labels.iter().enumerate() {
        validate_label_text(label, role, limits)?;
        if labels[..ordinal].iter().any(|candidate| candidate == label) {
            return Err(SymbolicaIntegralInputError::DuplicateLabel {
                role,
                label: label.clone(),
            });
        }
    }
    Ok(())
}

fn atom_i64(atom: AtomView<'_>) -> Option<i64> {
    let value = Rational::try_from(atom).ok()?;
    if !value.is_integer() {
        return None;
    }
    value.numerator().to_i64()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AtomResourceCensus {
    nodes: usize,
    maximum_depth: usize,
    /// A zero-allocation upper bound: every byte in an exact packed numeric
    /// node is charged as eight possible integer payload bits.
    integer_bits: usize,
    packed_bytes: usize,
}

fn census_atom_resources<'a>(
    atom: AtomView<'a>,
    max_nodes: usize,
    max_depth: usize,
) -> Result<AtomResourceCensus, SymbolicaIntegralInputError> {
    check_limit("Atom nodes", 1, max_nodes)?;
    let packed_bytes = atom.get_byte_size();
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "Atom census stack",
            requested: 1,
        })?;
    pending.push((atom, 0usize));
    let mut nodes = 1usize;
    let mut maximum_depth = 0usize;
    let mut integer_bits = 0usize;
    while let Some((current, depth)) = pending.pop() {
        check_limit("Atom nesting depth", depth, max_depth)?;
        maximum_depth = maximum_depth.max(depth);
        match current {
            AtomView::Fun(function) => {
                for child in function.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Pow(power) => {
                for child in power.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Mul(product) => {
                for child in product.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Add(sum) => {
                for child in sum.iter() {
                    schedule_atom_census_child(
                        &mut pending,
                        child,
                        depth,
                        &mut nodes,
                        max_nodes,
                        max_depth,
                    )?;
                }
            }
            AtomView::Num(number) => {
                match number.get_coeff_view() {
                    CoefficientView::Natural(_, _, imaginary_numerator, _)
                        if imaginary_numerator == 0 => {}
                    CoefficientView::Large(_, imaginary) if imaginary.is_zero() => {}
                    other => {
                        return Err(SymbolicaIntegralInputError::UnsupportedToken {
                            detail: format!(
                                "non-exact-real numeric Atom is outside the v1 grammar: {other:?}"
                            ),
                        });
                    }
                }
                integer_bits = checked_add(
                    "packed Atom integer bits",
                    integer_bits,
                    checked_mul(
                        "packed Atom integer bits",
                        current.get_byte_size(),
                        u8::BITS as usize,
                    )?,
                )?;
            }
            AtomView::Var(_) => {}
        }
    }
    Ok(AtomResourceCensus {
        nodes,
        maximum_depth,
        integer_bits,
        packed_bytes,
    })
}

fn schedule_atom_census_child<'a>(
    pending: &mut Vec<(AtomView<'a>, usize)>,
    child: AtomView<'a>,
    parent_depth: usize,
    nodes: &mut usize,
    max_nodes: usize,
    max_depth: usize,
) -> Result<(), SymbolicaIntegralInputError> {
    let child_depth =
        parent_depth
            .checked_add(1)
            .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "Atom nesting depth",
            })?;
    check_limit("Atom nesting depth", child_depth, max_depth)?;
    let requested =
        nodes
            .checked_add(1)
            .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "Atom nodes",
            })?;
    check_limit("Atom nodes", requested, max_nodes)?;
    pending
        .try_reserve(1)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "Atom census stack",
            requested,
        })?;
    pending.push((child, child_depth));
    *nodes = requested;
    Ok(())
}

fn census_atom(
    atom: AtomView<'_>,
    max_nodes: usize,
    max_depth: usize,
) -> Result<(usize, usize), SymbolicaIntegralInputError> {
    let census = census_atom_resources(atom, max_nodes, max_depth)?;
    Ok((census.nodes, census.maximum_depth))
}

fn census_project_parts(
    parts: &NormalizedProjectPartsV1,
    limits: SymbolicaIntegralInputLimits,
) -> Result<SymbolicaIntegralInputStats, SymbolicaIntegralInputError> {
    let mut stats = SymbolicaIntegralInputStats::default();
    let mut inspect = |atom: &Atom| -> Result<(), SymbolicaIntegralInputError> {
        let census = census_atom_resources(
            atom.as_view(),
            limits.max_atom_nodes,
            limits.max_nesting_depth,
        )?;
        stats.atom_nodes = checked_add(
            "explicit project Atom nodes",
            stats.atom_nodes,
            census.nodes,
        )?;
        check_limit(
            "explicit project Atom nodes",
            stats.atom_nodes,
            limits.max_atom_nodes,
        )?;
        stats.maximum_depth = stats.maximum_depth.max(census.maximum_depth);
        stats.retained_atom_integer_bits = checked_add(
            "aggregate project Atom integer bits",
            stats.retained_atom_integer_bits,
            census.integer_bits,
        )?;
        check_limit(
            "aggregate project Atom integer bits",
            stats.retained_atom_integer_bits,
            limits.max_retained_atom_integer_bits,
        )?;
        stats.retained_atom_bytes = checked_add(
            "aggregate project Atom bytes",
            stats.retained_atom_bytes,
            census.packed_bytes,
        )?;
        check_limit(
            "aggregate project Atom bytes",
            stats.retained_atom_bytes,
            limits.max_retained_atom_bytes,
        )?;
        Ok(())
    };
    inspect(&parts.dimension)?;
    for propagator in &parts.propagators {
        inspect(&propagator.expression)?;
        if let Some(shift) = &propagator.power_shift {
            inspect(shift)?;
        }
    }
    for entry in &parts.external_gram {
        inspect(&entry.value)?;
    }
    if let Some(numerator) = &parts.numerator {
        inspect(numerator)?;
    }
    Ok(stats)
}

fn authenticate_project_parts(
    parts: &NormalizedProjectPartsV1,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    authenticate_atom_tree(parts.dimension.as_view(), limits)?;
    validate_expression_atom_tree(
        parts.dimension.as_view(),
        ExpressionHeadPolicy::BaseCoefficient,
        &parts.loop_momenta,
        &parts.external_momenta,
        limits,
    )?;
    for propagator in &parts.propagators {
        authenticate_atom_tree(propagator.expression.as_view(), limits)?;
        validate_expression_atom_tree(
            propagator.expression.as_view(),
            ExpressionHeadPolicy::Denominator,
            &parts.loop_momenta,
            &parts.external_momenta,
            limits,
        )?;
        if let Some(shift) = &propagator.power_shift {
            authenticate_atom_tree(shift.as_view(), limits)?;
            validate_expression_atom_tree(
                shift.as_view(),
                ExpressionHeadPolicy::BaseCoefficient,
                &parts.loop_momenta,
                &parts.external_momenta,
                limits,
            )?;
        }
    }
    for entry in &parts.external_gram {
        authenticate_atom_tree(entry.value.as_view(), limits)?;
        validate_expression_atom_tree(
            entry.value.as_view(),
            ExpressionHeadPolicy::BaseCoefficient,
            &parts.loop_momenta,
            &parts.external_momenta,
            limits,
        )?;
    }
    if let Some(numerator) = &parts.numerator {
        authenticate_atom_tree(numerator.as_view(), limits)?;
        validate_expression_atom_tree(
            numerator.as_view(),
            ExpressionHeadPolicy::Tensor,
            &parts.loop_momenta,
            &parts.external_momenta,
            limits,
        )?;
    }
    Ok(())
}

fn validate_expression_atom_tree(
    atom: AtomView<'_>,
    policy: ExpressionHeadPolicy,
    loop_momenta: &[String],
    external_momenta: &[String],
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    let mut pending = Vec::<AtomView<'_>>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "position-sensitive Atom validation",
            requested: 1,
        })?;
    pending.push(atom);
    while let Some(current) = pending.pop() {
        match current {
            AtomView::Fun(function) => {
                let head = symbol_label(function.get_symbol(), "expression head", limits)?;
                let allowed = match policy {
                    ExpressionHeadPolicy::BaseCoefficient => false,
                    ExpressionHeadPolicy::Denominator => head == "sp",
                    ExpressionHeadPolicy::Tensor | ExpressionHeadPolicy::General => {
                        matches!(head.as_str(), "sp" | "vec" | "metric" | "J")
                    }
                };
                if !allowed {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!(
                            "function head {head:?} is not allowed in a {policy:?} expression"
                        ),
                    });
                }
                if matches!(head.as_str(), "sp" | "vec" | "metric") && function.get_nargs() != 2 {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("expression head {head:?} needs exactly 2 arguments"),
                    });
                }
                append_pending_atoms(&mut pending, function.iter(), limits)?;
            }
            AtomView::Pow(power) => append_pending_atoms(&mut pending, power.iter(), limits)?,
            AtomView::Mul(product) => {
                append_pending_atoms(&mut pending, product.iter(), limits)?;
            }
            AtomView::Add(sum) => append_pending_atoms(&mut pending, sum.iter(), limits)?,
            AtomView::Var(variable) => {
                if policy == ExpressionHeadPolicy::BaseCoefficient {
                    let label = symbol_label(variable.get_symbol(), "base coefficient", limits)?;
                    if loop_momenta.iter().any(|momentum| momentum == &label)
                        || external_momenta.iter().any(|momentum| momentum == &label)
                    {
                        return Err(SymbolicaIntegralInputError::UnsupportedToken {
                            detail: format!(
                                "momentum {label:?} is not allowed in a base-coefficient field"
                            ),
                        });
                    }
                }
            }
            AtomView::Num(_) => {}
        }
    }
    Ok(())
}

fn authenticate_atom_tree(
    atom: AtomView<'_>,
    limits: SymbolicaIntegralInputLimits,
) -> Result<(), SymbolicaIntegralInputError> {
    let mut pending = Vec::<AtomView<'_>>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| SymbolicaIntegralInputError::AllocationFailure {
            resource: "Atom symbol authentication",
            requested: 1,
        })?;
    pending.push(atom);
    let mut inspected = 0usize;
    while let Some(current) = pending.pop() {
        inspected = checked_add("Atom symbol authentication", inspected, 1)?;
        check_limit(
            "Atom symbol authentication",
            inspected,
            limits.max_symbol_inspections,
        )?;
        match current {
            AtomView::Var(variable) => {
                let symbol = variable.get_symbol();
                let qualified = symbol.get_name();
                let logical = rustred_identifier(qualified)?;
                validate_identifier_text(logical, limits)?;
                authenticate_symbol_properties(symbol, qualified, 0)?;
            }
            AtomView::Fun(function) => {
                let symbol = function.get_symbol();
                let qualified = symbol.get_name();
                let head = rustred_identifier(qualified)?;
                if !RESERVED_NAMES.contains(&head) {
                    return Err(SymbolicaIntegralInputError::UnsupportedToken {
                        detail: format!("function head {head:?} is outside the v1 grammar"),
                    });
                }
                authenticate_symbol_properties(symbol, qualified, 0)?;
                append_pending_atoms(&mut pending, function.iter(), limits)?;
            }
            AtomView::Pow(power) => append_pending_atoms(&mut pending, power.iter(), limits)?,
            AtomView::Mul(product) => append_pending_atoms(&mut pending, product.iter(), limits)?,
            AtomView::Add(sum) => append_pending_atoms(&mut pending, sum.iter(), limits)?,
            AtomView::Num(_) => {}
        }
    }
    Ok(())
}

fn checked_scalar_product_count(
    loops: usize,
    externals: usize,
) -> Result<usize, SymbolicaIntegralInputError> {
    let successor =
        loops
            .checked_add(1)
            .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "scalar products",
            })?;
    let triangular = if loops % 2 == 0 {
        (loops / 2).checked_mul(successor)
    } else {
        loops.checked_mul(successor / 2)
    }
    .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
        resource: "scalar products",
    })?;
    triangular
        .checked_add(loops.checked_mul(externals).ok_or(
            SymbolicaIntegralInputError::ResourceCountOverflow {
                resource: "scalar products",
            },
        )?)
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow {
            resource: "scalar products",
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaIntegralInputError> {
    left.checked_add(right)
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SymbolicaIntegralInputError> {
    left.checked_mul(right)
        .ok_or(SymbolicaIntegralInputError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaIntegralInputError> {
    if requested > limit {
        Err(SymbolicaIntegralInputError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn guarded_symbolica<T>(
    operation: &'static str,
    work: impl FnOnce() -> Result<T, SymbolicaIntegralInputError>,
) -> Result<T, SymbolicaIntegralInputError> {
    catch_unwind(AssertUnwindSafe(work))
        .map_err(|_| SymbolicaIntegralInputError::SymbolicaPanic { operation })?
}

fn guarded_lowering<T>(
    operation: &'static str,
    work: impl FnOnce() -> Result<T, SymbolicaProjectLoweringError>,
) -> Result<T, SymbolicaProjectLoweringError> {
    catch_unwind(AssertUnwindSafe(work))
        .map_err(|_| SymbolicaProjectLoweringError::SymbolicaPanic { operation })?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiler() -> SymbolicaIntegralInputCompiler {
        SymbolicaIntegralInputCompiler::new(SymbolicaIntegralInputLimits::default())
            .expect("plain RustRed input grammar must initialize")
    }

    fn compiler_with(
        update: impl FnOnce(&mut SymbolicaIntegralInputLimits),
    ) -> SymbolicaIntegralInputCompiler {
        let mut limits = SymbolicaIntegralInputLimits::default();
        update(&mut limits);
        SymbolicaIntegralInputCompiler::new(limits)
            .expect("bounded RustRed input grammar must initialize")
    }

    fn one_loop_source(target: i64, numerator: &str) -> String {
        format!(
            "I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,{target}),numerator({numerator}))"
        )
    }

    #[test]
    fn omitted_parameters_are_inferred_only_from_family_scalars() {
        let normalized = compiler()
            .compile_str(&one_loop_source(2, "vec(k,mu)*tensor_only"))
            .expect("compact one-loop family should normalize");
        assert_eq!(normalized.parameter_source(), ParameterSourceV1::Inferred);
        assert_eq!(
            normalized.parameter_names(),
            &["d".to_owned(), "m2".to_owned()]
        );
        assert!(!normalized.parameter_names().iter().any(|name| name == "mu"));
        assert!(
            !normalized
                .parameter_names()
                .iter()
                .any(|name| name == "tensor_only")
        );
    }

    #[test]
    fn hybrid_parameter_override_must_match_an_internal_declaration() {
        let compiler = compiler();
        let source = "I(loops(k),externals(),parameters(m2,d),dimension(d),prop(D1,k^2-m2,1))";
        let matching = compiler
            .compile_str_with_parameter_override(
                source,
                Some(vec!["m2".to_owned(), "d".to_owned()]),
            )
            .expect("identical ordered declarations should agree");
        assert_eq!(
            matching.parameter_names(),
            &["m2".to_owned(), "d".to_owned()]
        );
        assert_eq!(
            matching.operational_parameter_names(),
            &["d".to_owned(), "m2".to_owned()]
        );

        let conflict = compiler.compile_str_with_parameter_override(
            source,
            Some(vec!["d".to_owned(), "m2".to_owned()]),
        );
        assert!(matches!(
            conflict,
            Err(SymbolicaIntegralInputError::ConflictingParameterOverride)
        ));
    }

    #[test]
    fn canonical_rendering_round_trips_once_from_fully_qualified_names() {
        let compiler = compiler();
        let normalized = compiler
            .compile_str(
                "I(loops(k),externals(),parameters(d,m2,tensor_only),dimension(d),prop(D1,k^2-m2,1),numerator(tensor_only*vec(k,mu)))",
            )
            .expect("compact input should normalize");
        let canonical = normalized.canonical_string();
        assert!(canonical.contains("rustred::"));
        let round_trip = compiler
            .compile_str(&canonical)
            .expect("fully-qualified canonical output must be valid raw input");
        assert_eq!(round_trip.canonical_atom(), normalized.canonical_atom());
        assert_eq!(round_trip.parameter_names(), normalized.parameter_names());
    }

    #[test]
    fn raw_and_text_parts_share_canonical_family_identity() {
        let compiler = compiler();
        let raw = compiler
            .compile_str(&one_loop_source(1, "1"))
            .expect("raw family should normalize");
        let explicit = compiler
            .compile_text_parts(TextProjectPartsV1 {
                name: None,
                parameters: None,
                loop_momenta: vec!["k".to_owned()],
                external_momenta: vec![],
                dimension: "d".to_owned(),
                propagators: vec![TextPropagatorInputV1 {
                    id: "D1".to_owned(),
                    expression: "k^2-m2".to_owned(),
                    target_power: 1,
                    power_shift: None,
                }],
                external_gram: vec![],
                numerator: None,
            })
            .expect("text fields should normalize");
        assert_eq!(raw.canonical_atom(), explicit.canonical_atom());
        let raw_family = raw
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("raw family should lower");
        let explicit_family = explicit
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("explicit family should lower");
        assert_eq!(
            raw_family.family().fingerprint_ref(),
            explicit_family.family().fingerprint_ref()
        );
        assert_eq!(
            raw_family.denominators()[0].source(),
            raw_family.normalized().propagators()[0].expression(),
        );
    }

    #[test]
    fn outer_only_parameters_and_all_three_frontends_converge() {
        let compiler = compiler();
        let source = one_loop_source(1, "1");
        let raw = compiler
            .compile_str(&source)
            .expect("raw inferred family should normalize");
        let hybrid = compiler
            .compile_str_with_parameter_override(
                &source,
                Some(vec!["d".to_owned(), "m2".to_owned()]),
            )
            .expect("an outer-only strict allowlist should normalize");
        let explicit = compiler
            .compile_text_parts(TextProjectPartsV1 {
                name: None,
                parameters: Some(vec!["d".to_owned(), "m2".to_owned()]),
                loop_momenta: vec!["k".to_owned()],
                external_momenta: vec![],
                dimension: "d".to_owned(),
                propagators: vec![TextPropagatorInputV1 {
                    id: "D1".to_owned(),
                    expression: "k^2-m2".to_owned(),
                    target_power: 1,
                    power_shift: None,
                }],
                external_gram: vec![],
                numerator: Some("1".to_owned()),
            })
            .expect("explicit fields should normalize");
        assert_eq!(raw.canonical_atom(), hybrid.canonical_atom());
        assert_eq!(raw.canonical_atom(), explicit.canonical_atom());
        assert_eq!(hybrid.parameter_names(), &["d".to_owned(), "m2".to_owned()]);

        let raw = raw
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("raw family should lower");
        let hybrid = hybrid
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("hybrid family should lower");
        let explicit = explicit
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("explicit family should lower");
        assert_eq!(
            raw.family().fingerprint_ref(),
            hybrid.family().fingerprint_ref()
        );
        assert_eq!(
            raw.family().fingerprint_ref(),
            explicit.family().fingerprint_ref()
        );
    }

    #[test]
    fn numerator_only_declared_extra_does_not_specialize_the_family() {
        let compiler = compiler();
        let inferred_source = one_loop_source(1, "tensor_only*vec(k,mu)");
        let declared_source = "I(loops(k),externals(),parameters(d,m2,tensor_only),dimension(d),prop(D1,k^2-m2,1),numerator(tensor_only*vec(k,mu)))";
        let inferred = compiler
            .compile_str(&inferred_source)
            .expect("numerator-only symbols must not be inferred");
        let declared = compiler
            .compile_str(declared_source)
            .expect("a declared numerator-only extra should be retained");
        assert_eq!(
            declared.parameter_names(),
            &["d".to_owned(), "m2".to_owned(), "tensor_only".to_owned()]
        );
        assert_eq!(
            declared.operational_parameter_names(),
            &["d".to_owned(), "m2".to_owned()]
        );
        assert_ne!(inferred.canonical_atom(), declared.canonical_atom());
        assert_eq!(
            inferred.operational_parameter_names(),
            declared.operational_parameter_names()
        );

        let inferred = inferred
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("inferred family should lower");
        let declared = declared
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("declared family should lower");
        assert_eq!(
            inferred.family().fingerprint_ref(),
            declared.family().fingerprint_ref()
        );
    }

    #[test]
    fn target_and_tensor_numerator_do_not_specialize_the_derived_family() {
        let first = compiler()
            .compile_str(&one_loop_source(3, "vec(k,mu)"))
            .expect("first target should normalize");
        let second = compiler()
            .compile_str(&one_loop_source(-1, "metric(mu,nu)*vec(k,nu)"))
            .expect("second target should normalize");
        assert_eq!(first.target().powers(), &[3]);
        assert_eq!(second.target().powers(), &[-1]);
        assert_eq!(
            first.target().derive_disposition(),
            "not_processed_by_derive"
        );
        let first = first
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("first target family should lower");
        let second = second
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("second target family should lower");
        assert_eq!(
            first.family().fingerprint_ref(),
            second.family().fingerprint_ref()
        );
    }

    #[test]
    fn one_external_gram_entry_lowers_a_complete_one_loop_basis() {
        let source = "I(loops(k),externals(p),dimension(d),prop(D1,k^2-m2,1),prop(D2,(k+p)^2-m2,1),gram(p,p,s))";
        let normalized = compiler()
            .compile_str(source)
            .expect("one-external family should normalize");
        assert_eq!(normalized.external_gram().len(), 1);
        assert_eq!(normalized.external_gram()[0].len(), 1);
        let lowered = normalized
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("complete one-external basis should lower");
        assert_eq!(lowered.family().external_momenta(), &["p".to_owned()]);
        assert_eq!(lowered.denominators().len(), 2);
    }

    #[test]
    fn wrong_and_duplicate_clauses_fail_closed() {
        let compiler = compiler();
        let duplicate = compiler
            .compile_str("I(loops(k),externals(),dimension(d),dimension(d),prop(D1,k^2-m2,1))");
        assert!(matches!(
            duplicate,
            Err(SymbolicaIntegralInputError::DuplicateClause { kind: "dimension" })
        ));

        let wrong_arity =
            compiler.compile_str("I(loops(k),externals(),dimension(d,4),prop(D1,k^2-m2,1))");
        assert!(matches!(
            wrong_arity,
            Err(SymbolicaIntegralInputError::WrongClauseArity {
                kind: "dimension",
                ..
            })
        ));

        let unknown =
            compiler.compile_str("I(loops(k),externals(),dimension(d),bogus(x),prop(D1,k^2-m2,1))");
        assert!(matches!(
            unknown,
            Err(SymbolicaIntegralInputError::UnsupportedToken { .. })
        ));
    }

    #[test]
    fn grammar_clause_heads_are_rejected_inside_payload_expressions() {
        let compiler = compiler();
        let nested_numerator = compiler.compile_str(
            "I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1),numerator(prop(X,x,1)))",
        );
        assert!(matches!(
            nested_numerator,
            Err(SymbolicaIntegralInputError::UnsupportedToken { .. })
        ));

        let nested_scalar = compiler
            .compile_str("I(loops(k),externals(),dimension(gram(k,k,d)),prop(D1,k^2-m2,1))");
        assert!(matches!(
            nested_scalar,
            Err(SymbolicaIntegralInputError::UnsupportedToken { .. })
        ));
    }

    #[test]
    fn base_coefficient_fields_reject_scalar_products_and_momenta() {
        let compiler = compiler();
        let scalar_product_dimension = compiler.compile_str(
            "I(loops(k),externals(p),dimension(sp(p,p)),prop(D1,k^2-m2,1),prop(D2,(k+p)^2-m2,1),gram(p,p,s))",
        );
        assert!(matches!(
            scalar_product_dimension,
            Err(SymbolicaIntegralInputError::UnsupportedToken { .. })
        ));

        let momentum_shift = compiler.compile_str(
            "I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,1),power_shift(D1,k))",
        );
        assert!(matches!(
            momentum_shift,
            Err(SymbolicaIntegralInputError::UnsupportedToken { .. })
        ));

        let explicit_momentum_dimension = compiler.compile_text_parts(TextProjectPartsV1 {
            name: None,
            parameters: None,
            loop_momenta: vec!["k".to_owned()],
            external_momenta: vec![],
            dimension: "k".to_owned(),
            propagators: vec![TextPropagatorInputV1 {
                id: "D1".to_owned(),
                expression: "k^2-m2".to_owned(),
                target_power: 1,
                power_shift: None,
            }],
            external_gram: vec![],
            numerator: None,
        });
        assert!(matches!(
            explicit_momentum_dimension,
            Err(SymbolicaIntegralInputError::UnsupportedToken { .. })
        ));
    }

    #[test]
    fn signed_numbers_work_in_denominators_and_target_powers() {
        let normalized = compiler()
            .compile_str("I(loops(k),externals(),dimension(d),prop(D1,k^2-1,-2))")
            .expect("negative constants and target powers must remain valid exact integers");
        assert_eq!(normalized.target().powers(), &[-2]);
        normalized
            .into_lowered(SymbolicaProjectLoweringLimits::default())
            .expect("a denominator with a negative constant should lower");
    }

    #[test]
    fn numeric_preconversion_envelope_checks_boundary_and_power_growth() {
        let _ = compiler_with(|limits| limits.max_preconversion_integer_bits = 21)
            .parse_expression("12345")
            .expect("the exact conservative numeric-bit boundary should pass");
        let one_below = compiler_with(|limits| limits.max_preconversion_integer_bits = 20)
            .parse_expression("12345");
        assert!(matches!(
            one_below,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "pre-conversion integer bits",
                ..
            })
        ));

        let huge_power = format!("{}^256", "9".repeat(200_000));
        let rejected = compiler().parse_expression(&huge_power);
        assert!(matches!(
            rejected,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "pre-conversion integer bits",
                ..
            }) | Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "aggregate pre-conversion integer bits",
                ..
            })
        ));

        let inverse_growth = compiler_with(|limits| limits.max_preconversion_integer_bits = 100)
            .parse_expression("1/(99^4)");
        assert!(matches!(
            inverse_growth,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "aggregate pre-conversion integer bits",
                ..
            }) | Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "pre-conversion integer bits",
                ..
            })
        ));
    }

    #[test]
    fn explicit_text_fields_share_one_preconversion_integer_budget() {
        let compiler = compiler_with(|limits| limits.max_preconversion_integer_bits = 20);
        let _ = compiler
            .parse_expression("99")
            .expect("the dimension field is individually below the budget");
        let _ = compiler
            .parse_expression("k^2-99")
            .expect("the denominator field is individually below the budget");
        let aggregate = compiler.compile_text_parts(TextProjectPartsV1 {
            name: None,
            parameters: None,
            loop_momenta: vec!["k".to_owned()],
            external_momenta: vec![],
            dimension: "99".to_owned(),
            propagators: vec![TextPropagatorInputV1 {
                id: "D1".to_owned(),
                expression: "k^2-99".to_owned(),
                target_power: 1,
                power_shift: None,
            }],
            external_gram: vec![],
            numerator: None,
        });
        assert!(matches!(
            aggregate,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "aggregate pre-conversion integer bits",
                ..
            })
        ));
    }

    #[test]
    fn caller_owned_large_atom_is_bounded_before_project_clones() {
        let huge_integer = "9"
            .repeat(2_000)
            .parse::<Integer>()
            .expect("test integer should parse");
        let huge_dimension = Atom::num(huge_integer);
        let denominator = compiler()
            .parse_expression("k^2-1")
            .expect("small denominator should parse");
        let logical_bytes = huge_dimension
            .as_view()
            .get_byte_size()
            .checked_add(denominator.as_view().get_byte_size())
            .expect("test byte count should fit");
        let mut limits = SymbolicaIntegralInputLimits::default();
        limits.max_retained_atom_bytes = logical_bytes;
        let rejected = NormalizedProjectInputV1::try_from_parts(
            NormalizedProjectPartsV1 {
                name: None,
                parameters: None,
                loop_momenta: vec!["k".to_owned()],
                external_momenta: vec![],
                dimension: huge_dimension,
                propagators: vec![PropagatorInputV1 {
                    id: "D1".to_owned(),
                    expression: denominator,
                    target_power: 1,
                    power_shift: None,
                }],
                external_gram: vec![],
                numerator: None,
            },
            limits,
        );
        assert!(matches!(
            rejected,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "retained project Atom bytes",
                ..
            })
        ));
    }

    #[test]
    fn raw_preflight_rejects_depth_integer_and_unique_name_excesses() {
        let _ = compiler_with(|limits| limits.max_atom_nodes = 5)
            .parse_expression("a+b")
            .expect("the exact conservative flat lexical boundary should pass");
        let flat = compiler_with(|limits| limits.max_atom_nodes = 4).parse_expression("a+b");
        assert!(matches!(
            flat,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw lexical tokens",
                ..
            })
        ));

        let units = compiler_with(|limits| limits.max_raw_parser_units = 2).parse_expression("a+b");
        assert!(matches!(
            units,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw parser units",
                ..
            })
        ));

        let depth = compiler_with(|limits| limits.max_nesting_depth = 2)
            .compile_str("I(loops(k),externals(),dimension(d),prop(D1,(k)^2,1))");
        assert!(matches!(
            depth,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw parser nesting depth",
                ..
            })
        ));

        let integer =
            compiler_with(|limits| limits.max_raw_integer_digits = 2).parse_expression("123");
        assert!(matches!(
            integer,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw integer digits",
                ..
            })
        ));

        let separated_integer =
            compiler_with(|limits| limits.max_raw_integer_digits = 2).parse_expression("1_2_3");
        assert!(matches!(
            separated_integer,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw integer digits",
                ..
            })
        ));

        let parser_whitespace =
            compiler_with(|limits| limits.max_atom_nodes = 5).parse_expression("a\\b\\c");
        assert!(matches!(
            parser_whitespace,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw lexical tokens",
                ..
            })
        ));

        let unary_depth =
            compiler_with(|limits| limits.max_nesting_depth = 2).parse_expression("-/-/x");
        assert!(matches!(
            unary_depth,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw parser nesting depth",
                ..
            })
        ));

        let _ = compiler_with(|limits| limits.max_abs_power = 4)
            .parse_expression("x^4")
            .expect("the exact raw power boundary should be accepted");
        let power =
            compiler_with(|limits| limits.max_abs_power = 4).parse_expression("x^999999999");
        assert!(matches!(
            power,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "raw absolute power",
                ..
            })
        ));
        let symbolic_power = compiler().parse_expression("x^(a+1)");
        assert!(matches!(
            symbolic_power,
            Err(SymbolicaIntegralInputError::UnsupportedToken { .. })
        ));

        let identifiers =
            compiler_with(|limits| limits.max_unique_identifiers = 2).parse_expression("a+b+c");
        assert!(matches!(
            identifiers,
            Err(SymbolicaIntegralInputError::ResourceLimit {
                resource: "unique raw Symbolica identifiers",
                ..
            })
        ));
    }
}
