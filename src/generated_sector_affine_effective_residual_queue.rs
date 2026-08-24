//! Source-neutral residual work emitted by one generated affine sector owner.
//!
//! The queue is a lossless, owner-relative view: it retains exactly one owner
//! allocation and stores only authenticated residual locators.  Relations,
//! predicates, affine maps, and guards remain sealed inside that owner.

use std::collections::TryReserveError;
use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;

use crate::generated_residual_affine_group_effective_coverage::{
    GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    GeneratedResidualAffineGroupTargetDisposition, GeneratedResidualAffineResidualWorkKind,
    GeneratedResidualAffineTargetAttemptOutcome,
};
use crate::generated_residual_affine_when_bad_compilation::{
    GeneratedResidualAffineWhenBadCompilation, GeneratedResidualAffineWhenBadExceptionalKind,
    GeneratedResidualAffineWhenBadExceptionalLeafSourceView,
};
use crate::generated_sector_affine_effective_coverage::{
    GeneratedSectorAffineEffectiveCoverageCertificate, GeneratedSectorAffineEffectiveCoverageError,
    GeneratedSectorAffineExceptionalChildLocator, GeneratedSectorAffineGroupPassOutcome,
    GeneratedSectorAffineOrderedChildOutput, GeneratedSectorAffinePointDisposition,
    GeneratedSectorAffinePointError, GeneratedSectorAffinePointLimits,
    GeneratedSectorAffinePointStats, GeneratedSectorAffineResidualRootLocator,
    GeneratedSectorAffineTerminalDisposition, GeneratedSectorAffineTerminalRecord,
};
use crate::{
    GeneratedResidualAffineCaseLocator, GeneratedResidualAffineInventoryCase,
    GeneratedResidualAffineInventoryTerminal, GeneratedResidualAffineInventoryTerminalOutcome,
    IntegralFamily, ParametricCoefficientContext, ParametricPolynomial,
    ResidualAffineBranchGuardCompositionEntry, ResidualAffineBranchSystemOutcome,
    ResidualAffineBranchUnsupportedReason, ResidualAffineIntegerMap,
    ResidualProductLocusBooleanNodeOutcome, SymbolicPolynomialPredicateKind,
};

pub(crate) const GENERATED_SECTOR_AFFINE_EFFECTIVE_RESIDUAL_QUEUE_V1_SCHEMA: &str =
    "rustred-generated-sector-affine-effective-residual-queue-v1";

/// Owner-relative address of one source item for the next affine epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GeneratedSectorAffineEffectiveResidualWorkLocator {
    Root(GeneratedSectorAffineResidualRootLocator),
    Exceptional(GeneratedSectorAffineExceptionalChildLocator),
}

/// Private navigation back into the exact retained owner payload.
///
/// These slots are deliberately not exposed as ordinals: a later source view
/// must resolve them through the retained owner rather than accepting caller-
/// supplied provenance.
#[derive(Clone, Copy, PartialEq, Eq)]
struct UnsupportedProjectionAuthority {
    equal_zero_locus_count: usize,
    nonzero_locus_count: usize,
    structural_locus_count: usize,
    unsupported_reason_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TargetProjectionAuthority {
    guard_entry_count: usize,
    constant_count: usize,
    free_position_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GeneratedSectorAffineEffectiveResidualAuthoritySlot {
    UnsupportedInventoryTerminal {
        terminal_record_ordinal: usize,
        projection: UnsupportedProjectionAuthority,
    },
    UnprocessedActionableCase {
        terminal_record_ordinal: usize,
        projection: TargetProjectionAuthority,
    },
    UnconsumedTargetRoot {
        terminal_record_ordinal: usize,
        target_disposition_ordinal: usize,
        residual_work_ordinal: usize,
        projection: TargetProjectionAuthority,
    },
    ExceptionalChild {
        terminal_record_ordinal: usize,
        child_output_ordinal: usize,
        target_disposition_ordinal: usize,
        attempt_ordinal: usize,
        selected_target_position: usize,
        residual_work_ordinal: usize,
        projection: TargetProjectionAuthority,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualWorkItem {
    locator: GeneratedSectorAffineEffectiveResidualWorkLocator,
    authority: GeneratedSectorAffineEffectiveResidualAuthoritySlot,
}

impl GeneratedSectorAffineEffectiveResidualWorkItem {
    pub(crate) const fn locator(&self) -> GeneratedSectorAffineEffectiveResidualWorkLocator {
        self.locator
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualWorkItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualWorkItem")
            .field("locator", &self.locator)
            .field("private_authority", &"<redacted>")
            .finish()
    }
}

/// Narrow scalar identity of the exact retained terminal behind one queue item.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner> {
    work_item_ordinal: usize,
    locator: GeneratedSectorAffineEffectiveResidualWorkLocator,
    source_locator: GeneratedResidualAffineCaseLocator,
    source_outcome: GeneratedResidualAffineInventoryTerminalOutcome,
    lifetime: std::marker::PhantomData<&'owner ()>,
}

impl<'owner> GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner> {
    pub(crate) const fn work_item_ordinal(self) -> usize {
        self.work_item_ordinal
    }

    pub(crate) const fn locator(self) -> GeneratedSectorAffineEffectiveResidualWorkLocator {
        self.locator
    }

    pub(crate) const fn source_locator(self) -> GeneratedResidualAffineCaseLocator {
        self.source_locator
    }

    pub(crate) const fn source_outcome(self) -> GeneratedResidualAffineInventoryTerminalOutcome {
        self.source_outcome
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualTerminalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualTerminalSourceView")
            .field("work_item_ordinal", &self.work_item_ordinal)
            .field("locator", &self.locator)
            .field("source_locator", &self.source_locator)
            .field("source_outcome", &self.source_outcome)
            .field("private_terminal_authority", &"<redacted>")
            .finish()
    }
}

/// Typed unsupported Boolean-terminal source for a later affine epoch.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualUnsupportedSourceView<'owner> {
    terminal: GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner>,
    ready_terminal_ordinal: usize,
    equal_zero_locus_ordinals: &'owner [usize],
    nonzero_locus_ordinals: &'owner [usize],
    structural_loci: &'owner [ParametricPolynomial],
    unsupported_reasons: &'owner [ResidualAffineBranchUnsupportedReason],
}

/// Which retained Boolean fact selects one positional unsupported-source atom.
///
/// The position, rather than a caller-supplied global locus ordinal, is the
/// authority boundary. This keeps unrelated structural loci unreachable and
/// makes every successful lookup O(1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedSectorAffineEffectiveResidualAtomPolarity {
    EqualZero,
    NonZero,
}

/// One atom borrowed through an authenticated unsupported residual source.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualAtomSourceView<'owner> {
    locus_ordinal: usize,
    polynomial: &'owner ParametricPolynomial,
}

impl<'owner> GeneratedSectorAffineEffectiveResidualAtomSourceView<'owner> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn polynomial(self) -> &'owner ParametricPolynomial {
        self.polynomial
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualAtomSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualAtomSourceView")
            .field("locus_ordinal", &self.locus_ordinal)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl<'owner> GeneratedSectorAffineEffectiveResidualUnsupportedSourceView<'owner> {
    pub(crate) const fn terminal(
        self,
    ) -> GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner> {
        self.terminal
    }

    pub(crate) const fn ready_terminal_ordinal(self) -> usize {
        self.ready_terminal_ordinal
    }

    pub(crate) const fn equal_zero_locus_ordinals(self) -> &'owner [usize] {
        self.equal_zero_locus_ordinals
    }

    pub(crate) const fn nonzero_locus_ordinals(self) -> &'owner [usize] {
        self.nonzero_locus_ordinals
    }

    pub(crate) const fn atom_count(
        self,
        polarity: GeneratedSectorAffineEffectiveResidualAtomPolarity,
    ) -> usize {
        match polarity {
            GeneratedSectorAffineEffectiveResidualAtomPolarity::EqualZero => {
                self.equal_zero_locus_ordinals.len()
            }
            GeneratedSectorAffineEffectiveResidualAtomPolarity::NonZero => {
                self.nonzero_locus_ordinals.len()
            }
        }
    }

    /// Resolve only an atom selected positionally from this exact terminal.
    pub(crate) fn atom(
        self,
        polarity: GeneratedSectorAffineEffectiveResidualAtomPolarity,
        position: usize,
    ) -> Option<GeneratedSectorAffineEffectiveResidualAtomSourceView<'owner>> {
        let locus_ordinal = match polarity {
            GeneratedSectorAffineEffectiveResidualAtomPolarity::EqualZero => {
                self.equal_zero_locus_ordinals.get(position).copied()?
            }
            GeneratedSectorAffineEffectiveResidualAtomPolarity::NonZero => {
                self.nonzero_locus_ordinals.get(position).copied()?
            }
        };
        Some(GeneratedSectorAffineEffectiveResidualAtomSourceView {
            locus_ordinal,
            polynomial: self.structural_loci.get(locus_ordinal)?,
        })
    }

    pub(crate) fn polynomial_for_locus_ordinal(
        self,
        locus_ordinal: usize,
    ) -> Option<&'owner ParametricPolynomial> {
        if self
            .equal_zero_locus_ordinals
            .binary_search(&locus_ordinal)
            .is_err()
            && self
                .nonzero_locus_ordinals
                .binary_search(&locus_ordinal)
                .is_err()
        {
            return None;
        }
        self.structural_loci.get(locus_ordinal)
    }

    pub(crate) const fn unsupported_reasons(
        self,
    ) -> &'owner [ResidualAffineBranchUnsupportedReason] {
        self.unsupported_reasons
    }

    pub(crate) const fn unsupported_reason_count(self) -> usize {
        self.unsupported_reasons.len()
    }

    pub(crate) fn unsupported_reason(
        self,
        position: usize,
    ) -> Option<&'owner ResidualAffineBranchUnsupportedReason> {
        self.unsupported_reasons.get(position)
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualUnsupportedSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualUnsupportedSourceView")
            .field("terminal", &self.terminal)
            .field("ready_terminal_ordinal", &self.ready_terminal_ordinal)
            .field(
                "equal_zero_locus_count",
                &self.equal_zero_locus_ordinals.len(),
            )
            .field("nonzero_locus_count", &self.nonzero_locus_ordinals.len())
            .field("unsupported_reason_count", &self.unsupported_reasons.len())
            .field("private_boolean_source", &"<redacted>")
            .finish()
    }
}

/// Exact actionable target root, including its inherited affine map and
/// mapped nonzero guards through the retained inventory case allocation.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualTargetSourceView<'owner> {
    terminal: GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner>,
    case_ordinal: usize,
    source_locator: GeneratedResidualAffineCaseLocator,
    group_ordinal: usize,
    ordinal_within_group: usize,
    anchor_case_ordinal: usize,
    affine_map: &'owner ResidualAffineIntegerMap,
    guard_entries: &'owner [ResidualAffineBranchGuardCompositionEntry],
    constants: &'owner [Integer],
    free_positions: &'owner [usize],
}

impl<'owner> GeneratedSectorAffineEffectiveResidualTargetSourceView<'owner> {
    pub(crate) const fn terminal(
        self,
    ) -> GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner> {
        self.terminal
    }

    pub(crate) const fn case_ordinal(self) -> usize {
        self.case_ordinal
    }

    pub(crate) const fn source_locator(self) -> GeneratedResidualAffineCaseLocator {
        self.source_locator
    }

    pub(crate) const fn group_ordinal(self) -> usize {
        self.group_ordinal
    }

    pub(crate) const fn ordinal_within_group(self) -> usize {
        self.ordinal_within_group
    }

    pub(crate) const fn anchor_case_ordinal(self) -> usize {
        self.anchor_case_ordinal
    }

    pub(crate) const fn affine_map(self) -> &'owner ResidualAffineIntegerMap {
        self.affine_map
    }

    pub(crate) const fn guard_entries(self) -> &'owner [ResidualAffineBranchGuardCompositionEntry] {
        self.guard_entries
    }

    pub(crate) const fn guard_entry_count(self) -> usize {
        self.guard_entries.len()
    }

    pub(crate) fn guard_entry(
        self,
        position: usize,
    ) -> Option<&'owner ResidualAffineBranchGuardCompositionEntry> {
        self.guard_entries.get(position)
    }

    pub(crate) const fn constants(self) -> &'owner [Integer] {
        self.constants
    }

    pub(crate) const fn constant_count(self) -> usize {
        self.constants.len()
    }

    pub(crate) fn constant(self, position: usize) -> Option<&'owner Integer> {
        self.constants.get(position)
    }

    pub(crate) const fn free_positions(self) -> &'owner [usize] {
        self.free_positions
    }

    pub(crate) const fn free_position_count(self) -> usize {
        self.free_positions.len()
    }

    pub(crate) fn free_position(self, position: usize) -> Option<usize> {
        self.free_positions.get(position).copied()
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualTargetSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualTargetSourceView")
            .field("terminal", &self.terminal)
            .field("case_ordinal", &self.case_ordinal)
            .field("source_locator", &self.source_locator)
            .field("group_ordinal", &self.group_ordinal)
            .field("ordinal_within_group", &self.ordinal_within_group)
            .field("anchor_case_ordinal", &self.anchor_case_ordinal)
            .field("guard_entry_count", &self.guard_entries.len())
            .field("constant_count", &self.constants.len())
            .field("free_position_count", &self.free_positions.len())
            .field("private_affine_source", &"<redacted>")
            .finish()
    }
}

/// One consumed target's exact bad child.  The target view retains inherited
/// map/guard authority; `exceptional` borrows only the authenticated relative
/// case and its private predicate slice.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualExceptionalSourceView<'owner> {
    target: GeneratedSectorAffineEffectiveResidualTargetSourceView<'owner>,
    exceptional: GeneratedResidualAffineWhenBadExceptionalLeafSourceView<'owner>,
}

/// One private relative predicate borrowed through an authenticated
/// exceptional residual source. No relative case or partition handle crosses
/// this seam.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualExceptionalPredicateSourceView<'owner> {
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: &'owner ParametricPolynomial,
}

impl<'owner> GeneratedSectorAffineEffectiveResidualExceptionalPredicateSourceView<'owner> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(crate) const fn polynomial(self) -> &'owner ParametricPolynomial {
        self.polynomial
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualExceptionalPredicateSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualExceptionalPredicateSourceView")
            .field("locus_ordinal", &self.locus_ordinal)
            .field("kind", &self.kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl<'owner> GeneratedSectorAffineEffectiveResidualExceptionalSourceView<'owner> {
    pub(crate) const fn target(
        self,
    ) -> GeneratedSectorAffineEffectiveResidualTargetSourceView<'owner> {
        self.target
    }

    pub(crate) const fn exceptional(
        self,
    ) -> GeneratedResidualAffineWhenBadExceptionalLeafSourceView<'owner> {
        self.exceptional
    }

    pub(crate) const fn predicate_count(self) -> usize {
        self.exceptional.predicates().len()
    }

    /// Resolve one relative predicate by its position in this exact
    /// exceptional leaf. The broader relative-case and partition authority
    /// remain sealed.
    pub(crate) fn predicate(
        self,
        position: usize,
    ) -> Option<GeneratedSectorAffineEffectiveResidualExceptionalPredicateSourceView<'owner>> {
        let predicate = self.exceptional.predicates().get(position)?;
        Some(
            GeneratedSectorAffineEffectiveResidualExceptionalPredicateSourceView {
                locus_ordinal: predicate.locus_ordinal(),
                kind: predicate.kind(),
                polynomial: predicate.polynomial(),
            },
        )
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualExceptionalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualExceptionalSourceView")
            .field("target", &self.target)
            .field("exceptional", &self.exceptional)
            .field("private_predicates", &"<redacted>")
            .finish()
    }
}

/// Sealed, lifetime-bound semantic source for one item in the next epoch.
#[derive(Clone, Copy)]
pub(crate) enum GeneratedSectorAffineEffectiveResidualSourceView<'owner> {
    UnsupportedInventoryTerminal(
        GeneratedSectorAffineEffectiveResidualUnsupportedSourceView<'owner>,
    ),
    UnprocessedActionableCase(GeneratedSectorAffineEffectiveResidualTargetSourceView<'owner>),
    UnconsumedTargetRoot(GeneratedSectorAffineEffectiveResidualTargetSourceView<'owner>),
    ExceptionalDomain(GeneratedSectorAffineEffectiveResidualExceptionalSourceView<'owner>),
    ExceptionalLeak(GeneratedSectorAffineEffectiveResidualExceptionalSourceView<'owner>),
}

impl<'owner> GeneratedSectorAffineEffectiveResidualSourceView<'owner> {
    pub(crate) const fn terminal(
        self,
    ) -> GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner> {
        match self {
            Self::UnsupportedInventoryTerminal(view) => view.terminal(),
            Self::UnprocessedActionableCase(view) | Self::UnconsumedTargetRoot(view) => {
                view.terminal()
            }
            Self::ExceptionalDomain(view) | Self::ExceptionalLeak(view) => view.target().terminal(),
        }
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedInventoryTerminal(view) => formatter
                .debug_tuple("UnsupportedInventoryTerminal")
                .field(view)
                .finish(),
            Self::UnprocessedActionableCase(view) => formatter
                .debug_tuple("UnprocessedActionableCase")
                .field(view)
                .finish(),
            Self::UnconsumedTargetRoot(view) => formatter
                .debug_tuple("UnconsumedTargetRoot")
                .field(view)
                .finish(),
            Self::ExceptionalDomain(view) => formatter
                .debug_tuple("ExceptionalDomain")
                .field(view)
                .finish(),
            Self::ExceptionalLeak(view) => formatter
                .debug_tuple("ExceptionalLeak")
                .field(view)
                .finish(),
        }
    }
}

/// Authentication failures at the narrow source-view seam.
pub(crate) enum GeneratedSectorAffineEffectiveResidualSourceViewError {
    SchemaMismatch,
    WorkItemOutOfRange,
    AuthorityMismatch,
    ExceptionalAuthenticationFailed,
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualSourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::WorkItemOutOfRange => "WorkItemOutOfRange",
            Self::AuthorityMismatch => "AuthorityMismatch",
            Self::ExceptionalAuthenticationFailed => "ExceptionalAuthenticationFailed",
        };
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualSourceViewError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedSectorAffineEffectiveResidualSourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("effective residual source schema mismatch")
            }
            Self::WorkItemOutOfRange => {
                formatter.write_str("effective residual source work item is out of range")
            }
            Self::AuthorityMismatch => {
                formatter.write_str("effective residual source authority mismatch")
            }
            Self::ExceptionalAuthenticationFailed => {
                formatter.write_str("effective residual exceptional source authentication failed")
            }
        }
    }
}

impl std::error::Error for GeneratedSectorAffineEffectiveResidualSourceViewError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualQueueLimits {
    pub(crate) max_owner_replays: usize,
    pub(crate) max_terminal_record_visits: usize,
    pub(crate) max_ordered_child_output_visits: usize,
    pub(crate) max_authority_index_comparison_bound: usize,
    pub(crate) max_projection_payload_comparison_bound: usize,
    pub(crate) max_work_items: usize,
    pub(crate) max_retained_bytes: usize,
    pub(crate) max_temporary_bytes: usize,
    pub(crate) max_peak_visible_bytes: usize,
}

impl Default for GeneratedSectorAffineEffectiveResidualQueueLimits {
    fn default() -> Self {
        Self {
            max_owner_replays: 1,
            max_terminal_record_visits: 2_000_000_000,
            max_ordered_child_output_visits: 2_000_000_000,
            max_authority_index_comparison_bound: portable_usize(64_000_000_000),
            max_projection_payload_comparison_bound: portable_usize(64_000_000_000),
            max_work_items: 1_000_000_000,
            max_retained_bytes: portable_usize(64 * 1024 * 1024 * 1024),
            max_temporary_bytes: 1024 * 1024,
            max_peak_visible_bytes: portable_usize(64 * 1024 * 1024 * 1024),
        }
    }
}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualQueueStats {
    owner_replays: usize,
    terminal_record_visits: usize,
    ordered_child_output_visits: usize,
    authority_index_comparison_bound: usize,
    projection_payload_comparison_bound: usize,
    work_items: usize,
    owner_authority_retained_bytes: usize,
    retained_bytes: usize,
    temporary_bytes: usize,
    peak_visible_bytes: usize,
}

impl GeneratedSectorAffineEffectiveResidualQueueStats {
    pub(crate) const fn owner_replays(self) -> usize {
        self.owner_replays
    }
    pub(crate) const fn terminal_record_visits(self) -> usize {
        self.terminal_record_visits
    }
    pub(crate) const fn ordered_child_output_visits(self) -> usize {
        self.ordered_child_output_visits
    }
    pub(crate) const fn authority_index_comparison_bound(self) -> usize {
        self.authority_index_comparison_bound
    }
    pub(crate) const fn projection_payload_comparison_bound(self) -> usize {
        self.projection_payload_comparison_bound
    }
    pub(crate) const fn work_items(self) -> usize {
        self.work_items
    }
    pub(crate) const fn owner_authority_retained_bytes(self) -> usize {
        self.owner_authority_retained_bytes
    }
    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
    pub(crate) const fn temporary_bytes(self) -> usize {
        self.temporary_bytes
    }
    pub(crate) const fn peak_visible_bytes(self) -> usize {
        self.peak_visible_bytes
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualQueueStats {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualQueueStats")
            .field("owner_replays", &self.owner_replays)
            .field("terminal_record_visits", &self.terminal_record_visits)
            .field(
                "ordered_child_output_visits",
                &self.ordered_child_output_visits,
            )
            .field(
                "authority_index_comparison_bound",
                &self.authority_index_comparison_bound,
            )
            .field(
                "projection_payload_comparison_bound",
                &self.projection_payload_comparison_bound,
            )
            .field("work_items", &self.work_items)
            .field(
                "owner_authority_retained_bytes",
                &self.owner_authority_retained_bytes,
            )
            .field("retained_bytes", &self.retained_bytes)
            .field("temporary_bytes", &self.temporary_bytes)
            .field("peak_visible_bytes", &self.peak_visible_bytes)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QueueCensus {
    work_items: usize,
    terminal_records: usize,
    ordered_child_outputs: usize,
}

pub(crate) enum GeneratedSectorAffineEffectiveResidualQueueError {
    SchemaMismatch,
    Owner(GeneratedSectorAffineEffectiveCoverageError),
    OwnerCensusMismatch,
    MalformedOwnerAuthority,
    ReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailed {
        resource: &'static str,
        source: TryReserveError,
    },
    Point(GeneratedSectorAffinePointError),
    PointAuthorityMismatch,
    SymbolicaPanic,
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::Owner(_) => "Owner",
            Self::OwnerCensusMismatch => "OwnerCensusMismatch",
            Self::MalformedOwnerAuthority => "MalformedOwnerAuthority",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::AllocationFailed { .. } => "AllocationFailed",
            Self::Point(_) => "Point",
            Self::PointAuthorityMismatch => "PointAuthorityMismatch",
            Self::SymbolicaPanic => "SymbolicaPanic",
        };
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualQueueError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedSectorAffineEffectiveResidualQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("effective residual queue schema mismatch"),
            Self::Owner(_) => formatter.write_str("effective residual queue owner replay failed"),
            Self::OwnerCensusMismatch => {
                formatter.write_str("effective residual queue owner census mismatch")
            }
            Self::MalformedOwnerAuthority => {
                formatter.write_str("effective residual queue owner authority mismatch")
            }
            Self::ReplayMismatch => formatter.write_str("effective residual queue replay mismatch"),
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("effective residual queue resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("effective residual queue resource limit exceeded")
            }
            Self::AllocationFailed { .. } => {
                formatter.write_str("effective residual queue allocation failed")
            }
            Self::Point(_) => {
                formatter.write_str("effective residual queue point classification failed")
            }
            Self::PointAuthorityMismatch => {
                formatter.write_str("effective residual queue point authority mismatch")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during effective residual queue operation")
            }
        }
    }
}

impl std::error::Error for GeneratedSectorAffineEffectiveResidualQueueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::AllocationFailed { source, .. } => Some(source),
            Self::Point(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeneratedSectorAffineEffectiveCoverageError>
    for GeneratedSectorAffineEffectiveResidualQueueError
{
    fn from(value: GeneratedSectorAffineEffectiveCoverageError) -> Self {
        Self::Owner(value)
    }
}

impl From<GeneratedSectorAffinePointError> for GeneratedSectorAffineEffectiveResidualQueueError {
    fn from(value: GeneratedSectorAffinePointError) -> Self {
        Self::Point(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualQueuePointLimits {
    pub(crate) owner: GeneratedSectorAffinePointLimits,
    pub(crate) max_work_item_scans: usize,
}

impl Default for GeneratedSectorAffineEffectiveResidualQueuePointLimits {
    fn default() -> Self {
        Self {
            owner: GeneratedSectorAffinePointLimits::default(),
            max_work_item_scans: 1_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedSectorAffineEffectiveResidualQueuePointDisposition {
    Excluded,
    Work {
        work_item_ordinal: usize,
        locator: GeneratedSectorAffineEffectiveResidualWorkLocator,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualQueuePointClassification {
    disposition: GeneratedSectorAffineEffectiveResidualQueuePointDisposition,
    owner: GeneratedSectorAffinePointStats,
    work_item_scans: usize,
}

impl GeneratedSectorAffineEffectiveResidualQueuePointClassification {
    pub(crate) const fn disposition(
        self,
    ) -> GeneratedSectorAffineEffectiveResidualQueuePointDisposition {
        self.disposition
    }
    pub(crate) const fn owner_stats(self) -> GeneratedSectorAffinePointStats {
        self.owner
    }
    pub(crate) const fn work_item_scans(self) -> usize {
        self.work_item_scans
    }
}

#[derive(Clone)]
pub(crate) struct GeneratedSectorAffineEffectiveResidualQueueCertificate {
    schema: &'static str,
    owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
    work_items: Vec<GeneratedSectorAffineEffectiveResidualWorkItem>,
    limits: GeneratedSectorAffineEffectiveResidualQueueLimits,
    stats: GeneratedSectorAffineEffectiveResidualQueueStats,
}

impl GeneratedSectorAffineEffectiveResidualQueueCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn owner(&self) -> &Arc<GeneratedSectorAffineEffectiveCoverageCertificate> {
        &self.owner
    }
    pub(crate) fn work_items(&self) -> &[GeneratedSectorAffineEffectiveResidualWorkItem] {
        &self.work_items
    }

    /// Resolve one queue ordinal through its private owner-navigation slot.
    ///
    /// This operation is allocation-free.  It accepts no caller-created
    /// locator and returns only lifetime-bound references into this exact
    /// retained owner graph.
    pub(crate) fn authenticated_source_view(
        &self,
        work_item_ordinal: usize,
    ) -> Result<
        GeneratedSectorAffineEffectiveResidualSourceView<'_>,
        GeneratedSectorAffineEffectiveResidualSourceViewError,
    > {
        authenticated_source_view_inner(self, work_item_ordinal)
    }

    pub(crate) const fn len(&self) -> usize {
        self.work_items.len()
    }
    pub(crate) const fn is_empty(&self) -> bool {
        self.work_items.is_empty()
    }
    pub(crate) const fn limits(&self) -> GeneratedSectorAffineEffectiveResidualQueueLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedSectorAffineEffectiveResidualQueueStats {
        self.stats
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_authority(&mut self) -> bool {
        let Some(item) = self.work_items.first_mut() else {
            return false;
        };
        item.authority = match item.authority {
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnsupportedInventoryTerminal {
                terminal_record_ordinal,
                projection,
            } => {
                GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnsupportedInventoryTerminal {
                    terminal_record_ordinal: terminal_record_ordinal.saturating_add(1),
                    projection,
                }
            }
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnprocessedActionableCase {
                terminal_record_ordinal,
                projection,
            } => GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnprocessedActionableCase {
                terminal_record_ordinal: terminal_record_ordinal.saturating_add(1),
                projection,
            },
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnconsumedTargetRoot {
                terminal_record_ordinal,
                target_disposition_ordinal,
                residual_work_ordinal,
                projection,
            } => GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnconsumedTargetRoot {
                terminal_record_ordinal: terminal_record_ordinal.saturating_add(1),
                target_disposition_ordinal,
                residual_work_ordinal,
                projection,
            },
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                terminal_record_ordinal,
                child_output_ordinal,
                target_disposition_ordinal,
                attempt_ordinal,
                selected_target_position,
                residual_work_ordinal,
                projection,
            } => GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                terminal_record_ordinal,
                child_output_ordinal: child_output_ordinal.saturating_add(1),
                target_disposition_ordinal,
                attempt_ordinal,
                selected_target_position,
                residual_work_ordinal,
                projection,
            },
        };
        true
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_exceptional_target_disposition_index(&mut self) -> bool {
        self.work_items.iter_mut().any(|item| {
            let GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                target_disposition_ordinal,
                ..
            } = &mut item.authority
            else {
                return false;
            };
            *target_disposition_ordinal = target_disposition_ordinal.saturating_add(1);
            true
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_exceptional_attempt_index(&mut self) -> bool {
        self.work_items.iter_mut().any(|item| {
            let GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                attempt_ordinal,
                ..
            } = &mut item.authority
            else {
                return false;
            };
            *attempt_ordinal = attempt_ordinal.saturating_add(1);
            true
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_exceptional_selected_position(&mut self) -> bool {
        self.work_items.iter_mut().any(|item| {
            let GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                selected_target_position,
                ..
            } = &mut item.authority
            else {
                return false;
            };
            *selected_target_position = selected_target_position.saturating_add(1);
            true
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_exceptional_residual_index(&mut self) -> bool {
        self.work_items.iter_mut().any(|item| {
            let GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                residual_work_ordinal,
                ..
            } = &mut item.authority
            else {
                return false;
            };
            *residual_work_ordinal = residual_work_ordinal.saturating_add(1);
            true
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_unconsumed_target_disposition_index(&mut self) -> bool {
        self.work_items.iter_mut().any(|item| {
            let GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnconsumedTargetRoot {
                target_disposition_ordinal,
                ..
            } = &mut item.authority
            else {
                return false;
            };
            *target_disposition_ordinal = target_disposition_ordinal.saturating_add(1);
            true
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_unconsumed_residual_index(&mut self) -> bool {
        self.work_items.iter_mut().any(|item| {
            let GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnconsumedTargetRoot {
                residual_work_ordinal,
                ..
            } = &mut item.authority
            else {
                return false;
            };
            *residual_work_ordinal = residual_work_ordinal.saturating_add(1);
            true
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_projection_witness(&mut self) -> Option<usize> {
        self.work_items
            .iter_mut()
            .enumerate()
            .find_map(|(ordinal, item)| {
                match &mut item.authority {
                    GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnsupportedInventoryTerminal {
                        projection, ..
                    } => {
                        projection.structural_locus_count =
                            projection.structural_locus_count.saturating_add(1);
                    }
                    GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnprocessedActionableCase {
                        projection, ..
                    }
                    | GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnconsumedTargetRoot {
                        projection, ..
                    }
                    | GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                        projection, ..
                    } => {
                        projection.free_position_count =
                            projection.free_position_count.saturating_add(1);
                    }
                }
                Some(ordinal)
            })
    }

    /// Reauthenticate the exact owner and validate this queue in place.  The
    /// replay cursor compares expected items directly and never constructs a
    /// second output vector.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
        catch_unwind(AssertUnwindSafe(|| replay_inner(self, family, context)))
            .map_err(|_| GeneratedSectorAffineEffectiveResidualQueueError::SymbolicaPanic)?
    }

    /// Classify through the retained owner, then restrict the result to this
    /// exact residual union. Global coverage, generated rules, and points
    /// outside the source sector are excluded from the next epoch.
    pub(crate) fn classification_for_indices(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedSectorAffineEffectiveResidualQueuePointLimits,
    ) -> Result<
        GeneratedSectorAffineEffectiveResidualQueuePointClassification,
        GeneratedSectorAffineEffectiveResidualQueueError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            classify_point_inner(self, family, context, indices, limits)
        }))
        .map_err(|_| GeneratedSectorAffineEffectiveResidualQueueError::SymbolicaPanic)?
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveResidualQueueCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveResidualQueueCertificate")
            .field("schema", &self.schema)
            .field("work_item_count", &self.work_items.len())
            .field("stats", &self.stats)
            .field("private_owner", &"<redacted>")
            .finish()
    }
}

struct ResolvedTerminalAuthority<'owner> {
    view: GeneratedSectorAffineEffectiveResidualTerminalSourceView<'owner>,
    terminal_record: &'owner GeneratedSectorAffineTerminalRecord,
    inventory_terminal: &'owner GeneratedResidualAffineInventoryTerminal,
}

struct ResolvedTargetAuthority<'owner> {
    view: GeneratedSectorAffineEffectiveResidualTargetSourceView<'owner>,
    inventory_case: &'owner GeneratedResidualAffineInventoryCase,
}

fn authenticated_source_view_inner(
    certificate: &GeneratedSectorAffineEffectiveResidualQueueCertificate,
    work_item_ordinal: usize,
) -> Result<
    GeneratedSectorAffineEffectiveResidualSourceView<'_>,
    GeneratedSectorAffineEffectiveResidualSourceViewError,
> {
    if certificate.schema != GENERATED_SECTOR_AFFINE_EFFECTIVE_RESIDUAL_QUEUE_V1_SCHEMA
        || certificate.owner.schema()
            != crate::generated_sector_affine_effective_coverage::GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA
        || certificate.owner.inventory().schema()
            != crate::GENERATED_RESIDUAL_AFFINE_CASE_INVENTORY_V1_SCHEMA
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::SchemaMismatch);
    }
    if !Arc::ptr_eq(
        certificate.owner.source_queue(),
        certificate.owner.inventory().source_queue(),
    ) {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    let item = certificate
        .work_items
        .get(work_item_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::WorkItemOutOfRange)?;

    match (item.locator, item.authority) {
        (
            GeneratedSectorAffineEffectiveResidualWorkLocator::Root(
                locator @ GeneratedSectorAffineResidualRootLocator::UnsupportedInventoryTerminal {
                    terminal_ordinal,
                },
            ),
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnsupportedInventoryTerminal {
                terminal_record_ordinal,
                projection,
            },
        ) if terminal_ordinal == terminal_record_ordinal => {
            let terminal = resolve_terminal_source(
                certificate,
                work_item_ordinal,
                item.locator,
                terminal_record_ordinal,
            )?;
            if terminal.terminal_record.disposition()
                != GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator)
                || terminal.inventory_terminal.outcome()
                    != GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported
                || terminal.inventory_terminal.guard_composition().is_some()
            {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            }
            let source_branch = terminal
                .inventory_terminal
                .source_branch()
                .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
            let ResidualAffineBranchSystemOutcome::Unsupported { reasons } =
                source_branch.outcome()
            else {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            };
            let ready = source_branch
                .ready_terminal()
                .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
            let structural_loci = source_branch
                .source_cover()
                .source_queue()
                .discovery()
                .coverage()
                .structural_loci();
            if source_branch.schema() != crate::RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA
                || !Arc::ptr_eq(
                    source_branch.source_cover(),
                    terminal.inventory_terminal.source_cover(),
                )
                || ready.ordinal() != terminal.inventory_terminal.locator().terminal_ordinal()
                || !matches!(
                    ready.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                )
                || !ready.remaining_clauses().is_empty()
                || ready.equal_zero_atoms().len() != projection.equal_zero_locus_count
                || ready.nonzero_atoms().len() != projection.nonzero_locus_count
                || source_branch.nonzero_guard_locus_ordinals().len()
                    != projection.nonzero_locus_count
                || structural_loci.len() != projection.structural_locus_count
                || reasons.len() != projection.unsupported_reason_count
            {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            }
            Ok(
                GeneratedSectorAffineEffectiveResidualSourceView::UnsupportedInventoryTerminal(
                    GeneratedSectorAffineEffectiveResidualUnsupportedSourceView {
                        terminal: terminal.view,
                        ready_terminal_ordinal: ready.ordinal(),
                        equal_zero_locus_ordinals: ready.equal_zero_atoms(),
                        nonzero_locus_ordinals: ready.nonzero_atoms(),
                        structural_loci,
                        unsupported_reasons: reasons,
                    },
                ),
            )
        }
        (
            GeneratedSectorAffineEffectiveResidualWorkLocator::Root(
                locator @ GeneratedSectorAffineResidualRootLocator::UnprocessedActionableCase {
                    case_ordinal,
                },
            ),
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnprocessedActionableCase {
                terminal_record_ordinal,
                projection,
            },
        ) => {
            let terminal = resolve_terminal_source(
                certificate,
                work_item_ordinal,
                item.locator,
                terminal_record_ordinal,
            )?;
            if terminal.terminal_record.disposition()
                != GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator)
            {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            }
            let target = resolve_target_source(certificate, terminal, case_ordinal, projection)?;
            let (pass, group) = resolve_case_pass(certificate, target.inventory_case)?;
            let GeneratedSectorAffineGroupPassOutcome::NoAvailableRows(no_rows) = pass.outcome()
            else {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            };
            let anchor = certificate
                .owner
                .inventory()
                .cases()
                .get(group.anchor_case_ordinal())
                .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
            if no_rows.schema() != crate::GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA
                || !Arc::ptr_eq(no_rows.branch(), anchor.source_branch())
                || !Arc::ptr_eq(no_rows.branch_guards(), anchor.guard_composition())
            {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            }
            Ok(
                GeneratedSectorAffineEffectiveResidualSourceView::UnprocessedActionableCase(
                    target.view,
                ),
            )
        }
        (
            GeneratedSectorAffineEffectiveResidualWorkLocator::Root(
                locator @ GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
                    group_pass_ordinal,
                    target_case_ordinal,
                },
            ),
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnconsumedTargetRoot {
                terminal_record_ordinal,
                target_disposition_ordinal,
                residual_work_ordinal,
                projection,
            },
        ) => {
            let terminal = resolve_terminal_source(
                certificate,
                work_item_ordinal,
                item.locator,
                terminal_record_ordinal,
            )?;
            if terminal.terminal_record.disposition()
                != GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator)
            {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            }
            let target =
                resolve_target_source(certificate, terminal, target_case_ordinal, projection)?;
            let effective =
                resolve_effective_pass(certificate, target.inventory_case, group_pass_ordinal)?;
            authenticate_unconsumed_target(
                effective,
                target.inventory_case,
                target_disposition_ordinal,
                residual_work_ordinal,
            )?;
            Ok(GeneratedSectorAffineEffectiveResidualSourceView::UnconsumedTargetRoot(target.view))
        }
        (
            GeneratedSectorAffineEffectiveResidualWorkLocator::Exceptional(locator),
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
                terminal_record_ordinal,
                child_output_ordinal,
                target_disposition_ordinal,
                attempt_ordinal,
                selected_target_position,
                residual_work_ordinal,
                projection,
            },
        ) => resolve_exceptional_source(
            certificate,
            work_item_ordinal,
            item.locator,
            terminal_record_ordinal,
            child_output_ordinal,
            target_disposition_ordinal,
            attempt_ordinal,
            selected_target_position,
            residual_work_ordinal,
            projection,
            locator,
        ),
        _ => Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch),
    }
}

fn resolve_terminal_source<'owner>(
    certificate: &'owner GeneratedSectorAffineEffectiveResidualQueueCertificate,
    work_item_ordinal: usize,
    locator: GeneratedSectorAffineEffectiveResidualWorkLocator,
    terminal_record_ordinal: usize,
) -> Result<ResolvedTerminalAuthority<'owner>, GeneratedSectorAffineEffectiveResidualSourceViewError>
{
    let terminal_record = certificate
        .owner
        .terminal_records()
        .get(terminal_record_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let inventory_terminal = certificate
        .owner
        .inventory()
        .terminals()
        .get(terminal_record_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let source_locator = inventory_terminal.locator();
    let source_cover = inventory_terminal.source_cover();
    if terminal_record.inventory_terminal_ordinal() != terminal_record_ordinal
        || terminal_record.source_locator() != source_locator
        || terminal_record.source_outcome() != inventory_terminal.outcome()
        || source_cover.source_work_item_ordinal() != source_locator.work_item_ordinal()
        || source_cover.source_case() != source_locator.source_case()
        || !Arc::ptr_eq(
            source_cover.source_queue(),
            certificate.owner.source_queue(),
        )
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    if let Some(branch) = inventory_terminal.source_branch()
        && !Arc::ptr_eq(branch.source_cover(), source_cover)
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    if let Some(guards) = inventory_terminal.guard_composition()
        && (!Arc::ptr_eq(guards.source_cover(), source_cover)
            || !inventory_terminal
                .source_branch()
                .is_some_and(|branch| Arc::ptr_eq(guards.source_branch(), branch)))
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    if source_cover.schema() != crate::RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA
        || certificate.owner.source_queue().schema()
            != crate::GENERATED_SECTOR_LIVE_LEAF_QUEUE_V1_SCHEMA
            && certificate.owner.source_queue().schema()
                != crate::GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::SchemaMismatch);
    }
    Ok(ResolvedTerminalAuthority {
        view: GeneratedSectorAffineEffectiveResidualTerminalSourceView {
            work_item_ordinal,
            locator,
            source_locator,
            source_outcome: inventory_terminal.outcome(),
            lifetime: std::marker::PhantomData,
        },
        terminal_record,
        inventory_terminal,
    })
}

fn resolve_target_source<'owner>(
    certificate: &'owner GeneratedSectorAffineEffectiveResidualQueueCertificate,
    terminal: ResolvedTerminalAuthority<'owner>,
    case_ordinal: usize,
    projection: TargetProjectionAuthority,
) -> Result<ResolvedTargetAuthority<'owner>, GeneratedSectorAffineEffectiveResidualSourceViewError>
{
    if terminal.inventory_terminal.outcome()
        != (GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal })
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    let inventory_case = certificate
        .owner
        .inventory()
        .cases()
        .get(case_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let source_cover = inventory_case.source_cover();
    let source_branch = inventory_case.source_branch();
    let guard_composition = inventory_case.guard_composition();
    if inventory_case.ordinal() != case_ordinal
        || inventory_case.locator() != terminal.inventory_terminal.locator()
        || !Arc::ptr_eq(
            inventory_case.source_cover(),
            terminal.inventory_terminal.source_cover(),
        )
        || !terminal
            .inventory_terminal
            .source_branch()
            .is_some_and(|branch| Arc::ptr_eq(inventory_case.source_branch(), branch))
        || !terminal
            .inventory_terminal
            .guard_composition()
            .is_some_and(|guards| Arc::ptr_eq(inventory_case.guard_composition(), guards))
        || source_cover.schema() != crate::RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA
        || source_branch.schema() != crate::RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA
        || guard_composition.schema() != crate::RESIDUAL_AFFINE_BRANCH_GUARD_COMPOSITION_V1_SCHEMA
        || !matches!(
            source_branch.outcome(),
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        )
        || guard_composition.has_contradiction()
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    let affine_map = source_branch
        .affine_map()
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let group = certificate
        .owner
        .inventory()
        .groups()
        .get(inventory_case.group_ordinal())
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let anchor = certificate
        .owner
        .inventory()
        .cases()
        .get(group.anchor_case_ordinal())
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    if group.ordinal() != inventory_case.group_ordinal()
        || group
            .case_ordinals()
            .get(inventory_case.ordinal_within_group())
            != Some(&case_ordinal)
        || group.case_ordinals().first() != Some(&group.anchor_case_ordinal())
        || anchor.ordinal() != group.anchor_case_ordinal()
        || anchor.group_ordinal() != group.ordinal()
        || anchor.ordinal_within_group() != 0
        || group.ambient_arity() != affine_map.ambient_arity()
        || inventory_case.constants().len() != affine_map.ambient_arity()
        || guard_composition.entries().len() != projection.guard_entry_count
        || inventory_case.constants().len() != projection.constant_count
        || group.free_positions().len() != projection.free_position_count
        || affine_map.free_positions().len() != projection.free_position_count
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    Ok(ResolvedTargetAuthority {
        view: GeneratedSectorAffineEffectiveResidualTargetSourceView {
            terminal: terminal.view,
            case_ordinal,
            source_locator: inventory_case.locator(),
            group_ordinal: inventory_case.group_ordinal(),
            ordinal_within_group: inventory_case.ordinal_within_group(),
            anchor_case_ordinal: group.anchor_case_ordinal(),
            affine_map,
            guard_entries: guard_composition.entries(),
            constants: inventory_case.constants(),
            free_positions: group.free_positions(),
        },
        inventory_case,
    })
}

fn resolve_case_pass<'owner>(
    certificate: &'owner GeneratedSectorAffineEffectiveResidualQueueCertificate,
    inventory_case: &GeneratedResidualAffineInventoryCase,
) -> Result<
    (
        &'owner crate::generated_sector_affine_effective_coverage::GeneratedSectorAffineGroupPass,
        &'owner crate::GeneratedResidualAffineContiguousCaseGroup,
    ),
    GeneratedSectorAffineEffectiveResidualSourceViewError,
> {
    let inventory = certificate.owner.inventory();
    let group = inventory
        .groups()
        .get(inventory_case.group_ordinal())
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let pass = certificate
        .owner
        .group_passes()
        .get(inventory_case.group_ordinal())
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    if group.ordinal() != inventory_case.group_ordinal()
        || group
            .case_ordinals()
            .get(inventory_case.ordinal_within_group())
            != Some(&inventory_case.ordinal())
        || pass.pass_ordinal() != inventory_case.group_ordinal()
        || pass.group_ordinal() != group.ordinal()
        || pass.source_case_ordinal() != group.anchor_case_ordinal()
        || group.case_ordinals().first() != Some(&group.anchor_case_ordinal())
        || !inventory
            .cases()
            .get(group.anchor_case_ordinal())
            .is_some_and(|anchor| {
                anchor.ordinal() == group.anchor_case_ordinal()
                    && anchor.group_ordinal() == group.ordinal()
                    && anchor.ordinal_within_group() == 0
            })
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    Ok((pass, group))
}

fn resolve_effective_pass<'owner>(
    certificate: &'owner GeneratedSectorAffineEffectiveResidualQueueCertificate,
    inventory_case: &GeneratedResidualAffineInventoryCase,
    group_pass_ordinal: usize,
) -> Result<
    &'owner GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    GeneratedSectorAffineEffectiveResidualSourceViewError,
> {
    let (pass, group) = resolve_case_pass(certificate, inventory_case)?;
    if pass.pass_ordinal() != group_pass_ordinal {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    let GeneratedSectorAffineGroupPassOutcome::Effective(effective) = pass.outcome() else {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    };
    if effective.schema()
        != crate::generated_residual_affine_group_effective_coverage::GENERATED_RESIDUAL_AFFINE_GROUP_EFFECTIVE_COVERAGE_V1_SCHEMA
        || effective.matcher().schema()
            != crate::GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA
        || !Arc::ptr_eq(
        effective.matcher().inventory(),
        certificate.owner.inventory(),
    ) || effective.matcher().source_group_ordinal() != group.ordinal()
        || effective.matcher().source_case_ordinal() != group.anchor_case_ordinal()
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    Ok(effective)
}

fn authenticate_unconsumed_target(
    effective: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    inventory_case: &GeneratedResidualAffineInventoryCase,
    target_disposition_ordinal: usize,
    residual_work_ordinal: usize,
) -> Result<(), GeneratedSectorAffineEffectiveResidualSourceViewError> {
    let target = effective
        .target_dispositions()
        .get(target_disposition_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    if target_disposition_ordinal != inventory_case.ordinal_within_group()
        || target.target_case_ordinal() != inventory_case.ordinal()
        || target.target_locator() != inventory_case.locator()
        || !matches!(
            target.disposition(),
            GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. }
        )
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    let residual = effective
        .residual_work()
        .get(residual_work_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    if residual.target_case_ordinal() != inventory_case.ordinal()
        || residual.target_locator() != inventory_case.locator()
        || residual.kind() != GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
        || residual.accepted_attempt_ordinal().is_some()
        || residual.leaf_ordinal().is_some()
        || residual.relative_case().is_some()
        || residual.when_bad().is_some()
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resolve_exceptional_source(
    certificate: &GeneratedSectorAffineEffectiveResidualQueueCertificate,
    work_item_ordinal: usize,
    item_locator: GeneratedSectorAffineEffectiveResidualWorkLocator,
    terminal_record_ordinal: usize,
    child_output_ordinal: usize,
    target_disposition_ordinal: usize,
    attempt_ordinal: usize,
    selected_target_position: usize,
    residual_work_ordinal: usize,
    projection: TargetProjectionAuthority,
    locator: GeneratedSectorAffineExceptionalChildLocator,
) -> Result<
    GeneratedSectorAffineEffectiveResidualSourceView<'_>,
    GeneratedSectorAffineEffectiveResidualSourceViewError,
> {
    let terminal = resolve_terminal_source(
        certificate,
        work_item_ordinal,
        item_locator,
        terminal_record_ordinal,
    )?;
    let (group_pass_ordinal, target_case_ordinal, first_child_output_ordinal, child_output_count) =
        match terminal.terminal_record.disposition() {
            GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                group_pass_ordinal,
                target_case_ordinal,
                first_child_output_ordinal,
                child_output_count,
            } => (
                group_pass_ordinal,
                target_case_ordinal,
                first_child_output_ordinal,
                child_output_count,
            ),
            _ => {
                return Err(
                    GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch,
                );
            }
        };
    let expected_child = first_child_output_ordinal
        .checked_add(locator.leaf_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let child_end = first_child_output_ordinal
        .checked_add(child_output_count)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    if group_pass_ordinal != locator.group_pass_ordinal
        || child_output_ordinal != expected_child
        || child_output_ordinal >= child_end
        || certificate
            .owner
            .ordered_child_outputs()
            .get(child_output_ordinal)
            != Some(&GeneratedSectorAffineOrderedChildOutput::Exceptional(
                locator,
            ))
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    let target = resolve_target_source(certificate, terminal, target_case_ordinal, projection)?;
    let effective = resolve_effective_pass(certificate, target.inventory_case, group_pass_ordinal)?;

    let target_record = effective
        .target_dispositions()
        .get(target_disposition_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let (accepted_attempt_ordinal, retained_when_bad) = match target_record.disposition() {
        GeneratedResidualAffineGroupTargetDisposition::Consumed {
            accepted_attempt_ordinal,
            when_bad,
        } => (*accepted_attempt_ordinal, when_bad),
        GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. } => {
            return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
        }
    };
    if target_disposition_ordinal != target.inventory_case.ordinal_within_group()
        || target_record.target_case_ordinal() != target_case_ordinal
        || target_record.target_locator() != target.inventory_case.locator()
        || accepted_attempt_ordinal != locator.accepted_attempt_ordinal
        || accepted_attempt_ordinal != attempt_ordinal
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }

    let attempt = effective
        .attempts()
        .get(attempt_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let GeneratedResidualAffineTargetAttemptOutcome::Accepted(attempt_when_bad) = attempt.outcome()
    else {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    };
    if attempt.attempt_ordinal() != attempt_ordinal
        || attempt.pivot_ordinal() != accepted_attempt_ordinal
        || attempt.selected_target_case_ordinal() != Some(target_case_ordinal)
        || attempt.selected_target_position() != Some(selected_target_position)
        || !Arc::ptr_eq(attempt_when_bad, retained_when_bad)
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }

    let residual = effective
        .residual_work()
        .get(residual_work_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    let relative_case = residual
        .relative_case()
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    if residual.target_case_ordinal() != target_case_ordinal
        || residual.accepted_attempt_ordinal() != Some(accepted_attempt_ordinal)
        || residual.leaf_ordinal() != Some(locator.leaf_ordinal)
        || residual.target_locator() != target.inventory_case.locator()
        || !residual
            .when_bad()
            .is_some_and(|when_bad| Arc::ptr_eq(when_bad, retained_when_bad))
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }
    let GeneratedResidualAffineWhenBadCompilation::Certified(certified) =
        retained_when_bad.as_ref()
    else {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    };
    let binding = certified.binding();
    let matcher = effective.matcher();
    let pending = matcher
        .outcomes()
        .get(attempt_ordinal)
        .and_then(|outcome| match outcome {
            crate::GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) => {
                Some(pending)
            }
            _ => None,
        })
        .ok_or(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)?;
    if effective.schema()
        != crate::generated_residual_affine_group_effective_coverage::GENERATED_RESIDUAL_AFFINE_GROUP_EFFECTIVE_COVERAGE_V1_SCHEMA
        || matcher.schema() != crate::GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA
        || certified.schema()
            != crate::generated_residual_affine_when_bad_compilation::GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA
        || matcher.outcomes().len() != effective.attempts().len()
        || pending.pivot_ordinal() != attempt_ordinal
        || pending
            .matching_target_case_ordinals()
            .get(selected_target_position)
            != Some(&target_case_ordinal)
        || binding.source_group_ordinal() != matcher.source_group_ordinal()
        || binding.source_case_ordinal() != effective.matcher().source_case_ordinal()
        || binding.pivot_ordinal() != attempt.pivot_ordinal()
        || binding.target_case_ordinal() != target_case_ordinal
        || binding.target_locator() != target.inventory_case.locator()
        || binding.target_position_in_matching_list() != selected_target_position
        || binding.target_ordinal_within_group()
            != target.inventory_case.ordinal_within_group()
        || binding.sector() != certificate.owner.source_queue().sector()
        || certified.leaf_classifications().len() != child_output_count
    {
        return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
    }

    let (kind, is_domain) = match residual.kind() {
        GeneratedResidualAffineResidualWorkKind::ExceptionalDomain { condition_ordinal } => (
            GeneratedResidualAffineWhenBadExceptionalKind::Domain { condition_ordinal },
            true,
        ),
        GeneratedResidualAffineResidualWorkKind::ExceptionalLeak { pullback_ordinal } => (
            GeneratedResidualAffineWhenBadExceptionalKind::Leak { pullback_ordinal },
            false,
        ),
        GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot => {
            return Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch);
        }
    };
    let exceptional = certified
        .exceptional_leaf_source_view(locator.leaf_ordinal, relative_case, kind)
        .map_err(|_| {
            GeneratedSectorAffineEffectiveResidualSourceViewError::ExceptionalAuthenticationFailed
        })?;
    let view = GeneratedSectorAffineEffectiveResidualExceptionalSourceView {
        target: target.view,
        exceptional,
    };
    Ok(if is_domain {
        GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalDomain(view)
    } else {
        GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalLeak(view)
    })
}

pub(crate) struct GeneratedSectorAffineEffectiveResidualQueueCompiler;

impl GeneratedSectorAffineEffectiveResidualQueueCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
        limits: GeneratedSectorAffineEffectiveResidualQueueLimits,
    ) -> Result<
        GeneratedSectorAffineEffectiveResidualQueueCertificate,
        GeneratedSectorAffineEffectiveResidualQueueError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inner(family, context, owner, limits)
        }))
        .map_err(|_| GeneratedSectorAffineEffectiveResidualQueueError::SymbolicaPanic)?
    }
}

fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
    limits: GeneratedSectorAffineEffectiveResidualQueueLimits,
) -> Result<
    GeneratedSectorAffineEffectiveResidualQueueCertificate,
    GeneratedSectorAffineEffectiveResidualQueueError,
> {
    // Admit every queue-local traversal and byte surface from the owner's O(1)
    // certified census before the first linear scan. The owner is an
    // immutable private capability; exact scan agreement and authenticated
    // child replay follow this non-publishing preflight.
    let declared_census = declared_census(&owner);
    let declared_stats = construction_stats(&owner, &declared_census, declared_census.work_items)?;
    enforce_work_limits(declared_stats, limits)?;

    // This first pass is allocation-free and occurs only after its complete
    // visit/work/retained/temporary/peak envelope was admitted above.
    let census = census_owner(&owner)?;
    if census != declared_census {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::OwnerCensusMismatch);
    }

    owner.replay(family, context)?;

    let mut work_items = Vec::new();
    work_items
        .try_reserve_exact(census.work_items)
        .map_err(
            |source| GeneratedSectorAffineEffectiveResidualQueueError::AllocationFailed {
                resource: "effective residual queue work items",
                source,
            },
        )?;
    fill_work_items(&owner, &mut work_items)?;
    if work_items.len() != census.work_items {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::OwnerCensusMismatch);
    }

    let stats = construction_stats(&owner, &census, work_items.capacity())?;
    enforce_work_limits(stats, limits)?;
    let certificate = GeneratedSectorAffineEffectiveResidualQueueCertificate {
        schema: GENERATED_SECTOR_AFFINE_EFFECTIVE_RESIDUAL_QUEUE_V1_SCHEMA,
        owner,
        work_items,
        limits,
        stats,
    };
    Ok(certificate)
}

fn replay_inner(
    certificate: &GeneratedSectorAffineEffectiveResidualQueueCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    if certificate.schema != GENERATED_SECTOR_AFFINE_EFFECTIVE_RESIDUAL_QUEUE_V1_SCHEMA {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::SchemaMismatch);
    }
    let declared_census = declared_census(&certificate.owner);
    let declared_stats = construction_stats(
        &certificate.owner,
        &declared_census,
        certificate.work_items.capacity(),
    )?;
    enforce_work_limits(declared_stats, certificate.limits)?;
    if declared_stats != certificate.stats
        || declared_census.work_items != certificate.work_items.len()
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch);
    }
    certificate.owner.replay(family, context)?;
    let census = census_owner(&certificate.owner)?;
    if census != declared_census {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch);
    }
    validate_items_in_place(&certificate.owner, &certificate.work_items)?;
    let rebuilt = construction_stats(
        &certificate.owner,
        &census,
        certificate.work_items.capacity(),
    )?;
    enforce_work_limits(rebuilt, certificate.limits)?;
    if rebuilt != certificate.stats {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch);
    }
    Ok(())
}

fn declared_census(owner: &GeneratedSectorAffineEffectiveCoverageCertificate) -> QueueCensus {
    let stats = owner.stats();
    QueueCensus {
        work_items: stats.residual_locators(),
        terminal_records: stats.terminal_records(),
        ordered_child_outputs: stats.ordered_child_outputs(),
    }
}

fn census_owner(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
) -> Result<QueueCensus, GeneratedSectorAffineEffectiveResidualQueueError> {
    let mut work_items = 0usize;
    let mut ordered_child_outputs = 0usize;
    for (terminal_record_ordinal, terminal) in owner.terminal_records().iter().enumerate() {
        if terminal.inventory_terminal_ordinal() != terminal_record_ordinal {
            return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
        }
        match terminal.disposition() {
            GeneratedSectorAffineTerminalDisposition::ProvedEmpty => {}
            GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator) => {
                validate_root(terminal_record_ordinal, terminal.source_outcome(), locator)?;
                work_items = checked_add("effective residual queue work items", work_items, 1)?;
            }
            GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                group_pass_ordinal,
                target_case_ordinal,
                first_child_output_ordinal,
                child_output_count,
            } => {
                validate_partition_source(
                    terminal.source_outcome(),
                    target_case_ordinal,
                    child_output_count,
                )?;
                let end = checked_add(
                    "effective residual queue child range",
                    first_child_output_ordinal,
                    child_output_count,
                )?;
                let children = owner
                    .ordered_child_outputs()
                    .get(first_child_output_ordinal..end)
                    .ok_or(
                        GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority,
                    )?;
                ordered_child_outputs = checked_add(
                    "effective residual queue ordered child outputs",
                    ordered_child_outputs,
                    children.len(),
                )?;
                for child in children {
                    match child {
                        GeneratedSectorAffineOrderedChildOutput::Rule(locator) => {
                            if locator.group_pass_ordinal != group_pass_ordinal {
                                return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
                            }
                        }
                        GeneratedSectorAffineOrderedChildOutput::Exceptional(locator) => {
                            if locator.group_pass_ordinal != group_pass_ordinal {
                                return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
                            }
                            work_items =
                                checked_add("effective residual queue work items", work_items, 1)?;
                        }
                    }
                }
            }
        }
    }
    if owner.stats().terminal_records() != owner.terminal_records().len()
        || owner.stats().ordered_child_outputs() != ordered_child_outputs
        || owner.stats().residual_locators() != work_items
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::OwnerCensusMismatch);
    }
    Ok(QueueCensus {
        work_items,
        terminal_records: owner.terminal_records().len(),
        ordered_child_outputs,
    })
}

fn fill_work_items(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    output: &mut Vec<GeneratedSectorAffineEffectiveResidualWorkItem>,
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    for (terminal_record_ordinal, terminal) in owner.terminal_records().iter().enumerate() {
        match terminal.disposition() {
            GeneratedSectorAffineTerminalDisposition::ProvedEmpty => {}
            GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator) => {
                output.push(root_item(owner, terminal_record_ordinal, locator)?);
            }
            GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                first_child_output_ordinal,
                child_output_count,
                ..
            } => {
                let end = checked_add(
                    "effective residual queue child range",
                    first_child_output_ordinal,
                    child_output_count,
                )?;
                for (relative_ordinal, child) in owner
                    .ordered_child_outputs()
                    .get(first_child_output_ordinal..end)
                    .ok_or(
                        GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority,
                    )?
                    .iter()
                    .enumerate()
                {
                    if let GeneratedSectorAffineOrderedChildOutput::Exceptional(locator) = child {
                        output.push(exceptional_item(
                            owner,
                            terminal_record_ordinal,
                            checked_add(
                                "effective residual queue child ordinal",
                                first_child_output_ordinal,
                                relative_ordinal,
                            )?,
                            *locator,
                        )?);
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_items_in_place(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    actual: &[GeneratedSectorAffineEffectiveResidualWorkItem],
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    let mut cursor = 0usize;
    for (terminal_record_ordinal, terminal) in owner.terminal_records().iter().enumerate() {
        match terminal.disposition() {
            GeneratedSectorAffineTerminalDisposition::ProvedEmpty => {}
            GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator) => {
                compare_next(
                    actual,
                    &mut cursor,
                    root_item(owner, terminal_record_ordinal, locator)?,
                )?;
            }
            GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                first_child_output_ordinal,
                child_output_count,
                ..
            } => {
                let end = checked_add(
                    "effective residual queue child range",
                    first_child_output_ordinal,
                    child_output_count,
                )?;
                for (relative_ordinal, child) in owner
                    .ordered_child_outputs()
                    .get(first_child_output_ordinal..end)
                    .ok_or(
                        GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority,
                    )?
                    .iter()
                    .enumerate()
                {
                    if let GeneratedSectorAffineOrderedChildOutput::Exceptional(locator) = child {
                        compare_next(
                            actual,
                            &mut cursor,
                            exceptional_item(
                                owner,
                                terminal_record_ordinal,
                                checked_add(
                                    "effective residual queue child ordinal",
                                    first_child_output_ordinal,
                                    relative_ordinal,
                                )?,
                                *locator,
                            )?,
                        )?;
                    }
                }
            }
        }
    }
    if cursor != actual.len() {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch);
    }
    Ok(())
}

fn compare_next(
    actual: &[GeneratedSectorAffineEffectiveResidualWorkItem],
    cursor: &mut usize,
    expected: GeneratedSectorAffineEffectiveResidualWorkItem,
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    if actual.get(*cursor) != Some(&expected) {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch);
    }
    *cursor = checked_add("effective residual queue replay cursor", *cursor, 1)?;
    Ok(())
}

/// Authenticate every collection later projected by an unsupported source
/// view. This runs only while building (or replaying) private authority slots,
/// after the retained owner has replayed. The prospective element work is
/// admitted by `projection_payload_comparison_bound`; resolution itself only
/// compares these sealed lengths.
fn unsupported_projection_authority(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    terminal_record_ordinal: usize,
) -> Result<UnsupportedProjectionAuthority, GeneratedSectorAffineEffectiveResidualQueueError> {
    let terminal = owner
        .inventory()
        .terminals()
        .get(terminal_record_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let branch = terminal
        .source_branch()
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let ready = branch
        .ready_terminal()
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let ResidualAffineBranchSystemOutcome::Unsupported { reasons } = branch.outcome() else {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    };
    let structural_loci = owner
        .source_queue()
        .discovery()
        .coverage()
        .structural_loci();
    let equal_zero = ready.equal_zero_atoms();
    let nonzero = ready.nonzero_atoms();
    let branch_nonzero = branch.nonzero_guard_locus_ordinals();
    let referenced_loci = checked_add(
        "effective residual queue unsupported referenced loci",
        equal_zero.len(),
        nonzero.len(),
    )?;

    // Reject every shape capable of exceeding the preflighted
    // `2 * structural_loci.len()` comparison envelope before either scan.
    if terminal.outcome() != GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported
        || !Arc::ptr_eq(branch.source_cover(), terminal.source_cover())
        || !Arc::ptr_eq(branch.source_cover().source_queue(), owner.source_queue())
        || ready.ordinal() != terminal.locator().terminal_ordinal()
        || branch_nonzero.len() != nonzero.len()
        || referenced_loci > structural_loci.len()
        || branch_nonzero != nonzero
        || equal_zero
            .iter()
            .chain(nonzero)
            .any(|&ordinal| ordinal >= structural_loci.len())
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    }

    Ok(UnsupportedProjectionAuthority {
        equal_zero_locus_count: equal_zero.len(),
        nonzero_locus_count: nonzero.len(),
        structural_locus_count: structural_loci.len(),
        unsupported_reason_count: reasons.len(),
    })
}

/// Authenticate the exact group/map position manifest once and seal only
/// collection lengths into the private work-item slot. Both manifest lengths
/// are rejected against sector arity before their bounded equality scan.
fn target_projection_authority(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    case_ordinal: usize,
) -> Result<TargetProjectionAuthority, GeneratedSectorAffineEffectiveResidualQueueError> {
    let inventory_case = owner
        .inventory()
        .cases()
        .get(case_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let group = owner
        .inventory()
        .groups()
        .get(inventory_case.group_ordinal())
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let affine_map = inventory_case
        .source_branch()
        .affine_map()
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let group_free_positions = group.free_positions();
    let map_free_positions = affine_map.free_positions();
    let sector_arity = owner.source_queue().sector().arity();

    if group.ambient_arity() != sector_arity
        || affine_map.ambient_arity() != sector_arity
        || group_free_positions.len() > sector_arity
        || map_free_positions.len() > sector_arity
        || group_free_positions != map_free_positions
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    }

    Ok(TargetProjectionAuthority {
        guard_entry_count: inventory_case.guard_composition().entries().len(),
        constant_count: inventory_case.constants().len(),
        free_position_count: group_free_positions.len(),
    })
}

fn root_item(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    terminal_record_ordinal: usize,
    locator: GeneratedSectorAffineResidualRootLocator,
) -> Result<
    GeneratedSectorAffineEffectiveResidualWorkItem,
    GeneratedSectorAffineEffectiveResidualQueueError,
> {
    let authority = match locator {
        GeneratedSectorAffineResidualRootLocator::UnsupportedInventoryTerminal { .. } => {
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnsupportedInventoryTerminal {
                terminal_record_ordinal,
                projection: unsupported_projection_authority(owner, terminal_record_ordinal)?,
            }
        }
        GeneratedSectorAffineResidualRootLocator::UnprocessedActionableCase { case_ordinal } => {
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnprocessedActionableCase {
                terminal_record_ordinal,
                projection: target_projection_authority(owner, case_ordinal)?,
            }
        }
        GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
            group_pass_ordinal,
            target_case_ordinal,
        } => {
            let (target_disposition_ordinal, effective) =
                indexed_effective_target(owner, group_pass_ordinal, target_case_ordinal)?;
            if !matches!(
                effective.target_dispositions()[target_disposition_ordinal].disposition(),
                GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. }
            ) {
                return Err(
                    GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority,
                );
            }
            let residual_work_ordinal = residual_work_index(effective, target_case_ordinal, None)?;
            GeneratedSectorAffineEffectiveResidualAuthoritySlot::UnconsumedTargetRoot {
                terminal_record_ordinal,
                target_disposition_ordinal,
                residual_work_ordinal,
                projection: target_projection_authority(owner, target_case_ordinal)?,
            }
        }
    };
    Ok(GeneratedSectorAffineEffectiveResidualWorkItem {
        locator: GeneratedSectorAffineEffectiveResidualWorkLocator::Root(locator),
        authority,
    })
}

fn exceptional_item(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    terminal_record_ordinal: usize,
    child_output_ordinal: usize,
    locator: GeneratedSectorAffineExceptionalChildLocator,
) -> Result<
    GeneratedSectorAffineEffectiveResidualWorkItem,
    GeneratedSectorAffineEffectiveResidualQueueError,
> {
    let terminal = owner
        .terminal_records()
        .get(terminal_record_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let (group_pass_ordinal, target_case_ordinal, first_child_output_ordinal, child_output_count) =
        match terminal.disposition() {
            GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                group_pass_ordinal,
                target_case_ordinal,
                first_child_output_ordinal,
                child_output_count,
            } => (
                group_pass_ordinal,
                target_case_ordinal,
                first_child_output_ordinal,
                child_output_count,
            ),
            _ => {
                return Err(
                    GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority,
                );
            }
        };
    let child_end = checked_add(
        "effective residual queue child range",
        first_child_output_ordinal,
        child_output_count,
    )?;
    if locator.group_pass_ordinal != group_pass_ordinal
        || child_output_ordinal < first_child_output_ordinal
        || child_output_ordinal >= child_end
        || owner.ordered_child_outputs().get(child_output_ordinal)
            != Some(&GeneratedSectorAffineOrderedChildOutput::Exceptional(
                locator,
            ))
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    }
    let (target_disposition_ordinal, effective) =
        indexed_effective_target(owner, group_pass_ordinal, target_case_ordinal)?;
    let target_record = effective
        .target_dispositions()
        .get(target_disposition_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let (attempt_ordinal, retained_when_bad) = match target_record.disposition() {
        GeneratedResidualAffineGroupTargetDisposition::Consumed {
            accepted_attempt_ordinal,
            when_bad,
        } => (*accepted_attempt_ordinal, when_bad),
        GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. } => {
            return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
        }
    };
    if attempt_ordinal != locator.accepted_attempt_ordinal {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    }
    let attempt = effective
        .attempts()
        .get(attempt_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let GeneratedResidualAffineTargetAttemptOutcome::Accepted(attempt_when_bad) = attempt.outcome()
    else {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    };
    let selected_target_position = attempt
        .selected_target_position()
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    if attempt.attempt_ordinal() != attempt_ordinal
        || attempt.pivot_ordinal() != attempt_ordinal
        || attempt.selected_target_case_ordinal() != Some(target_case_ordinal)
        || !Arc::ptr_eq(attempt_when_bad, retained_when_bad)
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    }
    let residual_work_ordinal =
        residual_work_index(effective, target_case_ordinal, Some(locator.leaf_ordinal))?;
    let residual = &effective.residual_work()[residual_work_ordinal];
    if residual.target_case_ordinal() != target_case_ordinal
        || residual.accepted_attempt_ordinal() != Some(attempt_ordinal)
        || residual.leaf_ordinal() != Some(locator.leaf_ordinal)
        || !residual
            .when_bad()
            .is_some_and(|when_bad| Arc::ptr_eq(when_bad, retained_when_bad))
        || matches!(
            residual.kind(),
            GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
        )
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    }
    Ok(GeneratedSectorAffineEffectiveResidualWorkItem {
        locator: GeneratedSectorAffineEffectiveResidualWorkLocator::Exceptional(locator),
        authority: GeneratedSectorAffineEffectiveResidualAuthoritySlot::ExceptionalChild {
            terminal_record_ordinal,
            child_output_ordinal,
            target_disposition_ordinal,
            attempt_ordinal,
            selected_target_position,
            residual_work_ordinal,
            projection: target_projection_authority(owner, target_case_ordinal)?,
        },
    })
}

fn indexed_effective_target(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    group_pass_ordinal: usize,
    target_case_ordinal: usize,
) -> Result<
    (
        usize,
        &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    ),
    GeneratedSectorAffineEffectiveResidualQueueError,
> {
    let target = owner
        .inventory()
        .cases()
        .get(target_case_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let group = owner
        .inventory()
        .groups()
        .get(target.group_ordinal())
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let pass = owner
        .group_passes()
        .get(group_pass_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let GeneratedSectorAffineGroupPassOutcome::Effective(effective) = pass.outcome() else {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    };
    let target_disposition_ordinal = target.ordinal_within_group();
    let target_record = effective
        .target_dispositions()
        .get(target_disposition_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    if target.ordinal() != target_case_ordinal
        || group.ordinal() != target.group_ordinal()
        || group.case_ordinals().get(target_disposition_ordinal) != Some(&target_case_ordinal)
        || group.case_ordinals().first() != Some(&group.anchor_case_ordinal())
        || pass.pass_ordinal() != group_pass_ordinal
        || pass.group_ordinal() != group.ordinal()
        || pass.source_case_ordinal() != group.anchor_case_ordinal()
        || effective.schema()
            != crate::generated_residual_affine_group_effective_coverage::GENERATED_RESIDUAL_AFFINE_GROUP_EFFECTIVE_COVERAGE_V1_SCHEMA
        || effective.matcher().schema()
            != crate::GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA
        || !Arc::ptr_eq(effective.matcher().inventory(), owner.inventory())
        || effective.matcher().source_group_ordinal() != group.ordinal()
        || effective.matcher().source_case_ordinal() != group.anchor_case_ordinal()
        || target_record.target_case_ordinal() != target_case_ordinal
        || target_record.target_locator() != target.locator()
    {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority);
    }
    Ok((target_disposition_ordinal, effective))
}

fn residual_work_index(
    effective: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    target_case_ordinal: usize,
    leaf_ordinal: Option<usize>,
) -> Result<usize, GeneratedSectorAffineEffectiveResidualQueueError> {
    let residuals = effective.residual_work();
    let start = residuals.partition_point(|leaf| leaf.target_case_ordinal() < target_case_ordinal);
    let end = residuals.partition_point(|leaf| leaf.target_case_ordinal() <= target_case_ordinal);
    let target_residuals = residuals
        .get(start..end)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)?;
    let relative = match leaf_ordinal {
        None => {
            if target_residuals.len() != 1
                || target_residuals[0].kind()
                    != GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
            {
                return Err(
                    GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority,
                );
            }
            0
        }
        Some(expected_leaf) => target_residuals
            .binary_search_by_key(&Some(expected_leaf), |leaf| leaf.leaf_ordinal())
            .map_err(|_| {
                GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority
            })?,
    };
    checked_add(
        "effective residual queue residual-work ordinal",
        start,
        relative,
    )
}

fn validate_root(
    terminal_record_ordinal: usize,
    source: GeneratedResidualAffineInventoryTerminalOutcome,
    locator: GeneratedSectorAffineResidualRootLocator,
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    let valid = match (source, locator) {
        (
            GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported,
            GeneratedSectorAffineResidualRootLocator::UnsupportedInventoryTerminal {
                terminal_ordinal,
            },
        ) => terminal_ordinal == terminal_record_ordinal,
        (
            GeneratedResidualAffineInventoryTerminalOutcome::Actionable {
                case_ordinal: source_case,
            },
            GeneratedSectorAffineResidualRootLocator::UnprocessedActionableCase { case_ordinal },
        ) => source_case == case_ordinal,
        (
            GeneratedResidualAffineInventoryTerminalOutcome::Actionable {
                case_ordinal: source_case,
            },
            GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
                target_case_ordinal,
                ..
            },
        ) => source_case == target_case_ordinal,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)
    }
}

fn validate_partition_source(
    source: GeneratedResidualAffineInventoryTerminalOutcome,
    target_case_ordinal: usize,
    child_output_count: usize,
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    if child_output_count > 0
        && matches!(
            source,
            GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal }
                if case_ordinal == target_case_ordinal
        )
    {
        Ok(())
    } else {
        Err(GeneratedSectorAffineEffectiveResidualQueueError::MalformedOwnerAuthority)
    }
}

fn construction_stats(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    census: &QueueCensus,
    capacity: usize,
) -> Result<
    GeneratedSectorAffineEffectiveResidualQueueStats,
    GeneratedSectorAffineEffectiveResidualQueueError,
> {
    if census.work_items != owner.stats().residual_locators() {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::OwnerCensusMismatch);
    }
    let temporary_bytes = checked_add(
        "effective residual queue temporary bytes",
        size_of::<QueueCensus>(),
        size_of::<GeneratedSectorAffineEffectiveResidualWorkItem>(),
    )?;
    let owner_authority_retained_bytes = owner.stats().outer_retained_bytes();
    let retained_bytes = retained_bytes_for_capacity(owner_authority_retained_bytes, capacity)?;
    let effective_residual_items = checked_add(
        "effective residual queue indexed authority items",
        owner.stats().unconsumed_target_roots(),
        owner.stats().exceptional_child_locators(),
    )?;
    let per_lookup_comparisons = checked_mul(
        "effective residual queue authority binary-search comparison bound",
        binary_search_comparison_bound(effective_residual_items),
        3,
    )?;
    let target_projection_items = checked_sum(
        "effective residual queue target projection items",
        [
            owner.stats().unprocessed_actionable_roots(),
            owner.stats().unconsumed_target_roots(),
            owner.stats().exceptional_child_locators(),
        ],
    )?;
    let target_projection_comparisons = checked_mul(
        "effective residual queue target projection comparison bound",
        target_projection_items,
        owner.source_queue().sector().arity(),
    )?;
    let unsupported_projection_comparisons = checked_mul(
        "effective residual queue unsupported projection comparison bound",
        checked_mul(
            "effective residual queue unsupported projection locus references",
            owner.stats().unsupported_residual_roots(),
            owner
                .source_queue()
                .discovery()
                .coverage()
                .structural_loci()
                .len(),
        )?,
        2,
    )?;
    Ok(GeneratedSectorAffineEffectiveResidualQueueStats {
        owner_replays: 1,
        terminal_record_visits: checked_mul(
            "effective residual queue terminal record visits",
            census.terminal_records,
            2,
        )?,
        ordered_child_output_visits: checked_mul(
            "effective residual queue ordered child output visits",
            census.ordered_child_outputs,
            2,
        )?,
        authority_index_comparison_bound: checked_mul(
            "effective residual queue authority index comparison bound",
            effective_residual_items,
            per_lookup_comparisons,
        )?,
        projection_payload_comparison_bound: checked_add(
            "effective residual queue projection payload comparison bound",
            target_projection_comparisons,
            unsupported_projection_comparisons,
        )?,
        work_items: census.work_items,
        owner_authority_retained_bytes,
        retained_bytes,
        temporary_bytes,
        peak_visible_bytes: checked_add(
            "effective residual queue peak visible bytes",
            retained_bytes,
            temporary_bytes,
        )?,
    })
}

fn binary_search_comparison_bound(len: usize) -> usize {
    if len == 0 {
        0
    } else {
        usize::BITS as usize - len.leading_zeros() as usize
    }
}

fn enforce_work_limits(
    stats: GeneratedSectorAffineEffectiveResidualQueueStats,
    limits: GeneratedSectorAffineEffectiveResidualQueueLimits,
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    for (resource, requested, limit) in [
        (
            "effective residual queue owner replays",
            stats.owner_replays,
            limits.max_owner_replays,
        ),
        (
            "effective residual queue terminal record visits",
            stats.terminal_record_visits,
            limits.max_terminal_record_visits,
        ),
        (
            "effective residual queue ordered child output visits",
            stats.ordered_child_output_visits,
            limits.max_ordered_child_output_visits,
        ),
        (
            "effective residual queue authority index comparison bound",
            stats.authority_index_comparison_bound,
            limits.max_authority_index_comparison_bound,
        ),
        (
            "effective residual queue projection payload comparison bound",
            stats.projection_payload_comparison_bound,
            limits.max_projection_payload_comparison_bound,
        ),
        (
            "effective residual queue work items",
            stats.work_items,
            limits.max_work_items,
        ),
        (
            "effective residual queue retained bytes",
            stats.retained_bytes,
            limits.max_retained_bytes,
        ),
        (
            "effective residual queue temporary bytes",
            stats.temporary_bytes,
            limits.max_temporary_bytes,
        ),
        (
            "effective residual queue peak visible bytes",
            stats.peak_visible_bytes,
            limits.max_peak_visible_bytes,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn retained_bytes(
    owner_authority_retained_bytes: usize,
    work_items: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveResidualQueueError> {
    checked_sum(
        "effective residual queue retained bytes",
        [
            owner_authority_retained_bytes,
            size_of::<GeneratedSectorAffineEffectiveResidualQueueCertificate>(),
            checked_mul(
                "effective residual queue retained work item bytes",
                work_items,
                size_of::<GeneratedSectorAffineEffectiveResidualWorkItem>(),
            )?,
        ],
    )
}

fn retained_bytes_for_capacity(
    owner_authority_retained_bytes: usize,
    capacity: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveResidualQueueError> {
    retained_bytes(owner_authority_retained_bytes, capacity)
}

fn classify_point_inner(
    certificate: &GeneratedSectorAffineEffectiveResidualQueueCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    limits: GeneratedSectorAffineEffectiveResidualQueuePointLimits,
) -> Result<
    GeneratedSectorAffineEffectiveResidualQueuePointClassification,
    GeneratedSectorAffineEffectiveResidualQueueError,
> {
    if certificate.schema != GENERATED_SECTOR_AFFINE_EFFECTIVE_RESIDUAL_QUEUE_V1_SCHEMA {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::SchemaMismatch);
    }
    let owner =
        certificate
            .owner
            .classification_for_indices(family, context, indices, limits.owner)?;
    let expected = match owner.disposition() {
        GeneratedSectorAffinePointDisposition::ResidualRoot(locator) => Some(
            GeneratedSectorAffineEffectiveResidualWorkLocator::Root(locator),
        ),
        GeneratedSectorAffinePointDisposition::Exceptional(locator) => {
            Some(GeneratedSectorAffineEffectiveResidualWorkLocator::Exceptional(locator))
        }
        GeneratedSectorAffinePointDisposition::OutsideSector
        | GeneratedSectorAffinePointDisposition::CoveredByGlobal { .. }
        | GeneratedSectorAffinePointDisposition::Rule(_) => None,
    };
    let Some(expected) = expected else {
        return Ok(
            GeneratedSectorAffineEffectiveResidualQueuePointClassification {
                disposition: GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Excluded,
                owner: owner.stats(),
                work_item_scans: 0,
            },
        );
    };
    check_limit(
        "effective residual queue point work item scans",
        certificate.work_items.len(),
        limits.max_work_item_scans,
    )?;
    let mut match_ordinal = None;
    let mut matches = 0usize;
    for (ordinal, item) in certificate.work_items.iter().enumerate() {
        if item.locator == expected {
            matches = checked_add("effective residual queue point matches", matches, 1)?;
            match_ordinal = Some(ordinal);
        }
    }
    if matches != 1 {
        return Err(GeneratedSectorAffineEffectiveResidualQueueError::PointAuthorityMismatch);
    }
    Ok(
        GeneratedSectorAffineEffectiveResidualQueuePointClassification {
            disposition: GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Work {
                work_item_ordinal: match_ordinal.ok_or(
                    GeneratedSectorAffineEffectiveResidualQueueError::PointAuthorityMismatch,
                )?,
                locator: expected,
            },
            owner: owner.stats(),
            work_item_scans: certificate.work_items.len(),
        },
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorAffineEffectiveResidualQueueError> {
    if requested > limit {
        Err(
            GeneratedSectorAffineEffectiveResidualQueueError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveResidualQueueError> {
    left.checked_add(right)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveResidualQueueError> {
    left.checked_mul(right)
        .ok_or(GeneratedSectorAffineEffectiveResidualQueueError::ResourceCountOverflow { resource })
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedSectorAffineEffectiveResidualQueueError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}
