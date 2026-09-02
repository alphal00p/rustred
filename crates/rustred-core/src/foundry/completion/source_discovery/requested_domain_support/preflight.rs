use std::mem::size_of;

use super::limits::{
    CANONICALIZATION_WORK, RAW_DOMAINS, RAW_PROVENANCE, RAW_SUPPORT, RAW_SUPPORT_CELLS,
    RETAINED_BYTES, UNIQUE_DOMAINS, UNIQUE_PROVENANCE, UNIQUE_SUPPORT, UNIQUE_SUPPORT_CELLS,
    check_limit, checked_add, checked_mul, checked_sum, logical_sort_work,
};
use super::model::retained_bytes_for_domain;
use super::{
    RequestedDomainSupportError, RequestedDomainSupportLimits, RequestedDomainSupportUnion,
    RequestedDomainSupportUnionCensus,
};

/// Allocation-free description of one trusted canonical proposal shape.
///
/// This seam is intentionally narrower than proposal construction: callers
/// attest that the eventual domains are pairwise distinct, every support is
/// already canonical and duplicate-free, and every proposal carries exactly
/// one distinct provenance record. Only scalar sizes cross this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomainSupportBatchShape {
    stable_scope_key_bytes: usize,
    arity: usize,
    symbolic_axes: usize,
    support_entries: usize,
    ordering_key_bytes: usize,
    obligation_key_bytes: usize,
}

impl RequestedDomainSupportBatchShape {
    pub(crate) const fn new(
        stable_scope_key_bytes: usize,
        arity: usize,
        symbolic_axes: usize,
        support_entries: usize,
        ordering_key_bytes: usize,
        obligation_key_bytes: usize,
    ) -> Self {
        Self {
            stable_scope_key_bytes,
            arity,
            symbolic_axes,
            support_entries,
            ordering_key_bytes,
            obligation_key_bytes,
        }
    }
}

/// Exact scalar admission result for a distinct canonical proposal batch.
///
/// The token grants no planner, source, owner, closure, or artifact authority.
/// It only proves that every atomic proposal and its eventual deterministic
/// union fit the supplied support resource envelope before output allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomainSupportBatchPreflight {
    union_census: RequestedDomainSupportUnionCensus,
}

impl RequestedDomainSupportBatchPreflight {
    pub(crate) const fn union_census(self) -> RequestedDomainSupportUnionCensus {
        self.union_census
    }
}

/// Preflight a trusted batch of pairwise-distinct canonical proposal shapes.
///
/// The arithmetic is shared with atomic proposal admission and canonical
/// union construction. No collection, string, coordinate, support, or scratch
/// allocation occurs here.
pub(crate) fn try_preflight_requested_domain_support_batch(
    shapes: impl ExactSizeIterator<Item = RequestedDomainSupportBatchShape>,
    limits: RequestedDomainSupportLimits,
) -> Result<RequestedDomainSupportBatchPreflight, RequestedDomainSupportError> {
    let domains = shapes.len();
    if domains == 0 {
        return Err(RequestedDomainSupportError::EmptyProposalBatch);
    }
    // Pairwise distinctness and one provenance record per shape make all four
    // domain/provenance counts exactly the iterator length. Reject them before
    // even inspecting the remaining scalar payload shapes.
    check_limit(RAW_DOMAINS, domains, limits.max_raw_domains)?;
    check_limit(UNIQUE_DOMAINS, domains, limits.max_unique_domains)?;
    check_limit(RAW_PROVENANCE, domains, limits.max_raw_provenance_records)?;
    check_limit(
        UNIQUE_PROVENANCE,
        domains,
        limits.max_unique_provenance_records,
    )?;

    let mut seen_domains = 0usize;
    let mut support_entries = 0usize;
    let mut support_cells = 0usize;
    let mut retained_bytes = size_of::<RequestedDomainSupportUnion>();

    for shape in shapes {
        if shape.stable_scope_key_bytes == 0 {
            return Err(RequestedDomainSupportError::EmptyIdentity {
                object: "stable scope key",
            });
        }
        if shape.ordering_key_bytes == 0 {
            return Err(RequestedDomainSupportError::EmptyIdentity {
                object: "ordering key",
            });
        }
        if shape.obligation_key_bytes == 0 {
            return Err(RequestedDomainSupportError::EmptyIdentity {
                object: "obligation key",
            });
        }
        if shape.support_entries == 0 {
            return Err(RequestedDomainSupportError::EmptyParentSupport);
        }
        check_limit("requested-domain arity", shape.arity, limits.max_arity)?;
        if shape.symbolic_axes > shape.arity {
            return Err(RequestedDomainSupportError::Noncanonical {
                object: "requested-domain symbolic axes",
            });
        }

        seen_domains = checked_add(RAW_DOMAINS, seen_domains, 1)?;
        support_entries = checked_add(RAW_SUPPORT, support_entries, shape.support_entries)?;
        let shape_support_cells =
            checked_mul(RAW_SUPPORT_CELLS, shape.support_entries, shape.arity)?;
        support_cells = checked_add(RAW_SUPPORT_CELLS, support_cells, shape_support_cells)?;

        let atomic_canonicalization_work = checked_sum(
            CANONICALIZATION_WORK,
            [
                shape.symbolic_axes,
                shape.support_entries,
                shape.support_entries - 1,
            ],
        )?;
        check_limit(
            CANONICALIZATION_WORK,
            atomic_canonicalization_work,
            limits.max_canonicalization_work,
        )?;
        let atomic_retained_bytes = retained_bytes_for_domain(
            shape.stable_scope_key_bytes,
            shape.arity,
            shape.symbolic_axes,
            shape.support_entries,
            shape_support_cells,
            1,
            shape.ordering_key_bytes,
            shape.obligation_key_bytes,
        )?;
        check_limit(
            RETAINED_BYTES,
            atomic_retained_bytes,
            limits.max_retained_bytes,
        )?;
        // Pairwise-distinct domains make the exact union payload equal to one
        // union record plus every complete atomic proposal payload.
        retained_bytes = checked_add(RETAINED_BYTES, retained_bytes, atomic_retained_bytes)?;
    }

    if seen_domains != domains {
        return Err(RequestedDomainSupportError::Invariant {
            detail: "requested-domain batch shape iterator length changed during preflight",
        });
    }
    check_limit(RAW_SUPPORT, support_entries, limits.max_raw_support_entries)?;
    check_limit(
        UNIQUE_SUPPORT,
        support_entries,
        limits.max_unique_support_entries,
    )?;
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

    // One provenance record belongs to every distinct domain.
    let canonicalization_work = checked_sum(
        CANONICALIZATION_WORK,
        [
            logical_sort_work(domains)?,
            logical_sort_work(domains)?,
            logical_sort_work(support_entries)?,
            domains,
            domains,
            support_entries,
        ],
    )?;
    check_limit(
        CANONICALIZATION_WORK,
        canonicalization_work,
        limits.max_canonicalization_work,
    )?;
    check_limit(RETAINED_BYTES, retained_bytes, limits.max_retained_bytes)?;

    Ok(RequestedDomainSupportBatchPreflight {
        union_census: RequestedDomainSupportUnionCensus {
            raw_domains: domains,
            unique_domains: domains,
            raw_provenance_records: domains,
            unique_provenance_records: domains,
            raw_support_entries: support_entries,
            unique_support_entries: support_entries,
            raw_support_coordinate_cells: support_cells,
            unique_support_coordinate_cells: support_cells,
            canonicalization_work,
            retained_bytes,
        },
    })
}
