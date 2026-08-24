//! Opaque Stage-A owner for one initial-global ready Boolean terminal after
//! fresh affine recognition and source-neutral guard composition.
//!
//! This module intentionally exposes no V1 cover, branch, guard entry, raw
//! class, source-case identity, or owning `Arc`.  The later one-replay Boolean
//! session may construct this value positionally; inventory and grouping are
//! deliberately outside this stage.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use crate::residual_affine_branch_guard_composition::{
    ResidualAffineBranchSealedGuardBundle, ResidualAffineBranchSealedGuardLogicalMemoryCensus,
    ResidualAffineBranchSealedGuardPayloadComparisonCensus,
    ResidualAffineBranchSealedGuardSourceView, sealed_guard_memory_envelope_parts_from_limits,
};
use crate::residual_affine_branch_system::{
    ResidualAffineBranchSystemFreshAuthenticatedParts, ResidualAffineBranchSystemFreshCompilation,
    ResidualAffineBranchSystemLogicalMemoryCensus,
    authenticate_residual_affine_branch_fresh_memory_census,
    residual_affine_branch_system_memory_envelope_from_limits,
};
use crate::{
    ParametricCoefficientContext, ResidualAffineBranchEmptyReason,
    ResidualAffineBranchGuardCompositionError, ResidualAffineBranchGuardCompositionLimits,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemError,
    ResidualAffineBranchSystemLimits, ResidualAffineBranchSystemOutcome,
    ResidualAffineBranchUnsupportedReason, ResidualAffineIntegerMap,
    ResidualProductLocusBooleanCoverCertificate,
};

pub(crate) const GENERATED_AFFINE_INITIAL_GLOBAL_AFFINE_TERMINAL_V1_SCHEMA: &str =
    "rustred-generated-affine-initial-global-affine-terminal-v1";

#[cfg(test)]
thread_local! {
    static GENERATED_AFFINE_INITIAL_TERMINAL_ADJACENT_AUTH_CALLS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static GENERATED_AFFINE_INITIAL_TERMINAL_MANIFEST_MISMATCHES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static GENERATED_AFFINE_INITIAL_TERMINAL_FRESH_COMPOSITIONS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_CALLS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_ENTRIES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_BYTES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static GENERATED_AFFINE_INITIAL_TERMINAL_STANDALONE_PAYLOAD_CENSUS_CALLS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test() {
    GENERATED_AFFINE_INITIAL_TERMINAL_ADJACENT_AUTH_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn generated_affine_initial_terminal_adjacent_auth_calls_for_test() -> usize {
    GENERATED_AFFINE_INITIAL_TERMINAL_ADJACENT_AUTH_CALLS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_generated_affine_initial_terminal_manifest_mismatches_for_test() {
    GENERATED_AFFINE_INITIAL_TERMINAL_MANIFEST_MISMATCHES.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn generated_affine_initial_terminal_manifest_mismatches_for_test() -> usize {
    GENERATED_AFFINE_INITIAL_TERMINAL_MANIFEST_MISMATCHES.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_generated_affine_initial_terminal_fresh_compositions_for_test() {
    GENERATED_AFFINE_INITIAL_TERMINAL_FRESH_COMPOSITIONS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn generated_affine_initial_terminal_fresh_compositions_for_test() -> usize {
    GENERATED_AFFINE_INITIAL_TERMINAL_FRESH_COMPOSITIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_generated_affine_initial_terminal_successful_manifest_census_for_test() {
    GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_CALLS.with(|value| value.set(0));
    GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_ENTRIES.with(|value| value.set(0));
    GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_BYTES.with(|value| value.set(0));
}

#[cfg(test)]
pub(crate) fn generated_affine_initial_terminal_successful_manifest_census_for_test()
-> (usize, usize, usize) {
    (
        GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_CALLS.with(std::cell::Cell::get),
        GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_ENTRIES.with(std::cell::Cell::get),
        GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_BYTES.with(std::cell::Cell::get),
    )
}

#[cfg(test)]
pub(crate) fn reset_generated_affine_initial_terminal_standalone_payload_census_calls_for_test() {
    GENERATED_AFFINE_INITIAL_TERMINAL_STANDALONE_PAYLOAD_CENSUS_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn generated_affine_initial_terminal_standalone_payload_census_calls_for_test() -> usize
{
    GENERATED_AFFINE_INITIAL_TERMINAL_STANDALONE_PAYLOAD_CENSUS_CALLS.with(std::cell::Cell::get)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineInitialGlobalAffineTerminalOutcome {
    ProvedEmpty,
    Unsupported,
    GuardContradiction,
    Actionable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalAffineTerminalLogicalMemoryCensus {
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl GeneratedAffineInitialGlobalAffineTerminalLogicalMemoryCensus {
    pub(crate) const fn retained_owned_logical_bytes(self) -> usize {
        self.retained_owned_logical_bytes
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }
}

/// Conservative prospective child memory derived only from the nested hard
/// limits.  The shared Boolean cover is excluded exactly as it is from the
/// concrete adjacent census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalAffineTerminalMemoryEnvelope {
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

/// Complete scalar cost of comparing two equal opaque initial children,
/// including their recursively authenticated branch and optional sealed-guard
/// comparison censes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

/// Exact scalar work performed while binding one freshly compiled child to
/// one selected Boolean node. Each positional comparison reads one word from
/// the child manifest and one word from the selected-node manifest; the two
/// manifest length checks are accounted in the same way. No manifest entry
/// or owning allocation crosses this seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineInitialGlobalAffineManifestBindingCensus {
    units: usize,
    bytes: usize,
}

impl GeneratedAffineInitialGlobalAffineManifestBindingCensus {
    pub(crate) const fn units(self) -> usize {
        self.units
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }
}

impl GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus {
    pub(crate) const fn units(self) -> usize {
        self.units
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }

    pub(crate) const fn integer_bits(self) -> usize {
        self.integer_bits
    }
}

impl GeneratedAffineInitialGlobalAffineTerminalMemoryEnvelope {
    pub(crate) const fn retained_owned_logical_bytes_upper_bound(self) -> usize {
        self.retained_owned_logical_bytes_upper_bound
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }
}

/// Positional unsupported-result projection.  The diagnostic integer system
/// retained privately by an unsupported branch is deliberately unreachable.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineInitialGlobalAffineUnsupportedSourceView<'source> {
    reasons: &'source [ResidualAffineBranchUnsupportedReason],
}

impl<'source> GeneratedAffineInitialGlobalAffineUnsupportedSourceView<'source> {
    pub(crate) const fn reason_count(self) -> usize {
        self.reasons.len()
    }

    pub(crate) fn reason(
        self,
        position: usize,
    ) -> Option<&'source ResidualAffineBranchUnsupportedReason> {
        self.reasons.get(position)
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalAffineUnsupportedSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalAffineUnsupportedSourceView")
            .field("reason_count", &self.reasons.len())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Source-neutral actionable or contradictory guarded-map projection.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineInitialGlobalAffineGuardedSourceView<'source> {
    affine_map: &'source ResidualAffineIntegerMap,
    guards: ResidualAffineBranchSealedGuardSourceView<'source>,
}

impl<'source> GeneratedAffineInitialGlobalAffineGuardedSourceView<'source> {
    pub(crate) const fn affine_map(self) -> &'source ResidualAffineIntegerMap {
        self.affine_map
    }

    pub(crate) const fn guards(self) -> ResidualAffineBranchSealedGuardSourceView<'source> {
        self.guards
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalAffineGuardedSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalAffineGuardedSourceView")
            .field("ambient_arity", &self.affine_map.ambient_arity())
            .field("guard_count", &self.guards.guard_count())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Authenticated semantic disposition of one opaque initial child.  No
/// variant can reach a raw branch, guard entry/class, condition/provenance,
/// Boolean cover, or owning allocation.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GeneratedAffineInitialGlobalAffineTerminalSourceView<'source> {
    ProvedEmpty(&'source ResidualAffineBranchEmptyReason),
    Unsupported(GeneratedAffineInitialGlobalAffineUnsupportedSourceView<'source>),
    GuardContradiction(GeneratedAffineInitialGlobalAffineGuardedSourceView<'source>),
    Actionable(GeneratedAffineInitialGlobalAffineGuardedSourceView<'source>),
}

pub(crate) struct GeneratedAffineInitialGlobalAffineTerminal {
    schema: &'static str,
    source_work_item_ordinal: usize,
    local_terminal_ordinal: usize,
    private_branch: Arc<ResidualAffineBranchSystemCertificate>,
    private_sealed_guards: Option<ResidualAffineBranchSealedGuardBundle>,
    outcome: GeneratedAffineInitialGlobalAffineTerminalOutcome,
    branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    guard_memory: Option<ResidualAffineBranchSealedGuardLogicalMemoryCensus>,
    memory: GeneratedAffineInitialGlobalAffineTerminalLogicalMemoryCensus,
}

/// Non-Clone proof that one freshly compiled opaque child has already passed
/// its sole complete adjacent authentication and is bound to the exact raw
/// Boolean cover and positional manifests supplied by the sealed source
/// adapter.  The Boolean replay session must consume this proof before it can
/// retain the terminal.
pub(crate) struct GeneratedAffineInitialGlobalAffineBoundTerminal {
    terminal: GeneratedAffineInitialGlobalAffineTerminal,
    manifest_binding_census: GeneratedAffineInitialGlobalAffineManifestBindingCensus,
    payload_comparison_census: GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
}

impl fmt::Debug for GeneratedAffineInitialGlobalAffineBoundTerminal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalAffineBoundTerminal")
            .field(
                "source_work_item_ordinal",
                &self.terminal.source_work_item_ordinal,
            )
            .field(
                "local_terminal_ordinal",
                &self.terminal.local_terminal_ordinal,
            )
            .field("private_payload", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl GeneratedAffineInitialGlobalAffineBoundTerminal {
    pub(crate) fn compile_and_bind(
        context: &ParametricCoefficientContext,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
        fresh_branch: ResidualAffineBranchSystemFreshCompilation,
        guard_limits: ResidualAffineBranchGuardCompositionLimits,
        expected_source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
        expected_equal_zero_manifest: &[usize],
        expected_nonzero_manifest: &[usize],
    ) -> Result<Self, GeneratedAffineInitialGlobalAffineTerminalError> {
        // Checked scalar preflight must precede the proportional positional
        // manifest scan (and the child compiler) even when an enclosing
        // Boolean owner has already computed the same source-neutral census.
        let manifest_binding_census = initial_terminal_manifest_binding_census(
            expected_equal_zero_manifest.len(),
            expected_nonzero_manifest.len(),
        )?;
        let (terminal, payload_comparison_census) =
            GeneratedAffineInitialGlobalAffineTerminal::compile_from_fresh_branch_with_payload_comparison_census(
                context,
                source_work_item_ordinal,
                local_terminal_ordinal,
                fresh_branch,
                guard_limits,
            )?;
        if !Arc::ptr_eq(
            terminal.private_branch.source_cover(),
            expected_source_cover,
        ) {
            return Err(
                GeneratedAffineInitialGlobalAffineTerminalError::SourceCoverAllocationMismatch,
            );
        }
        terminal.authenticate_boolean_manifests(
            expected_equal_zero_manifest,
            expected_nonzero_manifest,
        )?;
        Ok(Self {
            terminal,
            manifest_binding_census,
            payload_comparison_census,
        })
    }

    /// Return only the authenticated scalar cost of the already-consumed
    /// one-child manifest scan. The non-Clone bound proof continues to own the
    /// terminal until the Boolean replay session consumes it positionally.
    pub(crate) const fn manifest_binding_census(
        &self,
    ) -> GeneratedAffineInitialGlobalAffineManifestBindingCensus {
        self.manifest_binding_census
    }

    /// Exact recursive child-comparison census computed by the sole final
    /// adjacent authentication during fresh construction.  Carrying this
    /// scalar proof avoids a later standalone branch/guard traversal.
    pub(crate) const fn payload_comparison_census(
        &self,
    ) -> GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus {
        self.payload_comparison_census
    }

    pub(crate) fn into_terminal_for_locator(
        self,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminal,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        if self.terminal.source_work_item_ordinal != source_work_item_ordinal
            || self.terminal.local_terminal_ordinal != local_terminal_ordinal
        {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::LocatorMismatch);
        }
        Ok(self.terminal)
    }
}

impl fmt::Debug for GeneratedAffineInitialGlobalAffineTerminal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineInitialGlobalAffineTerminal")
            .field("schema", &self.schema)
            .field("source_work_item_ordinal", &self.source_work_item_ordinal)
            .field("local_terminal_ordinal", &self.local_terminal_ordinal)
            .field("outcome", &self.outcome)
            .field("private_affine_payload", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl GeneratedAffineInitialGlobalAffineTerminal {
    pub(crate) fn compile_from_fresh_branch(
        context: &ParametricCoefficientContext,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
        fresh_branch: ResidualAffineBranchSystemFreshCompilation,
        guard_limits: ResidualAffineBranchGuardCompositionLimits,
    ) -> Result<Self, GeneratedAffineInitialGlobalAffineTerminalError> {
        Self::compile_from_fresh_branch_with_payload_comparison_census(
            context,
            source_work_item_ordinal,
            local_terminal_ordinal,
            fresh_branch,
            guard_limits,
        )
        .map(|(terminal, _)| terminal)
    }

    fn compile_from_fresh_branch_with_payload_comparison_census(
        context: &ParametricCoefficientContext,
        source_work_item_ordinal: usize,
        local_terminal_ordinal: usize,
        fresh_branch: ResidualAffineBranchSystemFreshCompilation,
        guard_limits: ResidualAffineBranchGuardCompositionLimits,
    ) -> Result<
        (
            Self,
            GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
        ),
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        let parts = fresh_branch.into_authenticated_parts(context)?;
        let (private_branch, private_sealed_guards, outcome, branch_memory, guard_memory) =
            match parts {
                ResidualAffineBranchSystemFreshAuthenticatedParts::Guarded {
                    branch,
                    authorization,
                    memory,
                } => {
                    let guards = ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(
                        context,
                        Arc::clone(&branch),
                        authorization,
                        guard_limits,
                    )?;
                    let outcome = if guards.has_contradiction() {
                        GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction
                    } else {
                        GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable
                    };
                    let guard_memory = guards.memory();
                    (branch, Some(guards), outcome, memory, Some(guard_memory))
                }
                ResidualAffineBranchSystemFreshAuthenticatedParts::Terminal { branch, memory } => {
                    let outcome = match branch.outcome() {
                        ResidualAffineBranchSystemOutcome::ProvedEmpty(_) => {
                            GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty
                        }
                        ResidualAffineBranchSystemOutcome::Unsupported { .. } => {
                            GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported
                        }
                        ResidualAffineBranchSystemOutcome::GuardedAffineMap => {
                            return Err(
                                GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant,
                            );
                        }
                    };
                    (branch, None, outcome, memory, None)
                }
            };
        let memory = initial_terminal_logical_memory_census(branch_memory, guard_memory)?;
        let terminal = Self {
            schema: GENERATED_AFFINE_INITIAL_GLOBAL_AFFINE_TERMINAL_V1_SCHEMA,
            source_work_item_ordinal,
            local_terminal_ordinal,
            private_branch,
            private_sealed_guards,
            outcome,
            branch_memory,
            guard_memory,
            memory,
        };
        #[cfg(test)]
        GENERATED_AFFINE_INITIAL_TERMINAL_FRESH_COMPOSITIONS.with(|calls| {
            calls.set(calls.get().saturating_add(1));
        });
        let payload_comparison_census =
            terminal.authenticate_adjacent_census_with_payload_comparison(context)?;
        Ok((terminal, payload_comparison_census))
    }

    pub(crate) const fn source_work_item_ordinal(&self) -> usize {
        self.source_work_item_ordinal
    }

    pub(crate) const fn local_terminal_ordinal(&self) -> usize {
        self.local_terminal_ordinal
    }

    pub(crate) const fn outcome(&self) -> GeneratedAffineInitialGlobalAffineTerminalOutcome {
        self.outcome
    }

    pub(crate) fn guard_count(&self) -> usize {
        match &self.private_sealed_guards {
            Some(guards) => guards.guard_count(),
            None => 0,
        }
    }

    pub(crate) const fn memory(
        &self,
    ) -> GeneratedAffineInitialGlobalAffineTerminalLogicalMemoryCensus {
        self.memory
    }

    /// Return the semantic source-neutral child projection only after the
    /// entire adjacent branch/guard census and allocation chain authenticates.
    pub(crate) fn authenticated_source_view(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalSourceView<'_>,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        let authenticated_guards =
            self.authenticate_adjacent_census_and_guard_source_view(context)?;
        self.source_view_from_authenticated_adjacent(authenticated_guards)
    }

    /// Authenticate the complete child once, bind it to the exact private
    /// Boolean-cover allocation and ordered manifests supplied by its sealed
    /// parent, then return only the source-neutral semantic projection.
    ///
    /// This is deliberately one combined operation: callers cannot first
    /// authenticate a binding and then project through a second adjacent
    /// branch/guard traversal.
    pub(crate) fn authenticated_source_view_for_boolean_binding(
        &self,
        context: &ParametricCoefficientContext,
        expected_source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
        expected_equal_zero_manifest: &[usize],
        expected_nonzero_manifest: &[usize],
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalSourceView<'_>,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        let authenticated_guards =
            self.authenticate_adjacent_census_and_guard_source_view(context)?;
        if !Arc::ptr_eq(self.private_branch.source_cover(), expected_source_cover) {
            return Err(
                GeneratedAffineInitialGlobalAffineTerminalError::SourceCoverAllocationMismatch,
            );
        }
        self.authenticate_boolean_manifests(
            expected_equal_zero_manifest,
            expected_nonzero_manifest,
        )?;
        self.source_view_from_authenticated_adjacent(authenticated_guards)
    }

    /// Construct a projection from the already-authenticated adjacent view.
    /// This helper performs no branch, guard, cover, or manifest scan.
    fn source_view_from_authenticated_adjacent<'terminal>(
        &'terminal self,
        authenticated_guards: Option<ResidualAffineBranchSealedGuardSourceView<'terminal>>,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalSourceView<'terminal>,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        match self.private_branch.outcome() {
            ResidualAffineBranchSystemOutcome::ProvedEmpty(reason) => {
                if self.outcome != GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty {
                    return Err(GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant);
                }
                Ok(GeneratedAffineInitialGlobalAffineTerminalSourceView::ProvedEmpty(reason))
            }
            ResidualAffineBranchSystemOutcome::Unsupported { reasons } => {
                if self.outcome != GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported {
                    return Err(GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant);
                }
                Ok(
                    GeneratedAffineInitialGlobalAffineTerminalSourceView::Unsupported(
                        GeneratedAffineInitialGlobalAffineUnsupportedSourceView { reasons },
                    ),
                )
            }
            ResidualAffineBranchSystemOutcome::GuardedAffineMap => {
                let affine_map = self
                    .private_branch
                    .affine_map()
                    .ok_or(GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant)?;
                let guards = self
                    .private_sealed_guards
                    .as_ref()
                    .and(authenticated_guards)
                    .ok_or(GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant)?;
                let guarded =
                    GeneratedAffineInitialGlobalAffineGuardedSourceView { affine_map, guards };
                match self.outcome {
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction
                        if guards.first_contradiction_entry_ordinal().is_some() =>
                    {
                        Ok(GeneratedAffineInitialGlobalAffineTerminalSourceView::GuardContradiction(
                            guarded,
                        ))
                    }
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable
                        if guards.first_contradiction_entry_ordinal().is_none() =>
                    {
                        Ok(GeneratedAffineInitialGlobalAffineTerminalSourceView::Actionable(
                            guarded,
                        ))
                    }
                    _ => Err(GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant),
                }
            }
        }
    }

    /// Recompute and authenticate the scalar equal-payload comparison census.
    /// This reveals no private child and is used by the inventory before it
    /// enters recursive checked equality.
    pub(crate) fn authenticated_payload_comparison_census(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        #[cfg(test)]
        GENERATED_AFFINE_INITIAL_TERMINAL_STANDALONE_PAYLOAD_CENSUS_CALLS.with(|calls| {
            calls.set(calls.get().saturating_add(1));
        });
        self.authenticate_adjacent_census_with_payload_comparison(context)
    }

    /// Compare every hidden child field after requiring both children to bind
    /// to the exact same retained Boolean-cover allocation.  Only a Boolean
    /// parent can supply that allocation; the operation returns no owner.
    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
        context: &ParametricCoefficientContext,
        expected_source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
        expected_equal_zero_manifest: &[usize],
        expected_nonzero_manifest: &[usize],
    ) -> Result<bool, GeneratedAffineInitialGlobalAffineTerminalError> {
        let _ = self.authenticate_source_cover_allocation_with_payload_comparison(
            context,
            expected_source_cover,
        )?;
        self.authenticate_boolean_manifests(
            expected_equal_zero_manifest,
            expected_nonzero_manifest,
        )?;
        let _ = other.authenticate_source_cover_allocation_with_payload_comparison(
            context,
            expected_source_cover,
        )?;
        other.authenticate_boolean_manifests(
            expected_equal_zero_manifest,
            expected_nonzero_manifest,
        )?;
        if self.schema != other.schema
            || self.source_work_item_ordinal != other.source_work_item_ordinal
            || self.local_terminal_ordinal != other.local_terminal_ordinal
            || self.outcome != other.outcome
            || self.branch_memory != other.branch_memory
            || self.guard_memory != other.guard_memory
            || self.memory != other.memory
        {
            return Ok(false);
        }
        match (&self.private_sealed_guards, &other.private_sealed_guards) {
            (None, None) => self
                .private_branch
                .payload_eq_checked(&other.private_branch)
                .map_err(GeneratedAffineInitialGlobalAffineTerminalError::Branch),
            (Some(left), Some(right)) => left
                .payload_eq_checked(right)
                .map_err(GeneratedAffineInitialGlobalAffineTerminalError::Guard),
            _ => Ok(false),
        }
    }

    /// Reauthenticate this opaque child against the exact sealed Boolean-cover
    /// allocation retained by its V2 parent.  The cover is accepted only as an
    /// input to an identity check; neither it nor the private branch is ever
    /// returned across this seam.
    pub(crate) fn authenticate_source_cover_allocation(
        &self,
        context: &ParametricCoefficientContext,
        expected_source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
    ) -> Result<(), GeneratedAffineInitialGlobalAffineTerminalError> {
        self.authenticate_adjacent_census(context)?;
        if !Arc::ptr_eq(self.private_branch.source_cover(), expected_source_cover) {
            return Err(
                GeneratedAffineInitialGlobalAffineTerminalError::SourceCoverAllocationMismatch,
            );
        }
        Ok(())
    }

    /// Authenticate exact positional Boolean manifests without returning the
    /// private recognition or guard sequences.
    pub(crate) fn authenticate_source_cover_allocation_and_boolean_manifests(
        &self,
        context: &ParametricCoefficientContext,
        expected_source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
        expected_equal_zero_manifest: &[usize],
        expected_nonzero_manifest: &[usize],
    ) -> Result<(), GeneratedAffineInitialGlobalAffineTerminalError> {
        self.authenticate_source_cover_allocation(context, expected_source_cover)?;
        self.authenticate_boolean_manifests(expected_equal_zero_manifest, expected_nonzero_manifest)
    }

    fn authenticate_boolean_manifests(
        &self,
        expected_equal_zero_manifest: &[usize],
        expected_nonzero_manifest: &[usize],
    ) -> Result<(), GeneratedAffineInitialGlobalAffineTerminalError> {
        if self.private_branch.zero_atom_recognitions().len() != expected_equal_zero_manifest.len()
            || self.private_branch.nonzero_guard_locus_ordinals() != expected_nonzero_manifest
            || self
                .private_branch
                .zero_atom_recognitions()
                .iter()
                .zip(expected_equal_zero_manifest)
                .any(|(recognition, expected)| recognition.structural_locus_ordinal() != *expected)
        {
            #[cfg(test)]
            GENERATED_AFFINE_INITIAL_TERMINAL_MANIFEST_MISMATCHES.with(|calls| {
                calls.set(calls.get().saturating_add(1));
            });
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::BooleanManifestMismatch);
        }
        #[cfg(test)]
        {
            let entries = expected_equal_zero_manifest
                .len()
                .saturating_add(expected_nonzero_manifest.len());
            let bytes = entries
                .saturating_add(2)
                .saturating_mul(2)
                .saturating_mul(size_of::<usize>());
            GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_CALLS.with(|value| {
                value.set(value.get().saturating_add(1));
            });
            GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_ENTRIES.with(|value| {
                value.set(value.get().saturating_add(entries));
            });
            GENERATED_AFFINE_INITIAL_TERMINAL_SUCCESSFUL_MANIFEST_BYTES.with(|value| {
                value.set(value.get().saturating_add(bytes));
            });
        }
        Ok(())
    }

    fn authenticate_source_cover_allocation_with_payload_comparison(
        &self,
        context: &ParametricCoefficientContext,
        expected_source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        let comparison = self.authenticate_adjacent_census_with_payload_comparison(context)?;
        if !Arc::ptr_eq(self.private_branch.source_cover(), expected_source_cover) {
            return Err(
                GeneratedAffineInitialGlobalAffineTerminalError::SourceCoverAllocationMismatch,
            );
        }
        Ok(comparison)
    }

    fn authenticate_adjacent_census(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineInitialGlobalAffineTerminalError> {
        self.authenticate_adjacent_census_inner(context, false)
            .map(|_| ())
    }

    fn authenticate_adjacent_census_with_payload_comparison(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        self.authenticate_adjacent_census_inner(context, true)?
            .ok_or(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch)
    }

    /// Terminal-controlled source-view authentication.  The guarded path
    /// traverses the branch once and asks the sealed guard to authenticate and
    /// project in the same call; no unchecked guard accessor exists.
    fn authenticate_adjacent_census_and_guard_source_view(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        Option<ResidualAffineBranchSealedGuardSourceView<'_>>,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        #[cfg(test)]
        GENERATED_AFFINE_INITIAL_TERMINAL_ADJACENT_AUTH_CALLS.with(|calls| {
            calls.set(calls.get().saturating_add(1));
        });
        if self.schema != GENERATED_AFFINE_INITIAL_GLOBAL_AFFINE_TERMINAL_V1_SCHEMA {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::SchemaMismatch);
        }
        if self.source_work_item_ordinal
            != self
                .private_branch
                .source_cover()
                .source_work_item_ordinal()
            || self.local_terminal_ordinal != self.private_branch.ready_terminal_ordinal()
        {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::LocatorMismatch);
        }
        let actual_guard_memory = self
            .private_sealed_guards
            .as_ref()
            .map(ResidualAffineBranchSealedGuardBundle::memory);
        if actual_guard_memory != self.guard_memory {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch);
        }
        authenticate_residual_affine_branch_fresh_memory_census(
            context,
            &self.private_branch,
            self.branch_memory,
        )?;
        let guard_source = match &self.private_sealed_guards {
            Some(guards) => Some(guards.authenticated_source_view(
                context,
                &self.private_branch,
                self.branch_memory,
            )?),
            None => None,
        };
        let expected =
            initial_terminal_logical_memory_census(self.branch_memory, actual_guard_memory)?;
        let expected_outcome = match self.private_branch.outcome() {
            ResidualAffineBranchSystemOutcome::ProvedEmpty(_) => {
                GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty
            }
            ResidualAffineBranchSystemOutcome::Unsupported { .. } => {
                GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported
            }
            ResidualAffineBranchSystemOutcome::GuardedAffineMap => {
                let guards = self
                    .private_sealed_guards
                    .as_ref()
                    .ok_or(GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant)?;
                if guards.has_contradiction() {
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction
                } else {
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable
                }
            }
        };
        if expected != self.memory
            || expected_outcome != self.outcome
            || self.branch_memory.retained_owned_logical_bytes() == 0
            || self
                .guard_memory
                .is_some_and(|memory| memory.retained_owned_logical_bytes() == 0)
            || self.private_branch.outcome() == &ResidualAffineBranchSystemOutcome::GuardedAffineMap
                && guard_source.is_none()
            || self.private_branch.outcome() != &ResidualAffineBranchSystemOutcome::GuardedAffineMap
                && guard_source.is_some()
        {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch);
        }
        Ok(guard_source)
    }

    fn authenticate_adjacent_census_inner(
        &self,
        context: &ParametricCoefficientContext,
        include_payload_comparison: bool,
    ) -> Result<
        Option<GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus>,
        GeneratedAffineInitialGlobalAffineTerminalError,
    > {
        #[cfg(test)]
        GENERATED_AFFINE_INITIAL_TERMINAL_ADJACENT_AUTH_CALLS.with(|calls| {
            calls.set(calls.get().saturating_add(1));
        });
        if self.schema != GENERATED_AFFINE_INITIAL_GLOBAL_AFFINE_TERMINAL_V1_SCHEMA {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::SchemaMismatch);
        }
        if self.source_work_item_ordinal
            != self
                .private_branch
                .source_cover()
                .source_work_item_ordinal()
            || self.local_terminal_ordinal != self.private_branch.ready_terminal_ordinal()
        {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::LocatorMismatch);
        }
        let actual_guard_memory = self
            .private_sealed_guards
            .as_ref()
            .map(ResidualAffineBranchSealedGuardBundle::memory);
        if actual_guard_memory != self.guard_memory {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch);
        }
        let (branch_comparison, guard_comparison) = match &self.private_sealed_guards {
            Some(guards) => {
                let branch_comparison = if include_payload_comparison {
                    Some(
                        self.private_branch
                            .authenticate_fresh_memory_and_payload_comparison_census(
                                context,
                                self.branch_memory,
                            )?,
                    )
                } else {
                    authenticate_residual_affine_branch_fresh_memory_census(
                        context,
                        &self.private_branch,
                        self.branch_memory,
                    )?;
                    None
                };
                let guard_comparison = if include_payload_comparison {
                    Some(
                        guards.authenticate_with_branch_memory_and_payload_comparison_census(
                            context,
                            &self.private_branch,
                            self.branch_memory,
                        )?,
                    )
                } else {
                    guards.authenticate_with_branch_memory(
                        context,
                        &self.private_branch,
                        self.branch_memory,
                    )?;
                    None
                };
                (branch_comparison, guard_comparison)
            }
            None => {
                let branch_comparison = if include_payload_comparison {
                    Some(
                        self.private_branch
                            .authenticate_fresh_memory_and_payload_comparison_census(
                                context,
                                self.branch_memory,
                            )?,
                    )
                } else {
                    authenticate_residual_affine_branch_fresh_memory_census(
                        context,
                        &self.private_branch,
                        self.branch_memory,
                    )?;
                    None
                };
                (branch_comparison, None)
            }
        };
        let expected =
            initial_terminal_logical_memory_census(self.branch_memory, actual_guard_memory)?;
        let expected_outcome = match self.private_branch.outcome() {
            ResidualAffineBranchSystemOutcome::ProvedEmpty(_) => {
                GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty
            }
            ResidualAffineBranchSystemOutcome::Unsupported { .. } => {
                GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported
            }
            ResidualAffineBranchSystemOutcome::GuardedAffineMap => {
                let guards = self
                    .private_sealed_guards
                    .as_ref()
                    .ok_or(GeneratedAffineInitialGlobalAffineTerminalError::OutcomeInvariant)?;
                if guards.has_contradiction() {
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction
                } else {
                    GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable
                }
            }
        };
        if expected != self.memory
            || expected_outcome != self.outcome
            || self.branch_memory.retained_owned_logical_bytes() == 0
            || self
                .guard_memory
                .is_some_and(|memory| memory.retained_owned_logical_bytes() == 0)
            || self.private_branch.outcome() == &ResidualAffineBranchSystemOutcome::GuardedAffineMap
                && self.private_sealed_guards.is_none()
            || self.private_branch.outcome() != &ResidualAffineBranchSystemOutcome::GuardedAffineMap
                && self.private_sealed_guards.is_some()
        {
            return Err(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch);
        }
        match branch_comparison {
            Some(branch) => Ok(Some(initial_terminal_payload_comparison_census(
                self.branch_memory,
                branch,
                guard_comparison,
            )?)),
            None if guard_comparison.is_none() => Ok(None),
            None => Err(GeneratedAffineInitialGlobalAffineTerminalError::AdjacentCensusMismatch),
        }
    }

    #[cfg(test)]
    pub(crate) fn reauthenticate_for_test(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineInitialGlobalAffineTerminalError> {
        self.authenticate_adjacent_census(context)
    }

    #[cfg(test)]
    pub(crate) fn projected_view_matches_private_for_test(
        &self,
        context: &ParametricCoefficientContext,
    ) -> bool {
        let Ok(view) = self.authenticated_source_view(context) else {
            return false;
        };
        match (self.private_branch.outcome(), view) {
            (
                ResidualAffineBranchSystemOutcome::ProvedEmpty(raw),
                GeneratedAffineInitialGlobalAffineTerminalSourceView::ProvedEmpty(projected),
            ) => raw == projected,
            (
                ResidualAffineBranchSystemOutcome::Unsupported { reasons },
                GeneratedAffineInitialGlobalAffineTerminalSourceView::Unsupported(projected),
            ) => {
                projected.reason_count() == reasons.len()
                    && reasons.iter().enumerate().all(|(position, raw)| {
                        projected.reason(position).is_some_and(|value| value == raw)
                    })
                    && projected.reason(reasons.len()).is_none()
            }
            (
                ResidualAffineBranchSystemOutcome::GuardedAffineMap,
                GeneratedAffineInitialGlobalAffineTerminalSourceView::GuardContradiction(projected)
                | GeneratedAffineInitialGlobalAffineTerminalSourceView::Actionable(projected),
            ) => {
                self.private_branch.affine_map() == Some(projected.affine_map())
                    && self.private_sealed_guards.as_ref().is_some_and(|guards| {
                        guards.projected_view_matches_private_for_test(
                            context,
                            &self.private_branch,
                            self.branch_memory,
                        )
                    })
            }
            _ => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn tamper_source_work_item_ordinal_for_test(&mut self) {
        self.source_work_item_ordinal = self.source_work_item_ordinal.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_local_terminal_ordinal_for_test(&mut self) {
        self.local_terminal_ordinal = self.local_terminal_ordinal.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_outcome_for_test(&mut self) {
        self.outcome = GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported;
    }

    #[cfg(test)]
    pub(crate) fn tamper_guard_memory_for_test(&mut self) {
        self.guard_memory = None;
    }

    #[cfg(test)]
    pub(crate) fn tamper_branch_memory_for_test(&mut self) {
        self.branch_memory = ResidualAffineBranchSystemLogicalMemoryCensus::default();
    }

    #[cfg(test)]
    pub(crate) fn tamper_branch_memory_and_outer_census_coherently_for_test(&mut self) {
        self.branch_memory
            .tamper_retained_and_peak_coherently_for_test();
        self.memory = initial_terminal_logical_memory_census(self.branch_memory, self.guard_memory)
            .expect("coherent test census remains representable");
    }

    #[cfg(test)]
    pub(crate) fn tamper_first_zero_manifest_for_test(&mut self) {
        self.replace_branch_with_test_tamper(|branch| {
            branch.tamper_first_zero_atom_ordinal_for_test();
        });
    }

    #[cfg(test)]
    pub(crate) fn tamper_first_nonzero_manifest_for_test(&mut self) {
        self.replace_branch_with_test_tamper(|branch| {
            branch.tamper_first_guard_ordinal_for_test();
        });
    }

    #[cfg(test)]
    fn replace_branch_with_test_tamper(
        &mut self,
        tamper: impl FnOnce(&mut ResidualAffineBranchSystemCertificate),
    ) {
        let mut replacement = Arc::new((*self.private_branch).clone());
        tamper(Arc::get_mut(&mut replacement).expect("fresh test branch is unique"));
        if let Some(guards) = &mut self.private_sealed_guards {
            guards.tamper_source_branch_for_test(Arc::clone(&replacement));
        }
        self.private_branch = replacement;
    }

    #[cfg(test)]
    pub(crate) fn rebind_ready_terminal_ordinal_coherently_for_test(
        &mut self,
        local_terminal_ordinal: usize,
    ) {
        self.replace_branch_with_test_tamper(|branch| {
            branch.set_ready_terminal_ordinal_for_test(local_terminal_ordinal);
        });
        self.local_terminal_ordinal = local_terminal_ordinal;
    }

    #[cfg(test)]
    pub(crate) fn tamper_guard_source_branch_for_test(
        &mut self,
        branch: Arc<ResidualAffineBranchSystemCertificate>,
    ) {
        self.private_sealed_guards
            .as_mut()
            .expect("test terminal has sealed guards")
            .tamper_source_branch_for_test(branch);
    }
}

/// Checked child envelope used by the inventory before the replay session is
/// allowed to compile its first ready terminal.
pub(crate) const fn generated_affine_initial_global_affine_bound_terminal_temporary_overhead()
-> usize {
    size_of::<GeneratedAffineInitialGlobalAffineBoundTerminal>()
        .saturating_sub(size_of::<GeneratedAffineInitialGlobalAffineTerminal>())
}

pub(crate) fn generated_affine_initial_global_affine_terminal_memory_envelope_from_limits(
    branch_limits: ResidualAffineBranchSystemLimits,
    guard_limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<
    GeneratedAffineInitialGlobalAffineTerminalMemoryEnvelope,
    GeneratedAffineInitialGlobalAffineTerminalError,
> {
    let branch = residual_affine_branch_system_memory_envelope_from_limits(branch_limits)?;
    let guard = sealed_guard_memory_envelope_parts_from_limits(guard_limits)?;
    let fixed = size_of::<GeneratedAffineInitialGlobalAffineTerminal>();
    let guard_retained = guard.guard_retained_owned_logical_bytes_upper_bound();
    let embedded_guard_wrapper = size_of::<ResidualAffineBranchSealedGuardBundle>();
    let retained_children = checked_add(
        "initial affine terminal retained logical bytes envelope",
        branch.retained_owned_logical_bytes_upper_bound(),
        guard_retained,
    )?
    .checked_sub(embedded_guard_wrapper)
    .ok_or(
        GeneratedAffineInitialGlobalAffineTerminalError::MemoryCountOverflow {
            resource: "initial affine terminal retained logical bytes envelope",
        },
    )?;
    let retained_owned_logical_bytes_upper_bound = checked_add(
        "initial affine terminal retained logical bytes envelope",
        fixed,
        retained_children,
    )?;
    let guarded_peak = checked_add(
        "initial affine terminal compilation logical peak envelope",
        branch.retained_owned_logical_bytes_upper_bound(),
        guard.compilation_owned_logical_peak_upper_bound(),
    )?;
    let compilation_owned_logical_peak_upper_bound = checked_add(
        "initial affine terminal compilation logical peak envelope",
        fixed,
        branch
            .compilation_owned_logical_peak_upper_bound()
            .max(guarded_peak),
    )?;
    Ok(GeneratedAffineInitialGlobalAffineTerminalMemoryEnvelope {
        retained_owned_logical_bytes_upper_bound,
        compilation_owned_logical_peak_upper_bound,
    })
}

fn initial_terminal_payload_comparison_census(
    branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    branch: crate::residual_affine_branch_system::ResidualAffineBranchSystemPayloadComparisonCensus,
    guard: Option<ResidualAffineBranchSealedGuardPayloadComparisonCensus>,
) -> Result<
    GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
    GeneratedAffineInitialGlobalAffineTerminalError,
> {
    // Two complete local operand representations.  Per operand: schema (1),
    // two locators (2), branch/optional-guard identity seams (2), outcome
    // (1), four top-level branch-memory scalars plus its raw-transient option
    // tag (5), option tag plus five guard-memory scalars (6), and two
    // aggregate-memory scalars (2): 19 fixed units.  A present raw transient
    // contributes its exact four scalar fields.
    const LOCAL_FIXED_UNITS_PER_OPERAND: usize = 19;
    let local_units_per_operand = checked_add(
        "initial affine terminal payload comparison units",
        LOCAL_FIXED_UNITS_PER_OPERAND,
        if branch_memory
            .integer_system_raw_transient_census()
            .is_some()
        {
            4
        } else {
            0
        },
    )?;
    let local_units = checked_mul(
        "initial affine terminal payload comparison units",
        2,
        local_units_per_operand,
    )?;
    let local_struct_bytes = checked_mul(
        "initial affine terminal payload comparison bytes",
        2,
        size_of::<GeneratedAffineInitialGlobalAffineTerminal>(),
    )?;
    let local_schema_bytes = checked_mul(
        "initial affine terminal payload comparison bytes",
        2,
        GENERATED_AFFINE_INITIAL_GLOBAL_AFFINE_TERMINAL_V1_SCHEMA.len(),
    )?;
    let local_bytes = checked_add(
        "initial affine terminal payload comparison bytes",
        local_struct_bytes,
        local_schema_bytes,
    )?;
    let guard_units = guard.map(|value| value.units()).unwrap_or(0);
    let guard_bytes = guard.map(|value| value.bytes()).unwrap_or(0);
    let guard_integer_bits = guard.map(|value| value.integer_bits()).unwrap_or(0);
    Ok(
        GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus {
            units: checked_add(
                "initial affine terminal payload comparison units",
                local_units,
                checked_add(
                    "initial affine terminal payload comparison units",
                    branch.units(),
                    guard_units,
                )?,
            )?,
            bytes: checked_add(
                "initial affine terminal payload comparison bytes",
                local_bytes,
                checked_add(
                    "initial affine terminal payload comparison bytes",
                    branch.bytes(),
                    guard_bytes,
                )?,
            )?,
            integer_bits: checked_add(
                "initial affine terminal payload comparison integer bits",
                branch.integer_bits(),
                guard_integer_bits,
            )?,
        },
    )
}

fn initial_terminal_logical_memory_census(
    branch: ResidualAffineBranchSystemLogicalMemoryCensus,
    guard: Option<ResidualAffineBranchSealedGuardLogicalMemoryCensus>,
) -> Result<
    GeneratedAffineInitialGlobalAffineTerminalLogicalMemoryCensus,
    GeneratedAffineInitialGlobalAffineTerminalError,
> {
    // The terminal struct itself already contains the branch Arc handle and
    // optional guard-bundle handle. Their control blocks/payloads are charged
    // by the child censes; subtract the standalone guard wrapper once because
    // it is embedded in this fixed terminal allocation.
    let fixed = size_of::<GeneratedAffineInitialGlobalAffineTerminal>();
    let guard_retained = guard
        .map(ResidualAffineBranchSealedGuardLogicalMemoryCensus::retained_owned_logical_bytes)
        .unwrap_or(0);
    let embedded_guard_wrapper = guard
        .map(|_| size_of::<ResidualAffineBranchSealedGuardBundle>())
        .unwrap_or(0);
    let retained_owned_logical_bytes = checked_add(
        "initial affine terminal retained logical bytes",
        fixed,
        checked_add(
            "initial affine terminal retained logical bytes",
            branch.retained_owned_logical_bytes(),
            guard_retained,
        )?
        .checked_sub(embedded_guard_wrapper)
        .ok_or(
            GeneratedAffineInitialGlobalAffineTerminalError::MemoryCountOverflow {
                resource: "initial affine terminal retained logical bytes",
            },
        )?,
    )?;
    let guarded_peak = match guard {
        Some(guard) => checked_add(
            "initial affine terminal guarded logical peak",
            branch.retained_owned_logical_bytes(),
            guard.compilation_owned_logical_peak_upper_bound(),
        )?,
        None => branch.retained_owned_logical_bytes(),
    };
    let compilation_owned_logical_peak_upper_bound = checked_add(
        "initial affine terminal compilation logical peak",
        fixed,
        branch
            .compilation_owned_logical_peak_upper_bound()
            .max(guarded_peak),
    )?;
    Ok(
        GeneratedAffineInitialGlobalAffineTerminalLogicalMemoryCensus {
            retained_owned_logical_bytes,
            compilation_owned_logical_peak_upper_bound,
        },
    )
}

fn initial_terminal_manifest_binding_census(
    equal_zero_count: usize,
    nonzero_count: usize,
) -> Result<
    GeneratedAffineInitialGlobalAffineManifestBindingCensus,
    GeneratedAffineInitialGlobalAffineTerminalError,
> {
    let entries = checked_add(
        "initial terminal manifest binding entry",
        equal_zero_count,
        nonzero_count,
    )?;
    let units = checked_add("initial terminal manifest binding unit", entries, 2)?;
    let bytes = checked_mul(
        "initial terminal manifest binding byte",
        checked_mul("initial terminal manifest binding word", units, 2)?,
        size_of::<usize>(),
    )?;
    Ok(GeneratedAffineInitialGlobalAffineManifestBindingCensus { units, bytes })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineInitialGlobalAffineTerminalError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineInitialGlobalAffineTerminalError::MemoryCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineInitialGlobalAffineTerminalError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineInitialGlobalAffineTerminalError::MemoryCountOverflow { resource })
}

#[derive(Debug)]
pub(crate) enum GeneratedAffineInitialGlobalAffineTerminalError {
    SchemaMismatch,
    LocatorMismatch,
    SourceCoverAllocationMismatch,
    BooleanManifestMismatch,
    OutcomeInvariant,
    AdjacentCensusMismatch,
    MemoryCountOverflow { resource: &'static str },
    Branch(ResidualAffineBranchSystemError),
    Guard(ResidualAffineBranchGuardCompositionError),
}

impl fmt::Display for GeneratedAffineInitialGlobalAffineTerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("initial affine terminal schema mismatch"),
            Self::LocatorMismatch => formatter.write_str(
                "initial affine terminal source-neutral locator does not match its private branch",
            ),
            Self::SourceCoverAllocationMismatch => formatter.write_str(
                "initial affine terminal does not retain the exact sealed Boolean-cover allocation",
            ),
            Self::BooleanManifestMismatch => formatter.write_str(
                "initial affine terminal Boolean atom manifests differ from its selected ready node",
            ),
            Self::OutcomeInvariant => formatter
                .write_str("initial affine terminal outcome and guarded authorization disagree"),
            Self::AdjacentCensusMismatch => formatter
                .write_str("initial affine terminal adjacent logical-memory census differs"),
            Self::MemoryCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::Branch(source) => source.fmt(formatter),
            Self::Guard(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedAffineInitialGlobalAffineTerminalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Branch(source) => Some(source),
            Self::Guard(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ResidualAffineBranchSystemError> for GeneratedAffineInitialGlobalAffineTerminalError {
    fn from(value: ResidualAffineBranchSystemError) -> Self {
        Self::Branch(value)
    }
}

impl From<ResidualAffineBranchGuardCompositionError>
    for GeneratedAffineInitialGlobalAffineTerminalError
{
    fn from(value: ResidualAffineBranchGuardCompositionError) -> Self {
        Self::Guard(value)
    }
}
