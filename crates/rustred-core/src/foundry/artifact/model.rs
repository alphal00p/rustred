use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::parametric::ParametricRule;
use crate::identity::ParametricRelation;
use crate::sector::Mask;

use super::error::ArtifactPersistenceError;

/// Stable schema identity of an installed closing artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArtifactSchemaVersion {
    V1,
}

impl ArtifactSchemaVersion {
    pub const CURRENT: Self = Self::V1;

    pub const fn as_u32(self) -> u32 {
        match self {
            Self::V1 => 1,
        }
    }

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::V1 => "rustred.closing-artifact.v1",
        }
    }
}

/// Exact analytic reason why every integral in one sector vanishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZeroTerminalProof {
    /// With every denominator absent, a vacuum integrand is polynomial in
    /// loop momenta and has no scale in dimensional regularization.
    ScalelessVacuumPolynomial,
}

/// Exact reason why a unit-scale reduction can restore one common mass by
/// denominator-power homogeneity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommonMassHomogeneityProof {
    /// Every physical denominator has the same nonzero mass squared and no
    /// other dimensionful scale. The full integral scales as
    /// `(m^2)^(L*d/2-sum powers)`; in a target/master reduction ratio the
    /// common loop-measure term cancels, leaving the stored power difference.
    UniformVacuumMassSquared,
}

impl CommonMassHomogeneityProof {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::UniformVacuumMassSquared => "rustred.homogeneity.uniform-vacuum-mass-squared.v1",
        }
    }
}

impl ZeroTerminalProof {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::ScalelessVacuumPolynomial => "rustred.zero.scaleless-vacuum-polynomial.v1",
        }
    }
}

/// One proof-backed sector terminal in an immutable artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZeroSectorTerminal {
    sector: Mask,
    proof: ZeroTerminalProof,
}

impl ZeroSectorTerminal {
    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    pub fn proof(&self) -> ZeroTerminalProof {
        self.proof
    }

    pub(super) fn new(sector: Mask, proof: ZeroTerminalProof) -> Self {
        Self { sector, proof }
    }
}

/// Counts sealed after the installer has checked the complete generated
/// source/rule/terminal chain once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactValidationWitness {
    source_rows: usize,
    replayed_source_rows: usize,
    replayed_shift_columns: usize,
    guarded_rules: usize,
    universally_applicable_guards: usize,
    master_terminals: usize,
    zero_sector_terminals: usize,
}

impl ArtifactValidationWitness {
    pub fn source_rows(self) -> usize {
        self.source_rows
    }

    pub fn replayed_source_rows(self) -> usize {
        self.replayed_source_rows
    }

    pub fn replayed_shift_columns(self) -> usize {
        self.replayed_shift_columns
    }

    pub fn guarded_rules(self) -> usize {
        self.guarded_rules
    }

    pub fn universally_applicable_guards(self) -> usize {
        self.universally_applicable_guards
    }

    pub fn master_terminals(self) -> usize {
        self.master_terminals
    }

    pub fn zero_sector_terminals(self) -> usize {
        self.zero_sector_terminals
    }
}

/// Immutable, family/context-bound closing artifact.
///
/// The runtime owner is independent of loop count and topology. Its ordered
/// rules, exact master keys, and proof-backed zero sectors are installed only
/// after one closure-specific verifier has discharged the whole lattice
/// partition. The first verifier currently accepts only the generated
/// one-loop unit-mass vacuum preset; unsupported candidate shapes never
/// become this sealed type.
#[derive(Debug)]
pub struct ClosedArtifact {
    pub(super) schema: ArtifactSchemaVersion,
    pub(super) algorithm_id: &'static str,
    pub(super) arity: usize,
    pub(super) family: IntegralFamily,
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context: IndexedCoefficientContext,
    pub(super) source_relations: Vec<ParametricRelation>,
    pub(super) rules: Vec<ParametricRule>,
    pub(super) masters: BTreeSet<IntegralKey>,
    pub(super) zero_sectors: Vec<ZeroSectorTerminal>,
    pub(super) common_mass_homogeneity: Option<CommonMassHomogeneityProof>,
    pub(super) validation: ArtifactValidationWitness,
}

impl ClosedArtifact {
    pub fn schema(&self) -> ArtifactSchemaVersion {
        self.schema
    }

    pub fn algorithm_id(&self) -> &'static str {
        self.algorithm_id
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn family(&self) -> &IntegralFamily {
        &self.family
    }

    pub fn context_fingerprint(&self) -> &str {
        self.context.fingerprint()
    }

    pub fn coefficient_context(&self) -> &CoefficientContext {
        self.context.base()
    }

    pub fn source_relations(&self) -> &[ParametricRelation] {
        &self.source_relations
    }

    /// Stable installer order used for deterministic first-applicable rule
    /// selection.
    pub fn rules(&self) -> &[ParametricRule] {
        &self.rules
    }

    pub fn masters(&self) -> &BTreeSet<IntegralKey> {
        &self.masters
    }

    pub fn zero_sectors(&self) -> &[ZeroSectorTerminal] {
        &self.zero_sectors
    }

    pub fn common_mass_homogeneity(&self) -> Option<CommonMassHomogeneityProof> {
        self.common_mass_homogeneity
    }

    pub fn validation(&self) -> ArtifactValidationWitness {
        self.validation
    }

    /// The first artifact slice is generated and sealed in process. A future
    /// durable format must retain enough source material to authenticate and
    /// replay untrusted bytes; returning a typed error keeps that frontier
    /// explicit meanwhile.
    pub fn encode_durable(&self) -> Result<Vec<u8>, ArtifactPersistenceError> {
        Err(ArtifactPersistenceError::DurableEncodingUnavailable {
            schema: self.schema,
        })
    }

    pub(crate) fn indexed_context(&self) -> &IndexedCoefficientContext {
        &self.context
    }

    pub(crate) fn family_fingerprint_owner(&self) -> Arc<String> {
        self.family_fingerprint.clone()
    }

    pub(crate) fn is_zero_terminal(&self, key: &IntegralKey) -> bool {
        if key.powers().len() != self.arity {
            return false;
        }
        self.zero_sectors.iter().any(|terminal| {
            terminal
                .sector
                .active_bits()
                .iter()
                .zip(key.powers())
                .all(|(&active, &power)| active == (power >= 1))
        })
    }

    #[cfg(test)]
    pub(crate) fn clear_rules_for_test(&mut self) {
        self.rules.clear();
    }

    #[cfg(test)]
    pub(crate) fn duplicate_first_rule_for_test(&mut self) {
        let rule = self.rules[0].clone();
        self.rules.push(rule);
    }

    #[cfg(test)]
    pub(crate) fn clear_common_mass_homogeneity_for_test(&mut self) {
        self.common_mass_homogeneity = None;
    }
}

impl ArtifactValidationWitness {
    pub(super) fn new(
        source_rows: usize,
        replayed_source_rows: usize,
        replayed_shift_columns: usize,
        guarded_rules: usize,
        universally_applicable_guards: usize,
        master_terminals: usize,
        zero_sector_terminals: usize,
    ) -> Self {
        Self {
            source_rows,
            replayed_source_rows,
            replayed_shift_columns,
            guarded_rules,
            universally_applicable_guards,
            master_terminals,
            zero_sector_terminals,
        }
    }
}
