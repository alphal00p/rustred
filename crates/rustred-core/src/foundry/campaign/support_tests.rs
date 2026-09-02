use crate::foundry::completion::source_discovery::leader_walk::LeaderWalkLimits;
use crate::foundry::completion::source_discovery::{
    RequestedDomainSupportLimits, RequestedDomainSupportProposal, RequestedSupportProposalOrigin,
    RequestedSupportProposalProvenanceInput, try_union_requested_domain_support,
};
use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::error::{FoundryCampaignError, FoundryCampaignSetupStage};
use super::requested::{K6_REQUESTED_DOMAIN_SCOPE_KEY, K6RequestedDomainSpec};
use super::run::try_materialize_requested_domains;

fn sector() -> Mask {
    Mask::try_new([true, true, false]).unwrap()
}

fn proposal(
    stable_scope_key: &str,
    sector: &Mask,
    point: &[u64],
    symbolic_axes: &[usize],
    obligation: &str,
) -> RequestedDomainSupportProposal {
    RequestedDomainSupportProposal::try_new(
        stable_scope_key,
        sector,
        point,
        symbolic_axes,
        &[IntegralShift::try_new([0, 0, 0]).unwrap()],
        RequestedSupportProposalProvenanceInput::new(
            1,
            1,
            1,
            "k6-test-ordering",
            obligation,
            RequestedSupportProposalOrigin::InvolutiveBasisLeader,
        ),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap()
}

fn support(
    sector: &Mask,
    domains: &[(&[u64], &[usize], &str)],
) -> crate::foundry::completion::source_discovery::RequestedDomainSupportUnion {
    try_union_requested_domain_support(
        domains
            .iter()
            .map(|(point, axes, obligation)| {
                proposal(
                    K6_REQUESTED_DOMAIN_SCOPE_KEY,
                    sector,
                    point,
                    axes,
                    obligation,
                )
            })
            .collect(),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap()
}

fn spec(point: &[u64], symbolic_axes: &[usize]) -> K6RequestedDomainSpec {
    K6RequestedDomainSpec::new(point.into(), symbolic_axes.into())
}

fn geometry(
    requests: &[crate::foundry::completion::source_discovery::leader_walk::RequestedDomain],
) -> Vec<(Vec<u64>, Vec<usize>)> {
    requests
        .iter()
        .map(|request| {
            (
                request.point().coordinates().to_vec(),
                request.symbolic_axes().to_vec(),
            )
        })
        .collect()
}

#[test]
fn support_only_domains_are_materialized_without_a_duplicate_explicit_schedule() {
    let sector = sector();
    let support = support(&sector, &[(&[0, 2, 0], &[1], "support-b")]);
    let requests = try_materialize_requested_domains(
        &sector,
        &[],
        Some(&support),
        LeaderWalkLimits::default(),
    )
    .unwrap();

    assert_eq!(geometry(&requests), vec![(vec![0, 2, 0], vec![1])]);
}

#[test]
fn explicit_chronology_precedes_canonical_support_only_domains_in_a_mixed_union() {
    let sector = sector();
    let support = support(
        &sector,
        &[
            (&[0, 2, 0], &[1], "support-b"),
            (&[1, 0, 0], &[0], "duplicate-a"),
        ],
    );
    let explicit = [spec(&[1, 0, 0], &[0]), spec(&[0, 0, 3], &[2])];
    let requests = try_materialize_requested_domains(
        &sector,
        &explicit,
        Some(&support),
        LeaderWalkLimits::default(),
    )
    .unwrap();

    assert_eq!(
        geometry(&requests),
        vec![
            (vec![1, 0, 0], vec![0]),
            (vec![0, 0, 3], vec![2]),
            (vec![0, 2, 0], vec![1]),
        ]
    );
}

#[test]
fn foreign_support_scope_or_sector_is_rejected_before_materialization() {
    let sector = sector();
    let foreign_scope = try_union_requested_domain_support(
        vec![proposal(
            "foreign.scope",
            &sector,
            &[0, 2, 0],
            &[1],
            "foreign-scope",
        )],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        try_materialize_requested_domains(
            &sector,
            &[],
            Some(&foreign_scope),
            LeaderWalkLimits::default(),
        ),
        Err(FoundryCampaignError::Invariant {
            detail: "a K6 requested-support proposal belongs to a foreign stable scope or sector",
        })
    ));

    let foreign_sector = Mask::try_new([true, false, true]).unwrap();
    let foreign_sector_support = support(&foreign_sector, &[(&[0, 0, 2], &[2], "foreign-sector")]);
    assert!(matches!(
        try_materialize_requested_domains(
            &sector,
            &[],
            Some(&foreign_sector_support),
            LeaderWalkLimits::default(),
        ),
        Err(FoundryCampaignError::Invariant {
            detail: "a K6 requested-support proposal belongs to a foreign stable scope or sector",
        })
    ));
}

#[test]
fn combined_domain_and_coordinate_limits_are_checked_before_allocation() {
    let sector = sector();
    let support = support(&sector, &[(&[0, 2, 0], &[1], "support-b")]);
    let explicit = [spec(&[1, 0, 0], &[0])];
    let task_limit = LeaderWalkLimits {
        max_tasks: 1,
        ..LeaderWalkLimits::default()
    };
    assert_eq!(
        try_materialize_requested_domains(&sector, &explicit, Some(&support), task_limit)
            .unwrap_err(),
        FoundryCampaignError::ResourceLimit {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource: "combined explicit and supported K6 requested domains",
            requested: 2,
            limit: 1,
        }
    );

    let coordinate_limit = LeaderWalkLimits {
        max_task_coordinate_cells: 7,
        ..LeaderWalkLimits::default()
    };
    assert_eq!(
        try_materialize_requested_domains(&sector, &explicit, Some(&support), coordinate_limit)
            .unwrap_err(),
        FoundryCampaignError::ResourceLimit {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource: "combined explicit and supported K6 requested-domain coordinate cells",
            requested: 8,
            limit: 7,
        }
    );
}
