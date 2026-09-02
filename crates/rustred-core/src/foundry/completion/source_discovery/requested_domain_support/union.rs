use std::mem::size_of;

use crate::identity::IntegralShift;

use super::limits::{
    CANONICALIZATION_WORK, RAW_DOMAINS, RAW_PROVENANCE, RAW_SUPPORT, RAW_SUPPORT_CELLS,
    RETAINED_BYTES, UNIQUE_DOMAINS, UNIQUE_PROVENANCE, UNIQUE_SUPPORT, UNIQUE_SUPPORT_CELLS,
    check_limit, checked_add, checked_mul, checked_sum, logical_sort_work, try_vec,
};
use super::model::retained_bytes_for_domain;
use super::{
    RequestedDomainSemanticKey, RequestedDomainSupportCensus, RequestedDomainSupportError,
    RequestedDomainSupportLimits, RequestedDomainSupportProposal,
    RequestedSupportProposalProvenance,
};

/// Aggregate accounting for one all-or-nothing canonical union.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomainSupportUnionCensus {
    pub(super) raw_domains: usize,
    pub(super) unique_domains: usize,
    pub(super) raw_provenance_records: usize,
    pub(super) unique_provenance_records: usize,
    pub(super) raw_support_entries: usize,
    pub(super) unique_support_entries: usize,
    pub(super) raw_support_coordinate_cells: usize,
    pub(super) unique_support_coordinate_cells: usize,
    pub(super) canonicalization_work: usize,
    pub(super) retained_bytes: usize,
}

impl RequestedDomainSupportUnionCensus {
    pub(crate) const fn raw_domains(self) -> usize {
        self.raw_domains
    }

    pub(crate) const fn unique_domains(self) -> usize {
        self.unique_domains
    }

    pub(crate) const fn raw_provenance_records(self) -> usize {
        self.raw_provenance_records
    }

    pub(crate) const fn unique_provenance_records(self) -> usize {
        self.unique_provenance_records
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

/// Canonical unique-domain support sidecar. It remains detached from planner
/// ordinals, ledger snapshots, replay results, and all owner authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequestedDomainSupportUnion {
    pub(super) proposals: Box<[RequestedDomainSupportProposal]>,
    pub(super) census: RequestedDomainSupportUnionCensus,
}

impl RequestedDomainSupportUnion {
    pub(crate) fn proposals(&self) -> &[RequestedDomainSupportProposal] {
        &self.proposals
    }

    pub(crate) const fn census(&self) -> RequestedDomainSupportUnionCensus {
        self.census
    }
}

#[derive(Clone, Copy)]
struct SupportRef<'a> {
    domain: &'a RequestedDomainSemanticKey,
    shift: &'a IntegralShift,
}

#[derive(Clone, Copy)]
struct ProvenanceRef<'a> {
    domain: &'a RequestedDomainSemanticKey,
    provenance: &'a RequestedSupportProposalProvenance,
}

/// Deterministically union every support proposal sharing the same semantic
/// requested domain. Input and worker completion order cannot affect output.
///
/// Raw limits and the complete logical canonicalization envelope are checked
/// before scratch allocation or sorting. Unique and retained limits are then
/// checked before any canonical output allocation, so every error returns no
/// partial result.
pub(crate) fn try_union_requested_domain_support(
    mut proposals: Vec<RequestedDomainSupportProposal>,
    limits: RequestedDomainSupportLimits,
) -> Result<RequestedDomainSupportUnion, RequestedDomainSupportError> {
    if proposals.is_empty() {
        return Err(RequestedDomainSupportError::EmptyProposalBatch);
    }
    let raw_domains = proposals.len();
    check_limit(RAW_DOMAINS, raw_domains, limits.max_raw_domains)?;

    let (raw_provenance_records, raw_support_entries, raw_support_coordinate_cells) =
        preflight_raw_payload(&proposals, limits)?;
    let canonicalization_work = checked_sum(
        CANONICALIZATION_WORK,
        [
            logical_sort_work(raw_domains)?,
            logical_sort_work(raw_provenance_records)?,
            logical_sort_work(raw_support_entries)?,
            raw_domains,
            raw_provenance_records,
            raw_support_entries,
        ],
    )?;
    check_limit(
        CANONICALIZATION_WORK,
        canonicalization_work,
        limits.max_canonicalization_work,
    )?;

    proposals.sort_unstable_by(|left, right| {
        left.domain
            .cmp(&right.domain)
            .then_with(|| left.provenance.cmp(&right.provenance))
            .then_with(|| left.parent_support.cmp(&right.parent_support))
    });
    let unique_domains = proposals
        .iter()
        .enumerate()
        .filter(|&(ordinal, proposal)| {
            ordinal == 0 || proposals[ordinal - 1].domain != proposal.domain
        })
        .count();
    check_limit(UNIQUE_DOMAINS, unique_domains, limits.max_unique_domains)?;

    let mut support_refs = try_vec(RAW_SUPPORT, raw_support_entries)?;
    let mut provenance_refs = try_vec(RAW_PROVENANCE, raw_provenance_records)?;
    for proposal in &proposals {
        support_refs.extend(proposal.parent_support.iter().map(|shift| SupportRef {
            domain: &proposal.domain,
            shift,
        }));
        provenance_refs.extend(proposal.provenance.iter().map(|provenance| ProvenanceRef {
            domain: &proposal.domain,
            provenance,
        }));
    }
    support_refs.sort_unstable_by(|left, right| {
        left.domain
            .cmp(right.domain)
            .then_with(|| left.shift.cmp(right.shift))
    });
    provenance_refs.sort_unstable_by(|left, right| {
        left.domain
            .cmp(right.domain)
            .then_with(|| left.provenance.cmp(right.provenance))
    });

    let unique_support_entries = count_unique_support(&support_refs);
    check_limit(
        UNIQUE_SUPPORT,
        unique_support_entries,
        limits.max_unique_support_entries,
    )?;
    let unique_support_coordinate_cells = unique_support_cells(&support_refs)?;
    check_limit(
        UNIQUE_SUPPORT_CELLS,
        unique_support_coordinate_cells,
        limits.max_unique_support_coordinate_cells,
    )?;
    let unique_provenance_records = count_unique_provenance(&provenance_refs);
    check_limit(
        UNIQUE_PROVENANCE,
        unique_provenance_records,
        limits.max_unique_provenance_records,
    )?;

    let retained_bytes =
        retained_bytes_for_union(&proposals, &support_refs, &provenance_refs, unique_domains)?;
    check_limit(RETAINED_BYTES, retained_bytes, limits.max_retained_bytes)?;

    let canonical =
        build_canonical_output(&proposals, &support_refs, &provenance_refs, unique_domains)?;
    Ok(RequestedDomainSupportUnion {
        proposals: canonical.into_boxed_slice(),
        census: RequestedDomainSupportUnionCensus {
            raw_domains,
            unique_domains,
            raw_provenance_records,
            unique_provenance_records,
            raw_support_entries,
            unique_support_entries,
            raw_support_coordinate_cells,
            unique_support_coordinate_cells,
            canonicalization_work,
            retained_bytes,
        },
    })
}

fn preflight_raw_payload(
    proposals: &[RequestedDomainSupportProposal],
    limits: RequestedDomainSupportLimits,
) -> Result<(usize, usize, usize), RequestedDomainSupportError> {
    let mut provenance_records = 0usize;
    let mut support_entries = 0usize;
    let mut support_cells = 0usize;
    for proposal in proposals {
        let arity = proposal.domain.sector().arity();
        check_limit("requested-domain arity", arity, limits.max_arity)?;
        if proposal.parent_support.is_empty() || proposal.provenance.is_empty() {
            return Err(RequestedDomainSupportError::Invariant {
                detail: "a retained support proposal has an empty payload",
            });
        }
        provenance_records = checked_add(
            RAW_PROVENANCE,
            provenance_records,
            proposal.provenance.len(),
        )?;
        support_entries = checked_add(RAW_SUPPORT, support_entries, proposal.parent_support.len())?;
        support_cells = checked_add(
            RAW_SUPPORT_CELLS,
            support_cells,
            checked_mul(RAW_SUPPORT_CELLS, proposal.parent_support.len(), arity)?,
        )?;
    }
    check_limit(
        RAW_PROVENANCE,
        provenance_records,
        limits.max_raw_provenance_records,
    )?;
    check_limit(RAW_SUPPORT, support_entries, limits.max_raw_support_entries)?;
    check_limit(
        RAW_SUPPORT_CELLS,
        support_cells,
        limits.max_raw_support_coordinate_cells,
    )?;
    Ok((provenance_records, support_entries, support_cells))
}

fn build_canonical_output(
    proposals: &[RequestedDomainSupportProposal],
    support_refs: &[SupportRef<'_>],
    provenance_refs: &[ProvenanceRef<'_>],
    unique_domains: usize,
) -> Result<Vec<RequestedDomainSupportProposal>, RequestedDomainSupportError> {
    let mut canonical = try_vec(UNIQUE_DOMAINS, unique_domains)?;
    let mut proposal_start = 0usize;
    let mut support_start = 0usize;
    let mut provenance_start = 0usize;
    while proposal_start < proposals.len() {
        let domain = &proposals[proposal_start].domain;
        let proposal_end = end_of_proposal_group(proposals, proposal_start);
        let support_end = end_of_support_group(support_refs, support_start, domain);
        let provenance_end = end_of_provenance_group(provenance_refs, provenance_start, domain);
        let group_support = &support_refs[support_start..support_end];
        let group_provenance = &provenance_refs[provenance_start..provenance_end];

        let group_unique_support = count_unique_support(group_support);
        let group_unique_provenance = count_unique_provenance(group_provenance);
        let mut parent_support = try_vec(UNIQUE_SUPPORT, group_unique_support)?;
        for (ordinal, current) in group_support.iter().enumerate() {
            if ordinal == 0 || group_support[ordinal - 1].shift != current.shift {
                parent_support.push(current.shift.clone());
            }
        }
        let mut provenance = try_vec(UNIQUE_PROVENANCE, group_unique_provenance)?;
        for (ordinal, current) in group_provenance.iter().enumerate() {
            if ordinal == 0 || group_provenance[ordinal - 1].provenance != current.provenance {
                provenance.push(current.provenance.clone());
            }
        }

        let group = &proposals[proposal_start..proposal_end];
        let raw_group_support = group.iter().try_fold(0usize, |count, proposal| {
            checked_add(RAW_SUPPORT, count, proposal.parent_support.len())
        })?;
        let raw_group_cells = checked_mul(
            RAW_SUPPORT_CELLS,
            raw_group_support,
            domain.sector().arity(),
        )?;
        let unique_group_cells = checked_mul(
            UNIQUE_SUPPORT_CELLS,
            parent_support.len(),
            domain.sector().arity(),
        )?;
        let group_canonicalization_work = checked_sum(
            CANONICALIZATION_WORK,
            [
                logical_sort_work(group.len())?,
                logical_sort_work(group_provenance.len())?,
                logical_sort_work(group_support.len())?,
                group.len(),
                group_provenance.len(),
                group_support.len(),
            ],
        )?;
        let ordering_bytes = provenance.iter().try_fold(0usize, |bytes, item| {
            checked_add(RETAINED_BYTES, bytes, item.ordering_key().len())
        })?;
        let obligation_bytes = provenance.iter().try_fold(0usize, |bytes, item| {
            checked_add(RETAINED_BYTES, bytes, item.obligation_key().len())
        })?;
        let group_retained_bytes = retained_bytes_for_domain(
            domain.stable_scope_key().len(),
            domain.sector().arity(),
            domain.symbolic_axes().len(),
            parent_support.len(),
            unique_group_cells,
            provenance.len(),
            ordering_bytes,
            obligation_bytes,
        )?;
        canonical.push(RequestedDomainSupportProposal {
            domain: domain.clone(),
            parent_support: parent_support.into_boxed_slice(),
            provenance: provenance.into_boxed_slice(),
            census: RequestedDomainSupportCensus {
                contributing_proposals: group.len(),
                provenance_records: group_unique_provenance,
                raw_support_entries: raw_group_support,
                unique_support_entries: group_unique_support,
                raw_support_coordinate_cells: raw_group_cells,
                unique_support_coordinate_cells: unique_group_cells,
                canonicalization_work: group_canonicalization_work,
                retained_bytes: group_retained_bytes,
            },
        });
        proposal_start = proposal_end;
        support_start = support_end;
        provenance_start = provenance_end;
    }
    if canonical.len() != unique_domains
        || support_start != support_refs.len()
        || provenance_start != provenance_refs.len()
    {
        return Err(RequestedDomainSupportError::Invariant {
            detail: "canonical requested-domain support union lost an input group",
        });
    }
    Ok(canonical)
}

fn end_of_proposal_group(proposals: &[RequestedDomainSupportProposal], start: usize) -> usize {
    let domain = &proposals[start].domain;
    let mut end = start + 1;
    while end < proposals.len() && proposals[end].domain == *domain {
        end += 1;
    }
    end
}

fn end_of_support_group(
    support: &[SupportRef<'_>],
    start: usize,
    domain: &RequestedDomainSemanticKey,
) -> usize {
    let mut end = start;
    while end < support.len() && support[end].domain == domain {
        end += 1;
    }
    end
}

fn end_of_provenance_group(
    provenance: &[ProvenanceRef<'_>],
    start: usize,
    domain: &RequestedDomainSemanticKey,
) -> usize {
    let mut end = start;
    while end < provenance.len() && provenance[end].domain == domain {
        end += 1;
    }
    end
}

fn count_unique_support(group: &[SupportRef<'_>]) -> usize {
    group
        .iter()
        .enumerate()
        .filter(|&(ordinal, current)| {
            ordinal == 0
                || group[ordinal - 1].domain != current.domain
                || group[ordinal - 1].shift != current.shift
        })
        .count()
}

fn unique_support_cells(support: &[SupportRef<'_>]) -> Result<usize, RequestedDomainSupportError> {
    support
        .iter()
        .enumerate()
        .filter(|&(ordinal, current)| {
            ordinal == 0
                || support[ordinal - 1].domain != current.domain
                || support[ordinal - 1].shift != current.shift
        })
        .try_fold(0usize, |cells, (_, current)| {
            checked_add(UNIQUE_SUPPORT_CELLS, cells, current.shift.len())
        })
}

fn count_unique_provenance(group: &[ProvenanceRef<'_>]) -> usize {
    group
        .iter()
        .enumerate()
        .filter(|&(ordinal, current)| {
            ordinal == 0
                || group[ordinal - 1].domain != current.domain
                || group[ordinal - 1].provenance != current.provenance
        })
        .count()
}

fn retained_bytes_for_union(
    proposals: &[RequestedDomainSupportProposal],
    support: &[SupportRef<'_>],
    provenance: &[ProvenanceRef<'_>],
    unique_domains: usize,
) -> Result<usize, RequestedDomainSupportError> {
    let mut bytes = size_of::<RequestedDomainSupportUnion>();
    bytes = checked_add(
        RETAINED_BYTES,
        bytes,
        checked_mul(
            RETAINED_BYTES,
            unique_domains,
            size_of::<RequestedDomainSupportProposal>(),
        )?,
    )?;
    for (ordinal, proposal) in proposals.iter().enumerate() {
        if ordinal != 0 && proposals[ordinal - 1].domain == proposal.domain {
            continue;
        }
        let domain = &proposal.domain;
        // Arc control blocks are excluded from this logical retained-byte
        // census. The fixed heap value owned behind every Arc is not.
        bytes = checked_add(RETAINED_BYTES, bytes, size_of::<String>())?;
        bytes = checked_add(RETAINED_BYTES, bytes, domain.stable_scope_key().len())?;
        bytes = checked_add(RETAINED_BYTES, bytes, size_of::<Vec<bool>>())?;
        bytes = checked_add(RETAINED_BYTES, bytes, domain.sector().arity())?;
        bytes = checked_add(RETAINED_BYTES, bytes, size_of::<Vec<u64>>())?;
        bytes = checked_add(
            RETAINED_BYTES,
            bytes,
            checked_mul(RETAINED_BYTES, domain.point().len(), size_of::<u64>())?,
        )?;
        bytes = checked_add(RETAINED_BYTES, bytes, size_of::<Vec<usize>>())?;
        bytes = checked_add(
            RETAINED_BYTES,
            bytes,
            checked_mul(
                RETAINED_BYTES,
                domain.symbolic_axes().len(),
                size_of::<usize>(),
            )?,
        )?;
    }
    for (ordinal, current) in support.iter().enumerate() {
        if ordinal != 0
            && support[ordinal - 1].domain == current.domain
            && support[ordinal - 1].shift == current.shift
        {
            continue;
        }
        bytes = checked_add(RETAINED_BYTES, bytes, size_of::<IntegralShift>())?;
        bytes = checked_add(RETAINED_BYTES, bytes, size_of::<Vec<i64>>())?;
        bytes = checked_add(
            RETAINED_BYTES,
            bytes,
            checked_mul(RETAINED_BYTES, current.shift.len(), size_of::<i64>())?,
        )?;
    }
    for (ordinal, current) in provenance.iter().enumerate() {
        if ordinal != 0
            && provenance[ordinal - 1].domain == current.domain
            && provenance[ordinal - 1].provenance == current.provenance
        {
            continue;
        }
        bytes = checked_add(
            RETAINED_BYTES,
            bytes,
            size_of::<RequestedSupportProposalProvenance>(),
        )?;
        bytes = checked_add(
            RETAINED_BYTES,
            bytes,
            checked_mul(RETAINED_BYTES, 2, size_of::<String>())?,
        )?;
        bytes = checked_add(
            RETAINED_BYTES,
            bytes,
            current.provenance.ordering_key().len(),
        )?;
        bytes = checked_add(
            RETAINED_BYTES,
            bytes,
            current.provenance.obligation_key().len(),
        )?;
    }
    Ok(bytes)
}
