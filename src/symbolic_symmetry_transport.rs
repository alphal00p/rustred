//! Sound symbolic transport of complete parametric identities through a
//! verified denominator permutation.
//!
//! A symmetry cannot canonicalize an isolated generic term `I(n+s)` to
//! `I(n+P s)`: the true image is `I(P n+P s)`.  This module therefore acts on
//! a **whole identity**, simultaneously substituting `n_i -> n_{P(i)}` in
//! every coefficient and permuting every integral shift.  The result is
//! another globally valid identity over the same authenticated `K(n)` map.
//!
//! This row transport is useful for augmenting a symbolic elimination system,
//! but it is not LiteRed's numeric `SR` quotient.  The latter is already the
//! concrete-specialize/zero-and-symmetry-collect/eliminate path in
//! `CertifiedFamilyRuleProvider`.

use std::fmt;
use std::sync::Arc;

use crate::{
    GuardOrigin, IntegralFamily, ParametricArithmeticLimits, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricRelation, ParametricRelationError, ParametricRowId,
    SymmetryVerificationLimits, VerifiedInternalFamilyPermutationSymmetry,
};

pub const SYMBOLIC_SYMMETRY_ROW_TRANSPORT_V1_SCHEMA: &str =
    "rustred-symbolic-symmetry-row-transport-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolicSymmetryRowTransportLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub symmetry: SymmetryVerificationLimits,
    pub max_source_terms: usize,
    pub max_source_guards: usize,
    pub max_symmetry_domain_conditions: usize,
    pub max_output_terms: usize,
    pub max_output_guards: usize,
    pub max_manifest_bytes: usize,
}

impl Default for SymbolicSymmetryRowTransportLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            symmetry: SymmetryVerificationLimits::default(),
            max_source_terms: 4_000_000,
            max_source_guards: 4_000_000,
            max_symmetry_domain_conditions: 1_000_000,
            max_output_terms: 4_000_000,
            max_output_guards: 5_000_000,
            max_manifest_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymbolicSymmetryRowTransportStats {
    source_terms: usize,
    source_guards: usize,
    symmetry_domain_conditions: usize,
    output_terms: usize,
    output_guards: usize,
    output_manifest_bytes: usize,
}

impl SymbolicSymmetryRowTransportStats {
    pub const fn source_terms(self) -> usize {
        self.source_terms
    }
    pub const fn source_guards(self) -> usize {
        self.source_guards
    }
    pub const fn symmetry_domain_conditions(self) -> usize {
        self.symmetry_domain_conditions
    }
    pub const fn output_terms(self) -> usize {
        self.output_terms
    }
    pub const fn output_guards(self) -> usize {
        self.output_guards
    }
    pub const fn output_manifest_bytes(self) -> usize {
        self.output_manifest_bytes
    }
}

/// Replayable proof that `transported` is the simultaneous coefficient/shift
/// image of `source` under `symmetry`.
///
/// This certificate deliberately does not assert that `source` is an IBP/LI
/// identity: [`crate::ParametricRelation`] is also a public algebraic type.
/// A production solver must nest this witness under generated-source
/// authentication before an elimination pivot can become a reduction rule.
#[derive(Clone, Debug)]
pub struct SymbolicSymmetryRowTransportCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    source: Arc<ParametricRelation>,
    symmetry: Arc<VerifiedInternalFamilyPermutationSymmetry>,
    symmetry_permutation: Box<[usize]>,
    symmetry_map_guard_polynomials: Box<[crate::generic_family::BasePolynomial]>,
    transported: ParametricRelation,
    limits: SymbolicSymmetryRowTransportLimits,
    stats: SymbolicSymmetryRowTransportStats,
}

impl SymbolicSymmetryRowTransportCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub fn source(&self) -> &ParametricRelation {
        &self.source
    }
    pub fn symmetry(&self) -> &VerifiedInternalFamilyPermutationSymmetry {
        &self.symmetry
    }
    pub fn symmetry_permutation(&self) -> &[usize] {
        &self.symmetry_permutation
    }
    pub fn symmetry_map_guard_polynomials(&self) -> &[crate::generic_family::BasePolynomial] {
        &self.symmetry_map_guard_polynomials
    }
    pub const fn transported_relation(&self) -> &ParametricRelation {
        &self.transported
    }
    pub const fn limits(&self) -> SymbolicSymmetryRowTransportLimits {
        self.limits
    }
    pub const fn stats(&self) -> SymbolicSymmetryRowTransportStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), SymbolicSymmetryRowTransportError> {
        if self.schema != SYMBOLIC_SYMMETRY_ROW_TRANSPORT_V1_SCHEMA {
            return Err(SymbolicSymmetryRowTransportError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(SymbolicSymmetryRowTransportError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(SymbolicSymmetryRowTransportError::WrongContext);
        }
        let rebuilt = SymbolicSymmetryRowTransportCompiler::compile(
            family,
            context,
            self.source.as_ref(),
            self.symmetry.as_ref(),
            self.limits,
        )?;
        if rebuilt.family_fingerprint == self.family_fingerprint
            && rebuilt.context_fingerprint == self.context_fingerprint
            && rebuilt.source.has_identical_guard_provenance(&self.source)
            && rebuilt.symmetry.denominator_permutation() == self.symmetry.denominator_permutation()
            && rebuilt.symmetry_permutation == self.symmetry_permutation
            && rebuilt.symmetry_map_guard_polynomials == self.symmetry_map_guard_polynomials
            && rebuilt
                .transported
                .has_identical_guard_provenance(&self.transported)
            && rebuilt.stats == self.stats
        {
            Ok(())
        } else {
            Err(SymbolicSymmetryRowTransportError::ReplayMismatch)
        }
    }
}

pub struct SymbolicSymmetryRowTransportCompiler;

impl SymbolicSymmetryRowTransportCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &ParametricRelation,
        symmetry: &VerifiedInternalFamilyPermutationSymmetry,
        limits: SymbolicSymmetryRowTransportLimits,
    ) -> Result<SymbolicSymmetryRowTransportCertificate, SymbolicSymmetryRowTransportError> {
        validate_scope(family, context, source, symmetry, limits)?;
        let permutation = symmetry.denominator_permutation();
        let map_conditions = symmetry.affine_map().replay_guards();
        check_limit(
            "symbolic symmetry-map domain conditions",
            map_conditions.len(),
            limits.max_symmetry_domain_conditions,
        )?;
        // A bijection preserves distinct sparse shifts and distinct source
        // guard polynomials.  These lower bounds can therefore be rejected
        // before cloning/substituting the complete row.
        check_limit(
            "symbolic symmetry-transport output terms",
            source.terms().len(),
            limits.max_output_terms,
        )?;
        check_limit(
            "symbolic symmetry-transport output guards",
            source.guarded_nonzero_conditions().len(),
            limits.max_output_guards,
        )?;
        let maximum_output_guards = source
            .guarded_nonzero_conditions()
            .len()
            .checked_add(map_conditions.len())
            .ok_or(SymbolicSymmetryRowTransportError::ResourceCountOverflow {
                resource: "symbolic symmetry-transport output guards",
            })?;
        check_limit(
            "symbolic symmetry-transport output guards",
            maximum_output_guards,
            limits.max_output_guards,
        )?;
        // Even an empty relation has a nonempty stable-manifest schema.
        check_limit(
            "symbolic symmetry-transport manifest bytes",
            1,
            limits.max_manifest_bytes,
        )?;
        let row_id = transported_row_id(source, permutation);
        let mut transported =
            source.permuted_indices(context, permutation, row_id, limits.arithmetic)?;

        for (condition_ordinal, condition) in map_conditions.iter().enumerate() {
            let polynomial = context.lift_base_polynomial(condition.polynomial())?;
            let condition = context.nonzero_condition_with_origins_and_limits(
                polynomial,
                [GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: permutation.to_vec().into_boxed_slice(),
                    condition_ordinal,
                }],
                limits.arithmetic.exact_algebra,
            )?;
            transported.add_guarded_nonzero_condition_with_limits(
                context,
                condition,
                limits.arithmetic,
            )?;
        }
        check_limit(
            "symbolic symmetry-transport output terms",
            transported.terms().len(),
            limits.max_output_terms,
        )?;
        check_limit(
            "symbolic symmetry-transport output guards",
            transported.guarded_nonzero_conditions().len(),
            limits.max_output_guards,
        )?;
        let manifest_bytes = transported.stable_manifest().len();
        check_limit(
            "symbolic symmetry-transport manifest bytes",
            manifest_bytes,
            limits.max_manifest_bytes,
        )?;
        let stats = SymbolicSymmetryRowTransportStats {
            source_terms: source.terms().len(),
            source_guards: source.guarded_nonzero_conditions().len(),
            symmetry_domain_conditions: map_conditions.len(),
            output_terms: transported.terms().len(),
            output_guards: transported.guarded_nonzero_conditions().len(),
            output_manifest_bytes: manifest_bytes,
        };
        let certificate = SymbolicSymmetryRowTransportCertificate {
            schema: SYMBOLIC_SYMMETRY_ROW_TRANSPORT_V1_SCHEMA,
            family_fingerprint: Arc::from(family.fingerprint()),
            context_fingerprint: Arc::from(context.fingerprint()),
            source: Arc::new(source.clone()),
            symmetry: Arc::new(symmetry.clone()),
            symmetry_permutation: permutation.to_vec().into_boxed_slice(),
            symmetry_map_guard_polynomials: map_conditions
                .iter()
                .map(|condition| condition.polynomial().clone())
                .collect(),
            transported,
            limits,
            stats,
        };
        // `validate_scope` replayed the authoritative affine proof before any
        // transformed row was accepted; the retained domain polynomials above
        // come directly from that replayed certificate.
        Ok(certificate)
    }
}

fn validate_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source: &ParametricRelation,
    symmetry: &VerifiedInternalFamilyPermutationSymmetry,
    limits: SymbolicSymmetryRowTransportLimits,
) -> Result<(), SymbolicSymmetryRowTransportError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
        || source.context_fingerprint() != context.fingerprint()
    {
        return Err(SymbolicSymmetryRowTransportError::WrongContext);
    }
    if source.family_fingerprint() != family.fingerprint()
        || symmetry.family_fingerprint() != family.fingerprint()
    {
        return Err(SymbolicSymmetryRowTransportError::WrongFamily);
    }
    if source.arity() != family.denominator_count()
        || symmetry.denominator_permutation().len() != family.denominator_count()
    {
        return Err(SymbolicSymmetryRowTransportError::WrongArity {
            expected: family.denominator_count(),
            actual: source.arity().min(symmetry.denominator_permutation().len()),
        });
    }
    check_limit(
        "symbolic symmetry-transport source terms",
        source.terms().len(),
        limits.max_source_terms,
    )?;
    check_limit(
        "symbolic symmetry-transport source guards",
        source.guarded_nonzero_conditions().len(),
        limits.max_source_guards,
    )?;
    symmetry.replay(family, symmetry.restrictions(), limits.symmetry)?;
    Ok(())
}

fn transported_row_id(source: &ParametricRelation, permutation: &[usize]) -> ParametricRowId {
    let permutation = permutation
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    ParametricRowId::Derived {
        label: Arc::from(format!(
            "{SYMBOLIC_SYMMETRY_ROW_TRANSPORT_V1_SCHEMA}|source={}|permutation=[{permutation}]",
            source.row_id().stable_string(),
        )),
    }
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicSymmetryRowTransportError> {
    if requested > limit {
        Err(SymbolicSymmetryRowTransportError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolicSymmetryRowTransportError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Coefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
    Symmetry(crate::InternalSymmetryReplayError),
    ReplayMismatch,
}

impl fmt::Display for SymbolicSymmetryRowTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("symbolic symmetry-transport schema mismatch")
            }
            Self::WrongFamily => {
                formatter.write_str("symbolic symmetry transport belongs to another family")
            }
            Self::WrongContext => {
                formatter.write_str("symbolic symmetry transport belongs to another K(n) context")
            }
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "symbolic symmetry transport has arity {actual}, expected {expected}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Symmetry(error) => error.fmt(formatter),
            Self::ReplayMismatch => {
                formatter.write_str("symbolic symmetry-transport replay mismatch")
            }
        }
    }
}

impl std::error::Error for SymbolicSymmetryRowTransportError {}

impl From<ParametricCoefficientError> for SymbolicSymmetryRowTransportError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<ParametricRelationError> for SymbolicSymmetryRowTransportError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<crate::InternalSymmetryReplayError> for SymbolicSymmetryRowTransportError {
    fn from(value: crate::InternalSymmetryReplayError) -> Self {
        Self::Symmetry(value)
    }
}
