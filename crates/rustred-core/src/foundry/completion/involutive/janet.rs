use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::sector::ShiftComplexityKey;

use super::super::{
    CompletionGeometryLimits, LatticeCardinality, LatticePoint, LeadingIdeal, UncoveredPartition,
};
use super::divisor_index::{JanetDivisorIndex, JanetDivisorScratch, JanetMonomialView};
use super::error::{
    check_limit, checked_add, checked_mul, checked_sort_coordinate_work, try_push_bounded, try_vec,
};
use super::limits::InvolutiveWorkBudget;
use super::{
    ForwardShift, InvolutiveError, InvolutiveLimits, OreActionIdentity, OreConsequence,
    OreOrderingAdapter,
};

/// Opaque identity shared by every immutable revision of one Janet basis.
#[derive(Clone, Debug)]
struct BasisInstanceIdentity(Arc<()>);

impl BasisInstanceIdentity {
    fn fresh() -> Self {
        Self(Arc::new(()))
    }

    fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for BasisInstanceIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.belongs_to(other)
    }
}

impl Eq for BasisInstanceIdentity {}

/// Opaque basis-instance identity plus its monotone immutable revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EpochId {
    instance: BasisInstanceIdentity,
    revision: u64,
}

impl EpochId {
    /// Start one private immutable Janet lineage.
    ///
    /// Parent-private visibility lets a coefficient-free exact-support epoch
    /// preserve the same staleness discipline without exposing identities
    /// outside involutive completion.
    pub(super) fn fresh_initial() -> Self {
        Self {
            instance: BasisInstanceIdentity::fresh(),
            revision: 0,
        }
    }

    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn same_instance(&self, other: &Self) -> bool {
        self.instance.belongs_to(&other.instance)
    }

    pub(super) fn try_successor(&self, limits: InvolutiveLimits) -> Result<Self, InvolutiveError> {
        let next = self
            .revision
            .checked_add(1)
            .ok_or(InvolutiveError::ResourceCountOverflow {
                resource: "Janet epoch",
            })?;
        if next > limits.max_epoch {
            return Err(InvolutiveError::EpochLimit {
                requested: next,
                limit: limits.max_epoch,
            });
        }
        Ok(Self {
            instance: self.instance.clone(),
            revision: next,
        })
    }
}

/// Multiplicative-variable decision for one Janet leading monomial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JanetMultiplicativeMask {
    bits: Arc<Vec<bool>>,
}

impl JanetMultiplicativeMask {
    /// Seal the bounded output of the shared Janet mask constructor.
    ///
    /// The divisor-index boundary still verifies the resulting arity.  Keeping
    /// this constructor parent-private lets an exact-support epoch reuse the
    /// mask algorithm without exposing mutable or unauthenticated masks to
    /// callers outside involutive completion.
    pub(super) fn from_sealed_bits(bits: Vec<bool>) -> Self {
        Self {
            bits: Arc::new(bits),
        }
    }

    pub(crate) fn bits(&self) -> &[bool] {
        self.bits.as_slice()
    }

    pub(crate) fn is_multiplicative(&self, position: usize) -> Result<bool, InvolutiveError> {
        self.bits
            .get(position)
            .copied()
            .ok_or(InvolutiveError::CoordinateOutOfRange {
                position,
                arity: self.bits.len(),
            })
    }

    pub(crate) fn janet_divides(&self, divisor: &ForwardShift, target: &ForwardShift) -> bool {
        divisor.arity() == self.bits.len()
            && target.arity() == self.bits.len()
            && divisor
                .values()
                .iter()
                .zip(target.values())
                .zip(self.bits.iter())
                .all(|((&left, &right), &multiplicative)| {
                    left <= right && (left == right || multiplicative)
                })
    }
}

/// One immutable basis row with its epoch-local Janet division data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JanetBasisElement {
    ordinal: usize,
    leading_shift: ForwardShift,
    leading_key: ShiftComplexityKey,
    multiplicative: JanetMultiplicativeMask,
    consequence: Arc<OreConsequence>,
}

impl JanetBasisElement {
    pub(crate) fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) fn leading_shift(&self) -> &ForwardShift {
        &self.leading_shift
    }

    pub(crate) fn leading_key(&self) -> &ShiftComplexityKey {
        &self.leading_key
    }

    pub(crate) fn multiplicative(&self) -> &JanetMultiplicativeMask {
        &self.multiplicative
    }

    pub(crate) fn consequence(&self) -> &OreConsequence {
        self.consequence.as_ref()
    }

    /// Stable sealed payload handle shared by immutable basis revisions.
    ///
    /// Epoch-local masks, ordinals, and divisor metadata are deliberately not
    /// part of this allocation. Sharing the handle therefore cannot make an
    /// old prolongation current in a successor epoch.
    pub(super) fn consequence_handle(&self) -> &Arc<OreConsequence> {
        &self.consequence
    }

    fn monomial_view(&self) -> JanetMonomialView<'_> {
        JanetMonomialView::new(self.ordinal, &self.leading_shift, &self.multiplicative)
    }
}

/// One mandatory nonmultiplicative prolongation tied to its source epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JanetProlongation {
    epoch: EpochId,
    basis_ordinal: usize,
    variable: usize,
    target_leading_shift: ForwardShift,
    target_key: ShiftComplexityKey,
}

impl JanetProlongation {
    pub(crate) fn epoch(&self) -> &EpochId {
        &self.epoch
    }

    pub(crate) fn basis_ordinal(&self) -> usize {
        self.basis_ordinal
    }

    pub(crate) fn variable(&self) -> usize {
        self.variable
    }

    pub(crate) fn target_leading_shift(&self) -> &ForwardShift {
        &self.target_leading_shift
    }

    pub(crate) fn target_key(&self) -> &ShiftComplexityKey {
        &self.target_key
    }
}

/// Pure-power witnesses for zero-dimensionality of a monomial complement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PurePowerCoverage {
    exponents: Box<[Option<u64>]>,
}

impl PurePowerCoverage {
    pub(crate) fn is_complete(&self) -> bool {
        self.exponents.iter().all(Option::is_some)
    }

    pub(crate) fn exponent(&self, position: usize) -> Option<u64> {
        self.exponents.get(position).copied().flatten()
    }

    pub(crate) fn missing_axes(&self) -> impl Iterator<Item = usize> + '_ {
        self.exponents
            .iter()
            .enumerate()
            .filter_map(|(position, exponent)| exponent.is_none().then_some(position))
    }
}

/// Immutable Janet division data needed by exact normal forms.
///
/// This deliberately excludes completion-only geometry and obligations. A
/// changed synchronous autoreduction pass can therefore construct its next
/// divisor epoch without paying for data that cannot be observed before the
/// autoreduction fixed point.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct JanetDivisionEpoch {
    epoch: EpochId,
    /// Last complete epoch observable through [`JanetBasisEpoch`]. Hidden
    /// division-only revisions preserve this ancestor until they are sealed.
    sealed_predecessor: Option<EpochId>,
    action: OreActionIdentity,
    arity: usize,
    elements: Box<[JanetBasisElement]>,
    divisor_index: JanetDivisorIndex,
}

/// Immutable complete basis view, exact leading complement, and mandatory
/// Janet queue.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct JanetBasisEpoch {
    division: JanetDivisionEpoch,
    prolongations: Box<[JanetProlongation]>,
    leading_ideal: LeadingIdeal,
    uncovered: UncoveredPartition,
    pure_power_coverage: PurePowerCoverage,
}

impl JanetBasisEpoch {
    pub(crate) fn try_initial(
        consequences: impl IntoIterator<Item = OreConsequence>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        geometry_limits: CompletionGeometryLimits,
    ) -> Result<Self, InvolutiveError> {
        let mut work = InvolutiveWorkBudget::default();
        Self::try_initial_with_budget(
            consequences,
            ordering,
            context,
            limits,
            geometry_limits,
            &mut work,
        )
    }

    pub(super) fn try_initial_with_budget(
        consequences: impl IntoIterator<Item = OreConsequence>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        geometry_limits: CompletionGeometryLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        let mut pending = Vec::new();
        for consequence in consequences {
            consequence.try_validate(ordering, context, limits)?;
            try_push_bounded(
                &mut pending,
                consequence,
                "Janet basis rows",
                limits.max_basis_rows,
            )?;
        }
        Self::try_initial_sealed_with_budget(
            pending,
            ordering,
            context,
            limits,
            geometry_limits,
            work,
        )
    }

    /// Construct an initial epoch from consequences already authenticated by
    /// the in-process preprocessing owner.
    pub(super) fn try_initial_sealed_with_budget(
        pending: Vec<OreConsequence>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        geometry_limits: CompletionGeometryLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        preflight_basis_shape(pending.len(), ordering.arity(), limits)?;
        preflight_basis_coefficient_payload(pending.iter(), limits)?;
        let mut retained = try_vec("initial Janet basis rows", pending.len())?;
        retained.extend(pending.into_iter().map(Arc::new));
        build_epoch(
            EpochId::fresh_initial(),
            None,
            retained,
            ordering,
            context,
            limits,
            geometry_limits,
            work,
        )
    }

    /// Recompute all Janet masks and obligations in a fresh immutable epoch.
    /// Existing exact consequences are shared; no old queue entry is carried.
    pub(crate) fn try_successor(
        &self,
        additions: impl IntoIterator<Item = OreConsequence>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        geometry_limits: CompletionGeometryLimits,
    ) -> Result<Self, InvolutiveError> {
        let mut work = InvolutiveWorkBudget::default();
        self.try_successor_with_budget(
            additions,
            ordering,
            context,
            limits,
            geometry_limits,
            &mut work,
        )
    }

    pub(super) fn try_successor_with_budget(
        &self,
        additions: impl IntoIterator<Item = OreConsequence>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        geometry_limits: CompletionGeometryLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        self.try_division_successor_with_budget(additions, ordering, context, limits, work)?
            .try_seal(ordering, limits, geometry_limits)
    }

    /// Construct only the division layer of an addition successor.
    ///
    /// Completion invokes this immediately before synchronous autoreduction;
    /// its queue and complement are intentionally deferred until that
    /// autoreduction reaches a fixed point.
    pub(super) fn try_division_successor_with_budget(
        &self,
        additions: impl IntoIterator<Item = OreConsequence>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<JanetDivisionEpoch, InvolutiveError> {
        self.division
            .try_addition_successor_after_sealed(additions, ordering, context, limits, work)
    }

    pub(crate) fn epoch(&self) -> &EpochId {
        self.division.epoch()
    }

    /// Last complete basis epoch in the observable lineage.
    ///
    /// Its revision need not be adjacent: synchronous autoreduction may have
    /// traversed division-only revisions before this epoch was sealed.
    pub(crate) fn predecessor(&self) -> Option<&EpochId> {
        self.division.predecessor()
    }

    pub(crate) fn arity(&self) -> usize {
        self.division.arity()
    }

    pub(crate) fn elements(&self) -> &[JanetBasisElement] {
        self.division.elements()
    }

    pub(crate) fn prolongations(&self) -> &[JanetProlongation] {
        &self.prolongations
    }

    pub(crate) fn leading_ideal(&self) -> &LeadingIdeal {
        &self.leading_ideal
    }

    pub(crate) fn uncovered_partition(&self) -> &UncoveredPartition {
        &self.uncovered
    }

    pub(crate) fn pure_power_coverage(&self) -> &PurePowerCoverage {
        &self.pure_power_coverage
    }

    pub(crate) fn try_uncovered_cardinality(
        &self,
        max_points: usize,
    ) -> Result<LatticeCardinality, InvolutiveError> {
        Ok(self.uncovered.try_cardinality(max_points)?)
    }

    pub(super) fn require_ordering(
        &self,
        ordering: &OreOrderingAdapter,
    ) -> Result<(), InvolutiveError> {
        self.division.require_ordering(ordering)
    }

    pub(crate) fn try_janet_divisor(
        &self,
        target: &ForwardShift,
    ) -> Result<Option<usize>, InvolutiveError> {
        let limits = InvolutiveLimits::default();
        let mut work = InvolutiveWorkBudget::default();
        let mut scratch = self.division.try_divisor_scratch(limits)?;
        self.division
            .try_janet_divisor_with_scratch(target, None, &mut scratch, limits, &mut work)
    }

    pub(super) fn try_divisor_scratch(
        &self,
        limits: InvolutiveLimits,
    ) -> Result<JanetDivisorScratch, InvolutiveError> {
        self.division.try_divisor_scratch(limits)
    }

    pub(super) fn try_janet_divisor_with_scratch(
        &self,
        target: &ForwardShift,
        excluded_ordinal: Option<usize>,
        scratch: &mut JanetDivisorScratch,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Option<usize>, InvolutiveError> {
        self.division.try_janet_divisor_with_scratch(
            target,
            excluded_ordinal,
            scratch,
            limits,
            work,
        )
    }

    pub(crate) fn divisor_index_retained_bytes(&self) -> usize {
        self.division.divisor_index_retained_bytes()
    }

    pub(crate) fn require_current(
        &self,
        prolongation: &JanetProlongation,
    ) -> Result<(), InvolutiveError> {
        if &prolongation.epoch == self.epoch() {
            Ok(())
        } else {
            Err(InvolutiveError::StaleEpoch {
                expected: self.epoch().clone(),
                actual: prolongation.epoch.clone(),
            })
        }
    }

    pub(crate) fn try_apply_prolongation(
        &self,
        prolongation: &JanetProlongation,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<OreConsequence, InvolutiveError> {
        let mut work = InvolutiveWorkBudget::default();
        self.try_apply_prolongation_with_budget(prolongation, ordering, context, limits, &mut work)
    }

    pub(super) fn try_apply_prolongation_with_budget(
        &self,
        prolongation: &JanetProlongation,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<OreConsequence, InvolutiveError> {
        self.require_ordering(ordering)?;
        self.require_current(prolongation)?;
        let element = self.elements().get(prolongation.basis_ordinal).ok_or(
            InvolutiveError::InvalidProlongation {
                detail: "basis ordinal is outside the current epoch",
            },
        )?;
        if element
            .multiplicative
            .is_multiplicative(prolongation.variable)?
        {
            return Err(InvolutiveError::InvalidProlongation {
                detail: "requested variable is multiplicative in the current epoch",
            });
        }
        let unit = ForwardShift::try_unit(self.arity(), prolongation.variable, limits)?;
        let expected = element.leading_shift.try_checked_add(&unit, limits)?;
        if expected != prolongation.target_leading_shift {
            return Err(InvolutiveError::InvalidProlongation {
                detail: "target leading shift does not match its basis row and variable",
            });
        }
        let accumulator = OreConsequence::try_zero(ordering, context, limits)?;
        let multiplier = context.one();
        // Preserve the public sealed-boundary validation order, but keep
        // those input censuses outside the diagnostic construction site: a
        // prolongation attempt denotes only its newly materialized result.
        accumulator.try_validate(ordering, context, limits)?;
        element
            .consequence()
            .try_validate(ordering, context, limits)?;
        context.validate_with_limits(&multiplier, limits.indexed_algebra.exact_algebra)?;
        super::with_coefficient_diagnostic_site!(
            Prolongation,
            accumulator.try_left_axpy_sealed(
                &multiplier,
                &unit,
                element.consequence(),
                ordering,
                context,
                limits,
                work,
            )
        )
    }

    #[cfg(test)]
    pub(super) fn try_replacement_successor(
        &self,
        replacements: Vec<Arc<OreConsequence>>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        geometry_limits: CompletionGeometryLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        self.try_replacement_division_successor(replacements, ordering, context, limits, work)?
            .try_seal(ordering, limits, geometry_limits)
    }

    pub(super) fn try_replacement_division_successor(
        &self,
        replacements: Vec<Arc<OreConsequence>>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<JanetDivisionEpoch, InvolutiveError> {
        self.division.require_ordering(ordering)?;
        let next_epoch = self.epoch().try_successor(limits)?;
        preflight_replacement_rows(
            replacements.as_slice(),
            self.arity(),
            ordering,
            context,
            limits,
        )?;
        build_division_epoch(
            next_epoch,
            Some(self.epoch().clone()),
            replacements,
            ordering,
            context,
            limits,
            work,
        )
    }

    pub(super) fn division(&self) -> &JanetDivisionEpoch {
        &self.division
    }
}

impl JanetDivisionEpoch {
    pub(super) fn epoch(&self) -> &EpochId {
        &self.epoch
    }

    fn predecessor(&self) -> Option<&EpochId> {
        self.sealed_predecessor.as_ref()
    }

    pub(super) fn arity(&self) -> usize {
        self.arity
    }

    pub(super) fn elements(&self) -> &[JanetBasisElement] {
        &self.elements
    }

    pub(super) fn require_ordering(
        &self,
        ordering: &OreOrderingAdapter,
    ) -> Result<(), InvolutiveError> {
        ordering.require_action(&self.action)
    }

    pub(super) fn try_divisor_scratch(
        &self,
        limits: InvolutiveLimits,
    ) -> Result<JanetDivisorScratch, InvolutiveError> {
        self.divisor_index.try_scratch(limits)
    }

    pub(super) fn try_janet_divisor_with_scratch(
        &self,
        target: &ForwardShift,
        excluded_ordinal: Option<usize>,
        scratch: &mut JanetDivisorScratch,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Option<usize>, InvolutiveError> {
        let selected = self.divisor_index.try_first_divisor(
            &self.epoch,
            target,
            excluded_ordinal,
            scratch,
            limits,
            work,
        )?;
        let Some(ordinal) = selected else {
            return Ok(None);
        };
        let element = self
            .elements
            .get(ordinal)
            .ok_or(InvolutiveError::Invariant {
                detail: "Janet divisor index returned an ordinal outside the basis",
            })?;
        if excluded_ordinal == Some(ordinal)
            || !element
                .multiplicative
                .janet_divides(&element.leading_shift, target)
        {
            return Err(InvolutiveError::Invariant {
                detail: "Janet divisor index returned a nondivisor",
            });
        }
        Ok(Some(ordinal))
    }

    fn divisor_index_retained_bytes(&self) -> usize {
        self.divisor_index.retained_bytes()
    }

    #[cfg(test)]
    pub(super) fn divisor_index(&self) -> &JanetDivisorIndex {
        &self.divisor_index
    }

    fn try_addition_successor_after_sealed(
        &self,
        additions: impl IntoIterator<Item = OreConsequence>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        self.require_ordering(ordering)?;
        ordering.require_arity("Janet predecessor", self.arity)?;
        let next_epoch = self.epoch.try_successor(limits)?;
        check_limit(
            "Janet basis rows",
            self.elements.len(),
            limits.max_basis_rows,
        )?;
        let remaining = limits.max_basis_rows - self.elements.len();
        let mut pending = Vec::new();
        for consequence in additions {
            consequence.try_validate(ordering, context, limits)?;
            try_push_bounded(
                &mut pending,
                consequence,
                "successor Janet additions",
                remaining,
            )?;
        }
        let total = checked_add("Janet basis rows", self.elements.len(), pending.len())?;
        preflight_basis_shape(total, self.arity, limits)?;
        preflight_basis_coefficient_payload(
            self.elements
                .iter()
                .map(JanetBasisElement::consequence)
                .chain(pending.iter()),
            limits,
        )?;

        // No predecessor Arc is cloned until the complete successor shape and
        // every new consequence have passed their resource/action checks.
        let mut retained = try_vec("successor Janet basis rows", total)?;
        retained.extend(
            self.elements
                .iter()
                .map(|element| Arc::clone(&element.consequence)),
        );
        retained.extend(pending.into_iter().map(Arc::new));
        build_division_epoch(
            next_epoch,
            Some(self.epoch.clone()),
            retained,
            ordering,
            context,
            limits,
            work,
        )
    }

    pub(super) fn try_replacement_successor(
        &self,
        replacements: Vec<Arc<OreConsequence>>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        self.require_ordering(ordering)?;
        let next_epoch = self.epoch.try_successor(limits)?;
        preflight_replacement_rows(
            replacements.as_slice(),
            self.arity,
            ordering,
            context,
            limits,
        )?;
        build_division_epoch(
            next_epoch,
            self.sealed_predecessor.clone(),
            replacements,
            ordering,
            context,
            limits,
            work,
        )
    }

    pub(super) fn try_seal(
        self,
        ordering: &OreOrderingAdapter,
        limits: InvolutiveLimits,
        geometry_limits: CompletionGeometryLimits,
    ) -> Result<JanetBasisEpoch, InvolutiveError> {
        self.require_ordering(ordering)?;
        // This is the sole retention boundary for completion-only state. Its
        // queue and geometry limits therefore remain exact without charging
        // transient division epochs for allocations they never perform.
        let prolongations =
            build_prolongation_queue(&self.epoch, &self.elements, ordering, limits)?;
        let pure_power_coverage = build_pure_power_coverage(self.arity, &self.elements)?;
        let mut generators = try_vec("Janet leading-ideal generators", self.elements.len())?;
        for element in &self.elements {
            generators.push(LatticePoint::try_new(
                element.leading_shift.values().iter().copied(),
            )?);
        }
        let leading_ideal = LeadingIdeal::try_new(self.arity, generators, geometry_limits)?;
        let uncovered = leading_ideal.uncovered_partition()?;
        debug_assert_eq!(
            pure_power_coverage.is_complete(),
            uncovered.is_finite(),
            "pure-power criterion and exact monomial complement disagree",
        );
        Ok(JanetBasisEpoch {
            division: self,
            prolongations,
            leading_ideal,
            uncovered,
            pure_power_coverage,
        })
    }
}

struct RankedConsequence {
    leading_shift: ForwardShift,
    leading_key: ShiftComplexityKey,
    consequence: Arc<OreConsequence>,
}

fn build_epoch(
    epoch: EpochId,
    sealed_predecessor: Option<EpochId>,
    consequences: Vec<Arc<OreConsequence>>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetBasisEpoch, InvolutiveError> {
    build_division_epoch(
        epoch,
        sealed_predecessor,
        consequences,
        ordering,
        context,
        limits,
        work,
    )?
    .try_seal(ordering, limits, geometry_limits)
}

fn build_division_epoch(
    epoch: EpochId,
    sealed_predecessor: Option<EpochId>,
    consequences: Vec<Arc<OreConsequence>>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetDivisionEpoch, InvolutiveError> {
    let arity = ordering.arity();
    if context.index_count() != arity {
        return Err(InvolutiveError::WrongArity {
            object: "Janet indexed coefficient context",
            expected: arity,
            actual: context.index_count(),
        });
    }
    preflight_basis_shape(consequences.len(), arity, limits)?;

    // Callers authenticate untrusted additions once before this sealed
    // boundary. Existing immutable epoch rows are already authenticated.
    // Preserve their Arcs on the monic fast path and allocate a replacement
    // only when exact projective normalization is required.
    let mut normalized = try_vec("monic Janet basis rows", consequences.len())?;
    let mut coefficient_census = super::CoefficientPayloadCensus::default();
    for consequence in consequences {
        let consequence = if let Some(monic) =
            consequence.try_monic_copy_sealed(ordering, context, limits, work)?
        {
            Arc::new(monic)
        } else {
            consequence
        };
        let next_census = coefficient_census.try_add(consequence.coefficient_census())?;
        preflight_basis_coefficient_census(next_census, limits)?;
        normalized.push(consequence);
        coefficient_census = next_census;
    }

    let mut ranked = try_vec("ranked Janet basis rows", normalized.len())?;
    for consequence in normalized {
        let Some((leading, key)) = consequence.row().try_leading_term(ordering)? else {
            return Err(InvolutiveError::ZeroBasisRow);
        };
        ranked.push(RankedConsequence {
            leading_shift: leading.shift().clone(),
            leading_key: key,
            consequence,
        });
    }
    ranked.sort_unstable_by(|left, right| left.leading_key.cmp(&right.leading_key));
    if ranked
        .windows(2)
        .any(|pair| pair[0].leading_shift == pair[1].leading_shift)
    {
        return Err(InvolutiveError::DuplicateLeadingShift);
    }

    let masks = try_compute_multiplicative_masks_from_geometry(
        ranked.len(),
        |ordinal| ranked.get(ordinal).map(|row| &row.leading_shift),
        ordering.variable_sequence(),
        limits,
    )?;
    let mut elements = try_vec("immutable Janet basis elements", ranked.len())?;
    for (ordinal, (ranked, bits)) in ranked.into_iter().zip(masks).enumerate() {
        elements.push(JanetBasisElement {
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
        elements.iter().map(JanetBasisElement::monomial_view),
        limits,
        work,
    )?;
    Ok(JanetDivisionEpoch {
        epoch,
        sealed_predecessor,
        action: ordering.identity().clone(),
        arity,
        elements: elements.into_boxed_slice(),
        divisor_index,
    })
}

pub(super) fn preflight_basis_coefficient_payload<'a>(
    consequences: impl IntoIterator<Item = &'a OreConsequence>,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    let mut coefficient_census = super::CoefficientPayloadCensus::default();
    for consequence in consequences {
        coefficient_census = coefficient_census.try_add(consequence.coefficient_census())?;
    }
    preflight_basis_coefficient_census(coefficient_census, limits)
}

fn preflight_replacement_rows(
    replacements: &[Arc<OreConsequence>],
    arity: usize,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    preflight_basis_shape(replacements.len(), arity, limits)?;
    for consequence in replacements {
        consequence.try_validate(ordering, context, limits)?;
    }
    preflight_basis_coefficient_payload(replacements.iter().map(Arc::as_ref), limits)
}

pub(super) fn preflight_basis_coefficient_census(
    coefficient_census: super::CoefficientPayloadCensus,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    check_limit(
        "Janet basis coefficient terms",
        coefficient_census.terms(),
        limits.max_basis_coefficient_terms,
    )?;
    check_limit(
        "Janet basis coefficient exponent cells",
        coefficient_census.exponent_cells(),
        limits.max_basis_coefficient_exponent_cells,
    )?;
    check_limit(
        "Janet basis coefficient retained bytes",
        coefficient_census.retained_bytes(),
        limits.max_basis_coefficient_retained_bytes,
    )
}

/// Compute Janet multiplicative masks from sealed leading-monomial geometry.
///
/// No coefficient row is observed here.  The callback form avoids allocating
/// or cloning an intermediate leader array for the existing exact epoch while
/// allowing a future exact-support epoch to use this identical implementation.
pub(super) fn try_compute_multiplicative_masks_from_geometry<'a>(
    leader_count: usize,
    leader_at: impl Fn(usize) -> Option<&'a ForwardShift>,
    variable_sequence: &[usize],
    limits: InvolutiveLimits,
) -> Result<Vec<Vec<bool>>, InvolutiveError> {
    let arity = variable_sequence.len();
    preflight_mask_shape(leader_count, arity, limits)?;
    let mut masks = try_vec("Janet multiplicative masks", leader_count)?;
    for _ in 0..leader_count {
        let mut bits = try_vec("Janet multiplicative-mask bits", arity)?;
        bits.resize(arity, false);
        masks.push(bits);
    }

    // One deterministic full lexicographic order makes every equal-prefix
    // class contiguous for every Janet variable. Each class is then scanned
    // once to obtain its maximum, replacing the former rows² * arity² search.
    let prefix_order = try_lexicographic_leader_order(leader_count, &leader_at, variable_sequence)?;
    for (sequence_position, &variable) in variable_sequence.iter().enumerate() {
        let prefix = &variable_sequence[..sequence_position];
        let mut start = 0usize;
        while start < prefix_order.len() {
            let mut end = start + 1;
            while end < prefix_order.len() {
                let first = try_geometry_leader(prefix_order[start], arity, &leader_at)?;
                let candidate = try_geometry_leader(prefix_order[end], arity, &leader_at)?;
                if !equal_prefix(first, candidate, prefix) {
                    break;
                }
                end += 1;
            }
            let first = *prefix_order[start..end]
                .first()
                .ok_or(InvolutiveError::Invariant {
                    detail: "a Janet prefix class did not contain its first monomial",
                })?;
            let mut maximum = try_geometry_leader(first, arity, &leader_at)?.values()[variable];
            for &ordinal in &prefix_order[start + 1..end] {
                maximum = maximum
                    .max(try_geometry_leader(ordinal, arity, &leader_at)?.values()[variable]);
            }
            for &ordinal in &prefix_order[start..end] {
                masks[ordinal][variable] =
                    try_geometry_leader(ordinal, arity, &leader_at)?.values()[variable] == maximum;
            }
            start = end;
        }
    }
    Ok(masks)
}

pub(super) fn preflight_basis_shape(
    rows: usize,
    arity: usize,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    check_limit("Janet basis rows", rows, limits.max_basis_rows)?;
    let coordinate_cells = checked_mul("Janet basis coordinate cells", rows, arity)?;
    check_limit(
        "Janet basis coordinate cells",
        coordinate_cells,
        limits.max_basis_coordinate_cells,
    )?;
    preflight_mask_shape(rows, arity, limits)
}

fn preflight_mask_shape(
    rows: usize,
    arity: usize,
    limits: InvolutiveLimits,
) -> Result<(), InvolutiveError> {
    let cells = checked_mul("Janet multiplicative-mask cells", rows, arity)?;
    check_limit(
        "Janet multiplicative-mask cells",
        cells,
        limits.max_basis_coordinate_cells,
    )?;

    let prefix_width = checked_mul(
        "Janet mask prefix comparisons",
        arity,
        arity.saturating_sub(1),
    )? / 2;
    let prefix_comparisons = checked_mul(
        "Janet mask prefix comparisons",
        rows.saturating_sub(1),
        prefix_width,
    )?;
    check_limit(
        "Janet mask prefix comparisons",
        prefix_comparisons,
        limits.max_mask_prefix_comparisons,
    )?;

    let sort_work =
        checked_sort_coordinate_work("Janet mask sort coordinate comparisons", rows, arity)?;
    check_limit(
        "Janet mask sort coordinate comparisons",
        sort_work,
        limits.max_mask_sort_coordinate_comparisons,
    )?;

    let payload_bytes = checked_mul(
        "Janet mask retained bytes",
        cells,
        std::mem::size_of::<bool>(),
    )?;
    let row_bytes = checked_mul(
        "Janet mask retained bytes",
        rows,
        std::mem::size_of::<Vec<bool>>(),
    )?;
    let retained_bytes = checked_add("Janet mask retained bytes", payload_bytes, row_bytes)?;
    check_limit(
        "Janet mask retained bytes",
        retained_bytes,
        limits.max_mask_retained_bytes,
    )
}

fn try_lexicographic_leader_order<'a>(
    leader_count: usize,
    leader_at: &impl Fn(usize) -> Option<&'a ForwardShift>,
    variable_sequence: &[usize],
) -> Result<Vec<usize>, InvolutiveError> {
    let mut order = try_vec("Janet grouped-prefix order", leader_count)?;
    order.extend(0..leader_count);
    let mut scratch = try_vec("Janet grouped-prefix merge scratch", leader_count)?;
    scratch.resize(leader_count, 0);

    let mut width = 1usize;
    while width < order.len() {
        let mut left = 0usize;
        while left < order.len() {
            let middle = left.saturating_add(width).min(order.len());
            let right = middle.saturating_add(width).min(order.len());
            let mut first = left;
            let mut second = middle;
            for output in left..right {
                let choose_first = second == right
                    || (first < middle
                        && compare_geometry_leaders(
                            order[first],
                            order[second],
                            variable_sequence.len(),
                            leader_at,
                            variable_sequence,
                        )? != std::cmp::Ordering::Greater);
                scratch[output] = if choose_first {
                    let selected = order[first];
                    first += 1;
                    selected
                } else {
                    let selected = order[second];
                    second += 1;
                    selected
                };
            }
            left = right;
        }
        std::mem::swap(&mut order, &mut scratch);
        width = width.saturating_mul(2);
    }
    Ok(order)
}

fn compare_geometry_leaders<'a>(
    left: usize,
    right: usize,
    arity: usize,
    leader_at: &impl Fn(usize) -> Option<&'a ForwardShift>,
    variable_sequence: &[usize],
) -> Result<std::cmp::Ordering, InvolutiveError> {
    let left_leader = try_geometry_leader(left, arity, leader_at)?;
    let right_leader = try_geometry_leader(right, arity, leader_at)?;
    for &variable in variable_sequence {
        let comparison = left_leader.values()[variable].cmp(&right_leader.values()[variable]);
        if comparison != std::cmp::Ordering::Equal {
            return Ok(comparison);
        }
    }
    Ok(left.cmp(&right))
}

fn try_geometry_leader<'a>(
    ordinal: usize,
    arity: usize,
    leader_at: &impl Fn(usize) -> Option<&'a ForwardShift>,
) -> Result<&'a ForwardShift, InvolutiveError> {
    let leader = leader_at(ordinal).ok_or(InvolutiveError::Invariant {
        detail: "Janet mask geometry omitted a leader ordinal",
    })?;
    if leader.arity() != arity {
        return Err(InvolutiveError::WrongArity {
            object: "Janet mask geometry leader",
            expected: arity,
            actual: leader.arity(),
        });
    }
    Ok(leader)
}

fn equal_prefix(left: &ForwardShift, right: &ForwardShift, prefix: &[usize]) -> bool {
    prefix
        .iter()
        .all(|&variable| left.values()[variable] == right.values()[variable])
}

fn build_prolongation_queue(
    epoch: &EpochId,
    elements: &[JanetBasisElement],
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
) -> Result<Box<[JanetProlongation]>, InvolutiveError> {
    let mut obligation_count = 0usize;
    for element in elements {
        for &variable in ordering.variable_sequence() {
            if !element.multiplicative.bits()[variable] {
                obligation_count = checked_add("Janet prolongations", obligation_count, 1)?;
            }
        }
    }
    check_limit(
        "Janet prolongations",
        obligation_count,
        limits.max_prolongations,
    )?;
    let payload_cells = checked_mul(
        "Janet prolongation coordinate cells",
        checked_mul(
            "Janet prolongation coordinate cells",
            obligation_count,
            ordering.arity(),
        )?,
        2,
    )?;
    check_limit(
        "Janet prolongation coordinate cells",
        payload_cells,
        limits.max_prolongation_coordinate_cells,
    )?;
    let shift_bytes = checked_mul(
        "Janet prolongation retained bytes",
        checked_mul(
            "Janet prolongation retained bytes",
            obligation_count,
            ordering.arity(),
        )?,
        checked_add(
            "Janet prolongation retained bytes",
            std::mem::size_of::<u64>(),
            std::mem::size_of::<i128>(),
        )?,
    )?;
    let struct_bytes = checked_mul(
        "Janet prolongation retained bytes",
        obligation_count,
        std::mem::size_of::<JanetProlongation>(),
    )?;
    let retained_bytes = checked_add(
        "Janet prolongation retained bytes",
        shift_bytes,
        struct_bytes,
    )?;
    check_limit(
        "Janet prolongation retained bytes",
        retained_bytes,
        limits.max_prolongation_retained_bytes,
    )?;

    let mut queue = try_vec("Janet prolongations", obligation_count)?;
    for element in elements {
        for &variable in ordering.variable_sequence() {
            if element.multiplicative.bits()[variable] {
                continue;
            }
            let target_leading_shift = element.leading_shift.try_increment(variable, limits)?;
            let target_key = ordering.try_key(&target_leading_shift)?;
            queue.push(JanetProlongation {
                epoch: epoch.clone(),
                basis_ordinal: element.ordinal,
                variable,
                target_leading_shift,
                target_key,
            });
        }
    }
    queue.sort_unstable_by(|left, right| {
        left.target_key
            .cmp(&right.target_key)
            .then_with(|| left.basis_ordinal.cmp(&right.basis_ordinal))
            .then_with(|| left.variable.cmp(&right.variable))
    });
    Ok(queue.into_boxed_slice())
}

fn build_pure_power_coverage(
    arity: usize,
    elements: &[JanetBasisElement],
) -> Result<PurePowerCoverage, InvolutiveError> {
    let mut exponents = try_vec("Janet pure-power coverage", arity)?;
    exponents.resize(arity, None);
    if elements
        .iter()
        .any(|element| element.leading_shift.is_zero())
    {
        exponents.fill(Some(0));
        return Ok(PurePowerCoverage {
            exponents: exponents.into_boxed_slice(),
        });
    }
    for position in 0..arity {
        exponents[position] = elements
            .iter()
            .filter(|element| element.leading_shift.is_pure_power(position))
            .map(|element| element.leading_shift.values()[position])
            .min();
    }
    Ok(PurePowerCoverage {
        exponents: exponents.into_boxed_slice(),
    })
}
