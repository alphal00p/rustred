//! Matcher-bound signed-descent phase for generated affine `WhenBad`.
//!
//! This phase accepts only the private authority produced by
//! [`crate::generated_residual_affine_when_bad_compilation`].  It never
//! accepts an arbitrary relation, and it deliberately stops before domain
//! conditions or affine boundary pullbacks are considered.

use std::fmt;
use std::mem::size_of;

use crate::affine_parametric_ordering::{
    AffineConstantRowSectorTransition, AffineSectorPrefixDescentWitness,
};
use crate::generated_residual_affine_when_bad_compilation::{
    AuthenticatedGeneratedResidualAffineWhenBadInput, GeneratedResidualAffineWhenBadBinding,
    GeneratedResidualAffineWhenBadError,
};
use crate::when_bad::{
    WhenBadBoundaryHazardKind, WhenBadCoreError, WhenBadDescentComponent,
    WhenBadUniformDescentWitness, WhenBadUnsupportedReason, finite_boundary_hazard_range,
    prove_uniform_same_sector_descent,
};
use crate::{IndexShift, IntegralOrderingPolicy};

/// Stable identity of this private, matcher-bound compilation phase.
pub(crate) const GENERATED_RESIDUAL_AFFINE_WHEN_BAD_DESCENT_V1_SCHEMA: &str =
    "rustred-generated-residual-affine-when-bad-descent-v1";

/// Resource and work census for one descent phase.
///
/// `descent_witnesses_precharged` is always the exact authenticated RHS
/// count.  An unsupported result can have a shorter proved prefix, but all
/// witnesses and all full-arity component buffers were charged before that
/// prefix was inspected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineWhenBadDescentStats {
    rhs_terms: usize,
    descent_witnesses_precharged: usize,
    descent_witnesses_attempted: usize,
    descent_witnesses_proved: usize,
    descent_witness_components: usize,
    descent_witness_components_observed: usize,
    private_rhs_shift_components_precharged: usize,
    private_rhs_shift_components_observed: usize,
    payload_comparison_units_precharged: usize,
    payload_comparison_units_observed: usize,
    target_sector_rows_precharged: usize,
    target_sector_rows_observed: usize,
    target_sector_formal_mask_components_precharged: usize,
    target_sector_formal_mask_components_observed: usize,
    target_sector_maximal_mask_components_precharged: usize,
    target_sector_maximal_mask_components_observed: usize,
    target_sector_constant_transition_components_precharged: usize,
    target_sector_constant_transition_components_observed: usize,
    target_sector_activation_obligation_components_precharged: usize,
    target_sector_activation_obligation_components_observed: usize,
    aggregate_descent_components_precharged: usize,
    aggregate_descent_components_observed: usize,
    target_sector_constant_additions_precharged: usize,
    target_sector_constant_additions_observed: usize,
    target_sector_integer_bit_work_precharged: usize,
    target_sector_integer_bit_work_observed: usize,
    target_sector_fallbacks_attempted: usize,
    target_sector_fallbacks_proved: usize,
    target_sector_whole_target_proved: usize,
    target_sector_applicable_nonzero_term_domain_proved: usize,
    target_sector_constant_rows_inspected: usize,
    target_sector_symbolic_rows_inspected: usize,
    target_sector_universal_active_pinches: usize,
    target_sector_universal_inactive_activations: usize,
    target_sector_symbolic_activation_obligations: usize,
    target_ordering_manifest_bytes: usize,
    retained_byte_envelope: usize,
    retained_bytes: usize,
}

impl GeneratedResidualAffineWhenBadDescentStats {
    pub(crate) const fn rhs_terms(self) -> usize {
        self.rhs_terms
    }

    pub(crate) const fn descent_witnesses_precharged(self) -> usize {
        self.descent_witnesses_precharged
    }

    pub(crate) const fn descent_witnesses_attempted(self) -> usize {
        self.descent_witnesses_attempted
    }

    pub(crate) const fn descent_witnesses_proved(self) -> usize {
        self.descent_witnesses_proved
    }

    pub(crate) const fn descent_witness_components(self) -> usize {
        self.descent_witness_components
    }

    pub(crate) const fn descent_witness_components_observed(self) -> usize {
        self.descent_witness_components_observed
    }

    pub(crate) const fn private_rhs_shift_components_precharged(self) -> usize {
        self.private_rhs_shift_components_precharged
    }

    pub(crate) const fn private_rhs_shift_components_observed(self) -> usize {
        self.private_rhs_shift_components_observed
    }

    pub(crate) const fn payload_comparison_units_precharged(self) -> usize {
        self.payload_comparison_units_precharged
    }

    pub(crate) const fn payload_comparison_units_observed(self) -> usize {
        self.payload_comparison_units_observed
    }

    pub(crate) const fn target_sector_rows_precharged(self) -> usize {
        self.target_sector_rows_precharged
    }

    pub(crate) const fn target_sector_rows_observed(self) -> usize {
        self.target_sector_rows_observed
    }

    pub(crate) const fn target_sector_formal_mask_components_precharged(self) -> usize {
        self.target_sector_formal_mask_components_precharged
    }

    pub(crate) const fn target_sector_formal_mask_components_observed(self) -> usize {
        self.target_sector_formal_mask_components_observed
    }

    pub(crate) const fn target_sector_maximal_mask_components_precharged(self) -> usize {
        self.target_sector_maximal_mask_components_precharged
    }

    pub(crate) const fn target_sector_maximal_mask_components_observed(self) -> usize {
        self.target_sector_maximal_mask_components_observed
    }

    pub(crate) const fn target_sector_constant_transition_components_precharged(self) -> usize {
        self.target_sector_constant_transition_components_precharged
    }

    pub(crate) const fn target_sector_constant_transition_components_observed(self) -> usize {
        self.target_sector_constant_transition_components_observed
    }

    pub(crate) const fn target_sector_activation_obligation_components_precharged(self) -> usize {
        self.target_sector_activation_obligation_components_precharged
    }

    pub(crate) const fn target_sector_activation_obligation_components_observed(self) -> usize {
        self.target_sector_activation_obligation_components_observed
    }

    pub(crate) const fn aggregate_descent_components_precharged(self) -> usize {
        self.aggregate_descent_components_precharged
    }

    pub(crate) const fn aggregate_descent_components_observed(self) -> usize {
        self.aggregate_descent_components_observed
    }

    pub(crate) const fn target_sector_constant_additions_precharged(self) -> usize {
        self.target_sector_constant_additions_precharged
    }

    pub(crate) const fn target_sector_constant_additions_observed(self) -> usize {
        self.target_sector_constant_additions_observed
    }

    pub(crate) const fn target_sector_integer_bit_work_precharged(self) -> usize {
        self.target_sector_integer_bit_work_precharged
    }

    pub(crate) const fn target_sector_integer_bit_work_observed(self) -> usize {
        self.target_sector_integer_bit_work_observed
    }

    pub(crate) const fn target_sector_fallbacks_attempted(self) -> usize {
        self.target_sector_fallbacks_attempted
    }

    pub(crate) const fn target_sector_fallbacks_proved(self) -> usize {
        self.target_sector_fallbacks_proved
    }

    pub(crate) const fn target_sector_whole_target_proved(self) -> usize {
        self.target_sector_whole_target_proved
    }

    pub(crate) const fn target_sector_applicable_nonzero_term_domain_proved(self) -> usize {
        self.target_sector_applicable_nonzero_term_domain_proved
    }

    pub(crate) const fn target_sector_constant_rows_inspected(self) -> usize {
        self.target_sector_constant_rows_inspected
    }

    pub(crate) const fn target_sector_symbolic_rows_inspected(self) -> usize {
        self.target_sector_symbolic_rows_inspected
    }

    pub(crate) const fn target_sector_universal_active_pinches(self) -> usize {
        self.target_sector_universal_active_pinches
    }

    pub(crate) const fn target_sector_universal_inactive_activations(self) -> usize {
        self.target_sector_universal_inactive_activations
    }

    pub(crate) const fn target_sector_symbolic_activation_obligations(self) -> usize {
        self.target_sector_symbolic_activation_obligations
    }

    pub(crate) const fn target_ordering_manifest_bytes(self) -> usize {
        self.target_ordering_manifest_bytes
    }

    pub(crate) const fn retained_byte_envelope(self) -> usize {
        self.retained_byte_envelope
    }

    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
}

/// A deliberately redacted reason why the authenticated row cannot orient
/// every same-sector RHS below its centered pivot.
///
/// In particular, this view contains no RHS shift and no private coefficient
/// or relation payload.  Its `Debug` representation is therefore safe for a
/// future target-local public outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineWhenBadDescentUnsupportedReason {
    UnsupportedOrderingPolicy {
        actual: IntegralOrderingPolicy,
    },
    NonUniformSameSectorDescent {
        rhs_ordinal: usize,
        first_nonzero_component: WhenBadDescentComponent,
    },
    ZeroSameSectorComplexityDelta {
        rhs_ordinal: usize,
    },
    UnboundedIndexAddition {
        rhs_ordinal: usize,
        coordinate: usize,
    },
    NoUniversalConstantPinch {
        rhs_ordinal: usize,
    },
    NonDescendingTargetSectorPrefix {
        rhs_ordinal: usize,
    },
}

impl GeneratedResidualAffineWhenBadDescentUnsupportedReason {
    pub(crate) const fn rhs_ordinal(self) -> Option<usize> {
        match self {
            Self::UnsupportedOrderingPolicy { .. } => None,
            Self::NonUniformSameSectorDescent { rhs_ordinal, .. }
            | Self::ZeroSameSectorComplexityDelta { rhs_ordinal }
            | Self::UnboundedIndexAddition { rhs_ordinal, .. }
            | Self::NoUniversalConstantPinch { rhs_ordinal }
            | Self::NonDescendingTargetSectorPrefix { rhs_ordinal } => Some(rhs_ordinal),
        }
    }
}

impl From<WhenBadUnsupportedReason> for GeneratedResidualAffineWhenBadDescentUnsupportedReason {
    fn from(value: WhenBadUnsupportedReason) -> Self {
        match value {
            WhenBadUnsupportedReason::NonUniformSameSectorDescent {
                rhs_ordinal,
                rhs_shift: _,
                first_nonzero_component,
                delta: _,
            } => Self::NonUniformSameSectorDescent {
                rhs_ordinal,
                first_nonzero_component,
            },
            WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta {
                rhs_ordinal,
                rhs_shift: _,
            } => Self::ZeroSameSectorComplexityDelta { rhs_ordinal },
            WhenBadUnsupportedReason::UnboundedIndexAddition {
                rhs_ordinal,
                rhs_shift: _,
                coordinate,
                delta: _,
            } => Self::UnboundedIndexAddition {
                rhs_ordinal,
                coordinate,
            },
        }
    }
}

/// One authoritative replay route for each nonpivot RHS, in canonical BTree
/// order.  The ordinal indexes are private and never appear in public output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineWhenBadRhsDescentProof {
    SameSector { witness_ordinal: usize },
    TargetSector { witness_ordinal: usize },
}

impl GeneratedResidualAffineWhenBadRhsDescentProof {
    pub(crate) const fn witness_ordinal(self) -> usize {
        match self {
            Self::SameSector { witness_ordinal } | Self::TargetSector { witness_ordinal } => {
                witness_ordinal
            }
        }
    }

    pub(crate) const fn is_target_sector(self) -> bool {
        matches!(self, Self::TargetSector { .. })
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadRhsDescentProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameSector { .. } => formatter.write_str("SameSector(<private witness>)"),
            Self::TargetSector { .. } => formatter.write_str("TargetSector(<private witness>)"),
        }
    }
}

/// Domain on which a target-sector prefix certificate is already sealed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineTargetSectorDescentScope {
    /// Even the maximal possible sector, including every simultaneous
    /// symbolic inactive activation, is below the source prefix.
    WholeTarget,
    /// The formal sector is below the source only after every retained
    /// symbolic activation obligation is discharged by the later hazard
    /// owner.  This is deliberately not a directly applicable rule.
    ApplicableNonzeroTermDomain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineConstantTransitionKind {
    UniversalActivePinch,
    UniversalInactiveActivation,
}

/// Private exact position of one simultaneous constant-row transition.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineConstantTransition {
    position: usize,
    kind: GeneratedResidualAffineConstantTransitionKind,
}

impl GeneratedResidualAffineConstantTransition {
    pub(crate) const fn position(self) -> usize {
        self.position
    }

    pub(crate) const fn kind(self) -> GeneratedResidualAffineConstantTransitionKind {
        self.kind
    }
}

impl fmt::Debug for GeneratedResidualAffineConstantTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineConstantTransition")
            .field("position", &"<redacted>")
            .field("kind", &self.kind)
            .finish()
    }
}

/// Private finite boundary range which must be sealed before a conditional
/// target-sector witness may become an applicable rule.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineSymbolicActivationObligation {
    position: usize,
    first: i64,
    last: i64,
    count: usize,
}

impl GeneratedResidualAffineSymbolicActivationObligation {
    pub(crate) const fn position(self) -> usize {
        self.position
    }

    pub(crate) const fn first(self) -> i64 {
        self.first
    }

    pub(crate) const fn last(self) -> i64 {
        self.last
    }

    pub(crate) const fn count(self) -> usize {
        self.count
    }
}

impl fmt::Debug for GeneratedResidualAffineSymbolicActivationObligation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineSymbolicActivationObligation")
            .field("position", &"<redacted>")
            .field("range", &"<redacted>")
            .finish()
    }
}

/// Flat offsets for one exact target-sector certificate.  Flat storage keeps
/// retained allocation counts bounded independently of the number of RHSs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineTargetSectorDescentWitness {
    rhs_ordinal: usize,
    formal_sector_offset: usize,
    maximal_sector_offset: usize,
    sector_arity: usize,
    transition_offset: usize,
    transition_count: usize,
    activation_obligation_offset: usize,
    activation_obligation_count: usize,
    scope: GeneratedResidualAffineTargetSectorDescentScope,
    sector_prefix: AffineSectorPrefixDescentWitness,
}

impl GeneratedResidualAffineTargetSectorDescentWitness {
    pub(crate) const fn rhs_ordinal(self) -> usize {
        self.rhs_ordinal
    }

    pub(crate) const fn scope(self) -> GeneratedResidualAffineTargetSectorDescentScope {
        self.scope
    }

    pub(crate) const fn sector_prefix(self) -> AffineSectorPrefixDescentWitness {
        self.sector_prefix
    }

    pub(crate) const fn activation_obligation_count(self) -> usize {
        self.activation_obligation_count
    }
}

impl fmt::Debug for GeneratedResidualAffineTargetSectorDescentWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineTargetSectorDescentWitness")
            .field("rhs_ordinal", &self.rhs_ordinal)
            .field("scope", &self.scope)
            .field("sector_prefix", &self.sector_prefix)
            .field("private_sector_bits", &"<redacted>")
            .field("private_transitions", &self.transition_count)
            .field(
                "private_activation_obligations",
                &self.activation_obligation_count,
            )
            .finish()
    }
}

/// Private flat replay payload for all cross-sector RHSs.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineTargetSectorDescentTranscript {
    witnesses: Vec<GeneratedResidualAffineTargetSectorDescentWitness>,
    formal_sector_bits: Vec<bool>,
    maximal_sector_bits: Vec<bool>,
    constant_transitions: Vec<GeneratedResidualAffineConstantTransition>,
    symbolic_activation_obligations: Vec<GeneratedResidualAffineSymbolicActivationObligation>,
}

impl GeneratedResidualAffineTargetSectorDescentTranscript {
    fn try_with_precharged_capacities(
        rhs_count: usize,
        component_count: usize,
    ) -> Result<Self, GeneratedResidualAffineWhenBadDescentError> {
        let mut result = Self {
            witnesses: Vec::new(),
            formal_sector_bits: Vec::new(),
            maximal_sector_bits: Vec::new(),
            constant_transitions: Vec::new(),
            symbolic_activation_obligations: Vec::new(),
        };
        try_reserve_with_capacity_envelope(
            "generated affine WhenBad target-sector witnesses",
            &mut result.witnesses,
            rhs_count,
        )?;
        try_reserve_with_capacity_envelope(
            "generated affine WhenBad target-sector formal mask components",
            &mut result.formal_sector_bits,
            component_count,
        )?;
        try_reserve_with_capacity_envelope(
            "generated affine WhenBad target-sector maximal mask components",
            &mut result.maximal_sector_bits,
            component_count,
        )?;
        try_reserve_with_capacity_envelope(
            "generated affine WhenBad target-sector constant transitions",
            &mut result.constant_transitions,
            component_count,
        )?;
        try_reserve_with_capacity_envelope(
            "generated affine WhenBad symbolic activation obligations",
            &mut result.symbolic_activation_obligations,
            component_count,
        )?;
        Ok(result)
    }

    pub(crate) fn witnesses(&self) -> &[GeneratedResidualAffineTargetSectorDescentWitness] {
        &self.witnesses
    }

    pub(crate) fn formal_sector_bits(
        &self,
        witness: GeneratedResidualAffineTargetSectorDescentWitness,
    ) -> Option<&[bool]> {
        checked_private_slice(
            &self.formal_sector_bits,
            witness.formal_sector_offset,
            witness.sector_arity,
        )
    }

    pub(crate) fn maximal_sector_bits(
        &self,
        witness: GeneratedResidualAffineTargetSectorDescentWitness,
    ) -> Option<&[bool]> {
        checked_private_slice(
            &self.maximal_sector_bits,
            witness.maximal_sector_offset,
            witness.sector_arity,
        )
    }

    pub(crate) fn constant_transitions(
        &self,
        witness: GeneratedResidualAffineTargetSectorDescentWitness,
    ) -> Option<&[GeneratedResidualAffineConstantTransition]> {
        checked_private_slice(
            &self.constant_transitions,
            witness.transition_offset,
            witness.transition_count,
        )
    }

    pub(crate) fn symbolic_activation_obligations(
        &self,
        witness: GeneratedResidualAffineTargetSectorDescentWitness,
    ) -> Option<&[GeneratedResidualAffineSymbolicActivationObligation]> {
        checked_private_slice(
            &self.symbolic_activation_obligations,
            witness.activation_obligation_offset,
            witness.activation_obligation_count,
        )
    }

    fn has_unsealed_conditional_witness(&self) -> bool {
        self.witnesses.iter().any(|witness| {
            witness.scope
                == GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain
        })
    }
}

impl fmt::Debug for GeneratedResidualAffineTargetSectorDescentTranscript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let conditional = self
            .witnesses
            .iter()
            .filter(|witness| {
                witness.scope
                    == GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain
            })
            .count();
        formatter
            .debug_struct("GeneratedResidualAffineTargetSectorDescentTranscript")
            .field("witnesses", &self.witnesses.len())
            .field("conditional_witnesses", &conditional)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Private certificate that every nonzero-centered RHS term descends in the
/// exact target ordering.
///
/// The authenticated input and the witness vector cannot be replaced through
/// this API.  Retaining them in one object binds every private witness to the
/// exact target ordering manifest authenticated by the preceding phase.
pub(crate) struct GeneratedResidualAffineWhenBadDescentReady {
    input: AuthenticatedGeneratedResidualAffineWhenBadInput,
    private_witnesses: Vec<WhenBadUniformDescentWitness>,
    private_rhs_proofs: Vec<GeneratedResidualAffineWhenBadRhsDescentProof>,
    private_target_sector_transcript: GeneratedResidualAffineTargetSectorDescentTranscript,
    stats: GeneratedResidualAffineWhenBadDescentStats,
}

impl GeneratedResidualAffineWhenBadDescentReady {
    pub(crate) const fn schema(&self) -> &'static str {
        GENERATED_RESIDUAL_AFFINE_WHEN_BAD_DESCENT_V1_SCHEMA
    }

    pub(crate) const fn input(&self) -> &AuthenticatedGeneratedResidualAffineWhenBadInput {
        &self.input
    }

    pub(crate) const fn binding(&self) -> &GeneratedResidualAffineWhenBadBinding {
        self.input.binding()
    }

    pub(crate) fn target_ordering_manifest(&self) -> &str {
        self.input.binding().target_ordering_manifest()
    }

    /// Private handoff for the later condition/pullback owner.  This is not a
    /// public certificate view because each witness contains the exact RHS
    /// shift.
    pub(crate) fn private_witnesses(&self) -> &[WhenBadUniformDescentWitness] {
        &self.private_witnesses
    }

    /// Exactly one authoritative proof route per nonpivot RHS, in canonical
    /// BTree order.  Consumers must use this ordinal index rather than merge
    /// the two private witness lists heuristically.
    pub(crate) fn private_rhs_proofs(&self) -> &[GeneratedResidualAffineWhenBadRhsDescentProof] {
        &self.private_rhs_proofs
    }

    pub(crate) const fn private_target_sector_transcript(
        &self,
    ) -> &GeneratedResidualAffineTargetSectorDescentTranscript {
        &self.private_target_sector_transcript
    }

    pub(crate) fn requires_symbolic_activation_hazard_seal(&self) -> bool {
        self.private_target_sector_transcript
            .has_unsealed_conditional_witness()
    }

    /// This phase is proof preparation only.  In particular, a conditional
    /// target-sector witness cannot be used as a rule until the later hazard
    /// owner has discharged every private activation obligation.
    pub(crate) const fn is_directly_applicable_rule(&self) -> bool {
        false
    }

    pub(crate) const fn stats(&self) -> GeneratedResidualAffineWhenBadDescentStats {
        self.stats
    }

    pub(crate) fn into_private_parts(
        self,
    ) -> (
        AuthenticatedGeneratedResidualAffineWhenBadInput,
        Vec<WhenBadUniformDescentWitness>,
        Vec<GeneratedResidualAffineWhenBadRhsDescentProof>,
        GeneratedResidualAffineTargetSectorDescentTranscript,
        GeneratedResidualAffineWhenBadDescentStats,
    ) {
        (
            self.input,
            self.private_witnesses,
            self.private_rhs_proofs,
            self.private_target_sector_transcript,
            self.stats,
        )
    }

    pub(crate) fn payload_eq_same_authority(&self, other: &Self) -> bool {
        self.input.payload_eq_same_authority(&other.input)
            && self.private_witnesses == other.private_witnesses
            && self.private_rhs_proofs == other.private_rhs_proofs
            && self.private_target_sector_transcript == other.private_target_sector_transcript
            && self.stats == other.stats
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadDescentReady {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadDescentReady")
            .field("schema", &self.schema())
            .field("binding", &self.binding())
            .field("stats", &self.stats)
            .field("private_witnesses", &"<redacted>")
            .field("private_rhs_proofs", &"<redacted>")
            .field("private_target_sector_transcript", &"<redacted>")
            .finish()
    }
}

/// Authenticated but non-orientable matcher-bound row.
///
/// The private authority is retained for a later complete outcome/replay, but
/// neither accessors nor `Debug` expose it.
pub(crate) struct GeneratedResidualAffineWhenBadDescentUnsupported {
    input: AuthenticatedGeneratedResidualAffineWhenBadInput,
    reason: GeneratedResidualAffineWhenBadDescentUnsupportedReason,
    stats: GeneratedResidualAffineWhenBadDescentStats,
}

impl GeneratedResidualAffineWhenBadDescentUnsupported {
    pub(crate) const fn schema(&self) -> &'static str {
        GENERATED_RESIDUAL_AFFINE_WHEN_BAD_DESCENT_V1_SCHEMA
    }

    pub(crate) const fn binding(&self) -> &GeneratedResidualAffineWhenBadBinding {
        self.input.binding()
    }

    pub(crate) const fn reason(&self) -> GeneratedResidualAffineWhenBadDescentUnsupportedReason {
        self.reason
    }

    pub(crate) const fn stats(&self) -> GeneratedResidualAffineWhenBadDescentStats {
        self.stats
    }

    pub(crate) const fn input(&self) -> &AuthenticatedGeneratedResidualAffineWhenBadInput {
        &self.input
    }

    pub(crate) fn payload_eq_same_authority(&self, other: &Self) -> bool {
        self.input.payload_eq_same_authority(&other.input)
            && self.reason == other.reason
            && self.stats == other.stats
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadDescentUnsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadDescentUnsupported")
            .field("schema", &self.schema())
            .field("binding", &self.binding())
            .field("reason", &self.reason)
            .field("stats", &self.stats)
            .finish()
    }
}

/// Result of the matcher-bound descent phase.
pub(crate) enum GeneratedResidualAffineWhenBadDescentCompilation {
    Ready(GeneratedResidualAffineWhenBadDescentReady),
    Unsupported(GeneratedResidualAffineWhenBadDescentUnsupported),
}

impl fmt::Debug for GeneratedResidualAffineWhenBadDescentCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(ready) => ready.fmt(formatter),
            Self::Unsupported(unsupported) => unsupported.fmt(formatter),
        }
    }
}

/// Hard structural, resource, allocation, or arithmetic failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineWhenBadDescentError {
    AuthenticatedRhsCountMismatch {
        binding: usize,
        stats: usize,
    },
    PrivateRhsCountMismatch {
        authenticated: usize,
        observed: usize,
    },
    Authority(GeneratedResidualAffineWhenBadError),
    Core(WhenBadCoreError),
}

impl fmt::Display for GeneratedResidualAffineWhenBadDescentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthenticatedRhsCountMismatch { binding, stats } => write!(
                formatter,
                "generated affine WhenBad descent authenticated RHS counts disagree: binding {binding}, stats {stats}"
            ),
            Self::PrivateRhsCountMismatch {
                authenticated,
                observed,
            } => write!(
                formatter,
                "generated affine WhenBad descent authenticated {authenticated} RHS terms, observed {observed}"
            ),
            Self::Authority(error) => error.fmt(formatter),
            Self::Core(error) => write_when_bad_core_error(formatter, error),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineWhenBadDescentError {}

impl From<GeneratedResidualAffineWhenBadError> for GeneratedResidualAffineWhenBadDescentError {
    fn from(value: GeneratedResidualAffineWhenBadError) -> Self {
        Self::Authority(value)
    }
}

impl From<WhenBadCoreError> for GeneratedResidualAffineWhenBadDescentError {
    fn from(value: WhenBadCoreError) -> Self {
        Self::Core(value)
    }
}

/// Prove signed descent for the exact matcher-bound private row.
///
/// All RHS-count, component-count, retained-byte, and outer-vector allocation
/// checks complete before the first call to
/// [`prove_uniform_same_sector_descent`].
pub(crate) fn compile_generated_residual_affine_when_bad_descent(
    input: AuthenticatedGeneratedResidualAffineWhenBadInput,
) -> Result<
    GeneratedResidualAffineWhenBadDescentCompilation,
    GeneratedResidualAffineWhenBadDescentError,
> {
    let target_ordering = input.target_ordering();
    let actual_policy = target_ordering.policy();

    let authenticated_rhs_count = input.binding().rhs_terms();
    let authenticated_stats_rhs_count = input.stats().rhs_terms();
    if authenticated_rhs_count != authenticated_stats_rhs_count {
        return Err(
            GeneratedResidualAffineWhenBadDescentError::AuthenticatedRhsCountMismatch {
                binding: authenticated_rhs_count,
                stats: authenticated_stats_rhs_count,
            },
        );
    }
    let observed_rhs_count = input
        .relation()
        .terms()
        .len()
        .checked_sub(1)
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    if observed_rhs_count != authenticated_rhs_count {
        return Err(
            GeneratedResidualAffineWhenBadDescentError::PrivateRhsCountMismatch {
                authenticated: authenticated_rhs_count,
                observed: observed_rhs_count,
            },
        );
    }

    let limits = input.limits();
    check_limit(
        "generated affine WhenBad descent RHS terms",
        observed_rhs_count,
        limits.max_rhs_terms,
    )?;
    check_limit(
        "generated affine WhenBad descent witnesses",
        observed_rhs_count,
        limits.max_descent_witnesses,
    )?;
    let ambient_arity = target_ordering.arity();
    if input.relation().arity() != ambient_arity {
        return Err(WhenBadCoreError::WrongArity {
            expected: ambient_arity,
            actual: input.relation().arity(),
        }
        .into());
    }
    let structural_precharge = target_sector_structural_precharge(
        observed_rhs_count,
        ambient_arity,
        target_ordering.constant_positions().len(),
        limits.max_descent_witness_components,
    )?;
    // Authentication already performed one complete relation-component pass.
    // This phase precharges every later read of a private relation shift:
    // three complete relation passes (integer-work preflight, construction,
    // replay), two complete RHS passes inside the legacy same-sector witness
    // constructor, one possible target-sector fallback pass, one final witness
    // replay pass, and one indexed read per constant row in preflight.  The
    // fallback pass is charged for every RHS before its route is known.
    let relation_shift_components_per_pass = checked_mul(
        "generated affine WhenBad private RHS shift components",
        input.relation().terms().len(),
        ambient_arity,
    )?;
    if input.stats().private_relation_shift_components() != relation_shift_components_per_pass {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    let rhs_shift_components_per_pass = checked_mul(
        "generated affine WhenBad private RHS shift components",
        observed_rhs_count,
        ambient_arity,
    )?;
    let complete_relation_passes = checked_mul(
        "generated affine WhenBad private RHS shift components",
        relation_shift_components_per_pass,
        3,
    )?;
    let routed_rhs_passes = checked_mul(
        "generated affine WhenBad private RHS shift components",
        rhs_shift_components_per_pass,
        4,
    )?;
    let constant_preflight_reads = checked_mul(
        "generated affine WhenBad private RHS shift components",
        observed_rhs_count,
        target_ordering.constant_positions().len(),
    )?;
    let local_rhs_shift_components_precharged = checked_add(
        "generated affine WhenBad private RHS shift components",
        complete_relation_passes,
        checked_add(
            "generated affine WhenBad private RHS shift components",
            routed_rhs_passes,
            constant_preflight_reads,
        )?,
    )?;
    let private_rhs_shift_components_precharged = checked_add(
        "generated affine WhenBad private RHS shift components",
        input.stats().private_relation_shift_components(),
        local_rhs_shift_components_precharged,
    )?;
    check_limit(
        "generated affine WhenBad private RHS shift components",
        private_rhs_shift_components_precharged,
        limits.max_private_relation_shift_components,
    )?;
    let payload_comparison_units_precharged = payload_comparison_work_precharge(
        observed_rhs_count,
        ambient_arity,
        target_ordering.constant_positions().len(),
    )?;
    check_limit(
        "generated affine WhenBad payload comparison units",
        payload_comparison_units_precharged,
        limits.max_payload_comparison_units,
    )?;

    let retained_byte_envelope = descent_retained_byte_envelope(
        input.stats().retained_byte_envelope(),
        observed_rhs_count,
        structural_precharge.descent_witness_components,
    )?;
    check_limit(
        "generated affine WhenBad descent retained bytes",
        retained_byte_envelope,
        limits.max_retained_bytes,
    )?;

    let target_ordering_manifest_bytes = target_ordering.stable_manifest().len();
    let mut stats = GeneratedResidualAffineWhenBadDescentStats {
        rhs_terms: observed_rhs_count,
        descent_witnesses_precharged: observed_rhs_count,
        descent_witnesses_attempted: 0,
        descent_witnesses_proved: 0,
        descent_witness_components: structural_precharge.descent_witness_components,
        descent_witness_components_observed: 0,
        private_rhs_shift_components_precharged,
        private_rhs_shift_components_observed: input.stats().private_relation_shift_components(),
        payload_comparison_units_precharged,
        payload_comparison_units_observed: 0,
        target_sector_rows_precharged: structural_precharge.target_sector_rows,
        target_sector_rows_observed: 0,
        target_sector_formal_mask_components_precharged: structural_precharge
            .formal_mask_components,
        target_sector_formal_mask_components_observed: 0,
        target_sector_maximal_mask_components_precharged: structural_precharge
            .maximal_mask_components,
        target_sector_maximal_mask_components_observed: 0,
        target_sector_constant_transition_components_precharged: structural_precharge
            .constant_transition_components,
        target_sector_constant_transition_components_observed: 0,
        target_sector_activation_obligation_components_precharged: structural_precharge
            .activation_obligation_components,
        target_sector_activation_obligation_components_observed: 0,
        aggregate_descent_components_precharged: structural_precharge.aggregate_components,
        aggregate_descent_components_observed: 0,
        target_sector_constant_additions_precharged: structural_precharge.constant_additions,
        target_sector_constant_additions_observed: 0,
        target_sector_integer_bit_work_precharged: 0,
        target_sector_integer_bit_work_observed: 0,
        target_sector_fallbacks_attempted: 0,
        target_sector_fallbacks_proved: 0,
        target_sector_whole_target_proved: 0,
        target_sector_applicable_nonzero_term_domain_proved: 0,
        target_sector_constant_rows_inspected: 0,
        target_sector_symbolic_rows_inspected: 0,
        target_sector_universal_active_pinches: 0,
        target_sector_universal_inactive_activations: 0,
        target_sector_symbolic_activation_obligations: 0,
        target_ordering_manifest_bytes,
        retained_byte_envelope,
        retained_bytes: 0,
    };

    // Ordering support is a typed, authenticated unsupported outcome rather
    // than a hard error.  Keep this future-safe branch after the complete RHS,
    // component, and retained-byte precharge and before any shift scan.
    if actual_policy != IntegralOrderingPolicy::RustRedUnshiftedV1 {
        validate_final_work_census(&stats)?;
        stats.retained_bytes = unsupported_observed_retained_bytes(&input)?;
        if stats.retained_bytes > retained_byte_envelope {
            return Err(
                GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                    observed: stats.retained_bytes,
                    admitted: retained_byte_envelope,
                }
                .into(),
            );
        }
        return Ok(
            GeneratedResidualAffineWhenBadDescentCompilation::Unsupported(
                GeneratedResidualAffineWhenBadDescentUnsupported {
                    input,
                    reason: GeneratedResidualAffineWhenBadDescentUnsupportedReason::
                        UnsupportedOrderingPolicy {
                            actual: actual_policy,
                        },
                    stats,
                },
            ),
        );
    }

    // Census every prospective constant-row GMP addition before the first
    // RHS proof.  This pass reads only authenticated integer geometry and
    // shifts; coefficient and condition payloads are never inspected.
    let mut target_sector_integer_bit_work_precharged = 0usize;
    for (shift, _) in input.relation().terms() {
        if !charge_complete_rhs_shift_scan(shift, &mut stats)? {
            continue;
        }
        for (constant_ordinal, &expected_position) in
            target_ordering.constant_positions().iter().enumerate()
        {
            observe_private_rhs_shift_components(&mut stats, 1)?;
            let displacement = shift.values()[expected_position];
            observe_payload_comparison_units(&mut stats, 1)?;
            let (position, construction_work) = target_ordering
                .constant_row_shift_integer_bit_work_bound_by_ordinal(
                    constant_ordinal,
                    displacement,
                )
                .map_err(GeneratedResidualAffineWhenBadError::from)?;
            let (replay_position, replay_work) = target_ordering
                .replay_constant_row_shift_integer_bit_work_bound_by_ordinal(
                    constant_ordinal,
                    displacement,
                )
                .map_err(GeneratedResidualAffineWhenBadError::from)?;
            observe_payload_comparison_units(&mut stats, 2)?;
            if position != expected_position || replay_position != expected_position {
                return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
            }
            target_sector_integer_bit_work_precharged = checked_add(
                "generated affine WhenBad target-sector integer-bit work",
                target_sector_integer_bit_work_precharged,
                checked_add(
                    "generated affine WhenBad target-sector integer-bit work",
                    construction_work,
                    replay_work,
                )?,
            )?;
        }
    }
    let aggregate_target_constant_integer_bits = checked_add(
        "generated affine WhenBad target constant comparison integer bits",
        input.stats().target_constant_comparison_integer_bits(),
        target_sector_integer_bit_work_precharged,
    )?;
    check_limit(
        "generated affine WhenBad target constant comparison integer bits",
        aggregate_target_constant_integer_bits,
        limits.max_target_constant_comparison_integer_bits,
    )?;
    stats.target_sector_integer_bit_work_precharged = target_sector_integer_bit_work_precharged;

    // Every retained flat vector is reserved before the first proof.  Actual
    // capacities are checked against the same factor-two allocator envelope
    // used by the legacy witness implementation.
    let mut private_witnesses = Vec::new();
    try_reserve_with_capacity_envelope(
        "generated affine WhenBad descent witnesses",
        &mut private_witnesses,
        observed_rhs_count,
    )?;
    let mut private_rhs_proofs = Vec::new();
    try_reserve_with_capacity_envelope(
        "generated affine WhenBad authoritative RHS proofs",
        &mut private_rhs_proofs,
        observed_rhs_count,
    )?;
    let mut private_target_sector_transcript =
        GeneratedResidualAffineTargetSectorDescentTranscript::try_with_precharged_capacities(
            observed_rhs_count,
            structural_precharge.descent_witness_components,
        )?;

    // BTreeMap iteration is the canonical private RHS order.  Skipping the
    // unique zero-centered pivot gives stable contiguous RHS ordinals.
    // Defer construction of an unsupported outcome until the iterator has
    // released its private borrow, so the authority can move into the result.
    let mut unsupported_reason = None;
    let mut rhs_ordinal = 0usize;
    for (shift, _coefficient) in input.relation().terms() {
        if !charge_complete_rhs_shift_scan(shift, &mut stats)? {
            continue;
        }
        stats.descent_witnesses_attempted = checked_add(
            "generated affine WhenBad descent witnesses attempted",
            stats.descent_witnesses_attempted,
            1,
        )?;
        // `prove_uniform_same_sector_descent` first computes every signed
        // component and then copies the RHS shift into its private witness or
        // unsupported payload: two complete reads of the authenticated shift.
        observe_private_rhs_shift_components(
            &mut stats,
            checked_mul(
                "generated affine WhenBad private RHS shift components observed",
                ambient_arity,
                2,
            )?,
        )?;
        let same_sector_attempt =
            prove_uniform_same_sector_descent(target_ordering.sector(), rhs_ordinal, shift)?;
        observe_descent_components(
            &mut stats,
            DescentComponentClass::SameSectorWitness,
            ambient_arity,
        )?;
        observe_payload_comparison_units(&mut stats, 1)?;
        match same_sector_attempt {
            Ok(witness) => {
                check_same_sector_witness_heap(&witness, ambient_arity)?;
                let witness_ordinal = private_witnesses.len();
                push_precharged(
                    "generated affine WhenBad descent witnesses",
                    &mut private_witnesses,
                    witness,
                )?;
                push_precharged(
                    "generated affine WhenBad authoritative RHS proofs",
                    &mut private_rhs_proofs,
                    GeneratedResidualAffineWhenBadRhsDescentProof::SameSector { witness_ordinal },
                )?;
            }
            Err(reason @ WhenBadUnsupportedReason::UnboundedIndexAddition { .. }) => {
                unsupported_reason = Some(reason.into());
                break;
            }
            Err(reason) => {
                stats.target_sector_fallbacks_attempted = checked_add(
                    "generated affine WhenBad target-sector fallbacks attempted",
                    stats.target_sector_fallbacks_attempted,
                    1,
                )?;
                match classify_and_prove_target_sector_descent(
                    target_ordering,
                    rhs_ordinal,
                    shift,
                    &mut private_target_sector_transcript,
                    &mut stats,
                )? {
                    TargetSectorDescentAttempt::NoConstantSectorTransition => {
                        unsupported_reason = Some(reason.into());
                        break;
                    }
                    TargetSectorDescentAttempt::Proved {
                        witness_ordinal,
                        scope,
                    } => {
                        push_precharged(
                            "generated affine WhenBad authoritative RHS proofs",
                            &mut private_rhs_proofs,
                            GeneratedResidualAffineWhenBadRhsDescentProof::TargetSector {
                                witness_ordinal,
                            },
                        )?;
                        stats.target_sector_fallbacks_proved = checked_add(
                            "generated affine WhenBad target-sector fallbacks proved",
                            stats.target_sector_fallbacks_proved,
                            1,
                        )?;
                        match scope {
                            GeneratedResidualAffineTargetSectorDescentScope::WholeTarget => {
                                stats.target_sector_whole_target_proved = checked_add(
                                    "generated affine WhenBad whole-target sector proofs",
                                    stats.target_sector_whole_target_proved,
                                    1,
                                )?;
                            }
                            GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain => {
                                stats.target_sector_applicable_nonzero_term_domain_proved =
                                    checked_add(
                                        "generated affine WhenBad conditional target-sector proofs",
                                        stats.target_sector_applicable_nonzero_term_domain_proved,
                                        1,
                                    )?;
                            }
                        }
                    }
                    TargetSectorDescentAttempt::Unsupported(fallback_reason) => {
                        unsupported_reason = Some(fallback_reason);
                        break;
                    }
                }
            }
        }
        stats.descent_witnesses_proved = checked_add(
            "generated affine WhenBad descent witnesses proved",
            stats.descent_witnesses_proved,
            1,
        )?;
        rhs_ordinal = checked_add(
            "generated affine WhenBad authoritative RHS ordinal",
            rhs_ordinal,
            1,
        )?;
    }
    if let Some(reason) = unsupported_reason {
        validate_final_work_census(&stats)?;
        stats.retained_bytes = unsupported_observed_retained_bytes(&input)?;
        if stats.retained_bytes > retained_byte_envelope {
            return Err(
                GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                    observed: stats.retained_bytes,
                    admitted: retained_byte_envelope,
                }
                .into(),
            );
        }
        return Ok(
            GeneratedResidualAffineWhenBadDescentCompilation::Unsupported(
                GeneratedResidualAffineWhenBadDescentUnsupported {
                    input,
                    reason,
                    stats,
                },
            ),
        );
    }
    if private_rhs_proofs.len() != observed_rhs_count {
        return Err(
            GeneratedResidualAffineWhenBadDescentError::PrivateRhsCountMismatch {
                authenticated: observed_rhs_count,
                observed: private_rhs_proofs.len(),
            },
        );
    }
    let private_proof_payloads = checked_add(
        "generated affine WhenBad authoritative RHS proofs",
        private_witnesses.len(),
        private_target_sector_transcript.witnesses.len(),
    )?;
    if private_proof_payloads != observed_rhs_count {
        return Err(
            GeneratedResidualAffineWhenBadDescentError::PrivateRhsCountMismatch {
                authenticated: observed_rhs_count,
                observed: private_proof_payloads,
            },
        );
    }
    validate_authoritative_rhs_proofs(
        &input,
        &private_rhs_proofs,
        &private_witnesses,
        &private_target_sector_transcript,
        &mut stats,
    )?;
    validate_final_work_census(&stats)?;

    stats.retained_bytes = ready_observed_retained_bytes(
        &input,
        &private_witnesses,
        private_witnesses.capacity(),
        private_rhs_proofs.capacity(),
        &private_target_sector_transcript,
    )?;
    if stats.retained_bytes > retained_byte_envelope {
        return Err(
            GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                observed: stats.retained_bytes,
                admitted: retained_byte_envelope,
            }
            .into(),
        );
    }
    Ok(GeneratedResidualAffineWhenBadDescentCompilation::Ready(
        GeneratedResidualAffineWhenBadDescentReady {
            input,
            private_witnesses,
            private_rhs_proofs,
            private_target_sector_transcript,
            stats,
        },
    ))
}

/// The payload ceiling is an outer preflight, not an estimate of retained
/// bytes.  It covers every possible fallback on every RHS, both construction
/// and independent replay, including three prefix scans in each pass.  The
/// exact observed counter below charges every actual row/component visit and
/// scalar proof comparison; this deliberately generous closed-form envelope
/// is checked before the first proof.
fn payload_comparison_work_precharge(
    rhs_count: usize,
    ambient_arity: usize,
    constant_positions: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadDescentError> {
    const UNITS_PER_RHS_ROW: usize = 128;
    const UNITS_PER_CONSTANT_PREFLIGHT: usize = 8;
    const FIXED_UNITS_PER_RHS: usize = 64;
    const FINAL_TRANSCRIPT_FIXED_UNITS: usize = 16;
    let rhs_rows = checked_mul(
        "generated affine WhenBad payload comparison units",
        rhs_count,
        ambient_arity,
    )?;
    let constant_visits = checked_mul(
        "generated affine WhenBad payload comparison units",
        rhs_count,
        constant_positions,
    )?;
    checked_add(
        "generated affine WhenBad payload comparison units",
        checked_add(
            "generated affine WhenBad payload comparison units",
            checked_mul(
                "generated affine WhenBad payload comparison units",
                rhs_rows,
                UNITS_PER_RHS_ROW,
            )?,
            checked_add(
                "generated affine WhenBad payload comparison units",
                checked_mul(
                    "generated affine WhenBad payload comparison units",
                    constant_visits,
                    UNITS_PER_CONSTANT_PREFLIGHT,
                )?,
                checked_mul(
                    "generated affine WhenBad payload comparison units",
                    rhs_count,
                    FIXED_UNITS_PER_RHS,
                )?,
            )?,
        )?,
        FINAL_TRANSCRIPT_FIXED_UNITS,
    )
}

fn observe_payload_comparison_units(
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
    units: usize,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    stats.payload_comparison_units_observed = checked_add(
        "generated affine WhenBad payload comparison units observed",
        stats.payload_comparison_units_observed,
        units,
    )?;
    check_limit(
        "generated affine WhenBad payload comparison units observed",
        stats.payload_comparison_units_observed,
        stats.payload_comparison_units_precharged,
    )
}

fn observe_constant_addition(
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    stats.target_sector_constant_additions_observed = checked_add(
        "generated affine WhenBad target-sector constant additions observed",
        stats.target_sector_constant_additions_observed,
        1,
    )?;
    check_limit(
        "generated affine WhenBad target-sector constant additions observed",
        stats.target_sector_constant_additions_observed,
        stats.target_sector_constant_additions_precharged,
    )
}

/// Scan all components even after the first nonzero component.  That makes
/// the local census exact and prevents BTree-key shape from changing charged
/// work through short-circuiting.
fn charge_complete_rhs_shift_scan(
    shift: &IndexShift,
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
) -> Result<bool, GeneratedResidualAffineWhenBadDescentError> {
    let mut nonzero = false;
    for &component in shift.values() {
        observe_private_rhs_shift_components(stats, 1)?;
        nonzero |= component != 0;
    }
    Ok(nonzero)
}

fn observe_private_rhs_shift_components(
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
    count: usize,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    stats.private_rhs_shift_components_observed = checked_add(
        "generated affine WhenBad private RHS shift components observed",
        stats.private_rhs_shift_components_observed,
        count,
    )?;
    check_limit(
        "generated affine WhenBad private RHS shift components observed",
        stats.private_rhs_shift_components_observed,
        stats.private_rhs_shift_components_precharged,
    )
}

#[derive(Clone, Copy)]
enum DescentComponentClass {
    SameSectorWitness,
    TargetSectorRow,
    FormalSectorMask,
    MaximalSectorMask,
    ConstantTransition,
    ActivationObligation,
}

fn observe_descent_components(
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
    class: DescentComponentClass,
    count: usize,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    let (resource, observed, precharged) = match class {
        DescentComponentClass::SameSectorWitness => (
            "generated affine WhenBad descent witness components observed",
            &mut stats.descent_witness_components_observed,
            stats.descent_witness_components,
        ),
        DescentComponentClass::TargetSectorRow => (
            "generated affine WhenBad target-sector rows observed",
            &mut stats.target_sector_rows_observed,
            stats.target_sector_rows_precharged,
        ),
        DescentComponentClass::FormalSectorMask => (
            "generated affine WhenBad target-sector formal mask components observed",
            &mut stats.target_sector_formal_mask_components_observed,
            stats.target_sector_formal_mask_components_precharged,
        ),
        DescentComponentClass::MaximalSectorMask => (
            "generated affine WhenBad target-sector maximal mask components observed",
            &mut stats.target_sector_maximal_mask_components_observed,
            stats.target_sector_maximal_mask_components_precharged,
        ),
        DescentComponentClass::ConstantTransition => (
            "generated affine WhenBad target-sector constant transition components observed",
            &mut stats.target_sector_constant_transition_components_observed,
            stats.target_sector_constant_transition_components_precharged,
        ),
        DescentComponentClass::ActivationObligation => (
            "generated affine WhenBad target-sector activation obligation components observed",
            &mut stats.target_sector_activation_obligation_components_observed,
            stats.target_sector_activation_obligation_components_precharged,
        ),
    };
    *observed = checked_add(resource, *observed, count)?;
    check_limit(resource, *observed, precharged)?;
    stats.aggregate_descent_components_observed = checked_add(
        "generated affine WhenBad aggregate descent components observed",
        stats.aggregate_descent_components_observed,
        count,
    )?;
    check_limit(
        "generated affine WhenBad aggregate descent components observed",
        stats.aggregate_descent_components_observed,
        stats.aggregate_descent_components_precharged,
    )
}

fn validate_final_work_census(
    stats: &GeneratedResidualAffineWhenBadDescentStats,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    let component_sum = [
        stats.descent_witness_components_observed,
        stats.target_sector_rows_observed,
        stats.target_sector_formal_mask_components_observed,
        stats.target_sector_maximal_mask_components_observed,
        stats.target_sector_constant_transition_components_observed,
        stats.target_sector_activation_obligation_components_observed,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add(
            "generated affine WhenBad aggregate descent components observed",
            sum,
            value,
        )
    })?;
    let target_rows_by_kind = checked_add(
        "generated affine WhenBad target-sector rows observed",
        stats.target_sector_constant_rows_inspected,
        stats.target_sector_symbolic_rows_inspected,
    )?;
    let constant_transitions_by_kind = checked_add(
        "generated affine WhenBad target-sector constant transitions observed",
        stats.target_sector_universal_active_pinches,
        stats.target_sector_universal_inactive_activations,
    )?;
    if component_sum != stats.aggregate_descent_components_observed
        || target_rows_by_kind != stats.target_sector_rows_observed
        || stats.target_sector_formal_mask_components_observed != stats.target_sector_rows_observed
        || stats.target_sector_maximal_mask_components_observed != stats.target_sector_rows_observed
        || constant_transitions_by_kind
            != stats.target_sector_constant_transition_components_observed
        || stats.target_sector_symbolic_activation_obligations
            != stats.target_sector_activation_obligation_components_observed
        || stats.descent_witnesses_attempted > stats.descent_witnesses_precharged
        || stats.descent_witnesses_proved > stats.descent_witnesses_precharged
        || stats.descent_witness_components_observed > stats.descent_witness_components
        || stats.private_rhs_shift_components_observed
            > stats.private_rhs_shift_components_precharged
        || stats.payload_comparison_units_observed > stats.payload_comparison_units_precharged
        || stats.target_sector_rows_observed > stats.target_sector_rows_precharged
        || stats.target_sector_formal_mask_components_observed
            > stats.target_sector_formal_mask_components_precharged
        || stats.target_sector_maximal_mask_components_observed
            > stats.target_sector_maximal_mask_components_precharged
        || stats.target_sector_constant_transition_components_observed
            > stats.target_sector_constant_transition_components_precharged
        || stats.target_sector_activation_obligation_components_observed
            > stats.target_sector_activation_obligation_components_precharged
        || stats.aggregate_descent_components_observed
            > stats.aggregate_descent_components_precharged
        || stats.target_sector_integer_bit_work_observed
            > stats.target_sector_integer_bit_work_precharged
        || stats.target_sector_constant_additions_observed
            > stats.target_sector_constant_additions_precharged
    {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TargetSectorStructuralPrecharge {
    descent_witness_components: usize,
    target_sector_rows: usize,
    formal_mask_components: usize,
    maximal_mask_components: usize,
    constant_transition_components: usize,
    activation_obligation_components: usize,
    aggregate_components: usize,
    constant_additions: usize,
}

fn target_sector_structural_precharge(
    rhs_count: usize,
    ambient_arity: usize,
    constant_positions: usize,
    component_limit: usize,
) -> Result<TargetSectorStructuralPrecharge, GeneratedResidualAffineWhenBadDescentError> {
    let descent_witness_components = checked_mul(
        "generated affine WhenBad descent witness components",
        rhs_count,
        ambient_arity,
    )?;
    check_limit(
        "generated affine WhenBad descent witness components",
        descent_witness_components,
        component_limit,
    )?;
    let target_sector_rows = checked_mul(
        "generated affine WhenBad target-sector rows",
        rhs_count,
        ambient_arity,
    )?;
    let formal_mask_components = checked_mul(
        "generated affine WhenBad target-sector formal mask components",
        rhs_count,
        ambient_arity,
    )?;
    let maximal_mask_components = checked_mul(
        "generated affine WhenBad target-sector maximal mask components",
        rhs_count,
        ambient_arity,
    )?;
    let constant_transition_components = checked_mul(
        "generated affine WhenBad target-sector constant transition components",
        rhs_count,
        ambient_arity,
    )?;
    let activation_obligation_components = checked_mul(
        "generated affine WhenBad target-sector activation obligation components",
        rhs_count,
        ambient_arity,
    )?;
    let aggregate_components = checked_add(
        "generated affine WhenBad aggregate descent components",
        checked_add(
            "generated affine WhenBad aggregate descent components",
            descent_witness_components,
            target_sector_rows,
        )?,
        checked_add(
            "generated affine WhenBad aggregate descent components",
            checked_add(
                "generated affine WhenBad aggregate descent components",
                formal_mask_components,
                maximal_mask_components,
            )?,
            checked_add(
                "generated affine WhenBad aggregate descent components",
                constant_transition_components,
                activation_obligation_components,
            )?,
        )?,
    )?;
    check_limit(
        "generated affine WhenBad aggregate descent components",
        aggregate_components,
        component_limit,
    )?;
    let constant_additions = checked_mul(
        "generated affine WhenBad target-sector constant additions",
        rhs_count,
        constant_positions,
    )?;
    Ok(TargetSectorStructuralPrecharge {
        descent_witness_components,
        target_sector_rows,
        formal_mask_components,
        maximal_mask_components,
        constant_transition_components,
        activation_obligation_components,
        aggregate_components,
        constant_additions,
    })
}

enum TargetSectorDescentAttempt {
    NoConstantSectorTransition,
    Proved {
        witness_ordinal: usize,
        scope: GeneratedResidualAffineTargetSectorDescentScope,
    },
    Unsupported(GeneratedResidualAffineWhenBadDescentUnsupportedReason),
}

fn classify_and_prove_target_sector_descent(
    target_ordering: &crate::AffineStartParametricEliminationOrdering,
    rhs_ordinal: usize,
    shift: &IndexShift,
    transcript: &mut GeneratedResidualAffineTargetSectorDescentTranscript,
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
) -> Result<TargetSectorDescentAttempt, GeneratedResidualAffineWhenBadDescentError> {
    if shift.arity() != target_ordering.arity() {
        return Err(WhenBadCoreError::WrongArity {
            expected: target_ordering.arity(),
            actual: shift.arity(),
        }
        .into());
    }
    observe_private_rhs_shift_components(stats, target_ordering.arity())?;
    let formal_sector_offset = transcript.formal_sector_bits.len();
    let maximal_sector_offset = transcript.maximal_sector_bits.len();
    let transition_offset = transcript.constant_transitions.len();
    let activation_obligation_offset = transcript.symbolic_activation_obligations.len();
    let mut has_universal_active_pinch = false;
    let mut constant_ordinal = 0usize;

    for position in 0..target_ordering.arity() {
        observe_descent_components(stats, DescentComponentClass::TargetSectorRow, 1)?;
        observe_payload_comparison_units(stats, 1)?;
        let source_active = target_ordering.sector().active_bits()[position];
        let displacement = shift.values()[position];
        let is_constant = target_ordering
            .constant_positions()
            .get(constant_ordinal)
            .is_some_and(|&constant_position| constant_position == position);
        if is_constant {
            let classification = target_ordering
                .classify_constant_row_shift_by_ordinal(constant_ordinal, displacement)
                .map_err(GeneratedResidualAffineWhenBadError::from)?;
            observe_constant_addition(stats)?;
            constant_ordinal = checked_add(
                "generated affine WhenBad target-sector constant ordinal",
                constant_ordinal,
                1,
            )?;
            observe_payload_comparison_units(stats, 2)?;
            if classification.position() != position
                || classification.source_active() != source_active
            {
                return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
            }
            stats.target_sector_constant_rows_inspected = checked_add(
                "generated affine WhenBad target-sector constant rows inspected",
                stats.target_sector_constant_rows_inspected,
                1,
            )?;
            stats.target_sector_integer_bit_work_observed = checked_add(
                "generated affine WhenBad target-sector integer-bit work observed",
                stats.target_sector_integer_bit_work_observed,
                classification.integer_bit_work(),
            )?;
            if stats.target_sector_integer_bit_work_observed
                > stats.target_sector_integer_bit_work_precharged
            {
                return Err(GeneratedResidualAffineWhenBadError::ResourceLimit {
                    resource: "generated affine WhenBad target-sector integer-bit work observed",
                    requested: stats.target_sector_integer_bit_work_observed,
                    limit: stats.target_sector_integer_bit_work_precharged,
                }
                .into());
            }
            push_precharged(
                "generated affine WhenBad target-sector formal mask components",
                &mut transcript.formal_sector_bits,
                classification.shifted_active(),
            )?;
            observe_descent_components(stats, DescentComponentClass::FormalSectorMask, 1)?;
            push_precharged(
                "generated affine WhenBad target-sector maximal mask components",
                &mut transcript.maximal_sector_bits,
                classification.shifted_active(),
            )?;
            observe_descent_components(stats, DescentComponentClass::MaximalSectorMask, 1)?;
            observe_payload_comparison_units(stats, 1)?;
            let transition = match classification.transition() {
                AffineConstantRowSectorTransition::StaysInSourceSector => None,
                AffineConstantRowSectorTransition::UniversalActivePinch => {
                    has_universal_active_pinch = true;
                    stats.target_sector_universal_active_pinches = checked_add(
                        "generated affine WhenBad universal active pinches",
                        stats.target_sector_universal_active_pinches,
                        1,
                    )?;
                    Some(GeneratedResidualAffineConstantTransitionKind::UniversalActivePinch)
                }
                AffineConstantRowSectorTransition::UniversalInactiveActivation => {
                    stats.target_sector_universal_inactive_activations = checked_add(
                        "generated affine WhenBad universal inactive activations",
                        stats.target_sector_universal_inactive_activations,
                        1,
                    )?;
                    Some(GeneratedResidualAffineConstantTransitionKind::UniversalInactiveActivation)
                }
            };
            if let Some(kind) = transition {
                push_precharged(
                    "generated affine WhenBad target-sector constant transitions",
                    &mut transcript.constant_transitions,
                    GeneratedResidualAffineConstantTransition { position, kind },
                )?;
                observe_descent_components(stats, DescentComponentClass::ConstantTransition, 1)?;
            }
        } else {
            stats.target_sector_symbolic_rows_inspected = checked_add(
                "generated affine WhenBad target-sector symbolic rows inspected",
                stats.target_sector_symbolic_rows_inspected,
                1,
            )?;
            let possible_inactive_activation = !source_active && displacement > 0;
            push_precharged(
                "generated affine WhenBad target-sector formal mask components",
                &mut transcript.formal_sector_bits,
                source_active,
            )?;
            observe_descent_components(stats, DescentComponentClass::FormalSectorMask, 1)?;
            push_precharged(
                "generated affine WhenBad target-sector maximal mask components",
                &mut transcript.maximal_sector_bits,
                source_active || possible_inactive_activation,
            )?;
            observe_descent_components(stats, DescentComponentClass::MaximalSectorMask, 1)?;
            if possible_inactive_activation {
                observe_payload_comparison_units(stats, 1)?;
                let hazard = finite_boundary_hazard_range(false, displacement, position)?
                    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
                observe_payload_comparison_units(stats, 1)?;
                if hazard.kind() != WhenBadBoundaryHazardKind::InactiveSectorActivation {
                    return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
                }
                push_precharged(
                    "generated affine WhenBad symbolic activation obligations",
                    &mut transcript.symbolic_activation_obligations,
                    GeneratedResidualAffineSymbolicActivationObligation {
                        position,
                        first: hazard.first(),
                        last: hazard.last(),
                        count: hazard.count(),
                    },
                )?;
                observe_descent_components(stats, DescentComponentClass::ActivationObligation, 1)?;
                stats.target_sector_symbolic_activation_obligations = checked_add(
                    "generated affine WhenBad symbolic activation obligations",
                    stats.target_sector_symbolic_activation_obligations,
                    1,
                )?;
            }
        }
    }
    observe_payload_comparison_units(stats, 1)?;
    if constant_ordinal != target_ordering.constant_positions().len() {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }

    let transition_count = transcript
        .constant_transitions
        .len()
        .checked_sub(transition_offset)
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    if transition_count == 0 {
        transcript.formal_sector_bits.truncate(formal_sector_offset);
        transcript
            .maximal_sector_bits
            .truncate(maximal_sector_offset);
        transcript
            .symbolic_activation_obligations
            .truncate(activation_obligation_offset);
        return Ok(TargetSectorDescentAttempt::NoConstantSectorTransition);
    }
    if !has_universal_active_pinch {
        return Ok(TargetSectorDescentAttempt::Unsupported(
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::NoUniversalConstantPinch {
                rhs_ordinal,
            },
        ));
    }

    let formal_sector_bits = checked_private_slice(
        &transcript.formal_sector_bits,
        formal_sector_offset,
        target_ordering.arity(),
    )
    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    let formal_census = target_ordering
        .prove_strict_sector_prefix_descent_bits_with_census(formal_sector_bits)
        .map_err(GeneratedResidualAffineWhenBadError::from)?;
    observe_payload_comparison_units(stats, formal_census.comparison_units())?;
    let Some(formal_prefix) = formal_census.witness() else {
        return Ok(TargetSectorDescentAttempt::Unsupported(
            GeneratedResidualAffineWhenBadDescentUnsupportedReason::
                NonDescendingTargetSectorPrefix { rhs_ordinal },
        ));
    };
    let maximal_sector_bits = checked_private_slice(
        &transcript.maximal_sector_bits,
        maximal_sector_offset,
        target_ordering.arity(),
    )
    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    let maximal_census = target_ordering
        .prove_strict_sector_prefix_descent_bits_with_census(maximal_sector_bits)
        .map_err(GeneratedResidualAffineWhenBadError::from)?;
    observe_payload_comparison_units(stats, maximal_census.comparison_units())?;
    let maximal_prefix = maximal_census.witness();
    let activation_obligation_count = transcript
        .symbolic_activation_obligations
        .len()
        .checked_sub(activation_obligation_offset)
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    // V1 compares propagator count and then the exact bit word.  Both fields
    // are monotone under clearing active bits.  `maximal_sector_bits` keeps
    // every symbolic source-active row active and activates every symbolic
    // inactive row which can cross, so every realized sector is a bitwise
    // subset.  If that maximum is not lower, `formal_sector_bits` is still a
    // valid upper sector only after all private activation ranges are sealed.
    let (scope, sector_prefix) = if let Some(maximal_prefix) = maximal_prefix {
        (
            GeneratedResidualAffineTargetSectorDescentScope::WholeTarget,
            maximal_prefix,
        )
    } else if activation_obligation_count > 0 {
        (
            GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain,
            formal_prefix,
        )
    } else {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    };
    let replay_sector_bits = match scope {
        GeneratedResidualAffineTargetSectorDescentScope::WholeTarget => maximal_sector_bits,
        GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain => {
            formal_sector_bits
        }
    };
    let (prefix_replayed, prefix_replay_units) = sector_prefix
        .replay_with_census(target_ordering, replay_sector_bits)
        .map_err(GeneratedResidualAffineWhenBadError::from)?;
    observe_payload_comparison_units(stats, prefix_replay_units)?;
    if !prefix_replayed {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    let witness_ordinal = transcript.witnesses.len();
    push_precharged(
        "generated affine WhenBad target-sector witnesses",
        &mut transcript.witnesses,
        GeneratedResidualAffineTargetSectorDescentWitness {
            rhs_ordinal,
            formal_sector_offset,
            maximal_sector_offset,
            sector_arity: target_ordering.arity(),
            transition_offset,
            transition_count,
            activation_obligation_offset,
            activation_obligation_count,
            scope,
            sector_prefix,
        },
    )?;
    Ok(TargetSectorDescentAttempt::Proved {
        witness_ordinal,
        scope,
    })
}

fn check_same_sector_witness_heap(
    witness: &WhenBadUniformDescentWitness,
    ambient_arity: usize,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    let witness_owned = witness.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad descent retained bytes",
        },
    )?;
    let witness_heap = witness_owned
        .checked_sub(size_of::<WhenBadUniformDescentWitness>())
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad descent retained bytes",
        })?;
    let admitted_witness_heap = checked_mul(
        "generated affine WhenBad descent retained bytes",
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            ambient_arity,
            2,
        )?,
        checked_add(
            "generated affine WhenBad descent retained bytes",
            size_of::<i64>(),
            size_of::<i128>(),
        )?,
    )?;
    if witness_heap > admitted_witness_heap {
        return Err(
            GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                observed: witness_heap,
                admitted: admitted_witness_heap,
            }
            .into(),
        );
    }
    Ok(())
}

fn validate_authoritative_rhs_proofs(
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    proofs: &[GeneratedResidualAffineWhenBadRhsDescentProof],
    same_sector: &[WhenBadUniformDescentWitness],
    target_sector: &GeneratedResidualAffineTargetSectorDescentTranscript,
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    let ordering = input.target_ordering();
    let mut next_same_sector = 0usize;
    let mut next_target_sector = 0usize;
    let mut next_formal_sector_offset = 0usize;
    let mut next_maximal_sector_offset = 0usize;
    let mut next_transition_offset = 0usize;
    let mut next_activation_obligation_offset = 0usize;
    let mut rhs_ordinal = 0usize;
    for (shift, _coefficient) in input.relation().terms() {
        if !charge_complete_rhs_shift_scan(shift, stats)? {
            continue;
        }
        observe_payload_comparison_units(stats, 2)?;
        let proof = proofs
            .get(rhs_ordinal)
            .copied()
            .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
        match proof {
            GeneratedResidualAffineWhenBadRhsDescentProof::SameSector { witness_ordinal } => {
                observe_payload_comparison_units(stats, 2)?;
                if witness_ordinal != next_same_sector {
                    return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
                }
                let witness = same_sector
                    .get(witness_ordinal)
                    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
                replay_same_sector_witness(ordering, rhs_ordinal, shift, witness, stats)?;
                next_same_sector = checked_add(
                    "generated affine WhenBad authoritative same-sector proofs",
                    next_same_sector,
                    1,
                )?;
            }
            GeneratedResidualAffineWhenBadRhsDescentProof::TargetSector { witness_ordinal } => {
                observe_payload_comparison_units(stats, 2)?;
                if witness_ordinal != next_target_sector {
                    return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
                }
                let witness = target_sector
                    .witnesses
                    .get(witness_ordinal)
                    .copied()
                    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
                replay_target_sector_witness(
                    ordering,
                    rhs_ordinal,
                    shift,
                    witness,
                    target_sector,
                    &mut next_formal_sector_offset,
                    &mut next_maximal_sector_offset,
                    &mut next_transition_offset,
                    &mut next_activation_obligation_offset,
                    stats,
                )?;
                next_target_sector = checked_add(
                    "generated affine WhenBad authoritative target-sector proofs",
                    next_target_sector,
                    1,
                )?;
            }
        }
        rhs_ordinal = checked_add(
            "generated affine WhenBad authoritative RHS proofs",
            rhs_ordinal,
            1,
        )?;
    }
    observe_payload_comparison_units(stats, 12)?;
    let expected_same_sector_components = checked_mul(
        "generated affine WhenBad replay same-sector components",
        rhs_ordinal,
        ordering.arity(),
    )?;
    let expected_target_sector_rows = checked_mul(
        "generated affine WhenBad replay target-sector rows",
        next_target_sector,
        ordering.arity(),
    )?;
    let expected_constant_additions = checked_mul(
        "generated affine WhenBad replay constant additions",
        next_target_sector,
        ordering.constant_positions().len(),
    )?;
    let target_scope_count = checked_add(
        "generated affine WhenBad replay target-sector scopes",
        stats.target_sector_whole_target_proved,
        stats.target_sector_applicable_nonzero_term_domain_proved,
    )?;
    if rhs_ordinal != proofs.len()
        || rhs_ordinal != input.binding().rhs_terms()
        || next_same_sector != same_sector.len()
        || next_target_sector != target_sector.witnesses.len()
        || stats.descent_witnesses_attempted != rhs_ordinal
        || stats.descent_witnesses_proved != rhs_ordinal
        || stats.descent_witness_components_observed != expected_same_sector_components
        || stats.target_sector_fallbacks_attempted != next_target_sector
        || stats.target_sector_fallbacks_proved != next_target_sector
        || target_scope_count != next_target_sector
        || stats.target_sector_rows_observed != expected_target_sector_rows
        || stats.target_sector_constant_additions_observed != expected_constant_additions
        || next_formal_sector_offset != target_sector.formal_sector_bits.len()
        || next_maximal_sector_offset != target_sector.maximal_sector_bits.len()
        || next_transition_offset != target_sector.constant_transitions.len()
        || next_activation_obligation_offset != target_sector.symbolic_activation_obligations.len()
    {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    Ok(())
}

fn replay_same_sector_witness(
    ordering: &crate::AffineStartParametricEliminationOrdering,
    rhs_ordinal: usize,
    shift: &IndexShift,
    witness: &WhenBadUniformDescentWitness,
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    observe_payload_comparison_units(stats, 8)?;
    if shift.arity() != ordering.arity()
        || witness.rhs_ordinal() != rhs_ordinal
        || witness.rhs_shift().arity() != shift.arity()
        || witness.index_excess_deltas().len() != shift.arity()
    {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    observe_private_rhs_shift_components(stats, ordering.arity())?;
    let mut corner_delta = 0i128;
    let mut dot_delta = 0i128;
    let mut numerator_delta = 0i128;
    let mut first_index_excess = None;
    for (position, (&active, &delta)) in ordering
        .sector()
        .active_bits()
        .iter()
        .zip(shift.values())
        .enumerate()
    {
        observe_payload_comparison_units(stats, 3)?;
        if witness.rhs_shift().values().get(position).copied() != Some(delta) {
            return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
        }
        let delta = i128::from(delta);
        let excess_delta = if active { delta } else { -delta };
        if witness.index_excess_deltas().get(position).copied() != Some(excess_delta) {
            return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
        }
        corner_delta = corner_delta
            .checked_add(excess_delta)
            .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        if active {
            dot_delta = dot_delta
                .checked_add(delta)
                .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        } else {
            numerator_delta = numerator_delta
                .checked_sub(delta)
                .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        }
        if first_index_excess.is_none() && excess_delta != 0 {
            first_index_excess = Some((position, excess_delta));
        }
    }
    let decisive = if corner_delta != 0 {
        Some((WhenBadDescentComponent::CornerDistance, corner_delta))
    } else if dot_delta != 0 {
        Some((WhenBadDescentComponent::DotPower, dot_delta))
    } else if numerator_delta != 0 {
        Some((WhenBadDescentComponent::NumeratorPower, numerator_delta))
    } else {
        first_index_excess
            .map(|(position, delta)| (WhenBadDescentComponent::IndexExcess { position }, delta))
    };
    observe_payload_comparison_units(stats, 6)?;
    let Some((decisive_component, decisive_delta)) = decisive else {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    };
    if decisive_delta >= 0
        || witness.corner_delta() != corner_delta
        || witness.dot_delta() != dot_delta
        || witness.numerator_delta() != numerator_delta
        || witness.decisive_component() != decisive_component
    {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_target_sector_witness(
    ordering: &crate::AffineStartParametricEliminationOrdering,
    rhs_ordinal: usize,
    shift: &IndexShift,
    witness: GeneratedResidualAffineTargetSectorDescentWitness,
    transcript: &GeneratedResidualAffineTargetSectorDescentTranscript,
    next_formal_sector_offset: &mut usize,
    next_maximal_sector_offset: &mut usize,
    next_transition_offset: &mut usize,
    next_activation_obligation_offset: &mut usize,
    stats: &mut GeneratedResidualAffineWhenBadDescentStats,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    observe_payload_comparison_units(stats, 12)?;
    if shift.arity() != ordering.arity()
        || witness.rhs_ordinal != rhs_ordinal
        || witness.sector_arity != ordering.arity()
        || witness.formal_sector_offset != *next_formal_sector_offset
        || witness.maximal_sector_offset != *next_maximal_sector_offset
        || witness.transition_offset != *next_transition_offset
        || witness.activation_obligation_offset != *next_activation_obligation_offset
    {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    observe_private_rhs_shift_components(stats, ordering.arity())?;
    let formal_bits = transcript
        .formal_sector_bits(witness)
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    let maximal_bits = transcript
        .maximal_sector_bits(witness)
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    let transitions = transcript
        .constant_transitions(witness)
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    let obligations = transcript
        .symbolic_activation_obligations(witness)
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    let mut constant_ordinal = 0usize;
    let mut transition_ordinal = 0usize;
    let mut obligation_ordinal = 0usize;
    let mut has_universal_active_pinch = false;
    let mut corner_delta = 0i128;
    let mut dot_delta = 0i128;
    let mut numerator_delta = 0i128;
    let mut first_index_excess_delta = None;
    for position in 0..ordering.arity() {
        observe_payload_comparison_units(stats, 4)?;
        let source_active = ordering.sector().active_bits()[position];
        let displacement = shift.values()[position];
        let signed_displacement = i128::from(displacement);
        let index_excess_delta = if source_active {
            signed_displacement
        } else {
            -signed_displacement
        };
        corner_delta = corner_delta
            .checked_add(index_excess_delta)
            .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        if source_active {
            dot_delta = dot_delta
                .checked_add(signed_displacement)
                .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        } else {
            numerator_delta = numerator_delta
                .checked_sub(signed_displacement)
                .ok_or(WhenBadCoreError::DescentArithmeticOverflow)?;
        }
        if first_index_excess_delta.is_none() && index_excess_delta != 0 {
            first_index_excess_delta = Some(index_excess_delta);
        }
        let is_constant = ordering
            .constant_positions()
            .get(constant_ordinal)
            .is_some_and(|&constant_position| constant_position == position);
        if is_constant {
            let classification = ordering
                .replay_classify_constant_row_shift_by_ordinal(constant_ordinal, displacement)
                .map_err(GeneratedResidualAffineWhenBadError::from)?;
            stats.target_sector_integer_bit_work_observed = checked_add(
                "generated affine WhenBad target-sector integer-bit work observed",
                stats.target_sector_integer_bit_work_observed,
                classification.integer_bit_work(),
            )?;
            check_limit(
                "generated affine WhenBad target-sector integer-bit work observed",
                stats.target_sector_integer_bit_work_observed,
                stats.target_sector_integer_bit_work_precharged,
            )?;
            constant_ordinal = checked_add(
                "generated affine WhenBad replay constant ordinal",
                constant_ordinal,
                1,
            )?;
            observe_payload_comparison_units(stats, 5)?;
            if classification.position() != position
                || classification.source_active() != source_active
                || formal_bits.get(position).copied() != Some(classification.shifted_active())
                || maximal_bits.get(position).copied() != Some(classification.shifted_active())
            {
                return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
            }
            let expected_transition = match classification.transition() {
                AffineConstantRowSectorTransition::StaysInSourceSector => None,
                AffineConstantRowSectorTransition::UniversalActivePinch => {
                    has_universal_active_pinch = true;
                    Some(GeneratedResidualAffineConstantTransitionKind::UniversalActivePinch)
                }
                AffineConstantRowSectorTransition::UniversalInactiveActivation => {
                    Some(GeneratedResidualAffineConstantTransitionKind::UniversalInactiveActivation)
                }
            };
            if let Some(expected_kind) = expected_transition {
                let actual = transitions
                    .get(transition_ordinal)
                    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
                observe_payload_comparison_units(stats, 2)?;
                if actual.position != position || actual.kind != expected_kind {
                    return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
                }
                transition_ordinal = checked_add(
                    "generated affine WhenBad replay transition ordinal",
                    transition_ordinal,
                    1,
                )?;
            }
        } else {
            let possible_inactive_activation = !source_active && displacement > 0;
            observe_payload_comparison_units(stats, 2)?;
            if formal_bits.get(position).copied() != Some(source_active)
                || maximal_bits.get(position).copied()
                    != Some(source_active || possible_inactive_activation)
            {
                return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
            }
            if possible_inactive_activation {
                let hazard = finite_boundary_hazard_range(false, displacement, position)?
                    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
                let actual = obligations
                    .get(obligation_ordinal)
                    .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
                observe_payload_comparison_units(stats, 5)?;
                if hazard.kind() != WhenBadBoundaryHazardKind::InactiveSectorActivation
                    || actual.position != position
                    || actual.first != hazard.first()
                    || actual.last != hazard.last()
                    || actual.count != hazard.count()
                {
                    return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
                }
                obligation_ordinal = checked_add(
                    "generated affine WhenBad replay activation-obligation ordinal",
                    obligation_ordinal,
                    1,
                )?;
            }
        }
    }
    let same_sector_decisive_delta = [corner_delta, dot_delta, numerator_delta]
        .into_iter()
        .find(|delta| *delta != 0)
        .or(first_index_excess_delta);
    observe_payload_comparison_units(stats, 10)?;
    if constant_ordinal != ordering.constant_positions().len()
        || transition_ordinal != transitions.len()
        || obligation_ordinal != obligations.len()
        || !has_universal_active_pinch
        || same_sector_decisive_delta.is_some_and(|delta| delta < 0)
    {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    let formal_census = ordering
        .prove_strict_sector_prefix_descent_bits_with_census(formal_bits)
        .map_err(GeneratedResidualAffineWhenBadError::from)?;
    observe_payload_comparison_units(stats, formal_census.comparison_units())?;
    let formal_prefix = formal_census
        .witness()
        .ok_or(GeneratedResidualAffineWhenBadError::ReplayMismatch)?;
    let maximal_census = ordering
        .prove_strict_sector_prefix_descent_bits_with_census(maximal_bits)
        .map_err(GeneratedResidualAffineWhenBadError::from)?;
    observe_payload_comparison_units(stats, maximal_census.comparison_units())?;
    let expected = if let Some(maximal_prefix) = maximal_census.witness() {
        (
            GeneratedResidualAffineTargetSectorDescentScope::WholeTarget,
            maximal_prefix,
            maximal_bits,
        )
    } else if !obligations.is_empty() {
        (
            GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain,
            formal_prefix,
            formal_bits,
        )
    } else {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    };
    observe_payload_comparison_units(stats, 3)?;
    if witness.scope != expected.0 || witness.sector_prefix != expected.1 {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    let (prefix_replayed, prefix_replay_units) = witness
        .sector_prefix
        .replay_with_census(ordering, expected.2)
        .map_err(GeneratedResidualAffineWhenBadError::from)?;
    observe_payload_comparison_units(stats, prefix_replay_units)?;
    if !prefix_replayed {
        return Err(GeneratedResidualAffineWhenBadError::ReplayMismatch.into());
    }
    *next_formal_sector_offset = checked_add(
        "generated affine WhenBad replay formal-sector offset",
        *next_formal_sector_offset,
        ordering.arity(),
    )?;
    *next_maximal_sector_offset = checked_add(
        "generated affine WhenBad replay maximal-sector offset",
        *next_maximal_sector_offset,
        ordering.arity(),
    )?;
    *next_transition_offset = checked_add(
        "generated affine WhenBad replay transition offset",
        *next_transition_offset,
        transitions.len(),
    )?;
    *next_activation_obligation_offset = checked_add(
        "generated affine WhenBad replay activation-obligation offset",
        *next_activation_obligation_offset,
        obligations.len(),
    )?;
    Ok(())
}

fn checked_private_slice<T>(values: &[T], offset: usize, count: usize) -> Option<&[T]> {
    values.get(offset..offset.checked_add(count)?)
}

fn try_reserve_with_capacity_envelope<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    requested: usize,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    let admitted_capacity = checked_mul(resource, requested, 2)?;
    target.try_reserve_exact(requested).map_err(|_| {
        GeneratedResidualAffineWhenBadError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    if target.capacity() > admitted_capacity {
        return Err(
            GeneratedResidualAffineWhenBadError::RetainedByteEnvelopeExceeded {
                observed: checked_mul(resource, target.capacity(), size_of::<T>())?,
                admitted: checked_mul(resource, admitted_capacity, size_of::<T>())?,
            }
            .into(),
        );
    }
    Ok(())
}

fn push_precharged<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    value: T,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    if target.len() == target.capacity() {
        return Err(GeneratedResidualAffineWhenBadError::ResourceLimit {
            resource,
            requested: target.len().saturating_add(1),
            limit: target.capacity(),
        }
        .into());
    }
    target.push(value);
    Ok(())
}

fn descent_retained_byte_envelope(
    authenticated_input_envelope: usize,
    rhs_count: usize,
    witness_components: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadDescentError> {
    let input_inline = size_of::<AuthenticatedGeneratedResidualAffineWhenBadInput>();
    let output_inline = size_of::<GeneratedResidualAffineWhenBadDescentReady>()
        .max(size_of::<GeneratedResidualAffineWhenBadDescentUnsupported>());
    let extra_inline = output_inline.checked_sub(input_inline).ok_or(
        GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad descent retained bytes",
        },
    )?;
    let outer_witness_bytes = checked_mul(
        "generated affine WhenBad descent retained bytes",
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            rhs_count,
            2,
        )?,
        size_of::<WhenBadUniformDescentWitness>(),
    )?;
    let authoritative_proof_bytes = checked_mul(
        "generated affine WhenBad descent retained bytes",
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            rhs_count,
            2,
        )?,
        size_of::<GeneratedResidualAffineWhenBadRhsDescentProof>(),
    )?;
    let target_witness_bytes = checked_mul(
        "generated affine WhenBad descent retained bytes",
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            rhs_count,
            2,
        )?,
        size_of::<GeneratedResidualAffineTargetSectorDescentWitness>(),
    )?;
    let per_component_bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        size_of::<i64>(),
        size_of::<i128>(),
    )?;
    let private_component_bytes = checked_mul(
        "generated affine WhenBad descent retained bytes",
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            witness_components,
            2,
        )?,
        per_component_bytes,
    )?;
    let target_component_unit_bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            size_of::<bool>(),
            2,
        )?,
        checked_add(
            "generated affine WhenBad descent retained bytes",
            size_of::<GeneratedResidualAffineConstantTransition>(),
            size_of::<GeneratedResidualAffineSymbolicActivationObligation>(),
        )?,
    )?;
    let target_component_bytes = checked_mul(
        "generated affine WhenBad descent retained bytes",
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            witness_components,
            2,
        )?,
        target_component_unit_bytes,
    )?;
    checked_add(
        "generated affine WhenBad descent retained bytes",
        checked_add(
            "generated affine WhenBad descent retained bytes",
            authenticated_input_envelope,
            extra_inline,
        )?,
        checked_add(
            "generated affine WhenBad descent retained bytes",
            checked_add(
                "generated affine WhenBad descent retained bytes",
                checked_add(
                    "generated affine WhenBad descent retained bytes",
                    outer_witness_bytes,
                    authoritative_proof_bytes,
                )?,
                target_witness_bytes,
            )?,
            checked_add(
                "generated affine WhenBad descent retained bytes",
                private_component_bytes,
                target_component_bytes,
            )?,
        )?,
    )
}

fn ready_observed_retained_bytes(
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
    witnesses: &[WhenBadUniformDescentWitness],
    witness_capacity: usize,
    private_rhs_proof_capacity: usize,
    target_sector: &GeneratedResidualAffineTargetSectorDescentTranscript,
) -> Result<usize, GeneratedResidualAffineWhenBadDescentError> {
    let input_inline = size_of::<AuthenticatedGeneratedResidualAffineWhenBadInput>();
    let extra_inline = size_of::<GeneratedResidualAffineWhenBadDescentReady>()
        .checked_sub(input_inline)
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad descent retained bytes",
        })?;
    let mut bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        input.stats().retained_bytes(),
        extra_inline,
    )?;
    bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            witness_capacity,
            size_of::<WhenBadUniformDescentWitness>(),
        )?,
    )?;
    for witness in witnesses {
        let owned = witness.owned_retained_byte_bound().ok_or(
            GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                resource: "generated affine WhenBad descent retained bytes",
            },
        )?;
        bytes = checked_add(
            "generated affine WhenBad descent retained bytes",
            bytes,
            owned
                .checked_sub(size_of::<WhenBadUniformDescentWitness>())
                .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
                    resource: "generated affine WhenBad descent retained bytes",
                })?,
        )?;
    }
    bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            private_rhs_proof_capacity,
            size_of::<GeneratedResidualAffineWhenBadRhsDescentProof>(),
        )?,
    )?;
    bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            target_sector.witnesses.capacity(),
            size_of::<GeneratedResidualAffineTargetSectorDescentWitness>(),
        )?,
    )?;
    bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            target_sector.formal_sector_bits.capacity(),
            size_of::<bool>(),
        )?,
    )?;
    bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            target_sector.maximal_sector_bits.capacity(),
            size_of::<bool>(),
        )?,
    )?;
    bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            target_sector.constant_transitions.capacity(),
            size_of::<GeneratedResidualAffineConstantTransition>(),
        )?,
    )?;
    bytes = checked_add(
        "generated affine WhenBad descent retained bytes",
        bytes,
        checked_mul(
            "generated affine WhenBad descent retained bytes",
            target_sector.symbolic_activation_obligations.capacity(),
            size_of::<GeneratedResidualAffineSymbolicActivationObligation>(),
        )?,
    )?;
    Ok(bytes)
}

fn unsupported_observed_retained_bytes(
    input: &AuthenticatedGeneratedResidualAffineWhenBadInput,
) -> Result<usize, GeneratedResidualAffineWhenBadDescentError> {
    let extra_inline = size_of::<GeneratedResidualAffineWhenBadDescentUnsupported>()
        .checked_sub(size_of::<AuthenticatedGeneratedResidualAffineWhenBadInput>())
        .ok_or(GeneratedResidualAffineWhenBadError::ResourceCountOverflow {
            resource: "generated affine WhenBad descent retained bytes",
        })?;
    checked_add(
        "generated affine WhenBad descent retained bytes",
        input.stats().retained_bytes(),
        extra_inline,
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadDescentError> {
    left.checked_add(right).ok_or_else(|| {
        GeneratedResidualAffineWhenBadError::ResourceCountOverflow { resource }.into()
    })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadDescentError> {
    left.checked_mul(right).ok_or_else(|| {
        GeneratedResidualAffineWhenBadError::ResourceCountOverflow { resource }.into()
    })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineWhenBadDescentError> {
    if requested > limit {
        Err(GeneratedResidualAffineWhenBadError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        .into())
    } else {
        Ok(())
    }
}

fn write_when_bad_core_error(
    formatter: &mut fmt::Formatter<'_>,
    error: &WhenBadCoreError,
) -> fmt::Result {
    match error {
        WhenBadCoreError::WrongArity { expected, actual } => write!(
            formatter,
            "generated affine WhenBad descent expected arity {expected}, got {actual}"
        ),
        WhenBadCoreError::BoundaryArithmeticOverflow { coordinate } => write!(
            formatter,
            "generated affine WhenBad descent boundary arithmetic overflow at coordinate {coordinate}"
        ),
        WhenBadCoreError::DescentArithmeticOverflow => {
            formatter.write_str("generated affine WhenBad descent arithmetic overflow")
        }
        WhenBadCoreError::RetainedCapacityEnvelopeExceeded {
            resource,
            observed_bytes,
            admitted_bytes,
        } => write!(
            formatter,
            "{resource} retained {observed_bytes} bytes after admitting {admitted_bytes}"
        ),
        WhenBadCoreError::ResourceCountOverflow { resource } => {
            write!(formatter, "{resource} count overflowed usize")
        }
        WhenBadCoreError::AllocationFailure {
            resource,
            requested,
        } => write!(
            formatter,
            "{resource} allocation of {requested} entries failed after bounded preflight"
        ),
        WhenBadCoreError::ParametricRelation(error) => write!(formatter, "{error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::{
        AffineParametricOrderingLimits, AffineStartParametricEliminationOrdering,
        CoefficientContext, CoordinateEqualityLocusExtractor, CoordinateEqualityLocusLimits,
        IndexShift, IntegralOrderingPolicy, ParametricCoefficientContext,
        ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapLimits, SectorMask,
        SymbolicPolynomialPredicateKind, SymbolicSectorCaseLimits,
        SymbolicSectorCasePartitionBuilder,
    };

    fn ordering_from_residual_unit_map(
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        source: Arc<crate::CoordinateEqualityLocusCertificate>,
        bound_position: usize,
    ) -> AffineStartParametricEliminationOrdering {
        let predicate_ordinal = source
            .unresolved_predicates()
            .iter()
            .find(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
            .expect("fixture must retain its nonliteral affine equality")
            .predicate_ordinal();
        let map = Arc::new(
            ResidualUnitAffineIndexMapCertificate::compile(
                context,
                source,
                predicate_ordinal,
                bound_position,
                ResidualUnitAffineIndexMapLimits::default(),
            )
            .unwrap(),
        );
        AffineStartParametricEliminationOrdering::try_new(
            context,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            sector,
            map,
            AffineParametricOrderingLimits::default(),
        )
        .unwrap()
    }

    /// Three-row affine start with one free row, one selected constant row,
    /// and one literal constant row. The nonliteral equality
    /// `n_bound+n_2=constant+literal` keeps the selected row as the map pivot.
    fn one_free_row_ordering(
        scope: &str,
        bound_position: usize,
        bound_constant: i64,
        free_active: bool,
        literal_constant: i64,
    ) -> AffineStartParametricEliminationOrdering {
        assert!(bound_position < 2);
        let free_position = 1 - bound_position;
        let context =
            ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, 3)
                .unwrap();
        let mut bits = [false; 3];
        bits[bound_position] = bound_constant >= 1;
        bits[free_position] = free_active;
        bits[2] = literal_constant >= 1;
        let sector = SectorMask::try_new(bits).unwrap();
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            &context,
            sector.clone(),
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap();
        let literal = context
            .sub(
                &context.index(2).unwrap(),
                &context.integer(literal_constant),
            )
            .unwrap();
        let root = builder.root_case();
        let mut leaf = builder
            .split_on_bad_polynomial(
                &context,
                root,
                context.numerator_condition(&literal).unwrap(),
            )
            .unwrap()
            .equal_zero_case();
        let affine = context
            .sub(
                &context
                    .add(
                        &context.index(bound_position).unwrap(),
                        &context.index(2).unwrap(),
                    )
                    .unwrap(),
                &context.integer(bound_constant + literal_constant),
            )
            .unwrap();
        leaf = builder
            .split_on_bad_polynomial(
                &context,
                leaf,
                context.numerator_condition(&affine).unwrap(),
            )
            .unwrap()
            .equal_zero_case();
        let partition = builder.finish(&context).unwrap();
        let source = Arc::new(
            CoordinateEqualityLocusExtractor::extract(
                &context,
                &partition,
                leaf,
                CoordinateEqualityLocusLimits::default(),
            )
            .unwrap(),
        );
        let ordering = ordering_from_residual_unit_map(&context, sector, source, bound_position);
        assert_eq!(ordering.constant_positions(), &[bound_position, 2]);
        assert_eq!(ordering.symbolic_positions(), &[free_position]);
        ordering
    }

    /// Fully constant three-row start. Two coordinate literals are folded
    /// into the one nonliteral sum equality used as the selected affine row.
    fn fully_constant_ordering(
        scope: &str,
        constants: [i64; 3],
        bound_position: usize,
    ) -> AffineStartParametricEliminationOrdering {
        let context =
            ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, 3)
                .unwrap();
        let sector = SectorMask::try_new(constants.map(|value| value >= 1)).unwrap();
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            &context,
            sector.clone(),
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap();
        let mut leaf = builder.root_case();
        for position in 0..3 {
            if position == bound_position {
                continue;
            }
            let literal = context
                .sub(
                    &context.index(position).unwrap(),
                    &context.integer(constants[position]),
                )
                .unwrap();
            leaf = builder
                .split_on_bad_polynomial(
                    &context,
                    leaf,
                    context.numerator_condition(&literal).unwrap(),
                )
                .unwrap()
                .equal_zero_case();
        }
        let sum = context
            .add(
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.index(2).unwrap(),
            )
            .unwrap();
        let affine = context
            .sub(&sum, &context.integer(constants.into_iter().sum::<i64>()))
            .unwrap();
        leaf = builder
            .split_on_bad_polynomial(
                &context,
                leaf,
                context.numerator_condition(&affine).unwrap(),
            )
            .unwrap()
            .equal_zero_case();
        let partition = builder.finish(&context).unwrap();
        let source = Arc::new(
            CoordinateEqualityLocusExtractor::extract(
                &context,
                &partition,
                leaf,
                CoordinateEqualityLocusLimits::default(),
            )
            .unwrap(),
        );
        let ordering = ordering_from_residual_unit_map(&context, sector, source, bound_position);
        assert_eq!(ordering.constant_positions(), &[0, 1, 2]);
        assert!(ordering.symbolic_positions().is_empty());
        ordering
    }

    fn target_attempt(
        ordering: &AffineStartParametricEliminationOrdering,
        shift: &IndexShift,
    ) -> (
        TargetSectorDescentAttempt,
        GeneratedResidualAffineTargetSectorDescentTranscript,
        GeneratedResidualAffineWhenBadDescentStats,
    ) {
        let arity = ordering.arity();
        let structural = target_sector_structural_precharge(
            1,
            arity,
            ordering.constant_positions().len(),
            usize::MAX,
        )
        .unwrap();
        let integer_bit_work = ordering
            .constant_positions()
            .iter()
            .enumerate()
            .map(|(ordinal, &position)| {
                ordering
                    .constant_row_shift_integer_bit_work_bound_by_ordinal(
                        ordinal,
                        shift.values()[position],
                    )
                    .unwrap()
                    .1
            })
            .sum();
        let mut stats = GeneratedResidualAffineWhenBadDescentStats {
            rhs_terms: 1,
            descent_witnesses_precharged: 1,
            descent_witness_components: structural.descent_witness_components,
            private_rhs_shift_components_precharged: arity,
            payload_comparison_units_precharged: payload_comparison_work_precharge(
                1,
                arity,
                ordering.constant_positions().len(),
            )
            .unwrap(),
            target_sector_rows_precharged: structural.target_sector_rows,
            target_sector_formal_mask_components_precharged: structural.formal_mask_components,
            target_sector_maximal_mask_components_precharged: structural.maximal_mask_components,
            target_sector_constant_transition_components_precharged: structural
                .constant_transition_components,
            target_sector_activation_obligation_components_precharged: structural
                .activation_obligation_components,
            aggregate_descent_components_precharged: structural.aggregate_components,
            target_sector_constant_additions_precharged: structural.constant_additions,
            target_sector_integer_bit_work_precharged: integer_bit_work,
            ..GeneratedResidualAffineWhenBadDescentStats::default()
        };
        let mut transcript =
            GeneratedResidualAffineTargetSectorDescentTranscript::try_with_precharged_capacities(
                1,
                structural.descent_witness_components,
            )
            .unwrap();
        let attempt = classify_and_prove_target_sector_descent(
            ordering,
            0,
            shift,
            &mut transcript,
            &mut stats,
        )
        .unwrap();
        validate_final_work_census(&stats).unwrap();
        (attempt, transcript, stats)
    }

    #[test]
    fn target_local_unsupported_debug_redacts_exact_rhs_shift() {
        let sector = SectorMask::try_new([true]).unwrap();
        let private_shift = IndexShift::try_new([991], 1).unwrap();
        let detailed = prove_uniform_same_sector_descent(&sector, 7, &private_shift)
            .unwrap()
            .unwrap_err();
        let reason = GeneratedResidualAffineWhenBadDescentUnsupportedReason::from(detailed);
        assert_eq!(reason.rhs_ordinal(), Some(7));
        let debug = format!("{reason:?}");
        assert!(!debug.contains("991"));
        assert!(debug.contains("NonUniformSameSectorDescent"));
    }

    #[test]
    fn target_sector_structural_precharge_has_exact_and_one_below_boundaries() {
        let exact = target_sector_structural_precharge(3, 4, 2, 72).unwrap();
        assert_eq!(exact.descent_witness_components, 12);
        assert_eq!(exact.target_sector_rows, 12);
        assert_eq!(exact.formal_mask_components, 12);
        assert_eq!(exact.maximal_mask_components, 12);
        assert_eq!(exact.constant_transition_components, 12);
        assert_eq!(exact.activation_obligation_components, 12);
        assert_eq!(exact.aggregate_components, 72);
        assert_eq!(exact.constant_additions, 6);

        let error = target_sector_structural_precharge(3, 4, 2, 71).unwrap_err();
        assert!(matches!(
            error,
            GeneratedResidualAffineWhenBadDescentError::Authority(
                GeneratedResidualAffineWhenBadError::ResourceLimit {
                    resource: "generated affine WhenBad aggregate descent components",
                    requested: 72,
                    limit: 71,
                }
            )
        ));
    }

    #[test]
    fn target_sector_whole_target_keeps_simultaneous_pinch_and_activation() {
        let ordering = one_free_row_ordering("target-sector-whole-target", 0, 1, false, 0);
        let shift = IndexShift::try_new([-1, 1, -3], 3).unwrap();
        assert!(
            prove_uniform_same_sector_descent(ordering.sector(), 0, &shift)
                .unwrap()
                .is_err()
        );

        let (attempt, transcript, stats) = target_attempt(&ordering, &shift);
        let TargetSectorDescentAttempt::Proved {
            witness_ordinal,
            scope,
        } = attempt
        else {
            panic!("simultaneous target-sector transition was not proved")
        };
        assert_eq!(witness_ordinal, 0);
        assert_eq!(
            scope,
            GeneratedResidualAffineTargetSectorDescentScope::WholeTarget
        );
        let witness = transcript.witnesses()[0];
        assert_eq!(witness.rhs_ordinal(), 0);
        assert_eq!(witness.scope(), scope);
        assert_eq!(
            witness.sector_prefix().decisive_component(),
            crate::affine_parametric_ordering::AffineSectorPrefixDescentComponent::SectorBits
        );
        assert_eq!(
            transcript.formal_sector_bits(witness).unwrap(),
            &[false, false, false]
        );
        assert_eq!(
            transcript.maximal_sector_bits(witness).unwrap(),
            &[false, true, false]
        );
        let transitions = transcript.constant_transitions(witness).unwrap();
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].position(), 0);
        assert_eq!(
            transitions[0].kind(),
            GeneratedResidualAffineConstantTransitionKind::UniversalActivePinch
        );
        let obligations = transcript.symbolic_activation_obligations(witness).unwrap();
        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].position(), 1);
        assert_eq!(
            (
                obligations[0].first(),
                obligations[0].last(),
                obligations[0].count()
            ),
            (0, 0, 1)
        );
        assert_eq!(stats.target_sector_rows_observed(), 3);
        assert_eq!(stats.target_sector_constant_rows_inspected(), 2);
        assert_eq!(stats.target_sector_symbolic_rows_inspected(), 1);
        assert_eq!(stats.target_sector_universal_active_pinches(), 1);
        assert_eq!(stats.target_sector_universal_inactive_activations(), 0);
        assert_eq!(stats.target_sector_symbolic_activation_obligations(), 1);
        assert_eq!(stats.target_sector_constant_additions_observed(), 2);
        assert!(
            stats.target_sector_integer_bit_work_observed()
                <= stats.target_sector_integer_bit_work_precharged()
        );
    }

    #[test]
    fn target_sector_conditional_scope_retains_exact_activation_range() {
        let ordering = one_free_row_ordering("target-sector-conditional", 1, 1, false, 0);
        let shift = IndexShift::try_new([1, -1, -3], 3).unwrap();
        assert!(
            prove_uniform_same_sector_descent(ordering.sector(), 0, &shift)
                .unwrap()
                .is_err()
        );

        let (attempt, transcript, stats) = target_attempt(&ordering, &shift);
        let TargetSectorDescentAttempt::Proved {
            witness_ordinal,
            scope,
        } = attempt
        else {
            panic!("conditional target-sector transition was not proved")
        };
        assert_eq!(witness_ordinal, 0);
        assert_eq!(
            scope,
            GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain
        );
        let witness = transcript.witnesses()[0];
        assert_eq!(
            witness.sector_prefix().decisive_component(),
            crate::affine_parametric_ordering::AffineSectorPrefixDescentComponent::PropagatorCount
        );
        assert_eq!(
            transcript.formal_sector_bits(witness).unwrap(),
            &[false, false, false]
        );
        assert_eq!(
            transcript.maximal_sector_bits(witness).unwrap(),
            &[true, false, false]
        );
        let obligations = transcript.symbolic_activation_obligations(witness).unwrap();
        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].position(), 0);
        assert_eq!(obligations[0].first(), 0);
        assert_eq!(obligations[0].last(), 0);
        assert_eq!(obligations[0].count(), 1);
        assert_eq!(stats.target_sector_fallbacks_attempted(), 0);
        assert_eq!(stats.target_sector_symbolic_activation_obligations(), 1);
    }

    #[test]
    fn target_sector_rejects_no_transition_no_pinch_and_non_descent() {
        let no_transition_ordering =
            one_free_row_ordering("target-sector-no-transition", 0, 1, false, 0);
        let no_transition_shift = IndexShift::try_new([0, 1, -3], 3).unwrap();
        assert!(
            prove_uniform_same_sector_descent(
                no_transition_ordering.sector(),
                0,
                &no_transition_shift,
            )
            .unwrap()
            .is_err()
        );
        let (attempt, transcript, _stats) =
            target_attempt(&no_transition_ordering, &no_transition_shift);
        assert!(matches!(
            attempt,
            TargetSectorDescentAttempt::NoConstantSectorTransition
        ));
        assert!(transcript.witnesses().is_empty());
        assert!(transcript.formal_sector_bits.is_empty());
        assert!(transcript.maximal_sector_bits.is_empty());
        assert!(transcript.symbolic_activation_obligations.is_empty());

        let no_pinch_ordering = one_free_row_ordering("target-sector-no-pinch", 1, 0, false, 0);
        let no_pinch_shift = IndexShift::try_new([0, 1, -3], 3).unwrap();
        assert!(
            prove_uniform_same_sector_descent(no_pinch_ordering.sector(), 0, &no_pinch_shift)
                .unwrap()
                .is_err()
        );
        let (attempt, _transcript, _stats) = target_attempt(&no_pinch_ordering, &no_pinch_shift);
        assert!(matches!(
            attempt,
            TargetSectorDescentAttempt::Unsupported(
                GeneratedResidualAffineWhenBadDescentUnsupportedReason::NoUniversalConstantPinch {
                    rhs_ordinal: 0,
                }
            )
        ));

        let non_descending_ordering =
            fully_constant_ordering("target-sector-non-descending", [0, 1, 0], 1);
        let non_descending_shift = IndexShift::try_new([1, -1, -3], 3).unwrap();
        assert!(
            prove_uniform_same_sector_descent(
                non_descending_ordering.sector(),
                0,
                &non_descending_shift,
            )
            .unwrap()
            .is_err()
        );
        let (attempt, _transcript, _stats) =
            target_attempt(&non_descending_ordering, &non_descending_shift);
        assert!(matches!(
            attempt,
            TargetSectorDescentAttempt::Unsupported(
                GeneratedResidualAffineWhenBadDescentUnsupportedReason::NonDescendingTargetSectorPrefix {
                    rhs_ordinal: 0,
                }
            )
        ));
    }

    #[test]
    fn zero_rhs_structural_and_retained_precharges_are_empty_and_bounded() {
        let structural = target_sector_structural_precharge(0, 4096, 2048, 0).unwrap();
        assert_eq!(structural.descent_witness_components, 0);
        assert_eq!(structural.target_sector_rows, 0);
        assert_eq!(structural.formal_mask_components, 0);
        assert_eq!(structural.maximal_mask_components, 0);
        assert_eq!(structural.constant_transition_components, 0);
        assert_eq!(structural.activation_obligation_components, 0);
        assert_eq!(structural.aggregate_components, 0);
        assert_eq!(structural.constant_additions, 0);
        let transcript =
            GeneratedResidualAffineTargetSectorDescentTranscript::try_with_precharged_capacities(
                0, 0,
            )
            .unwrap();
        assert!(transcript.witnesses.is_empty());
        assert_eq!(transcript.witnesses.capacity(), 0);
        assert_eq!(transcript.formal_sector_bits.capacity(), 0);
        assert_eq!(transcript.maximal_sector_bits.capacity(), 0);
        assert_eq!(transcript.constant_transitions.capacity(), 0);
        assert_eq!(transcript.symbolic_activation_obligations.capacity(), 0);
        let retained = descent_retained_byte_envelope(1234, 0, 0).unwrap();
        assert!(retained >= 1234);
    }

    #[test]
    fn target_sector_private_payload_debug_is_redacted() {
        let transition = GeneratedResidualAffineConstantTransition {
            position: 991,
            kind: GeneratedResidualAffineConstantTransitionKind::UniversalActivePinch,
        };
        let obligation = GeneratedResidualAffineSymbolicActivationObligation {
            position: 992,
            first: -993,
            last: 994,
            count: 1_988,
        };

        let transition_debug = format!("{transition:?}");
        let obligation_debug = format!("{obligation:?}");
        for private_number in ["991", "992", "993", "994", "1988"] {
            assert!(!transition_debug.contains(private_number));
            assert!(!obligation_debug.contains(private_number));
        }
        assert!(transition_debug.contains("UniversalActivePinch"));
        assert!(obligation_debug.contains("<redacted>"));
    }
}
