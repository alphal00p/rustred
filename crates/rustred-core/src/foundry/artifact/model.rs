use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::cell::RuleCell;
use crate::foundry::parametric::ParametricRule;
use crate::identity::ParametricRelation;
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, symmetry::Canonicalizer};

use super::error::ArtifactPersistenceError;
use super::factorization::FactorizationRule;
use super::factorized_product_moments::FactorizedProductMomentProgram;

/// Stable schema identity of an installed closing artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArtifactSchemaVersion {
    V4,
}

impl ArtifactSchemaVersion {
    pub const CURRENT: Self = Self::V4;

    pub const fn as_u32(self) -> u32 {
        match self {
            Self::V4 => 4,
        }
    }

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::V4 => "rustred.closing-artifact.v4",
        }
    }
}

/// Exact analytic reason why every integral in one sector vanishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZeroTerminalProof {
    /// With every denominator absent, a vacuum integrand is polynomial in
    /// loop momenta and has no scale in dimensional regularization.
    ScalelessVacuumPolynomial,
    /// The exact Lee--Pomeransky exponent matrix is rank deficient on this
    /// sector. Installation reruns the generic Symbolica-backed analyzer and
    /// accepts this tag only when its primitive integer kernel replays.
    LeePomeranskyRankDeficiency,
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
            Self::LeePomeranskyRankDeficiency => "rustred.zero.lee-pomeransky-rank-deficiency.v1",
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
/// partition. Registered verifiers accept the generated unit-mass `K = 1`
/// tadpole and `K = 3` sunset families, plus a `K = 6` campaign only after all
/// six proof-bearing full-rank sector waves publish; unsupported candidate
/// shapes never become this sealed type.
#[derive(Debug)]
pub struct ClosedArtifact {
    pub(super) schema: ArtifactSchemaVersion,
    pub(super) algorithm_id: &'static str,
    pub(super) arity: usize,
    pub(super) ordering: OrderingPolicy,
    pub(super) supported_root_power_bounds: Box<[InteriorBounds]>,
    pub(super) family: IntegralFamily,
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context: IndexedCoefficientContext,
    pub(super) source_relations: Vec<ParametricRelation>,
    pub(super) rules: Vec<ParametricRule>,
    pub(super) rule_cells: Vec<Arc<RuleCell>>,
    pub(super) canonicalizer: Option<Canonicalizer>,
    pub(super) dependencies: Vec<Box<ClosedArtifact>>,
    pub(super) factorization_rules: Vec<FactorizationRule>,
    /// Process-local executor-safe product programs rebuilt from authenticated
    /// factorization recipes at installation/load. This derived payload is not
    /// serialized independently.
    pub(super) factorized_product_programs: Vec<Option<FactorizedProductMomentProgram>>,
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

    /// Single persisted ordering authority shared by every rule, rule cell,
    /// and canonicalizer in this artifact.
    pub fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    /// Certified rectangular machine-index domain accepted at the public
    /// reduction boundary. Descendants may temporarily reach representation
    /// endpoints outside this root box when their sealed descent proof makes
    /// that arithmetic safe.
    pub fn supported_root_power_bounds(&self) -> &[InteriorBounds] {
        &self.supported_root_power_bounds
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

    /// Stable installer order used for deterministic first-applicable direct
    /// rule selection. The reducer checks every retained guard before a direct
    /// rule may apply, just as it does for proof-bearing rule cells.
    pub fn rules(&self) -> &[ParametricRule] {
        &self.rules
    }

    /// Shared proof-bearing exceptional/application cells in deterministic
    /// first-applicable order. Each cell retains its translated source span,
    /// exact guard domain, and any coefficient-dead boundary pruning. Cloning
    /// one returned [`Arc`] shares that immutable proof payload without
    /// rebuilding it.
    pub fn rule_cells(&self) -> &[Arc<RuleCell>] {
        &self.rule_cells
    }

    /// Exact family-internal orbit owner used before terminal lookup and
    /// memoization.  Absence means that the family has no installed symmetry
    /// action beyond the identity.
    pub fn canonicalizer(&self) -> Option<&Canonicalizer> {
        self.canonicalizer.as_ref()
    }

    /// Immutable lower-family artifacts used by exact factorization cells.
    pub fn dependencies(&self) -> &[Box<ClosedArtifact>] {
        &self.dependencies
    }

    /// Deterministically ordered exact factorization actions.
    pub fn factorization_rules(&self) -> &[FactorizationRule] {
        &self.factorization_rules
    }

    pub(crate) fn factorized_product_programs(&self) -> &[Option<FactorizedProductMomentProgram>] {
        &self.factorized_product_programs
    }

    /// Rectangular lookup hull. Exact product authority, when present, is
    /// retained by [`Self::factorization_product_domain`].
    pub(crate) fn factorization_application_hull(
        &self,
        ordinal: usize,
    ) -> Option<&crate::sector::SectorInteriorDomain> {
        self.factorized_product_programs
            .get(ordinal)
            .and_then(Option::as_ref)
            .map(FactorizedProductMomentProgram::application_hull)
            .or_else(|| {
                self.factorization_rules
                    .get(ordinal)
                    .map(FactorizationRule::application_domain)
            })
    }

    pub(crate) fn factorization_product_domain(
        &self,
        ordinal: usize,
    ) -> Option<&super::factorized_product_moments::ProductApplicationDomain> {
        self.factorized_product_programs
            .get(ordinal)
            .and_then(Option::as_ref)
            .map(FactorizedProductMomentProgram::exact_application_domain)
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

    pub fn encode_durable(&self) -> Result<Vec<u8>, ArtifactPersistenceError> {
        super::persistence::encode(self)
    }

    /// Encode under explicit total, container, string, sparse-coefficient,
    /// and semantic-witness resource policies.
    pub fn encode_durable_with_limits(
        &self,
        limits: super::persistence::ArtifactEncodingLimits,
    ) -> Result<Vec<u8>, ArtifactPersistenceError> {
        super::persistence::encode_with_limits(self, limits)
    }

    /// Load and authenticate one deterministic durable artifact under the
    /// default resource policy.
    pub fn decode_durable(bytes: &[u8]) -> Result<Self, ArtifactPersistenceError> {
        Self::decode_durable_with_limits(bytes, Default::default())
    }

    /// Load and authenticate one deterministic durable artifact once at the
    /// untrusted boundary. The returned sealed owner needs no replay or
    /// authentication in reducer hot paths.
    pub fn decode_durable_with_limits(
        bytes: &[u8],
        limits: super::persistence::ArtifactLoadLimits,
    ) -> Result<Self, ArtifactPersistenceError> {
        super::persistence::decode(bytes, limits)
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
    pub(crate) fn duplicate_first_rule_with_ordering_for_test(&mut self, ordering: OrderingPolicy) {
        let mut rule = self.rules[0].clone();
        rule.replace_ordering_for_artifact_test(ordering);
        self.rules.push(rule);
    }

    #[cfg(test)]
    pub(crate) fn replace_cell_rule_ordering_for_test(
        &mut self,
        ordinal: usize,
        ordering: OrderingPolicy,
    ) {
        Arc::get_mut(&mut self.rule_cells[ordinal])
            .expect("test-only RuleCell mutation requires unique artifact ownership")
            .replace_rule_ordering_for_artifact_test(ordering);
    }

    #[cfg(test)]
    pub(crate) fn replace_all_cell_rule_orderings_for_test(&mut self, ordering: OrderingPolicy) {
        for cell in &mut self.rule_cells {
            Arc::get_mut(cell)
                .expect("test-only RuleCell mutation requires unique artifact ownership")
                .replace_rule_ordering_for_artifact_test(ordering);
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_first_raw_rule_guard_for_test(
        &mut self,
        polynomial: crate::algebra::IndexedPolynomial,
    ) {
        self.rules[0].replace_first_guard_polynomial_for_test(polynomial);
    }

    #[cfg(test)]
    pub(crate) fn inject_guard_failing_cell_raw_fallback_for_test(
        &mut self,
        polynomial: crate::algebra::IndexedPolynomial,
    ) {
        let cell = Arc::get_mut(&mut self.rule_cells[0])
            .expect("test-only RuleCell mutation requires unique artifact ownership");
        cell.replace_first_guard_polynomial_for_test(polynomial.clone());
        let mut raw = cell.rule().clone();
        raw.replace_first_guard_polynomial_for_test(polynomial);
        self.rules.push(raw);
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
