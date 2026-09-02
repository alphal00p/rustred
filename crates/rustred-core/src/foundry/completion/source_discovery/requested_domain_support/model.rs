use std::mem::size_of;
use std::sync::Arc;

use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::limits::{
    CANONICALIZATION_WORK, RAW_DOMAINS, RAW_PROVENANCE, RAW_SUPPORT, RAW_SUPPORT_CELLS,
    RETAINED_BYTES, UNIQUE_DOMAINS, UNIQUE_PROVENANCE, UNIQUE_SUPPORT, UNIQUE_SUPPORT_CELLS,
    check_limit, checked_mul, checked_sum, try_copy_slice, try_copy_string, try_vec,
};
use super::{RequestedDomainSupportError, RequestedDomainSupportLimits};

/// Non-authoritative origin class for one support proposal.
///
/// Adding an origin is an explicit review point. No variant can assert that a
/// recurrence, owner cover, or artifact is valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum RequestedSupportProposalOrigin {
    InvolutiveProlongation,
    /// A domain nominated from a row retained in the final autoreduced Janet
    /// basis.  This is deliberately distinct from an outstanding
    /// nonmultiplicative prolongation: neither origin carries owner or closure
    /// authority.
    InvolutiveBasisLeader,
}

/// Borrowed provenance used to construct one bounded proposal atomically.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RequestedSupportProposalProvenanceInput<'a> {
    proposal_schema_revision: u32,
    algorithm_revision: u32,
    basis_revision: u64,
    ordering_key: &'a str,
    obligation_key: &'a str,
    origin: RequestedSupportProposalOrigin,
}

impl<'a> RequestedSupportProposalProvenanceInput<'a> {
    pub(crate) const fn new(
        proposal_schema_revision: u32,
        algorithm_revision: u32,
        basis_revision: u64,
        ordering_key: &'a str,
        obligation_key: &'a str,
        origin: RequestedSupportProposalOrigin,
    ) -> Self {
        Self {
            proposal_schema_revision,
            algorithm_revision,
            basis_revision,
            ordering_key,
            obligation_key,
            origin,
        }
    }
}

/// Detached diagnostic provenance retained in canonical order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RequestedSupportProposalProvenance {
    pub(super) proposal_schema_revision: u32,
    pub(super) algorithm_revision: u32,
    pub(super) basis_revision: u64,
    pub(super) ordering_key: Arc<String>,
    pub(super) obligation_key: Arc<String>,
    pub(super) origin: RequestedSupportProposalOrigin,
}

impl RequestedSupportProposalProvenance {
    pub(crate) const fn proposal_schema_revision(&self) -> u32 {
        self.proposal_schema_revision
    }

    pub(crate) const fn algorithm_revision(&self) -> u32 {
        self.algorithm_revision
    }

    pub(crate) const fn basis_revision(&self) -> u64 {
        self.basis_revision
    }

    pub(crate) fn ordering_key(&self) -> &str {
        self.ordering_key.as_str()
    }

    pub(crate) fn obligation_key(&self) -> &str {
        self.obligation_key.as_str()
    }

    pub(crate) const fn origin(&self) -> RequestedSupportProposalOrigin {
        self.origin
    }
}

/// Stable semantic identity shared by a request and every residual task
/// replanned from it. Requested ordinals and residual parent boxes are
/// intentionally absent.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RequestedDomainSemanticKey {
    pub(super) stable_scope_key: Arc<String>,
    pub(super) sector: Mask,
    pub(super) point: Arc<Vec<u64>>,
    pub(super) symbolic_axes: Arc<Vec<usize>>,
}

impl RequestedDomainSemanticKey {
    pub(crate) fn stable_scope_key(&self) -> &str {
        self.stable_scope_key.as_str()
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) fn point(&self) -> &[u64] {
        self.point.as_slice()
    }

    pub(crate) fn symbolic_axes(&self) -> &[usize] {
        self.symbolic_axes.as_slice()
    }
}

/// Per-domain accounting retained beside a canonical support proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomainSupportCensus {
    pub(super) contributing_proposals: usize,
    pub(super) provenance_records: usize,
    pub(super) raw_support_entries: usize,
    pub(super) unique_support_entries: usize,
    pub(super) raw_support_coordinate_cells: usize,
    pub(super) unique_support_coordinate_cells: usize,
    pub(super) canonicalization_work: usize,
    pub(super) retained_bytes: usize,
}

impl RequestedDomainSupportCensus {
    pub(crate) const fn contributing_proposals(self) -> usize {
        self.contributing_proposals
    }

    pub(crate) const fn provenance_records(self) -> usize {
        self.provenance_records
    }

    pub(crate) const fn raw_support_entries(self) -> usize {
        self.raw_support_entries
    }

    pub(crate) const fn unique_support_entries(self) -> usize {
        self.unique_support_entries
    }

    pub(crate) const fn raw_support_coordinate_cells(self) -> usize {
        self.raw_support_coordinate_cells
    }

    pub(crate) const fn unique_support_coordinate_cells(self) -> usize {
        self.unique_support_coordinate_cells
    }

    pub(crate) const fn canonicalization_work(self) -> usize {
        self.canonicalization_work
    }

    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
}

/// One authority-minimal requested-domain parent-support proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomainSupportProposal {
    pub(super) domain: RequestedDomainSemanticKey,
    pub(super) parent_support: Box<[IntegralShift]>,
    pub(super) provenance: Box<[RequestedSupportProposalProvenance]>,
    pub(super) census: RequestedDomainSupportCensus,
}

impl RequestedDomainSupportProposal {
    /// Validate and retain one atomic proposal. Every variable-sized output
    /// allocation occurs only after the complete proposal has passed its
    /// arity, canonicality, work, cell, and retained-byte preflight.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        stable_scope_key: &str,
        sector: &Mask,
        point: &[u64],
        symbolic_axes: &[usize],
        parent_support: &[IntegralShift],
        provenance: RequestedSupportProposalProvenanceInput<'_>,
        limits: RequestedDomainSupportLimits,
    ) -> Result<Self, RequestedDomainSupportError> {
        if stable_scope_key.is_empty() {
            return Err(RequestedDomainSupportError::EmptyIdentity {
                object: "stable scope key",
            });
        }
        if provenance.ordering_key.is_empty() {
            return Err(RequestedDomainSupportError::EmptyIdentity {
                object: "ordering key",
            });
        }
        if provenance.obligation_key.is_empty() {
            return Err(RequestedDomainSupportError::EmptyIdentity {
                object: "obligation key",
            });
        }
        let arity = sector.arity();
        check_limit("requested-domain arity", arity, limits.max_arity)?;
        require_arity("requested-domain point", arity, point.len())?;
        if parent_support.is_empty() {
            return Err(RequestedDomainSupportError::EmptyParentSupport);
        }

        // Every limit derivable from slice lengths is decided before either
        // untrusted slice is inspected. Besides preserving atomic admission,
        // this keeps malformed oversized inputs from consuming uncharged
        // linear validation work.
        check_limit(RAW_DOMAINS, 1, limits.max_raw_domains)?;
        check_limit(UNIQUE_DOMAINS, 1, limits.max_unique_domains)?;
        check_limit(RAW_PROVENANCE, 1, limits.max_raw_provenance_records)?;
        check_limit(UNIQUE_PROVENANCE, 1, limits.max_unique_provenance_records)?;
        check_limit(
            RAW_SUPPORT,
            parent_support.len(),
            limits.max_raw_support_entries,
        )?;
        check_limit(
            UNIQUE_SUPPORT,
            parent_support.len(),
            limits.max_unique_support_entries,
        )?;
        let support_cells = checked_mul(RAW_SUPPORT_CELLS, parent_support.len(), arity)?;
        check_limit(
            RAW_SUPPORT_CELLS,
            support_cells,
            limits.max_raw_support_coordinate_cells,
        )?;
        check_limit(
            UNIQUE_SUPPORT_CELLS,
            support_cells,
            limits.max_unique_support_coordinate_cells,
        )?;
        // Symbolic-axis validation performs at most one logical inspection
        // per axis (`windows(2)` plus the final range check). Support
        // validation inspects every arity and every adjacent ordering pair.
        // Nonempty support was established above, so the subtraction is
        // exact and cannot underflow.
        let canonicalization_work = checked_sum(
            CANONICALIZATION_WORK,
            [
                symbolic_axes.len(),
                parent_support.len(),
                parent_support.len() - 1,
            ],
        )?;
        check_limit(
            CANONICALIZATION_WORK,
            canonicalization_work,
            limits.max_canonicalization_work,
        )?;
        let retained_bytes = retained_bytes_for_domain(
            stable_scope_key.len(),
            arity,
            symbolic_axes.len(),
            parent_support.len(),
            support_cells,
            1,
            provenance.ordering_key.len(),
            provenance.obligation_key.len(),
        )?;
        check_limit(RETAINED_BYTES, retained_bytes, limits.max_retained_bytes)?;

        if symbolic_axes.windows(2).any(|pair| pair[0] >= pair[1])
            || symbolic_axes.last().is_some_and(|&axis| axis >= arity)
        {
            return Err(RequestedDomainSupportError::Noncanonical {
                object: "requested-domain symbolic axes",
            });
        }
        for shift in parent_support {
            require_arity("requested-domain parent-support shift", arity, shift.len())?;
        }
        if parent_support.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(RequestedDomainSupportError::Noncanonical {
                object: "requested-domain parent support",
            });
        }

        let stable_scope_key = Arc::new(try_copy_string(
            "requested-domain stable scope key bytes",
            stable_scope_key,
        )?);
        let point = Arc::new(try_copy_slice("requested-domain point coordinates", point)?);
        let symbolic_axes = Arc::new(try_copy_slice(
            "requested-domain symbolic axes",
            symbolic_axes,
        )?);
        let parent_support =
            try_copy_slice("requested-domain parent-support entries", parent_support)?
                .into_boxed_slice();
        let support_entries = parent_support.len();
        let owned_provenance = RequestedSupportProposalProvenance {
            proposal_schema_revision: provenance.proposal_schema_revision,
            algorithm_revision: provenance.algorithm_revision,
            basis_revision: provenance.basis_revision,
            ordering_key: Arc::new(try_copy_string(
                "requested-domain ordering-key bytes",
                provenance.ordering_key,
            )?),
            obligation_key: Arc::new(try_copy_string(
                "requested-domain obligation-key bytes",
                provenance.obligation_key,
            )?),
            origin: provenance.origin,
        };
        let mut retained_provenance = try_vec(UNIQUE_PROVENANCE, 1)?;
        retained_provenance.push(owned_provenance);
        Ok(Self {
            domain: RequestedDomainSemanticKey {
                stable_scope_key,
                sector: sector.clone(),
                point,
                symbolic_axes,
            },
            parent_support,
            provenance: retained_provenance.into_boxed_slice(),
            census: RequestedDomainSupportCensus {
                contributing_proposals: 1,
                provenance_records: 1,
                raw_support_entries: support_entries,
                unique_support_entries: support_entries,
                raw_support_coordinate_cells: support_cells,
                unique_support_coordinate_cells: support_cells,
                canonicalization_work,
                retained_bytes,
            },
        })
    }

    pub(crate) const fn domain(&self) -> &RequestedDomainSemanticKey {
        &self.domain
    }

    pub(crate) fn parent_support(&self) -> &[IntegralShift] {
        &self.parent_support
    }

    pub(crate) fn provenance(&self) -> &[RequestedSupportProposalProvenance] {
        &self.provenance
    }

    pub(crate) const fn census(&self) -> RequestedDomainSupportCensus {
        self.census
    }
}

pub(super) fn retained_bytes_for_domain(
    stable_scope_key_bytes: usize,
    arity: usize,
    symbolic_axes: usize,
    support_entries: usize,
    support_cells: usize,
    provenance_records: usize,
    ordering_key_bytes: usize,
    obligation_key_bytes: usize,
) -> Result<usize, RequestedDomainSupportError> {
    checked_sum(
        RETAINED_BYTES,
        [
            size_of::<RequestedDomainSupportProposal>(),
            // Arc control blocks are deliberately excluded, but each
            // Arc-owned heap value and its logical payload are retained.
            size_of::<String>(),
            stable_scope_key_bytes,
            size_of::<Vec<bool>>(),
            arity,
            size_of::<Vec<u64>>(),
            checked_mul(RETAINED_BYTES, arity, size_of::<u64>())?,
            size_of::<Vec<usize>>(),
            checked_mul(RETAINED_BYTES, symbolic_axes, size_of::<usize>())?,
            checked_mul(RETAINED_BYTES, support_entries, size_of::<IntegralShift>())?,
            checked_mul(RETAINED_BYTES, support_entries, size_of::<Vec<i64>>())?,
            checked_mul(RETAINED_BYTES, support_cells, size_of::<i64>())?,
            checked_mul(
                RETAINED_BYTES,
                provenance_records,
                size_of::<RequestedSupportProposalProvenance>(),
            )?,
            checked_mul(
                RETAINED_BYTES,
                provenance_records,
                checked_mul(RETAINED_BYTES, 2, size_of::<String>())?,
            )?,
            ordering_key_bytes,
            obligation_key_bytes,
        ],
    )
}

fn require_arity(
    object: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), RequestedDomainSupportError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RequestedDomainSupportError::WrongArity {
            object,
            expected,
            actual,
        })
    }
}
