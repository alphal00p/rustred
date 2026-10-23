//! Exact finite equations from denominator-preserving changes of variables.
//!
//! Discovery is deliberately heuristic: signed line transpositions and bridge
//! reflections need not generate every symmetry of a support. Each accepted
//! equation nevertheless has a complete, replayable change-of-variables proof.
//! No numerator monomial is averaged in isolation and no unresolved offspring
//! is declared a master. This module generates no integration-by-parts rules.
//!
//! The current shared transport service admits rational affine coefficients.
//! Unit-mass normalized families and constant rational unequal masses are
//! supported. A symbolic mass can remain in a fixed denominator, but maps
//! inducing symbolic mass differences in inactive rows fail explicitly; this
//! module does not broaden or silently bypass that existing transport boundary.

mod codec;
mod discovery;
#[cfg(test)]
mod p102_native_tests;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::domains::SelfRing;

use crate::algebra::ExactAlgebraLimits;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::terminal_relations::TerminalEquation;
use crate::sector::Mask;
use crate::sector::symmetry::integral_transport::{self, Prepared};
use crate::sector::symmetry::{self, DenominatorAction, Jacobian, MomentumMap, VerifiedMap};

use super::{ProductSkipReason, TerminalAliasError, TerminalNormalizationError as Error};

/// Caller-owned work policy, with no new implicit search/operation ceiling.
/// Exact representation overflow, fallible allocation and authority checks
/// remain in the shared services. Trusted campaigns supply their own RAM guard.
#[derive(Clone, Copy, Debug)]
pub struct TerminalNumeratorSymmetryLimits {
    pub expansion: integral_transport::ExpansionLimits,
    pub verification: symmetry::Limits,
}

impl Default for TerminalNumeratorSymmetryLimits {
    fn default() -> Self {
        Self {
            expansion: integral_transport::ExpansionLimits::unbounded_resources(),
            verification: symmetry::Limits {
                exact_algebra: ExactAlgebraLimits::unbounded_resources(),
                max_matrix_entries: usize::MAX,
                max_exact_operations: usize::MAX,
                max_symbolica_single_matrix_entries: usize::MAX,
                max_symbolica_live_matrix_entries: usize::MAX,
                max_symbolica_input_retained_bytes: usize::MAX,
                max_symbolica_output_retained_bytes: usize::MAX,
                max_nonzero_conditions: usize::MAX,
                max_condition_sources: usize::MAX,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminalNumeratorSymmetrySkipReason {
    ExternalMomenta,
    AnalyticPowerShifts,
    NoNumerator,
    MomentumGeometry(ProductSkipReason),
    RankDeficientSupport,
    NoProposedSymmetry,
}

/// Counts describe a finite proposal search, not independent-master numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalNumeratorSymmetryStatistics {
    pub input_keys: usize,
    pub analyzed_supports: usize,
    pub analyzed_denominators: usize,
    pub proposed_generators: usize,
    pub verified_generators: usize,
    pub equation_targets: usize,
    pub equations: usize,
    pub output_terms: usize,
    pub trivial_equations: usize,
    pub rejected_power_permutations: usize,
    pub skipped: BTreeMap<TerminalNumeratorSymmetrySkipReason, usize>,
}

/// Complete homogeneous identity and its exact integration-variable witness.
///
/// Fields are private. Construction verifies the full weighted denominator
/// multiset and transports the complete negative-index numerator polynomial,
/// including every affine mass constant. The unit Jacobian is verified, never
/// inferred from graph isomorphism or from a sampled polynomial identity.
#[derive(Clone, Debug)]
pub struct TerminalNumeratorSymmetryWitness {
    pub(super) family: Arc<String>,
    pub(super) source: IntegralKey,
    pub(super) map: Arc<VerifiedMap>,
    pub(super) equation: TerminalEquation,
}

impl TerminalNumeratorSymmetryWitness {
    /// Authenticate an externally proposed map through the same exact gates
    /// used by automatic discovery. Invalid unequal masses/dots are errors.
    pub fn verify(
        family: Arc<IntegralFamily>,
        source: &IntegralKey,
        map: MomentumMap,
        limits: TerminalNumeratorSymmetryLimits,
    ) -> Result<Self, Error> {
        eligible(&family)?;
        validate_key(&family, source)?;
        let verified = Arc::new(
            symmetry::verify(&family, &family, map, limits.verification).map_err(algebra)?,
        );
        let prepared = prepare_weighted(&family, source, Arc::clone(&verified), limits)?;
        let equation = equation(&family, source, &prepared, limits)?;
        Ok(Self {
            family: family.fingerprint_owner(),
            source: source.clone(),
            map: verified,
            equation,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family
    }

    pub fn source(&self) -> &IntegralKey {
        &self.source
    }

    pub fn verified_map(&self) -> &VerifiedMap {
        &self.map
    }

    pub fn equation(&self) -> &TerminalEquation {
        &self.equation
    }

    /// Cold replay derives a new native verified map and complete transported
    /// row. Merely decoding a stored equation never grants relation authority.
    pub fn replay(
        &self,
        family: Arc<IntegralFamily>,
        limits: TerminalNumeratorSymmetryLimits,
    ) -> Result<(), Error> {
        if self.family_fingerprint() != family.fingerprint() {
            return Err(Error::InvalidWitness("numerator symmetry family mismatch"));
        }
        let fresh = Self::verify(family, &self.source, self.map.momentum().clone(), limits)?;
        if fresh.equation != self.equation {
            return Err(Error::InvalidWitness("numerator symmetry replay differs"));
        }
        Ok(())
    }
}

/// Finite immutable provider for existing exact terminal-refinement engines.
/// Original keys and identities are preserved: this object supplies equations,
/// not new terminal names or destructive replacements of published records.
#[derive(Debug)]
pub struct TerminalNumeratorSymmetryEquations {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) input_keys: BTreeSet<IntegralKey>,
    pub(super) rows: BTreeMap<IntegralKey, Vec<TerminalEquation>>,
    pub(super) witnesses: BTreeMap<IntegralKey, Vec<TerminalNumeratorSymmetryWitness>>,
    pub(super) statistics: TerminalNumeratorSymmetryStatistics,
}

impl TerminalNumeratorSymmetryEquations {
    pub fn prepare(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        limits: TerminalNumeratorSymmetryLimits,
    ) -> Result<Self, Error> {
        discovery::prepare(family, keys, limits)
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn input_keys(&self) -> &BTreeSet<IntegralKey> {
        &self.input_keys
    }

    /// Only explicitly prepared keys supply rows. Offspring are retained as
    /// ordinary auxiliary columns; no recursive orbit-closure claim is made.
    pub fn equations(&self, key: &IntegralKey) -> &[TerminalEquation] {
        self.rows.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn witnesses(&self, key: &IntegralKey) -> &[TerminalNumeratorSymmetryWitness] {
        self.witnesses.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn statistics(&self) -> &TerminalNumeratorSymmetryStatistics {
        &self.statistics
    }
}

fn eligible(family: &IntegralFamily) -> Result<(), Error> {
    if family.external_count() != 0 {
        return Err(Error::InvalidWitness(
            "numerator symmetry currently requires vacuum kinematics",
        ));
    }
    if family.power_shifts().iter().any(|shift| !shift.is_zero()) {
        return Err(Error::InvalidWitness(
            "numerator symmetry does not authorize analytic power shifts",
        ));
    }
    Ok(())
}

fn validate_key(family: &IntegralFamily, key: &IntegralKey) -> Result<(), Error> {
    if key.powers().len() != family.denominator_count() {
        return Err(TerminalAliasError::WrongArity {
            expected: family.denominator_count(),
            actual: key.powers().len(),
        }
        .into());
    }
    Ok(())
}

fn prepare_weighted(
    family: &Arc<IntegralFamily>,
    key: &IntegralKey,
    map: Arc<VerifiedMap>,
    limits: TerminalNumeratorSymmetryLimits,
) -> Result<Prepared, Error> {
    if !matches!(map.jacobian(), Jacobian::Unit { .. }) {
        return Err(Error::InvalidWitness(
            "numerator symmetry has a nonunit Jacobian",
        ));
    }
    let context = family.coefficient_context();
    for (slot, &power) in key.powers().iter().enumerate() {
        if power <= 0 {
            continue;
        }
        let DenominatorAction::Monomial { target, scale } = &map.row_actions()[slot] else {
            return Err(Error::InvalidWitness(
                "numerator symmetry does not preserve a denominator",
            ));
        };
        if !context.contains(scale) || !scale.is_one() || key.powers()[*target] != power {
            return Err(Error::InvalidWitness(
                "numerator symmetry does not preserve denominator powers",
            ));
        }
    }
    let root = Mask::try_from_indices(key.powers()).map_err(algebra)?;
    integral_transport::compile(
        family,
        Arc::clone(family),
        map,
        root.clone(),
        root,
        limits.expansion,
    )
    .map_err(Error::from)
}

fn equation(
    family: &IntegralFamily,
    source: &IntegralKey,
    prepared: &Prepared,
    limits: TerminalNumeratorSymmetryLimits,
) -> Result<TerminalEquation, Error> {
    let transported = prepared.transport(source, limits.expansion)?;
    let context = family.coefficient_context();
    let mut terms = BTreeMap::from([(source.clone(), context.one())]);
    for endpoint in transported.terms() {
        let coefficient = match terms.get(endpoint.key()) {
            Some(old) => {
                context.try_sub(old, endpoint.coefficient(), limits.expansion.exact_algebra)
            }
            None => context.try_neg(endpoint.coefficient(), limits.expansion.exact_algebra),
        }
        .map_err(algebra)?;
        if coefficient.is_zero() {
            terms.remove(endpoint.key());
        } else {
            terms.insert(endpoint.key().clone(), coefficient);
        }
    }
    // integral_transport::compile admits only unconditional rational constant
    // maps, retaining no hidden generic guard or missing measure factor.
    Ok(TerminalEquation {
        terms,
        nonzero_conditions: Vec::new(),
    })
}

fn algebra(error: impl std::fmt::Display) -> Error {
    Error::ExactAlgebra(error.to_string())
}
