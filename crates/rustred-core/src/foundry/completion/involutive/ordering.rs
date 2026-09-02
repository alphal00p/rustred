use std::sync::Arc;

use crate::identity::CompletedIbpSourceRows;
use crate::sector::{Mask, OrderingPolicy, ShiftComplexityKey};

use super::error::{check_limit, try_vec};
use super::{ForwardShift, InvolutiveError, InvolutiveLimits};

/// Sector-aware bridge from chart-forward Ore exponents to RustRed's exact
/// persisted integral ordering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OreOrderingAdapter {
    identity: OreActionIdentity,
    policy: OrderingPolicy,
    sector: Mask,
    variable_sequence: Box<[usize]>,
}

/// Opaque identity of one frozen coefficient localization/guard branch.
#[derive(Clone, Debug)]
pub(crate) struct OreLocalizationIdentity(Arc<()>);

impl OreLocalizationIdentity {
    pub(crate) fn fresh() -> Self {
        Self(Arc::new(()))
    }

    pub(crate) fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for OreLocalizationIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.belongs_to(other)
    }
}

impl Eq for OreLocalizationIdentity {}

/// Opaque authority of one sealed ordinary-source execution transcript.
///
/// A matching indexed context is not sufficient: sparse source ordinals are
/// meaningful only relative to the exact completed source module that fixed
/// their row chronology.
#[derive(Clone, Debug)]
pub(crate) struct OreSourceModuleIdentity {
    owner: Arc<()>,
    source_count: Option<usize>,
}

impl OreSourceModuleIdentity {
    fn fresh_synthetic() -> Self {
        Self {
            owner: Arc::new(()),
            source_count: None,
        }
    }

    fn for_completed(completed: &CompletedIbpSourceRows) -> Self {
        Self {
            owner: completed.identity_owner(),
            source_count: Some(completed.source_row_count()),
        }
    }

    fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner) && self.source_count == other.source_count
    }

    fn owns_completed(&self, completed: &CompletedIbpSourceRows) -> bool {
        completed.owns_identity(&self.owner)
            && self.source_count == Some(completed.source_row_count())
    }

    fn accepts_source_ordinal(&self, source_ordinal: usize) -> bool {
        self.source_count
            .is_none_or(|source_count| source_ordinal < source_count)
    }

    fn source_count(&self) -> Option<usize> {
        self.source_count
    }
}

impl PartialEq for OreSourceModuleIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.belongs_to(other)
    }
}

impl Eq for OreSourceModuleIdentity {}

/// Process-local authority for one frozen Ore action and ranking.
///
/// Structural equality of a sector mask is deliberately insufficient: every
/// row, provenance witness, and Janet epoch must descend from the exact
/// adapter instance that fixed both the coefficient automorphism and the
/// ranking. Cloning an adapter preserves this opaque authority; independently
/// rebuilding an equal-looking adapter does not.
#[derive(Clone, Debug)]
pub(crate) struct OreActionIdentity {
    token: Arc<()>,
    localization: OreLocalizationIdentity,
    source_module: OreSourceModuleIdentity,
}

impl OreActionIdentity {
    fn fresh(
        localization: OreLocalizationIdentity,
        source_module: OreSourceModuleIdentity,
    ) -> Self {
        Self {
            token: Arc::new(()),
            localization,
            source_module,
        }
    }

    pub(crate) fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.token, &other.token)
            && self.localization.belongs_to(&other.localization)
            && self.source_module.belongs_to(&other.source_module)
    }
}

impl PartialEq for OreActionIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.belongs_to(other)
    }
}

impl Eq for OreActionIdentity {}

impl OreOrderingAdapter {
    /// Construct an isolated synthetic action for local algebra tests.
    /// Production ordinary-source lifting must use
    /// [`Self::try_new_for_completed`] so source ordinals retain their opaque
    /// execution-owner authority.
    #[cfg(test)]
    pub(crate) fn try_new(
        policy: OrderingPolicy,
        sector: Mask,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        Self::try_new_in_scope(
            policy,
            sector,
            OreLocalizationIdentity::fresh(),
            OreSourceModuleIdentity::fresh_synthetic(),
            limits,
        )
    }

    /// Freeze one Ore chart against the exact sealed ordinary-source module
    /// whose row ordinals its provenance will carry.
    pub(crate) fn try_new_for_completed(
        policy: OrderingPolicy,
        sector: Mask,
        completed: &CompletedIbpSourceRows,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        Self::try_new_in_scope(
            policy,
            sector,
            OreLocalizationIdentity::fresh(),
            OreSourceModuleIdentity::for_completed(completed),
            limits,
        )
    }

    #[cfg(test)]
    pub(crate) fn try_new_in_localization(
        policy: OrderingPolicy,
        sector: Mask,
        localization: OreLocalizationIdentity,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        Self::try_new_in_scope(
            policy,
            sector,
            localization,
            OreSourceModuleIdentity::fresh_synthetic(),
            limits,
        )
    }

    fn try_new_in_scope(
        policy: OrderingPolicy,
        sector: Mask,
        localization: OreLocalizationIdentity,
        source_module: OreSourceModuleIdentity,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let arity = sector.arity();
        check_limit("Ore ordering arity", arity, limits.max_arity)?;
        let zero = ForwardShift::try_zero(arity, limits)?;
        let physical = physical_translation(&sector, &zero)?;
        // This is the single ordering authority. It also rejects a packed
        // coordinate-priority policy with the wrong arity.
        policy.shift_complexity_key(&sector, &physical)?;

        let mut variable_sequence = try_vec("Janet variable sequence", arity)?;
        match policy.try_coordinate_priority()? {
            Some(priority) => {
                if priority.arity() != arity {
                    return Err(InvolutiveError::WrongArity {
                        object: "ordering coordinate priority",
                        expected: arity,
                        actual: priority.arity(),
                    });
                }
                for rank in 0..arity {
                    let position = priority
                        .rank_by_slot()
                        .iter()
                        .position(|&candidate| candidate == rank)
                        .ok_or(InvolutiveError::Invariant {
                            detail: "validated coordinate priority is not a bijection",
                        })?;
                    variable_sequence.push(position);
                }
            }
            None => variable_sequence.extend(0..arity),
        }
        Ok(Self {
            identity: OreActionIdentity::fresh(localization, source_module),
            policy,
            sector,
            variable_sequence: variable_sequence.into_boxed_slice(),
        })
    }

    pub(crate) fn identity(&self) -> &OreActionIdentity {
        &self.identity
    }

    pub(crate) fn owns_completed_source_module(&self, completed: &CompletedIbpSourceRows) -> bool {
        self.identity.source_module.owns_completed(completed)
    }

    pub(crate) fn require_source_ordinal(
        &self,
        source_ordinal: usize,
    ) -> Result<(), InvolutiveError> {
        if self
            .identity
            .source_module
            .accepts_source_ordinal(source_ordinal)
        {
            Ok(())
        } else {
            Err(InvolutiveError::SourceOrdinalOutOfRange {
                source_ordinal,
                source_count: self
                    .identity
                    .source_module
                    .source_count()
                    .expect("a rejected source ordinal has a sealed source count"),
            })
        }
    }

    pub(crate) fn policy(&self) -> OrderingPolicy {
        self.policy
    }

    pub(crate) fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) fn arity(&self) -> usize {
        self.sector.arity()
    }

    pub(crate) fn variable_sequence(&self) -> &[usize] {
        &self.variable_sequence
    }

    pub(crate) fn try_key(
        &self,
        shift: &ForwardShift,
    ) -> Result<ShiftComplexityKey, InvolutiveError> {
        self.require_arity("ranked forward shift", shift.arity())?;
        let physical = self.try_physical_translation(shift)?;
        Ok(self.policy.shift_complexity_key(&self.sector, &physical)?)
    }

    /// Exact coefficient automorphism induced by a chart-forward operator.
    pub(crate) fn try_physical_translation(
        &self,
        shift: &ForwardShift,
    ) -> Result<Vec<i64>, InvolutiveError> {
        self.require_arity("Ore forward shift", shift.arity())?;
        physical_translation(&self.sector, shift)
    }

    pub(crate) fn require_arity(
        &self,
        object: &'static str,
        actual: usize,
    ) -> Result<(), InvolutiveError> {
        if actual == self.arity() {
            Ok(())
        } else {
            Err(InvolutiveError::WrongArity {
                object,
                expected: self.arity(),
                actual,
            })
        }
    }

    pub(crate) fn require_action(&self, actual: &OreActionIdentity) -> Result<(), InvolutiveError> {
        if self.identity.belongs_to(actual) {
            Ok(())
        } else {
            Err(InvolutiveError::ForeignOreAction)
        }
    }
}

fn physical_translation(sector: &Mask, shift: &ForwardShift) -> Result<Vec<i64>, InvolutiveError> {
    if sector.arity() != shift.arity() {
        return Err(InvolutiveError::WrongArity {
            object: "Ore forward shift",
            expected: sector.arity(),
            actual: shift.arity(),
        });
    }
    let mut result = try_vec("Ore physical translation", shift.arity())?;
    for (position, (&active, &coordinate)) in
        sector.active_bits().iter().zip(shift.values()).enumerate()
    {
        let magnitude = i64::try_from(coordinate).map_err(|_| {
            InvolutiveError::ShiftCoordinateNotRepresentable {
                position,
                coordinate,
            }
        })?;
        result.push(if active { magnitude } else { -magnitude });
    }
    Ok(result)
}
