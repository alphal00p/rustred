//! Concrete epoch-source adapter for the sealed inventory port.

use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use super::protocol::{
    CommittedExceptionalPredicateView, CommittedExceptionalSourceAllocationIdentity,
    CommittedExceptionalSourceCensusOverflow, CommittedExceptionalSourceOwner,
    CommittedExceptionalSourcePort, CommittedExceptionalSourceRowStats,
    CommittedExceptionalSourceRowView, erased_arc_retained_byte_bound,
};
use crate::exact_identity::{ExactIdentityError, ExactIdentityLimits, ExactStructuralIdentity};
use crate::solver::closure::case_inventory::{
    CommittedExceptionalSingletonAuthorityAssembly, GeneratedAffineResidualCaseAuthority,
    GeneratedAffineResidualCaseAuthorityError, GeneratedAffineResidualCaseAuthorityLimits,
    GeneratedAffineResidualCaseSourceRowLimits,
};
use crate::solver::closure::publication_handoff::publication_epoch_owner::CommittedExceptionalSingletonSource;
use crate::{
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricNonZeroCondition, SectorMask,
};

/// The sole ingress from a consumed publication-epoch source into inventory
/// authority.  Keeping the concrete type in this signature prevents arbitrary
/// crate modules from supplying a source-port implementation.
pub(in crate::solver::closure) fn try_new_authority(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source: CommittedExceptionalSingletonSource,
    limits: GeneratedAffineResidualCaseAuthorityLimits,
) -> Result<GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError> {
    catch_unwind(AssertUnwindSafe(|| {
        let scope_comparison_bytes = checked_sum(
            "committed exceptional singleton scope comparison bytes",
            [
                family.fingerprint_ref().len(),
                source.family_fingerprint().len(),
                context.fingerprint().len(),
                source.context_fingerprint().len(),
            ],
        )?;
        check_limit(
            "committed exceptional singleton scope comparison bytes",
            scope_comparison_bytes,
            limits.max_scope_comparison_bytes,
        )?;
        if family.fingerprint_ref() != source.family_fingerprint()
            || context.fingerprint() != source.context_fingerprint()
            || context.index_count() != source.ambient_arity()
            || source.sector().arity() != source.ambient_arity()
            || source.constants().len() != source.ambient_arity()
            || source.compact_affine_matrix().len()
                != source
                    .ambient_arity()
                    .checked_mul(source.free_positions().len())
                    .ok_or(
                        GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                            resource: "committed exceptional compact geometry entries",
                        },
                    )?
        {
            return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
        }
        let domain_scans = source
            .target_premises()
            .len()
            .checked_add(source.predicate_count())
            .ok_or(
                GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                    resource: "committed exceptional domain scans",
                },
            )?;
        for (resource, requested, limit) in [
            (
                "committed exceptional source replays",
                1,
                limits.max_direct_terminal_replays,
            ),
            (
                "committed exceptional source authentications",
                1,
                limits.max_direct_terminal_authentications,
            ),
            (
                "committed exceptional case authentications",
                1,
                limits.max_direct_case_authentications,
            ),
            (
                "committed exceptional group authentications",
                1,
                limits.max_direct_group_authentications,
            ),
            (
                "committed exceptional domain scans",
                domain_scans,
                limits.max_direct_guard_scans,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }
        let anchor_entries = source.ambient_arity();
        let prospective_anchor_bytes = checked_sum(
            "committed exceptional singleton anchor-offset bytes",
            [
                arc_payload_control_and_padding_byte_bound::<Vec<Vec<Integer>>>()?,
                size_of::<Vec<Integer>>(),
                anchor_entries.checked_mul(size_of::<Integer>()).ok_or(
                    GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                        resource: "committed exceptional singleton anchor-offset bytes",
                    },
                )?,
            ],
        )?;
        for (resource, requested, limit) in [
            (
                "committed exceptional singleton anchor-offset entries",
                anchor_entries,
                limits.max_direct_anchor_offset_entries,
            ),
            (
                "committed exceptional singleton anchor-offset integer bits",
                0,
                limits.max_direct_anchor_offset_integer_bits,
            ),
            (
                "committed exceptional singleton anchor-offset bytes",
                prospective_anchor_bytes,
                limits.max_direct_anchor_offset_bytes,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }

        // Keep the concrete, unallocated source through all admission and
        // replay work.  Type erasure occurs only after the complete retained
        // authority is ready to assemble.
        source.replay(family, context)?;
        let stable_identity = source
            .encode_durable_identity(
                family,
                context,
                limits.committed_parent_source_row,
                limits.direct_source_identity,
            )
            .map_err(GeneratedAffineResidualCaseAuthorityError::StableIdentity)?;

        let mut zero_offset = Vec::new();
        zero_offset.try_reserve_exact(anchor_entries).map_err(|_| {
            GeneratedAffineResidualCaseAuthorityError::AllocationFailure {
                resource: "committed exceptional singleton anchor offset",
                requested: anchor_entries,
            }
        })?;
        zero_offset.resize_with(anchor_entries, || Integer::from(0));
        let mut offsets = Vec::new();
        offsets.try_reserve_exact(1).map_err(|_| {
            GeneratedAffineResidualCaseAuthorityError::AllocationFailure {
                resource: "committed exceptional singleton anchor-offset table",
                requested: 1,
            }
        })?;
        offsets.push(zero_offset);
        let observed_anchor_bytes = checked_sum(
            "committed exceptional singleton anchor-offset bytes",
            [
                arc_payload_control_and_padding_byte_bound::<Vec<Vec<Integer>>>()?,
                offsets
                    .capacity()
                    .checked_mul(size_of::<Vec<Integer>>())
                    .ok_or(
                        GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                            resource: "committed exceptional singleton anchor-offset bytes",
                        },
                    )?,
                offsets[0]
                    .capacity()
                    .checked_mul(size_of::<Integer>())
                    .ok_or(
                        GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                            resource: "committed exceptional singleton anchor-offset bytes",
                        },
                    )?,
            ],
        )?;
        check_limit(
            "committed exceptional singleton anchor-offset bytes",
            observed_anchor_bytes,
            limits.max_direct_anchor_offset_bytes,
        )?;

        let source_arc_retained_bytes =
            erased_arc_retained_byte_bound(&source).map_err(|error| {
                GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                    resource: error.resource(),
                }
            })?;
        let owner_retained_bytes_excluding_shared_ancestry = checked_sum(
            "committed exceptional case-authority retained bytes excluding shared ancestry",
            [
                size_of::<GeneratedAffineResidualCaseAuthority>(),
                observed_anchor_bytes,
                source_arc_retained_bytes,
                arc_string_owned_byte_bound(stable_identity.bytes())?,
            ],
        )?;
        let owner = CommittedExceptionalSourceOwner::new(source);
        Ok(
            GeneratedAffineResidualCaseAuthority::assemble_committed_exceptional_singleton(
                CommittedExceptionalSingletonAuthorityAssembly {
                    source: owner,
                    anchor_offsets: Arc::new(offsets),
                    stable_identity,
                    limits,
                    scope_comparison_bytes,
                    domain_scans,
                    observed_anchor_bytes,
                    owner_retained_bytes_excluding_shared_ancestry,
                },
            ),
        )
    }))
    .map_err(|_| GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic)?
}

impl
    CommittedExceptionalSourcePort<
        GeneratedAffineResidualCaseAuthorityError,
        GeneratedAffineResidualCaseSourceRowLimits,
    > for CommittedExceptionalSingletonSource
{
    fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualCaseAuthorityError> {
        CommittedExceptionalSingletonSource::replay(self, family, context)
    }

    fn family_fingerprint(&self) -> &str {
        CommittedExceptionalSingletonSource::family_fingerprint(self)
    }

    fn context_fingerprint(&self) -> &str {
        CommittedExceptionalSingletonSource::context_fingerprint(self)
    }

    fn sector(&self) -> &SectorMask {
        CommittedExceptionalSingletonSource::sector(self)
    }

    fn ordering(&self) -> IntegralOrderingPolicy {
        CommittedExceptionalSingletonSource::ordering(self)
    }

    fn ambient_arity(&self) -> usize {
        CommittedExceptionalSingletonSource::ambient_arity(self)
    }

    fn constants(&self) -> &[Integer] {
        CommittedExceptionalSingletonSource::constants(self)
    }

    fn free_positions(&self) -> &[usize] {
        CommittedExceptionalSingletonSource::free_positions(self)
    }

    fn compact_affine_matrix(&self) -> &[Integer] {
        CommittedExceptionalSingletonSource::compact_affine_matrix(self)
    }

    fn target_premises(&self) -> &[ParametricNonZeroCondition] {
        CommittedExceptionalSingletonSource::target_premises(self)
    }

    fn predicate_count(&self) -> usize {
        CommittedExceptionalSingletonSource::predicate_count(self)
    }

    fn predicate(&self, ordinal: usize) -> Option<CommittedExceptionalPredicateView<'_>> {
        let predicate = CommittedExceptionalSingletonSource::predicate(self, ordinal)?;
        Some(CommittedExceptionalPredicateView::new(
            ordinal,
            predicate.locus_ordinal(),
            predicate.kind(),
            predicate.polynomial(),
        ))
    }

    fn source_row_count(&self) -> usize {
        CommittedExceptionalSingletonSource::source_row_count(self)
    }

    fn authenticated_source_row_view<'source>(
        &'source self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_ordinal: usize,
        limits: GeneratedAffineResidualCaseSourceRowLimits,
    ) -> Result<CommittedExceptionalSourceRowView<'source>, GeneratedAffineResidualCaseAuthorityError>
    {
        let row = CommittedExceptionalSingletonSource::authenticated_source_row_view(
            self,
            family,
            context,
            source_row_ordinal,
            limits,
        )?;
        let stats = row.stats();
        Ok(CommittedExceptionalSourceRowView::new(
            row.source_row_ordinal(),
            row.relation(),
            CommittedExceptionalSourceRowStats::new(
                stats.scope_comparison_bytes(),
                stats.source_rows(),
                stats.relation_terms(),
                stats.guard_conditions(),
            ),
        ))
    }

    fn allocation_identity(&self) -> CommittedExceptionalSourceAllocationIdentity {
        CommittedExceptionalSourceAllocationIdentity::new(
            self.event_allocation_identity_for_closure(),
            self.leaf_ordinal(),
        )
    }

    fn event_ordinal(&self) -> usize {
        CommittedExceptionalSingletonSource::event_ordinal(self)
    }

    fn leaf_ordinal(&self) -> usize {
        CommittedExceptionalSingletonSource::leaf_ordinal(self)
    }

    fn retained_parent_plan_manifest(&self) -> &str {
        CommittedExceptionalSingletonSource::retained_parent_plan_manifest(self)
    }

    fn durable_identity_schema(&self) -> &'static str {
        CommittedExceptionalSingletonSource::durable_identity_schema(self)
    }

    fn encode_durable_identity(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_limits: GeneratedAffineResidualCaseSourceRowLimits,
        limits: ExactIdentityLimits,
    ) -> Result<ExactStructuralIdentity, ExactIdentityError> {
        CommittedExceptionalSingletonSource::encode_durable_identity(
            self,
            family,
            context,
            source_row_limits,
            limits,
        )
    }

    fn owner_local_deep_retained_bytes(
        &self,
    ) -> Result<usize, CommittedExceptionalSourceCensusOverflow> {
        // The source owns only an inline event Arc and leaf ordinal.  Event
        // payload and retained parent-plan ancestry are shared allocations
        // charged by their dedicated campaign owners.
        Ok(0)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow { resource })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn arc_payload_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    let controls = 2usize.checked_mul(size_of::<usize>()).ok_or(
        GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
            resource: "direct singleton anchor-offset bytes",
        },
    )?;
    let alignment = align_of::<T>();
    let padding = (alignment - (controls % alignment)) % alignment;
    checked_sum(
        "direct singleton anchor-offset bytes",
        [controls, padding, size_of::<T>()],
    )
}

fn arc_string_owned_byte_bound(
    value: &Arc<String>,
) -> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    checked_add(
        "direct stable source-identity bytes",
        arc_payload_control_and_padding_byte_bound::<String>()?,
        value.capacity(),
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseAuthorityError> {
    if requested > limit {
        Err(GeneratedAffineResidualCaseAuthorityError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
