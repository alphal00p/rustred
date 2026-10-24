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
            diagnostics_enabled: true,
            max_predicates: limit,
            ..Default::default()
        },
    )
}

#[test]
fn dynamic_atom_ids_cross_word_boundaries_without_changing_cover_semantics() {
    for count in [33, 65, 512] {
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
fn iterative_traversal_handles_512_atoms_on_the_normal_test_stack() {
    // Every false child is contradicted by a=b=0 on the remaining face. The
    // true path visits all 512 atoms before reporting the actual free-c hole:
    // linear-size pruning, no giant-stack workaround or exponential census.
    assert!(matches!(
        check_many_atoms(512, 512, false),
        Err(PredicateCoverError::Uncovered {
            unbounded_boxes: 1,
            ..
        })
    ));
}

#[test]
fn ordinary_zero_templates_and_large_diagnostic_atom_allowances_are_supported() {
    let boxes = [full::<1>()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &[],
    }];
    let zero = PredicateCoverLimits {
        max_predicates: 0,
        max_clauses: 0,
        max_boolean_nodes: 0,
        max_consistency_work: 0,
        geometry: CompletionGeometryLimits {
            max_arity: 0,
            max_requested_boxes: 0,
            max_requested_box_coordinate_cells: 0,
            max_uncovered_boxes: 0,
            max_uncovered_box_coordinate_cells: 0,
            max_split_operations: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(certify_predicate_cover(&[true], &owners, &[], zero).is_ok());
    assert!(certify_predicate_cover(&[true], &owners, &[], zero.with_diagnostics()).is_err());
    for max_predicates in [257, usize::MAX] {
        assert!(
            certify_predicate_cover(
                &[true],
                &owners,
                &[],
                PredicateCoverLimits {
                    max_predicates,
                    ..PredicateCoverLimits::default().with_diagnostics()
                }
            )
            .is_ok()
        );
    }
}

#[test]
fn shared_resource_refusal_is_typed_and_next_cover_remains_healthy() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
    let boxes = [full::<1>()];
    let owners = [PredicateCoveragePiece {
        boxes: &boxes,
        affine_target: None,
        affine_exclusions: &[],
    }];
    let budget = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(1),
            ..Default::default()
        },
        || Ok(2),
    );
    let error = budget
        .run(|| certify_predicate_cover(&[true], &owners, &[], Default::default()))
        .unwrap_err();
    assert!(matches!(
        error,
        PredicateCoverError::Resources(ResourceLimitError::Memory { .. })
    ));
    assert!(crate::reduction::resource_limit_error(&error).is_some());
    assert!(certify_predicate_cover(&[true], &owners, &[], Default::default()).is_ok());
}
