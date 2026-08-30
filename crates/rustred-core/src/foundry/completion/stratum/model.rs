use std::sync::Arc;

use crate::algebra::{ExactAlgebraLimits, IndexedCoefficientContext, IndexedPolynomial};
use crate::sector::SectorMonotoneDomain;

use super::identity::{
    BoundedIdentityBuilder, try_copy_identity, try_indexed_polynomial_guard_identity_and_associate,
};
use super::{StratumRegistryError, StratumRegistryLimits, check_limit, checked_add, try_reserve};

/// Which exact branch of one coefficient predicate defines a stratum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum GuardBranch {
    Zero,
    NonZero,
}

impl GuardBranch {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::NonZero => "nonzero",
        }
    }
}

/// Which exact layer owns the predicate payload behind a branch identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum GuardPredicateAuthority {
    /// A stable proof identity owned by another exact subsystem. The label is
    /// not sufficient for exact-circuit guard admission.
    BoundExternalProof,
    /// Canonical primitive sparse polynomial plus indexed-context identity.
    IndexedPolynomial,
}

impl GuardPredicateAuthority {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::BoundExternalProof => "external",
            Self::IndexedPolynomial => "indexed-polynomial",
        }
    }
}

/// Stable identity of one already-proved guard branch.
///
/// This value binds discovery evidence to a predicate owned by another exact
/// proof layer. A caller-provided label is not itself a nonvanishing or
/// vanishing certificate.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GuardBranchIdentity {
    predicate: Arc<String>,
    authority: GuardPredicateAuthority,
    branch: GuardBranch,
}

impl GuardBranchIdentity {
    pub(crate) fn try_new(
        predicate: &str,
        branch: GuardBranch,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        if predicate.is_empty() {
            return Err(StratumRegistryError::EmptyIdentity {
                identity: "guard predicate identity",
            });
        }
        let predicate = try_copy_identity(
            predicate,
            "guard predicate identity bytes",
            limits.max_guard_identity_bytes,
        )?;
        Ok(Self {
            predicate: Arc::new(predicate),
            authority: GuardPredicateAuthority::BoundExternalProof,
            branch,
        })
    }

    /// Bind a branch to the canonical primitive associate of one exact
    /// indexed polynomial. Unlike [`Self::try_new`], this identity can satisfy
    /// an exact-circuit guard during guard-stratum refinement.
    pub(crate) fn try_from_indexed_polynomial(
        context: &IndexedCoefficientContext,
        polynomial: &IndexedPolynomial,
        branch: GuardBranch,
        algebra_limits: ExactAlgebraLimits,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        Self::try_from_indexed_polynomial_retaining_associate(
            context,
            polynomial,
            branch,
            algebra_limits,
            limits,
        )
        .map(|(identity, _)| identity)
    }

    /// Build the canonical identity and return the exact primitive associate
    /// used to serialize it. This avoids repeating Symbolica normalization
    /// when another sealed proof object must retain the predicate payload.
    pub(crate) fn try_from_indexed_polynomial_retaining_associate(
        context: &IndexedCoefficientContext,
        polynomial: &IndexedPolynomial,
        branch: GuardBranch,
        algebra_limits: ExactAlgebraLimits,
        limits: StratumRegistryLimits,
    ) -> Result<(Self, IndexedPolynomial), StratumRegistryError> {
        let (predicate, associate) = try_indexed_polynomial_guard_identity_and_associate(
            context,
            polynomial,
            algebra_limits,
            limits.max_guard_identity_bytes,
        )?;
        let identity = Self {
            predicate: Arc::new(predicate),
            authority: GuardPredicateAuthority::IndexedPolynomial,
            branch,
        };
        Ok((identity, associate))
    }

    pub(crate) fn predicate(&self) -> &str {
        self.predicate.as_str()
    }

    pub(crate) const fn authority(&self) -> GuardPredicateAuthority {
        self.authority
    }

    pub(crate) const fn branch(&self) -> GuardBranch {
        self.branch
    }

    pub(crate) fn same_predicate(&self, other: &Self) -> bool {
        self.authority == other.authority && self.predicate == other.predicate
    }

    pub(crate) fn with_branch(&self, branch: GuardBranch) -> Self {
        Self {
            predicate: self.predicate.clone(),
            authority: self.authority,
            branch,
        }
    }
}

/// Versioned, exact execution identity for one decorated lattice stratum.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DecoratedStratumId(Arc<String>);

impl DecoratedStratumId {
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// One family/context-bound parent-sector box and its exact guard branch.
///
/// The domain lives on the finite `i64` carrier. No endpoint in this value is
/// interpreted as mathematical infinity; a future outer-extension proof must
/// discharge that separate obligation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DecoratedStratum {
    family_fingerprint: Arc<String>,
    context_fingerprint: Arc<String>,
    domain: SectorMonotoneDomain,
    guards: Arc<[GuardBranchIdentity]>,
    id: DecoratedStratumId,
}

impl DecoratedStratum {
    pub(crate) fn try_guard_blind(
        family_fingerprint: &str,
        context_fingerprint: &str,
        domain: SectorMonotoneDomain,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        Self::try_new(family_fingerprint, context_fingerprint, domain, [], limits)
    }

    pub(crate) fn try_new(
        family_fingerprint: &str,
        context_fingerprint: &str,
        domain: SectorMonotoneDomain,
        guards: impl IntoIterator<Item = GuardBranchIdentity>,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        require_identity(family_fingerprint, "family fingerprint")?;
        require_identity(context_fingerprint, "coefficient-context fingerprint")?;
        let mut retained_guards: Vec<GuardBranchIdentity> = Vec::new();
        let mut guard_bytes = 0usize;
        for guard in guards {
            let requested =
                checked_add("decorated-stratum guard branches", retained_guards.len(), 1)?;
            check_limit(
                "decorated-stratum guard branches",
                requested,
                limits.max_guard_branches,
            )?;
            guard_bytes = checked_add(
                "decorated-stratum guard identity bytes",
                guard_bytes,
                guard.predicate().len(),
            )?;
            check_limit(
                "decorated-stratum guard identity bytes",
                guard_bytes,
                limits.max_guard_identity_bytes,
            )?;
            try_reserve(&mut retained_guards, 1, "decorated-stratum guard branches")?;
            retained_guards.push(guard);
        }
        retained_guards.sort_unstable();
        for pair in retained_guards.windows(2) {
            if pair[0].same_predicate(&pair[1]) {
                return Err(if pair[0].branch() == pair[1].branch() {
                    StratumRegistryError::DuplicateGuardPredicate {
                        predicate: pair[0].predicate().to_owned(),
                    }
                } else {
                    StratumRegistryError::ContradictoryGuardPredicate {
                        predicate: pair[0].predicate().to_owned(),
                    }
                });
            }
        }
        let guards: Arc<[GuardBranchIdentity]> = Arc::from(retained_guards);
        let id = build_id(
            family_fingerprint,
            context_fingerprint,
            &domain,
            &guards,
            limits,
        )?;
        let family_fingerprint = Arc::new(try_copy_identity(
            family_fingerprint,
            "decorated-stratum family fingerprint bytes",
            limits.max_stratum_identity_bytes,
        )?);
        let context_fingerprint = Arc::new(try_copy_identity(
            context_fingerprint,
            "decorated-stratum coefficient-context fingerprint bytes",
            limits.max_stratum_identity_bytes,
        )?);
        Ok(Self {
            family_fingerprint,
            context_fingerprint,
            domain,
            guards,
            id,
        })
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) const fn domain(&self) -> &SectorMonotoneDomain {
        &self.domain
    }

    pub(crate) fn guards(&self) -> &[GuardBranchIdentity] {
        &self.guards
    }

    pub(crate) const fn id(&self) -> &DecoratedStratumId {
        &self.id
    }

    pub(crate) fn try_verify(
        &self,
        limits: StratumRegistryLimits,
    ) -> Result<bool, StratumRegistryError> {
        Ok(Self::try_new(
            self.family_fingerprint(),
            self.context_fingerprint(),
            self.domain.clone(),
            self.guards.iter().cloned(),
            limits,
        )? == *self)
    }
}

fn build_id(
    family: &str,
    context: &str,
    domain: &SectorMonotoneDomain,
    guards: &[GuardBranchIdentity],
    limits: StratumRegistryLimits,
) -> Result<DecoratedStratumId, StratumRegistryError> {
    let mut stable = BoundedIdentityBuilder::new(
        limits.max_stratum_identity_bytes,
        "decorated-stratum identity bytes",
    );
    stable.push("rustred.decorated-stratum.v3:")?;
    stable.push_usize(family.len())?;
    stable.push("#")?;
    stable.push(family)?;
    stable.push(":")?;
    stable.push_usize(context.len())?;
    stable.push("#")?;
    stable.push(context)?;
    stable.push(":[")?;
    for (position, active) in domain.sector().active_bits().iter().enumerate() {
        if position != 0 {
            stable.push(",")?;
        }
        stable.push(if *active { "1" } else { "0" })?;
    }
    stable.push("]:[")?;
    for (position, bounds) in domain.bounds().iter().enumerate() {
        if position != 0 {
            stable.push(",")?;
        }
        stable.push_i64(bounds.lower())?;
        stable.push("..")?;
        stable.push_i64(bounds.upper())?;
    }
    stable.push("]:[")?;
    for (ordinal, guard) in guards.iter().enumerate() {
        if ordinal != 0 {
            stable.push(",")?;
        }
        stable.push_usize(guard.predicate().len())?;
        stable.push("#")?;
        stable.push(guard.predicate())?;
        stable.push("=")?;
        stable.push(guard.authority().stable_id())?;
        stable.push("/")?;
        stable.push(guard.branch().stable_id())?;
    }
    stable.push("]")?;
    Ok(DecoratedStratumId(Arc::new(stable.finish())))
}

fn require_identity(value: &str, identity: &'static str) -> Result<(), StratumRegistryError> {
    if value.is_empty() {
        Err(StratumRegistryError::EmptyIdentity { identity })
    } else {
        Ok(())
    }
}
