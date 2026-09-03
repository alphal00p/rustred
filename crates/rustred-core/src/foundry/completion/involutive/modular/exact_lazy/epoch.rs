//! Persistent immutable Janet epochs over committed exact-lazy consequences.
//!
//! Coefficient payloads remain behind [`Arc`] handles owned by one exact-lazy
//! session.  All masks, divisor postings, completion obligations, and exact
//! complement geometry are rebuilt from the shared coefficient-free Janet
//! implementation for every immutable snapshot.

use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::{
    CompletionGeometryLimits, LatticeCardinality, LeadingIdeal, UncoveredPartition,
};
use crate::sector::ShiftComplexityKey;

use super::super::super::divisor_index::{
    JanetDivisorIndex, JanetDivisorScratch, JanetMonomialView,
};
use super::super::super::janet::{
    JanetDivisionGeometry, geometry_authority, preflight_basis_shape,
    try_build_completion_geometry, try_compute_multiplicative_masks_from_geometry,
};
use super::super::super::limits::InvolutiveWorkBudget;
use super::super::super::{
    EpochId, ForwardShift, InvolutiveError, JanetMultiplicativeMask, JanetProlongation,
    OreActionIdentity, OreOrderingAdapter, PurePowerCoverage,
};
use super::error::{checked_add, try_vec};
use super::{
    ExactLazyCompletionLedger, ExactLazyCompletionLedgerId, ExactLazyConsequence, ExactLazyError,
    ExactLazyFrozenJanetEpoch, ExactLazyLimits, ExactLazyOwner, ExactLazySession,
};

const LAZY_EPOCH_ROWS: &str = "exact-lazy Janet epoch rows";

/// One immutable exact-lazy basis row plus snapshot-local Janet metadata.
#[derive(Debug)]
pub(super) struct ExactLazyJanetElement {
    ordinal: usize,
    leading_shift: ForwardShift,
    leading_key: ShiftComplexityKey,
    multiplicative: JanetMultiplicativeMask,
    consequence: Arc<ExactLazyConsequence>,
}

impl ExactLazyJanetElement {
    pub(super) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(super) fn leading_shift(&self) -> &ForwardShift {
        &self.leading_shift
    }

    pub(super) fn leading_key(&self) -> &ShiftComplexityKey {
        &self.leading_key
    }

    pub(super) fn multiplicative(&self) -> &JanetMultiplicativeMask {
        &self.multiplicative
    }

    pub(super) fn consequence(&self) -> &ExactLazyConsequence {
        self.consequence.as_ref()
    }

    pub(super) fn consequence_handle(&self) -> &Arc<ExactLazyConsequence> {
        &self.consequence
    }

    fn monomial_view(&self) -> JanetMonomialView<'_> {
        JanetMonomialView::new(self.ordinal, &self.leading_shift, &self.multiplicative)
    }
}

/// Immutable coefficient-free Janet division snapshot with shared lazy rows.
#[derive(Debug)]
pub(super) struct ExactLazyJanetDivisionEpoch {
    epoch: EpochId,
    /// Last complete snapshot in this lineage. Division-only replacements
    /// preserve it until the stable revision is sealed.
    sealed_predecessor: Option<EpochId>,
    owner: ExactLazyOwner,
    action: OreActionIdentity,
    ledger: ExactLazyCompletionLedgerId,
    arity: usize,
    elements: Box<[ExactLazyJanetElement]>,
    divisor_index: JanetDivisorIndex,
}

/// Immutable complete exact-lazy Janet snapshot and its exact complement.
#[derive(Debug)]
pub(super) struct ExactLazyJanetEpoch {
    division: ExactLazyJanetDivisionEpoch,
    prolongations: Box<[JanetProlongation]>,
    leading_ideal: LeadingIdeal,
    uncovered: UncoveredPartition,
    pure_power_coverage: PurePowerCoverage,
}

impl geometry_authority::Sealed for ExactLazyJanetDivisionEpoch {}

impl JanetDivisionGeometry for ExactLazyJanetDivisionEpoch {
    fn geometry_epoch(&self) -> &EpochId {
        &self.epoch
    }

    fn geometry_action(&self) -> &OreActionIdentity {
        &self.action
    }

    fn geometry_arity(&self) -> usize {
        self.arity
    }

    fn geometry_element_count(&self) -> usize {
        self.elements.len()
    }

    fn geometry_monomial(&self, ordinal: usize) -> Option<JanetMonomialView<'_>> {
        self.elements
            .get(ordinal)
            .map(ExactLazyJanetElement::monomial_view)
    }

    fn geometry_divisor_index(&self) -> &JanetDivisorIndex {
        &self.divisor_index
    }
}

impl ExactLazyJanetEpoch {
    /// Seal an initial complete epoch from the separately authenticated exact
    /// Janet ingress path. Arbitrary committed monic rows cannot use this
    /// constructor as completion admission.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_initial_from_frozen(
        session: &ExactLazySession<'_>,
        frozen: ExactLazyFrozenJanetEpoch<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        geometry_limits: CompletionGeometryLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<Self, ExactLazyError> {
        frozen.require_owner(session.owner())?;
        session.require_binding(ordering, context, limits)?;
        preflight_epoch_shape(frozen.len(), ordering.arity(), limits)?;
        let exact_division = frozen.division();
        if frozen.len() != exact_division.elements().len() {
            return Err(ExactLazyError::InvalidSupport {
                detail: "frozen exact and exact-lazy Janet row counts disagree",
            });
        }
        let mut validated = try_vec("validated frozen exact-lazy Janet rows", frozen.len())?;
        validated.extend(
            exact_division
                .elements()
                .iter()
                .zip(frozen.into_committed_divisors())
                .map(|(element, consequence)| ValidatedLazyRow {
                    leading_shift: element.leading_shift().clone(),
                    leading_key: element.leading_key().clone(),
                    consequence: Arc::new(consequence),
                }),
        );
        Self::try_initial_from_validated(
            session,
            validated,
            ordering,
            context,
            limits,
            geometry_limits,
            ledger,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_initial_from_validated(
        session: &ExactLazySession<'_>,
        validated: Vec<ValidatedLazyRow>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        geometry_limits: CompletionGeometryLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<Self, ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        let ledger_id = ledger.id().clone();
        ledger.require_binding(&ledger_id, session, ordering, context, limits)?;
        preflight_epoch_shape(validated.len(), ordering.arity(), limits)?;
        let division = {
            let work = ledger.try_work_budget(&ledger_id)?;
            build_division_epoch_from_validated(
                EpochId::fresh_initial(),
                None,
                session.owner().clone(),
                ledger_id,
                validated,
                ordering,
                limits,
                work,
            )?
        };
        division.try_seal(session, ordering, context, limits, geometry_limits, ledger)
    }

    /// Test-only seam for adversarial construction. Production successors
    /// consume opaque normal-form admission tokens instead of raw handles.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_initial_for_test(
        session: &ExactLazySession<'_>,
        consequences: Vec<Arc<ExactLazyConsequence>>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        geometry_limits: CompletionGeometryLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<Self, ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        preflight_epoch_shape(consequences.len(), ordering.arity(), limits)?;
        let validated = try_validate_new_rows(session, consequences, ordering)?;
        Self::try_initial_from_validated(
            session,
            validated,
            ordering,
            context,
            limits,
            geometry_limits,
            ledger,
        )
    }

    /// Add committed rows while sharing every predecessor payload Arc.
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn try_addition_successor_for_test(
        &self,
        session: &ExactLazySession<'_>,
        additions: Vec<Arc<ExactLazyConsequence>>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        geometry_limits: CompletionGeometryLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<Self, ExactLazyError> {
        self.try_addition_division_successor_for_test(
            session, additions, ordering, context, limits, ledger,
        )?
        .try_seal(session, ordering, context, limits, geometry_limits, ledger)
    }

    /// Build only the next division snapshot; completion geometry is deferred
    /// until synchronous autoreduction reaches a fixed point.
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn try_addition_division_successor_for_test(
        &self,
        session: &ExactLazySession<'_>,
        additions: Vec<Arc<ExactLazyConsequence>>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<ExactLazyJanetDivisionEpoch, ExactLazyError> {
        self.division
            .require_environment(session, ordering, context, limits, ledger)?;
        let next = self.epoch().try_successor(limits.exact)?;
        let total = checked_add(LAZY_EPOCH_ROWS, self.elements().len(), additions.len())?;
        preflight_epoch_shape(total, self.arity(), limits)?;
        let mut retained = self.division.try_share_validated_rows(total)?;
        retained.extend(try_validate_new_rows(session, additions, ordering)?);
        let work = ledger.try_work_budget(&self.division.ledger)?;
        build_division_epoch_from_validated(
            next,
            Some(self.epoch().clone()),
            self.division.owner.clone(),
            self.division.ledger.clone(),
            retained,
            ordering,
            limits,
            work,
        )
    }

    /// Replace the complete payload set in one persistent successor. Unchanged
    /// rows retain pointer identity when callers clone their existing Arcs.
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn try_replacement_successor_for_test(
        &self,
        session: &ExactLazySession<'_>,
        replacements: Vec<ExactLazyReplacementForTest>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        geometry_limits: CompletionGeometryLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<Self, ExactLazyError> {
        self.try_replacement_division_successor_for_test(
            session,
            replacements,
            ordering,
            context,
            limits,
            ledger,
        )?
        .try_seal(session, ordering, context, limits, geometry_limits, ledger)
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn try_replacement_division_successor_for_test(
        &self,
        session: &ExactLazySession<'_>,
        replacements: Vec<ExactLazyReplacementForTest>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<ExactLazyJanetDivisionEpoch, ExactLazyError> {
        self.division
            .require_environment(session, ordering, context, limits, ledger)?;
        let next = self.epoch().try_successor(limits.exact)?;
        let replacements =
            self.division
                .try_prepare_test_replacements(session, replacements, ordering, limits)?;
        let work = ledger.try_work_budget(&self.division.ledger)?;
        build_division_epoch_from_validated(
            next,
            Some(self.epoch().clone()),
            self.division.owner.clone(),
            self.division.ledger.clone(),
            replacements,
            ordering,
            limits,
            work,
        )
    }

    pub(super) fn epoch(&self) -> &EpochId {
        self.division.epoch()
    }

    pub(super) fn predecessor(&self) -> Option<&EpochId> {
        self.division.predecessor()
    }

    pub(super) const fn arity(&self) -> usize {
        self.division.arity()
    }

    pub(super) fn elements(&self) -> &[ExactLazyJanetElement] {
        self.division.elements()
    }

    pub(super) fn prolongations(&self) -> &[JanetProlongation] {
        &self.prolongations
    }

    pub(super) fn leading_ideal(&self) -> &LeadingIdeal {
        &self.leading_ideal
    }

    pub(super) fn uncovered_partition(&self) -> &UncoveredPartition {
        &self.uncovered
    }

    pub(super) fn pure_power_coverage(&self) -> &PurePowerCoverage {
        &self.pure_power_coverage
    }

    pub(super) fn try_uncovered_cardinality(
        &self,
        max_points: usize,
    ) -> Result<LatticeCardinality, ExactLazyError> {
        self.uncovered
            .try_cardinality(max_points)
            .map_err(InvolutiveError::from)
            .map_err(ExactLazyError::from)
    }

    pub(super) fn require_current(
        &self,
        prolongation: &JanetProlongation,
        ordering: &OreOrderingAdapter,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        self.division
            .require_current(prolongation, ordering, limits)
    }

    pub(super) fn division(&self) -> &ExactLazyJanetDivisionEpoch {
        &self.division
    }
}

impl ExactLazyJanetDivisionEpoch {
    pub(super) fn epoch(&self) -> &EpochId {
        &self.epoch
    }

    pub(super) fn predecessor(&self) -> Option<&EpochId> {
        self.sealed_predecessor.as_ref()
    }

    pub(super) const fn arity(&self) -> usize {
        self.arity
    }

    pub(super) fn elements(&self) -> &[ExactLazyJanetElement] {
        &self.elements
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn try_divisor_scratch(
        &self,
        ordering: &OreOrderingAdapter,
        limits: ExactLazyLimits,
    ) -> Result<JanetDivisorScratch, ExactLazyError> {
        self.require_ordering_limits(ordering, limits)?;
        Ok(self.try_geometry_divisor_scratch(limits.exact)?)
    }

    pub(super) fn require_divisor_query_environment(
        &self,
        excluded_ordinal: Option<usize>,
        scratch: &JanetDivisorScratch,
        ordering: &OreOrderingAdapter,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        self.require_ordering_limits(ordering, limits)?;
        Ok(self.require_geometry_query_environment(excluded_ordinal, scratch)?)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_janet_divisor_with_scratch(
        &self,
        target: &ForwardShift,
        excluded_ordinal: Option<usize>,
        scratch: &mut JanetDivisorScratch,
        ordering: &OreOrderingAdapter,
        limits: ExactLazyLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Option<usize>, ExactLazyError> {
        self.require_ordering_limits(ordering, limits)?;
        Ok(self.try_geometry_janet_divisor_with_scratch(
            target,
            excluded_ordinal,
            scratch,
            limits.exact,
            work,
        )?)
    }

    pub(super) fn require_current(
        &self,
        prolongation: &JanetProlongation,
        ordering: &OreOrderingAdapter,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        self.require_ordering_limits(ordering, limits)?;
        Ok(self.require_geometry_prolongation(prolongation, ordering)?)
    }

    /// Build another division-only revision in the same lineage. The last
    /// complete predecessor is preserved across any number of such revisions.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_replacement_successor_for_test(
        &self,
        session: &ExactLazySession<'_>,
        replacements: Vec<ExactLazyReplacementForTest>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        ledger: &mut ExactLazyCompletionLedger,
    ) -> Result<Self, ExactLazyError> {
        self.require_environment(session, ordering, context, limits, ledger)?;
        let next = self.epoch.try_successor(limits.exact)?;
        let replacements =
            self.try_prepare_test_replacements(session, replacements, ordering, limits)?;
        let work = ledger.try_work_budget(&self.ledger)?;
        build_division_epoch_from_validated(
            next,
            self.sealed_predecessor.clone(),
            self.owner.clone(),
            self.ledger.clone(),
            replacements,
            ordering,
            limits,
            work,
        )
    }

    /// Seal completion-only geometry after checking the immutable epoch's
    /// owner/action/limit environment. Row payloads were authenticated once
    /// before their private validated records entered this lineage.
    pub(super) fn try_seal(
        self,
        session: &ExactLazySession<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        geometry_limits: CompletionGeometryLimits,
        ledger: &ExactLazyCompletionLedger,
    ) -> Result<ExactLazyJanetEpoch, ExactLazyError> {
        self.require_environment(session, ordering, context, limits, ledger)?;
        let geometry =
            try_build_completion_geometry(&self, ordering, limits.exact, geometry_limits)?;
        let (prolongations, leading_ideal, uncovered, pure_power_coverage) = geometry.into_parts();
        Ok(ExactLazyJanetEpoch {
            division: self,
            prolongations,
            leading_ideal,
            uncovered,
            pure_power_coverage,
        })
    }

    fn require_ordering_limits(
        &self,
        ordering: &OreOrderingAdapter,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        self.owner.require_ordering(ordering)?;
        if self.owner.limits() != limits {
            return Err(ExactLazyError::WrongLimitsContract);
        }
        if !self.action.belongs_to(ordering.identity()) {
            return Err(ExactLazyError::WrongOreAction);
        }
        if self.arity != ordering.arity() {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy Janet epoch",
                expected: self.arity,
                actual: ordering.arity(),
            });
        }
        Ok(self.require_geometry_ordering(ordering)?)
    }

    fn require_environment(
        &self,
        session: &ExactLazySession<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
        ledger: &ExactLazyCompletionLedger,
    ) -> Result<(), ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        if !self.owner.belongs_to(session.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        self.require_ordering_limits(ordering, limits)?;
        ledger.require_binding(&self.ledger, session, ordering, context, limits)
    }

    fn try_share_validated_rows(
        &self,
        capacity: usize,
    ) -> Result<Vec<ValidatedLazyRow>, ExactLazyError> {
        let mut retained = try_vec(LAZY_EPOCH_ROWS, capacity)?;
        retained.extend(self.elements.iter().map(ValidatedLazyRow::from_element));
        Ok(retained)
    }

    #[cfg(test)]
    fn try_prepare_test_replacements(
        &self,
        session: &ExactLazySession<'_>,
        replacements: Vec<ExactLazyReplacementForTest>,
        ordering: &OreOrderingAdapter,
        limits: ExactLazyLimits,
    ) -> Result<Vec<ValidatedLazyRow>, ExactLazyError> {
        preflight_epoch_shape(replacements.len(), self.arity, limits)?;
        let mut retained = try_vec(LAZY_EPOCH_ROWS, replacements.len())?;
        for replacement in replacements {
            match replacement {
                ExactLazyReplacementForTest::Shared(ordinal) => {
                    let element = self.elements.get(ordinal).ok_or(
                        InvolutiveError::InvalidProlongation {
                            detail: "shared exact-lazy replacement ordinal is outside its epoch",
                        },
                    )?;
                    retained.push(ValidatedLazyRow::from_element(element));
                }
                ExactLazyReplacementForTest::New(consequence) => {
                    retained.push(try_validate_new_row(session, consequence, ordering)?);
                }
            }
        }
        Ok(retained)
    }
}

/// Private once-validated row record. Its construction is the sole payload
/// scan on initial ingress or genuine replacement/addition. The retained Arc
/// carries the committed transaction receipt; predecessor revisions copy this
/// record's coefficient-free metadata without reopening the row payload.
struct ValidatedLazyRow {
    leading_shift: ForwardShift,
    leading_key: ShiftComplexityKey,
    consequence: Arc<ExactLazyConsequence>,
}

impl ValidatedLazyRow {
    fn from_element(element: &ExactLazyJanetElement) -> Self {
        Self {
            leading_shift: element.leading_shift.clone(),
            leading_key: element.leading_key.clone(),
            consequence: Arc::clone(&element.consequence),
        }
    }
}

#[cfg(test)]
pub(super) enum ExactLazyReplacementForTest {
    Shared(usize),
    New(Arc<ExactLazyConsequence>),
}

#[allow(clippy::too_many_arguments)]
fn build_division_epoch_from_validated(
    epoch: EpochId,
    sealed_predecessor: Option<EpochId>,
    owner: ExactLazyOwner,
    ledger: ExactLazyCompletionLedgerId,
    mut ranked: Vec<ValidatedLazyRow>,
    ordering: &OreOrderingAdapter,
    limits: ExactLazyLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<ExactLazyJanetDivisionEpoch, ExactLazyError> {
    let arity = ordering.arity();
    owner.require_ordering(ordering)?;
    if owner.limits() != limits {
        return Err(ExactLazyError::WrongLimitsContract);
    }
    preflight_epoch_shape(ranked.len(), arity, limits)?;
    ranked.sort_unstable_by(|left, right| left.leading_key.cmp(&right.leading_key));
    if ranked
        .windows(2)
        .any(|pair| pair[0].leading_shift == pair[1].leading_shift)
    {
        return Err(InvolutiveError::DuplicateLeadingShift.into());
    }

    let masks = try_compute_multiplicative_masks_from_geometry(
        ranked.len(),
        |ordinal| ranked.get(ordinal).map(|row| &row.leading_shift),
        ordering.variable_sequence(),
        limits.exact,
    )?;
    let mut elements = try_vec("immutable exact-lazy Janet elements", ranked.len())?;
    for (ordinal, (ranked, bits)) in ranked.into_iter().zip(masks).enumerate() {
        elements.push(ExactLazyJanetElement {
            ordinal,
            leading_shift: ranked.leading_shift,
            leading_key: ranked.leading_key,
            multiplicative: JanetMultiplicativeMask::from_sealed_bits(bits),
            consequence: ranked.consequence,
        });
    }

    let divisor_index = JanetDivisorIndex::try_new_from_geometry(
        &epoch,
        arity,
        elements.len(),
        elements.iter().map(ExactLazyJanetElement::monomial_view),
        limits.exact,
        work,
    )?;
    Ok(ExactLazyJanetDivisionEpoch {
        epoch,
        sealed_predecessor,
        owner,
        action: ordering.identity().clone(),
        ledger,
        arity,
        elements: elements.into_boxed_slice(),
        divisor_index,
    })
}

fn preflight_epoch_shape(
    rows: usize,
    arity: usize,
    limits: ExactLazyLimits,
) -> Result<(), ExactLazyError> {
    Ok(preflight_basis_shape(rows, arity, limits.exact)?)
}

fn try_validate_new_rows(
    session: &ExactLazySession<'_>,
    consequences: Vec<Arc<ExactLazyConsequence>>,
    ordering: &OreOrderingAdapter,
) -> Result<Vec<ValidatedLazyRow>, ExactLazyError> {
    let mut validated = try_vec("validated exact-lazy Janet rows", consequences.len())?;
    for consequence in consequences {
        validated.push(try_validate_new_row(session, consequence, ordering)?);
    }
    Ok(validated)
}

fn try_validate_new_row(
    session: &ExactLazySession<'_>,
    consequence: Arc<ExactLazyConsequence>,
    ordering: &OreOrderingAdapter,
) -> Result<ValidatedLazyRow, ExactLazyError> {
    consequence.try_validate_live(session)?;
    let leader = consequence
        .row()
        .try_leading_term(session, ordering)?
        .ok_or(InvolutiveError::ZeroBasisRow)?;
    if leader.coefficient() != &session.one() {
        return Err(ExactLazyError::InvalidSupport {
            detail: "an exact-lazy Janet row is not monic structural one",
        });
    }
    Ok(ValidatedLazyRow {
        leading_shift: leader.shift().clone(),
        leading_key: ordering.try_key(leader.shift())?,
        consequence,
    })
}
