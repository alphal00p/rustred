use super::*;

fn check_many_atoms(
    count: usize,
    limit: usize,
    include_face: bool,
) -> Result<PredicateCoverCertificate, PredicateCoverError> {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sector = [false; 3];
    let domains: Vec<_> = (1..=count)
        .map(|factor| affine(&context, &sector, [None; 3], &[&format!("a-{factor}*b")]))
        .collect();
    let outside = [
        LatticeBox::try_new([1, 0, 0], [None; 3]).unwrap(),
        LatticeBox::try_new([0, 1, 0], [Some(0), None, None]).unwrap(),
    ];
    let face = [LatticeBox::try_new([0; 3], [Some(0), Some(0), None]).unwrap()];
    let mut owners = vec![PredicateCoveragePiece {
        boxes: &outside,
        affine_target: None,
        affine_exclusions: &[],
    }];
    owners.extend(domains.iter().map(|domain| PredicateCoveragePiece {
        boxes: &outside,
        affine_target: Some(domain),
        affine_exclusions: &[],
    }));
    if include_face {
        owners.push(PredicateCoveragePiece {
            boxes: &face,
            affine_target: Some(&domains[0]),
            affine_exclusions: &[],
        });
    }
    certify_predicate_cover(
        &sector,
        &owners,
        &[],
        PredicateCoverLimits {
            max_predicates: limit,
            ..Default::default()
        },
    )
}

#[test]
fn dynamic_atom_ids_cross_word_boundaries_without_changing_cover_semantics() {
    for count in [
        33,
        65,
        super::super::super::SourcePortLimits::MAX_PREDICATE_ATOMS,
    ] {
        assert!(matches!(
            check_many_atoms(count, 32, true),
            Err(PredicateCoverError::Budget("predicate atoms"))
        ));
        let proof = check_many_atoms(count, count, true).unwrap();
        assert_eq!(proof.predicates, count);
        assert!(proof.boolean_nodes <= 3);
    }
}

#[test]
fn larger_atom_allowance_does_not_certify_a_genuine_unbounded_hole() {
    assert!(matches!(
        check_many_atoms(33, 64, false),
        Err(PredicateCoverError::Uncovered {
            unbounded_boxes: 1,
            ..
        })
    ));
}

#[test]
fn supported_ceiling_traverses_all_256_atoms_on_the_normal_test_stack() {
    // Every false child is contradicted by a=b=0 on the remaining face. The
    // true path visits all 256 atoms before reporting the actual free-c hole:
    // linear-size pruning, no giant-stack workaround or exponential census.
    assert!(matches!(
        check_many_atoms(256, 256, false),
        Err(PredicateCoverError::Uncovered {
            unbounded_boxes: 1,
            ..
        })
    ));
}

#[test]
fn unsupported_policy_rejected_even_for_coordinate_only_covers() {
    let boxes = [full::<1>()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &[],
    }];
    for max_predicates in [257, usize::MAX] {
        assert!(matches!(
            certify_predicate_cover(
                &[true],
                &owners,
                &[],
                PredicateCoverLimits {
                    max_predicates,
                    ..Default::default()
                },
            ),
            Err(PredicateCoverError::Budget(
                "supported predicate atom policy"
            ))
        ));
    }
    assert!(
        certify_predicate_cover(
            &[true],
            &owners,
            &[],
            PredicateCoverLimits {
                max_predicates: 0,
                ..Default::default()
            },
        )
        .is_ok()
    );
}
