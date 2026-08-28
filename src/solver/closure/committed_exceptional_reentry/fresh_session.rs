//! Fresh exact-session construction from one committed exceptional source.
//!
//! This orchestration sits above the publication-epoch source and the concrete
//! inventory adapter. It preserves the consumed predecessor until the complete
//! successor and its conservative visible census have been constructed.

use std::mem::{align_of, size_of};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use super::capability::CommittedExceptionalAuthorityCopyPermit;
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError,
    GeneratedAffineResidualCaseAuthorityLimits,
};
use crate::solver::closure::committed_exceptional_source::epoch_adapter;
use crate::solver::closure::publication_handoff::publication_epoch_owner::CommittedExceptionalSingletonSource;
use crate::solver::exact_session::{
    GeneratedAffineResidualGroupExactSession, GeneratedAffineResidualGroupExactSessionError,
    GeneratedAffineResidualGroupExactSessionLimits,
};
use crate::solver::exact_session::{
    GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyError,
    GeneratedAffineResidualGroupPhysicalKeyLimits,
};
use crate::solver::exact_session::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanError,
    GeneratedAffineResidualGroupSolvePlanLimits,
};
use crate::{IntegralFamily, ParametricCoefficientContext};

/// Explicit topology-neutral constructor ceilings for one narrowed child
/// session. The campaign estimator remains responsible for the surrounding
/// retained/transient memory envelope and opaque Symbolica reserve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::solver::closure) struct CommittedExceptionalFreshSessionLimits {
    pub(in crate::solver::closure) authority: GeneratedAffineResidualCaseAuthorityLimits,
    pub(in crate::solver::closure) physical_frame: GeneratedAffineResidualGroupPhysicalKeyLimits,
    pub(in crate::solver::closure) solve_plan: GeneratedAffineResidualGroupSolvePlanLimits,
    pub(in crate::solver::closure) session: GeneratedAffineResidualGroupExactSessionLimits,
}

impl Default for CommittedExceptionalFreshSessionLimits {
    fn default() -> Self {
        Self {
            authority: GeneratedAffineResidualCaseAuthorityLimits::default(),
            physical_frame: GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            solve_plan: GeneratedAffineResidualGroupSolvePlanLimits::default(),
            session: GeneratedAffineResidualGroupExactSessionLimits::default(),
        }
    }
}

#[derive(Debug)]
pub(in crate::solver::closure) enum CommittedExceptionalFreshSessionBuildError {
    Authority(GeneratedAffineResidualCaseAuthorityError),
    InheritedSourceRowCountMismatch { expected: usize, actual: usize },
    PhysicalFrame(GeneratedAffineResidualGroupPhysicalKeyError),
    SolvePlan(GeneratedAffineResidualGroupSolvePlanError),
    Session(GeneratedAffineResidualGroupExactSessionError),
    ResourceCountOverflow { resource: &'static str },
}

/// Successfully built fresh child plus its conservative enumerated
/// owner-visible census. This is not an RSS estimate: allocator metadata,
/// native Symbolica state, TLS, and headroom remain in the campaign's nonzero
/// opaque reserve.
pub(in crate::solver::closure) struct CommittedExceptionalFreshSessionBuild {
    predecessor_source: CommittedExceptionalSingletonSource,
    candidate: CommittedExceptionalFreshSessionCandidate,
}

struct CommittedExceptionalFreshSessionCandidate {
    session: GeneratedAffineResidualGroupExactSession,
    inherited_source_rows: usize,
    conservative_visible_bytes_excluding_shared_ancestry: usize,
}

impl CommittedExceptionalFreshSessionBuild {
    pub(in crate::solver::closure) const fn session(
        &self,
    ) -> &GeneratedAffineResidualGroupExactSession {
        &self.candidate.session
    }

    pub(in crate::solver::closure) const fn conservative_visible_bytes_excluding_shared_ancestry(
        &self,
    ) -> usize {
        self.candidate
            .conservative_visible_bytes_excluding_shared_ancestry
    }

    pub(in crate::solver::closure) const fn inherited_source_rows(&self) -> usize {
        self.candidate.inherited_source_rows
    }

    pub(in crate::solver::closure) fn into_session(
        self,
    ) -> GeneratedAffineResidualGroupExactSession {
        self.candidate.session
    }

    pub(in crate::solver::closure) fn recover_predecessor_source(
        self,
    ) -> CommittedExceptionalSingletonSource {
        self.predecessor_source
    }
}

/// Transactional failure from consuming one committed exceptional source.
///
/// The exact source owner is returned intact so a failed admitted transform
/// can retry or restore its predecessor without minting a second authority.
pub(in crate::solver::closure) struct CommittedExceptionalFreshSessionBuildFailure {
    source: CommittedExceptionalSingletonSource,
    error: CommittedExceptionalFreshSessionBuildError,
}

impl CommittedExceptionalFreshSessionBuildFailure {
    pub(in crate::solver::closure) fn into_parts(
        self,
    ) -> (
        CommittedExceptionalSingletonSource,
        CommittedExceptionalFreshSessionBuildError,
    ) {
        (self.source, self.error)
    }
}

/// Consume one committed source to build one replacement fresh exact session.
///
/// A sealed shallow proof copy is transferred into the successor authority.
/// The original owner is returned intact on every error and is dropped only
/// after the successor has been fully constructed and its visible census
/// checked, making the one-shot capability structural.
///
/// The census deliberately charges the committed event allocation and
/// event-local payload in full for each child. It does not enumerate the
/// separately shared event-authority/parent-plan ancestry: production must keep
/// that graph in an admitted shared-lineage owner (or add a conservative
/// per-child ancestry estimate) before dropping the origin.
pub(in crate::solver::closure) fn try_build_fresh_exact_session_for_admitted_transform(
    source: CommittedExceptionalSingletonSource,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    database_epoch: usize,
    limits: CommittedExceptionalFreshSessionLimits,
) -> Result<CommittedExceptionalFreshSessionBuild, CommittedExceptionalFreshSessionBuildFailure> {
    match try_build_fresh_exact_session_inner(&source, family, context, database_epoch, limits) {
        Ok(candidate) => Ok(CommittedExceptionalFreshSessionBuild {
            predecessor_source: source,
            candidate,
        }),
        Err(error) => Err(CommittedExceptionalFreshSessionBuildFailure { source, error }),
    }
}

fn try_build_fresh_exact_session_inner(
    source: &CommittedExceptionalSingletonSource,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    database_epoch: usize,
    limits: CommittedExceptionalFreshSessionLimits,
) -> Result<CommittedExceptionalFreshSessionCandidate, CommittedExceptionalFreshSessionBuildError> {
    let authority = Arc::new(
        epoch_adapter::try_new_authority(
            family,
            context,
            source
                .clone_for_fresh_session_authority(CommittedExceptionalAuthorityCopyPermit::new()),
            limits.authority,
        )
        .map_err(CommittedExceptionalFreshSessionBuildError::Authority)?,
    );
    let inherited_source_rows = source.source_row_count();
    if authority.source_row_count() != inherited_source_rows {
        return Err(
            CommittedExceptionalFreshSessionBuildError::InheritedSourceRowCountMismatch {
                expected: inherited_source_rows,
                actual: authority.source_row_count(),
            },
        );
    }
    let physical_frame = Arc::new(
        GeneratedAffineResidualGroupPhysicalFrame::try_new(
            family,
            context,
            Arc::clone(&authority),
            limits.physical_frame,
        )
        .map_err(CommittedExceptionalFreshSessionBuildError::PhysicalFrame)?,
    );
    let solve_plan = Arc::new(
        GeneratedAffineResidualGroupSolvePlan::try_new_committed_exceptional_singleton(
            family,
            context,
            Arc::clone(&authority),
            Arc::clone(&physical_frame),
            limits.solve_plan,
        )
        .map_err(CommittedExceptionalFreshSessionBuildError::SolvePlan)?,
    );
    let session = GeneratedAffineResidualGroupExactSession::try_new(
        family,
        context,
        Arc::clone(&solve_plan),
        database_epoch,
        limits.session,
    )
    .map_err(CommittedExceptionalFreshSessionBuildError::Session)?;
    let snapshot = session.resident_resource_snapshot();
    // Each owner statistic below includes its pointee and deep local buffers
    // but excludes its newly allocated outer Arc. The authority statistic
    // already includes its nested Arc<Source> control, inline Source payload,
    // stable identity, and anchor offsets; adding a second size_of::<Source>()
    // here would double count that payload.
    let conservative_visible_bytes_excluding_shared_ancestry = [
        source.retained_event_bytes(),
        committed_exceptional_outer_arc_control_and_padding_byte_bound::<
            GeneratedAffineResidualCaseAuthority,
        >()?,
        authority.owner_retained_bytes_excluding_source(),
        committed_exceptional_outer_arc_control_and_padding_byte_bound::<
            GeneratedAffineResidualGroupPhysicalFrame,
        >()?,
        physical_frame.stats().frame_retained_bytes(),
        committed_exceptional_outer_arc_control_and_padding_byte_bound::<
            GeneratedAffineResidualGroupSolvePlan,
        >()?,
        solve_plan.stats().owner_retained_bytes(),
        size_of::<GeneratedAffineResidualGroupExactSession>(),
        snapshot.database_retained_bytes(),
        snapshot.target_state_combined_retained_byte_envelope(),
        snapshot.event_ledger_retained_bytes(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| total.checked_add(bytes))
    .ok_or(
        CommittedExceptionalFreshSessionBuildError::ResourceCountOverflow {
            resource: "committed exceptional fresh-session visible census",
        },
    )?;
    Ok(CommittedExceptionalFreshSessionCandidate {
        session,
        inherited_source_rows,
        conservative_visible_bytes_excluding_shared_ancestry,
    })
}

/// Conservative overhead for one distinct outer `Arc<T>` allocation when the
/// pointee's owner census already includes `size_of::<T>()` and deep buffers.
/// The two alignment slacks follow the established exact-target envelope; Rust
/// does not expose the allocator's actual Arc header layout.
fn committed_exceptional_outer_arc_control_and_padding_byte_bound<T>()
-> Result<usize, CommittedExceptionalFreshSessionBuildError> {
    let controls = 2usize.checked_mul(size_of::<AtomicUsize>()).ok_or(
        CommittedExceptionalFreshSessionBuildError::ResourceCountOverflow {
            resource: "committed exceptional fresh-session outer Arc controls",
        },
    )?;
    let padding = 2usize
        .checked_mul(align_of::<T>().saturating_sub(1))
        .ok_or(
            CommittedExceptionalFreshSessionBuildError::ResourceCountOverflow {
                resource: "committed exceptional fresh-session outer Arc padding",
            },
        )?;
    controls.checked_add(padding).ok_or(
        CommittedExceptionalFreshSessionBuildError::ResourceCountOverflow {
            resource: "committed exceptional fresh-session outer Arc control and padding",
        },
    )
}
