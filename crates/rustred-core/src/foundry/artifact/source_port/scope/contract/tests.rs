use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;
use crate::foundry::artifact::source_port::scope::successor::DestinationScope;
use crate::foundry::completion::CompletionGeometryLimits;

fn tadpole(name: &str) -> IntegralFamily {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn sunset() -> IntegralFamily {
    crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap()
}

fn cell<const N: usize>(lower: [u64; N], upper: [Option<u64>; N]) -> LatticeBox {
    LatticeBox::try_new(lower, upper).unwrap()
}

fn entry(family: &IntegralFamily, root: &[bool], bound: EntryDegreeBound) -> EntryScope {
    EntryScope::try_new(family, &Mask::try_new(root.iter().copied()).unwrap(), bound).unwrap()
}

#[test]
fn numerator_and_total_excess_have_distinct_exact_entry_semantics() {
    let family = sunset();
    let negative = entry(
        &family,
        &[true, false, true],
        EntryDegreeBound::MaxNegativeIndexDegree(3),
    );
    let total = entry(
        &family,
        &[true, false, true],
        EntryDegreeBound::MaxTotalExcessDegree(3),
    );
    assert!(negative.contains(&[i64::MAX, -2, -1]).unwrap());
    assert!(!total.contains(&[i64::MAX, -2, -1]).unwrap());
    assert!(total.contains(&[2, -2, 1]).unwrap());
    assert!(!total.contains(&[2, -2, 2]).unwrap());
    assert!(negative.contains(&[-1, -2, 0]).unwrap());
    assert!(!negative.contains(&[-2, -2, 0]).unwrap());
    assert!(!negative.contains(&[0, 1, 0]).unwrap());
    assert!(!total.contains(&[0, 1, 0]).unwrap());
    assert!(negative.contains(&[]).is_err());
    let zero = entry(
        &family,
        &[true, false, true],
        EntryDegreeBound::MaxTotalExcessDegree(0),
    );
    assert!(zero.contains(&[1, 0, 1]).unwrap());
    assert!(zero.contains(&[0, 0, 0]).unwrap());
    assert!(!zero.contains(&[2, 0, 0]).unwrap());
    assert!(!zero.contains(&[0, -1, 0]).unwrap());
}

#[test]
fn degree_accumulation_handles_minimum_machine_integer_without_wrapping() {
    let family = sunset();
    for bound in [
        EntryDegreeBound::MaxNegativeIndexDegree(u64::MAX),
        EntryDegreeBound::MaxTotalExcessDegree(u64::MAX),
    ] {
        let scope = entry(&family, &[false; 3], bound);
        assert!(scope.contains(&[i64::MIN, 0, 0]).unwrap());
        assert!(!scope.contains(&[i64::MIN, i64::MIN, 0]).unwrap());
    }
    let exact = entry(
        &family,
        &[false; 3],
        EntryDegreeBound::MaxNegativeIndexDegree(1_u64 << 63),
    );
    assert!(exact.contains(&[i64::MIN, 0, 0]).unwrap());
    assert!(!exact.contains(&[i64::MIN, -1, 0]).unwrap());
}

#[test]
fn family_root_and_arity_binding_are_explicit_and_immutable() {
    let family = tadpole("entry-contract-family");
    let foreign = tadpole("entry-contract-foreign-family");
    let root = Mask::try_new([true]).unwrap();
    let scope =
        EntryScope::try_new(&family, &root, EntryDegreeBound::MaxNegativeIndexDegree(30)).unwrap();
    scope.validate_binding(&family, &root).unwrap();
    assert_eq!(scope.family_fingerprint(), family.fingerprint());
    assert_eq!(scope.bound().limit(), 30);
    assert_eq!(scope.root(), &root);
    assert!(matches!(
        scope.validate_binding(&foreign, &root),
        Err(ArtifactError::WrongFamily)
    ));
    assert!(
        scope
            .validate_binding(&family, &Mask::try_new([false]).unwrap())
            .is_err()
    );
    assert!(
        EntryScope::try_new(&family, &Mask::try_new([true; 2]).unwrap(), scope.bound()).is_err()
    );
    assert!(
        scope
            .intersects_local_box(&[true, false], &cell([0; 2], [None; 2]))
            .is_err()
    );
}

#[test]
fn rectangle_minimum_is_a_sum_not_a_coordinatewise_rank_cube() {
    let family = sunset();
    let scope = entry(
        &family,
        &[true, false, false],
        EntryDegreeBound::MaxNegativeIndexDegree(3),
    );
    assert!(
        !scope
            .intersects_local_box(&[true, false, false], &cell([u64::MAX, 2, 2], [None; 3]))
            .unwrap()
    );
    assert!(
        scope
            .intersects_local_box(&[true, false, false], &cell([u64::MAX, 1, 2], [None; 3]))
            .unwrap()
    );
    assert!(
        !EntryDegreeBound::MaxTotalExcessDegree(3)
            .intersects_local_box(&[true, false, false], &cell([1, 1, 2], [None; 3]))
            .unwrap()
    );
    assert!(
        scope
            .intersects_local_box(&[false, true, false], &cell([0; 3], [None; 3]))
            .is_err()
    );
    assert!(
        !EntryDegreeBound::MaxTotalExcessDegree(u64::MAX)
            .intersects_local_box(&[false; 3], &cell([u64::MAX, 1, 0], [None; 3]))
            .unwrap()
    );
    assert!(
        !EntryDegreeBound::MaxNegativeIndexDegree(u64::MAX)
            .intersects_local_box(&[false; 2], &cell([u64::MAX; 2], [None; 2]))
            .unwrap()
    );
}

#[test]
fn envelope_contains_entries_without_capping_positive_rays_or_inventing_zero_evidence() {
    let family = tadpole("entry-ray");
    let scope = entry(
        &family,
        &[true],
        EntryDegreeBound::MaxNegativeIndexDegree(3),
    );
    let active = [cell([0], [None])];
    let inactive = [cell([0], [Some(3)])];
    let domains = [
        DestinationScope {
            sector: &[true],
            boxes: &active,
        },
        DestinationScope {
            sector: &[false],
            boxes: &inactive,
        },
    ];
    let envelope = ProposedProofEnvelope::try_new(&scope, &domains, Default::default()).unwrap();
    assert_eq!(envelope.entry_scope(), &scope);
    envelope.validate_binding(&family, scope.root()).unwrap();
    assert_eq!(envelope.destinations().len(), 2);
    assert_eq!(envelope.sector_boxes(&[true]).unwrap()[0].upper(), [None]);
    // The all-nonpositive sector contains a rank-zero entry, even if a separate
    // zero-sector theorem might later discharge it in an artifact installer.
    assert!(ProposedProofEnvelope::try_new(&scope, &domains[..1], Default::default()).is_err());
    let finite = [cell([0], [Some(100)])];
    let finite_domains = [
        DestinationScope {
            sector: &[true],
            boxes: &finite,
        },
        DestinationScope {
            sector: &[false],
            boxes: &inactive,
        },
    ];
    assert!(ProposedProofEnvelope::try_new(&scope, &finite_domains, Default::default()).is_err());
    let total = entry(&family, &[true], EntryDegreeBound::MaxTotalExcessDegree(3));
    ProposedProofEnvelope::try_new(&total, &finite_domains, Default::default()).unwrap();
    let too_short = [cell([0], [Some(2)])];
    assert!(
        ProposedProofEnvelope::try_new(
            &total,
            &[
                DestinationScope {
                    sector: &[true],
                    boxes: &too_short
                },
                DestinationScope {
                    sector: &[false],
                    boxes: &inactive
                },
            ],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn diagonal_holes_outside_the_degree_simplex_do_not_reject_containment() {
    let family = sunset();
    let scope = entry(
        &family,
        &[false; 3],
        EntryDegreeBound::MaxNegativeIndexDegree(3),
    );
    // Complement is x1>=2 AND x2>=2: each coordinate is <=3 at its corner,
    // but the minimum total degree is 4, outside the requested entry set.
    let safe = [
        cell([0; 3], [None, Some(1), None]),
        cell([0; 3], [None, None, Some(1)]),
    ];
    ProposedProofEnvelope::try_new(
        &scope,
        &[DestinationScope {
            sector: &[false; 3],
            boxes: &safe,
        }],
        Default::default(),
    )
    .unwrap();
    // Moving one face by one leaves a sum=3 entry uncovered.
    let hole = [
        cell([0; 3], [None, Some(0), None]),
        cell([0; 3], [None, None, Some(1)]),
    ];
    assert!(
        ProposedProofEnvelope::try_new(
            &scope,
            &[DestinationScope {
                sector: &[false; 3],
                boxes: &hole
            }],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn containment_does_not_enumerate_simplex_points_or_assume_machine_endpoints() {
    let family = sunset();
    let scope = entry(
        &family,
        &[false; 3],
        EntryDegreeBound::MaxNegativeIndexDegree(u64::MAX),
    );
    let full = [cell([0; 3], [None; 3])];
    ProposedProofEnvelope::try_new(
        &scope,
        &[DestinationScope {
            sector: &[false; 3],
            boxes: &full,
        }],
        Default::default(),
    )
    .unwrap();
}

#[test]
fn successor_envelope_may_be_wider_than_entry_but_every_image_is_checked() {
    let family = tadpole("entry-with-larger-envelope");
    let scope = entry(
        &family,
        &[true],
        EntryDegreeBound::MaxNegativeIndexDegree(0),
    );
    let active = [cell([0], [None])];
    let inactive = [cell([0], [Some(2)])];
    let mut envelope = ProposedProofEnvelope::try_new(
        &scope,
        &[
            DestinationScope {
                sector: &[true],
                boxes: &active,
            },
            DestinationScope {
                sector: &[false],
                boxes: &inactive,
            },
        ],
        Default::default(),
    )
    .unwrap();
    assert!(!scope.contains(&[-1]).unwrap());
    // n=1 -> n=-1 is admitted by the separate envelope, not the entry rank.
    envelope
        .check_rule_images(&cell([0], [Some(0)]), &[true], &[&[-2]])
        .unwrap();
    assert!(
        envelope
            .check_rule_images(&cell([0], [Some(0)]), &[true], &[&[-4]])
            .is_err()
    );
    // An empty RHS cannot make a source outside the envelope valid.
    assert!(
        envelope
            .check_rule_images(&cell([3], [Some(3)]), &[false], &[])
            .is_err()
    );
    assert!(
        envelope
            .check_rule_images(&cell([0], [Some(0)]), &[true], &[&[]])
            .is_err()
    );
}

#[test]
fn unsupported_root_sectors_are_rejected_even_for_empty_destination_input() {
    let family = tadpole("entry-inactive-root");
    let scope = entry(
        &family,
        &[false],
        EntryDegreeBound::MaxNegativeIndexDegree(0),
    );
    let full = [cell([0], [None])];
    assert!(
        ProposedProofEnvelope::try_new(
            &scope,
            &[
                DestinationScope {
                    sector: &[false],
                    boxes: &full
                },
                DestinationScope {
                    sector: &[true],
                    boxes: &[]
                },
            ],
            Default::default()
        )
        .is_err()
    );
    let mut envelope = ProposedProofEnvelope::try_new(
        &scope,
        &[DestinationScope {
            sector: &[false],
            boxes: &full,
        }],
        Default::default(),
    )
    .unwrap();
    assert!(envelope.sector_boxes(&[true]).is_err());
    assert!(
        envelope
            .check_rule_images(&cell([0], [Some(0)]), &[false], &[&[1]])
            .is_err()
    );
}

#[test]
fn one_cumulative_budget_covers_preparation_entry_checks_and_rule_images() {
    let family = tadpole("entry-budget");
    let scope = entry(
        &family,
        &[false],
        EntryDegreeBound::MaxNegativeIndexDegree(0),
    );
    let full = [cell([0], [None])];
    let limits = CompletionGeometryLimits {
        max_split_operations: 256,
        ..Default::default()
    };
    let mut envelope = ProposedProofEnvelope::try_new(
        &scope,
        &[DestinationScope {
            sector: &[false],
            boxes: &full,
        }],
        limits,
    )
    .unwrap();
    let mut successes = 0;
    loop {
        match envelope.check_rule_images(&full[0], &[false], &[&[0]]) {
            Ok(()) => successes += 1,
            Err(ArtifactError::ResourceLimit { .. }) => break,
            Err(other) => panic!("unexpected geometry failure: {other}"),
        }
        assert!(successes < 256);
    }
    assert!(successes > 0);
    for _ in 0..3 {
        assert!(matches!(
            envelope.check_rule_images(&full[0], &[false], &[&[0]]),
            Err(ArtifactError::ResourceLimit { .. })
        ));
    }
    let limits = CompletionGeometryLimits {
        max_split_operations: 0,
        ..Default::default()
    };
    assert!(matches!(
        ProposedProofEnvelope::try_new(
            &scope,
            &[DestinationScope {
                sector: &[false],
                boxes: &full
            },],
            limits
        ),
        Err(ArtifactError::ResourceLimit { .. })
    ));
}
