use std::mem::size_of;

use crate::identity::IntegralShift;
use crate::sector::Mask;

use super::limits::{
    CANONICALIZATION_WORK, RAW_DOMAINS, RAW_PROVENANCE, RAW_SUPPORT, RAW_SUPPORT_CELLS,
    RETAINED_BYTES, UNIQUE_DOMAINS, UNIQUE_PROVENANCE, UNIQUE_SUPPORT, UNIQUE_SUPPORT_CELLS,
};
use super::*;

fn shift(values: &[i64]) -> IntegralShift {
    IntegralShift::try_new(values.iter().copied()).unwrap()
}

fn proposal(
    scope: &str,
    point: &[u64],
    support: &[&[i64]],
    obligation: &str,
) -> RequestedDomainSupportProposal {
    let sector = Mask::try_new([true, false]).unwrap();
    let support = support
        .iter()
        .map(|values| shift(values))
        .collect::<Vec<_>>();
    RequestedDomainSupportProposal::try_new(
        scope,
        &sector,
        point,
        &[0],
        &support,
        RequestedSupportProposalProvenanceInput::new(
            1,
            7,
            11,
            "ore-order-v1",
            obligation,
            RequestedSupportProposalOrigin::InvolutiveProlongation,
        ),
        RequestedDomainSupportLimits::default(),
    )
    .unwrap()
}

#[test]
fn support_payload_exhaustively_contains_only_geometry_support_provenance_and_census() {
    let proposal = proposal("scope", &[2, 0], &[&[0_i64, 0]], "obligation");
    let RequestedDomainSupportProposal {
        domain,
        parent_support,
        provenance,
        census,
    } = proposal;
    let RequestedDomainSemanticKey {
        stable_scope_key,
        sector,
        point,
        symbolic_axes,
    } = domain;
    let [provenance] = provenance.as_ref() else {
        panic!("atomic proposal should retain one provenance record");
    };
    let RequestedSupportProposalProvenance {
        proposal_schema_revision,
        algorithm_revision,
        basis_revision,
        ordering_key,
        obligation_key,
        origin,
    } = provenance;
    let RequestedDomainSupportCensus {
        contributing_proposals,
        provenance_records,
        raw_support_entries,
        unique_support_entries,
        raw_support_coordinate_cells,
        unique_support_coordinate_cells,
        canonicalization_work,
        retained_bytes,
    } = census;
    assert_eq!(stable_scope_key.as_str(), "scope");
    assert_eq!(sector.active_bits(), &[true, false]);
    assert_eq!(point.as_slice(), &[2, 0]);
    assert_eq!(symbolic_axes.as_slice(), &[0]);
    assert_eq!(parent_support[0].values(), &[0, 0]);
    assert_eq!(*proposal_schema_revision, 1);
    assert_eq!(*algorithm_revision, 7);
    assert_eq!(*basis_revision, 11);
    assert_eq!(ordering_key.as_str(), "ore-order-v1");
    assert_eq!(obligation_key.as_str(), "obligation");
    assert_eq!(
        *origin,
        RequestedSupportProposalOrigin::InvolutiveProlongation
    );
    assert_eq!(contributing_proposals, 1);
    assert_eq!(provenance_records, 1);
    assert_eq!(raw_support_entries, 1);
    assert_eq!(unique_support_entries, 1);
    assert_eq!(raw_support_coordinate_cells, 2);
    assert_eq!(unique_support_coordinate_cells, 2);
    assert_eq!(canonicalization_work, 2);
    assert_eq!(retained_bytes, expected_atomic_fixture_retained_bytes());
    match origin {
        RequestedSupportProposalOrigin::InvolutiveProlongation => {}
        RequestedSupportProposalOrigin::InvolutiveBasisLeader => {
            panic!("fixture unexpectedly changed proposal origin")
        }
    }
}

#[test]
fn every_input_order_produces_the_same_canonical_union() {
    let permutations = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let make = || {
        [
            proposal("beta", &[3, 0], &[&[-1_i64, 0]], "beta"),
            proposal("alpha", &[2, 0], &[&[0_i64, 0]], "alpha-zero"),
            proposal("alpha", &[2, 0], &[&[1_i64, 0]], "alpha-one"),
        ]
    };
    let mut expected = None;
    for permutation in permutations {
        let mut source = make().into_iter().map(Some).collect::<Vec<_>>();
        let input = permutation
            .into_iter()
            .map(|ordinal| source[ordinal].take().unwrap())
            .collect();
        let actual =
            try_union_requested_domain_support(input, RequestedDomainSupportLimits::default())
                .unwrap();
        if let Some(expected) = &expected {
            assert_eq!(&actual, expected);
        } else {
            expected = Some(actual);
        }
    }
    let output = expected.unwrap();
    assert_eq!(output.proposals()[0].domain().stable_scope_key(), "alpha");
    assert_eq!(output.proposals()[1].domain().stable_scope_key(), "beta");
}

#[test]
fn disjoint_same_domain_support_is_preserved_as_a_sorted_union() {
    let output = try_union_requested_domain_support(
        vec![
            proposal("scope", &[2, 0], &[&[-1_i64, 0], &[1_i64, 0]], "left"),
            proposal("scope", &[2, 0], &[&[0_i64, 0], &[2_i64, 0]], "right"),
        ],
        RequestedDomainSupportLimits::default(),
    )
    .unwrap();
    assert_eq!(output.proposals().len(), 1);
    let merged = &output.proposals()[0];
    assert_eq!(
        merged
            .parent_support()
            .iter()
            .map(IntegralShift::values)
            .collect::<Vec<_>>(),
        vec![&[-1_i64, 0][..], &[0_i64, 0], &[1_i64, 0], &[2_i64, 0]]
    );
    assert_eq!(merged.provenance().len(), 2);
    assert_eq!(merged.census().contributing_proposals(), 2);
    assert_eq!(merged.census().raw_support_entries(), 4);
    assert_eq!(merged.census().unique_support_entries(), 4);
}

#[test]
fn wrong_arity_and_noncanonical_support_are_rejected() {
    let sector = Mask::try_new([true, false]).unwrap();
    let provenance = RequestedSupportProposalProvenanceInput::new(
        1,
        1,
        0,
        "order",
        "obligation",
        RequestedSupportProposalOrigin::InvolutiveProlongation,
    );
    let wrong = [shift(&[0])];
    assert!(matches!(
        RequestedDomainSupportProposal::try_new(
            "scope",
            &sector,
            &[1, 0],
            &[0],
            &wrong,
            provenance,
            RequestedDomainSupportLimits::default(),
        ),
        Err(RequestedDomainSupportError::WrongArity {
            object: "requested-domain parent-support shift",
            expected: 2,
            actual: 1,
        })
    ));
    let duplicate = [shift(&[0, 0]), shift(&[0, 0])];
    assert!(matches!(
        RequestedDomainSupportProposal::try_new(
            "scope",
            &sector,
            &[1, 0],
            &[0],
            &duplicate,
            provenance,
            RequestedDomainSupportLimits::default(),
        ),
        Err(RequestedDomainSupportError::Noncanonical {
            object: "requested-domain parent support",
        })
    ));
    let descending = [shift(&[1, 0]), shift(&[0, 0])];
    assert!(matches!(
        RequestedDomainSupportProposal::try_new(
            "scope",
            &sector,
            &[1, 0],
            &[0],
            &descending,
            provenance,
            RequestedDomainSupportLimits::default(),
        ),
        Err(RequestedDomainSupportError::Noncanonical {
            object: "requested-domain parent support",
        })
    ));
}

#[test]
fn atomic_length_limits_precede_untrusted_slice_scans() {
    let sector = Mask::try_new([true, false]).unwrap();
    let provenance = RequestedSupportProposalProvenanceInput::new(
        1,
        1,
        0,
        "order",
        "obligation",
        RequestedSupportProposalOrigin::InvolutiveProlongation,
    );
    let malformed_support = [shift(&[0]), shift(&[0])];

    let mut raw_limits = RequestedDomainSupportLimits::default();
    raw_limits.max_raw_support_entries = 1;
    assert!(matches!(
        RequestedDomainSupportProposal::try_new(
            "scope",
            &sector,
            &[1, 0],
            &[1, 0],
            &malformed_support,
            provenance,
            raw_limits,
        ),
        Err(RequestedDomainSupportError::ResourceLimit {
            resource: RAW_SUPPORT,
            requested: 2,
            limit: 1,
        })
    ));

    let mut cell_limits = RequestedDomainSupportLimits::default();
    cell_limits.max_raw_support_coordinate_cells = 3;
    assert!(matches!(
        RequestedDomainSupportProposal::try_new(
            "scope",
            &sector,
            &[1, 0],
            &[1, 0],
            &malformed_support,
            provenance,
            cell_limits,
        ),
        Err(RequestedDomainSupportError::ResourceLimit {
            resource: RAW_SUPPORT_CELLS,
            requested: 4,
            limit: 3,
        })
    ));

    // Two symbolic-axis inspections, two support arity inspections, and one
    // support-order comparison are all charged before either malformed slice
    // is scanned.
    let mut work_limits = RequestedDomainSupportLimits::default();
    work_limits.max_canonicalization_work = 4;
    assert!(matches!(
        RequestedDomainSupportProposal::try_new(
            "scope",
            &sector,
            &[1, 0],
            &[1, 0],
            &malformed_support,
            provenance,
            work_limits,
        ),
        Err(RequestedDomainSupportError::ResourceLimit {
            resource: CANONICALIZATION_WORK,
            requested: 5,
            limit: 4,
        })
    ));
}

#[test]
fn atomic_retained_byte_limit_uses_an_independent_exact_fixture_count() {
    let expected = expected_atomic_fixture_retained_bytes();
    let sector = Mask::try_new([true, false]).unwrap();
    let support = [shift(&[0, 0])];
    let provenance = RequestedSupportProposalProvenanceInput::new(
        1,
        7,
        11,
        "ore-order-v1",
        "obligation",
        RequestedSupportProposalOrigin::InvolutiveProlongation,
    );
    let mut limits = RequestedDomainSupportLimits::default();
    limits.max_retained_bytes = expected - 1;
    assert!(matches!(
        RequestedDomainSupportProposal::try_new(
            "scope",
            &sector,
            &[2, 0],
            &[0],
            &support,
            provenance,
            limits,
        ),
        Err(RequestedDomainSupportError::ResourceLimit {
            resource: RETAINED_BYTES,
            requested,
            limit,
        }) if requested == expected && limit == expected - 1
    ));
}

#[test]
fn every_tight_union_limit_fails_without_a_partial_output() {
    let make = || {
        vec![
            proposal("alpha", &[2, 0], &[&[-1_i64, 0], &[1_i64, 0]], "left"),
            proposal("alpha", &[2, 0], &[&[0_i64, 0], &[1_i64, 0]], "right"),
            proposal("beta", &[3, 0], &[&[2_i64, 0]], "beta"),
        ]
    };
    let baseline =
        try_union_requested_domain_support(make(), RequestedDomainSupportLimits::default())
            .unwrap()
            .census();
    let independently_counted_retained_bytes = expected_union_fixture_retained_bytes();
    assert_eq!(
        baseline.retained_bytes(),
        independently_counted_retained_bytes
    );
    let cases = [
        (RAW_DOMAINS, baseline.raw_domains()),
        (UNIQUE_DOMAINS, baseline.unique_domains()),
        (RAW_PROVENANCE, baseline.raw_provenance_records()),
        (UNIQUE_PROVENANCE, baseline.unique_provenance_records()),
        (RAW_SUPPORT, baseline.raw_support_entries()),
        (UNIQUE_SUPPORT, baseline.unique_support_entries()),
        (RAW_SUPPORT_CELLS, baseline.raw_support_coordinate_cells()),
        (
            UNIQUE_SUPPORT_CELLS,
            baseline.unique_support_coordinate_cells(),
        ),
        (CANONICALIZATION_WORK, baseline.canonicalization_work()),
        (RETAINED_BYTES, independently_counted_retained_bytes),
    ];
    for (resource, exact) in cases {
        assert!(exact > 0);
        let mut limits = RequestedDomainSupportLimits::default();
        match resource {
            RAW_DOMAINS => limits.max_raw_domains = exact - 1,
            UNIQUE_DOMAINS => limits.max_unique_domains = exact - 1,
            RAW_PROVENANCE => limits.max_raw_provenance_records = exact - 1,
            UNIQUE_PROVENANCE => limits.max_unique_provenance_records = exact - 1,
            RAW_SUPPORT => limits.max_raw_support_entries = exact - 1,
            UNIQUE_SUPPORT => limits.max_unique_support_entries = exact - 1,
            RAW_SUPPORT_CELLS => limits.max_raw_support_coordinate_cells = exact - 1,
            UNIQUE_SUPPORT_CELLS => {
                limits.max_unique_support_coordinate_cells = exact - 1;
            }
            CANONICALIZATION_WORK => limits.max_canonicalization_work = exact - 1,
            RETAINED_BYTES => limits.max_retained_bytes = exact - 1,
            _ => unreachable!(),
        }
        let result = try_union_requested_domain_support(make(), limits);
        assert!(matches!(
            result,
            Err(RequestedDomainSupportError::ResourceLimit {
                resource: actual,
                requested,
                limit,
            }) if actual == resource && requested == exact && limit == exact - 1
        ));
    }
}

fn expected_atomic_fixture_retained_bytes() -> usize {
    size_of::<RequestedDomainSupportProposal>()
        + size_of::<String>()
        + "scope".len()
        + size_of::<Vec<bool>>()
        + 2
        + size_of::<Vec<u64>>()
        + 2 * size_of::<u64>()
        + size_of::<Vec<usize>>()
        + size_of::<usize>()
        + size_of::<IntegralShift>()
        + size_of::<Vec<i64>>()
        + 2 * size_of::<i64>()
        + size_of::<RequestedSupportProposalProvenance>()
        + 2 * size_of::<String>()
        + "ore-order-v1".len()
        + "obligation".len()
}

fn expected_union_fixture_retained_bytes() -> usize {
    let fixed_domain_bytes = size_of::<String>()
        + size_of::<Vec<bool>>()
        + 2
        + size_of::<Vec<u64>>()
        + 2 * size_of::<u64>()
        + size_of::<Vec<usize>>()
        + size_of::<usize>();
    let domains = 2 * fixed_domain_bytes + "alpha".len() + "beta".len();
    let support = 4 * (size_of::<IntegralShift>() + size_of::<Vec<i64>>() + 2 * size_of::<i64>());
    let provenance = 3
        * (size_of::<RequestedSupportProposalProvenance>()
            + 2 * size_of::<String>()
            + "ore-order-v1".len())
        + "left".len()
        + "right".len()
        + "beta".len();
    size_of::<RequestedDomainSupportUnion>()
        + 2 * size_of::<RequestedDomainSupportProposal>()
        + domains
        + support
        + provenance
}
