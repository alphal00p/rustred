//! Syntax-authenticated and exactly lowered input models.

use symbolica::prelude::*;

use crate::algebra::Coefficient;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::symbolica_affine_denominator::CompiledSymbolicaAffineDenominator;

use super::limits::{Limits, LoweringLimits, Stats};

/// Stable schema identifier for the current exactly lowered project payload.
pub const LOWERED_SCHEMA: &str = "rustred.lowered-symbolica-project.v1";

/// Whether the exact base-field variable order was declared or inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterSource {
    Declared,
    Inferred,
}

impl ParameterSource {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Inferred => "inferred",
        }
    }
}

/// One ordered denominator expression before affine lowering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Propagator {
    pub(super) id: String,
    pub(super) expression: Atom,
    pub(super) target_power: i64,
    pub(super) power_shift: Atom,
    pub(super) power_shift_explicit: bool,
}

impl Propagator {
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

/// Normalized concrete target retained by `derive` without being processed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub(super) powers: Vec<i64>,
    pub(super) numerator: Atom,
    pub(super) numerator_explicit: bool,
}

impl Target {
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
/// it belongs to the application document and never affects family identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectSource {
    Symbolica { source: Atom },
    Explicit,
}

/// One named denominator after checked Symbolica evaluation and affine
/// projection. Source and canonical normalized Atoms are both retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredDenominator {
    pub(super) id: String,
    pub(super) compiled: CompiledSymbolicaAffineDenominator,
}

impl LoweredDenominator {
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
pub struct LoweredProject {
    pub(super) normalized: Project,
    pub(super) dimension: Coefficient,
    pub(super) denominators: Vec<LoweredDenominator>,
    pub(super) family: IntegralFamily,
    pub(super) limits: LoweringLimits,
}

impl LoweredProject {
    pub const fn schema(&self) -> &'static str {
        LOWERED_SCHEMA
    }

    pub const fn normalized(&self) -> &Project {
        &self.normalized
    }

    pub const fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    pub fn denominators(&self) -> &[LoweredDenominator] {
        &self.denominators
    }

    pub const fn family(&self) -> &IntegralFamily {
        &self.family
    }

    pub const fn limits(&self) -> LoweringLimits {
        self.limits
    }

    pub fn into_parts(self) -> (Project, Vec<LoweredDenominator>, IntegralFamily) {
        (self.normalized, self.denominators, self.family)
    }

    pub fn into_family(self) -> IntegralFamily {
        self.family
    }
}

/// One syntax-authenticated project, common to every input frontend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub(super) source: ProjectSource,
    pub(super) name: String,
    pub(super) name_explicit: bool,
    pub(super) parameter_names: Vec<String>,
    pub(super) operational_parameter_names: Vec<String>,
    pub(super) parameter_source: ParameterSource,
    pub(super) loop_momenta: Vec<String>,
    pub(super) external_momenta: Vec<String>,
    pub(super) dimension: Atom,
    pub(super) propagators: Vec<Propagator>,
    pub(super) external_gram: Vec<Vec<Atom>>,
    pub(super) target: Target,
    pub(super) canonical: Atom,
    pub(super) stats: Stats,
    pub(super) limits: Limits,
}

impl Project {
    pub const fn source(&self) -> &ProjectSource {
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
    pub fn operational_parameter_names(&self) -> &[String] {
        &self.operational_parameter_names
    }

    pub const fn parameter_source(&self) -> ParameterSource {
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

    pub fn propagators(&self) -> &[Propagator] {
        &self.propagators
    }

    pub fn external_gram(&self) -> &[Vec<Atom>] {
        &self.external_gram
    }

    pub const fn target(&self) -> &Target {
        &self.target
    }

    pub const fn canonical_atom(&self) -> &Atom {
        &self.canonical
    }

    pub fn canonical_string(&self) -> String {
        self.canonical.to_canonical_string()
    }

    pub const fn stats(&self) -> Stats {
        self.stats
    }

    pub const fn limits(&self) -> Limits {
        self.limits
    }
}
