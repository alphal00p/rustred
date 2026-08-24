//! Natural generated sunset validation for the family-scoped residual affine
//! branch compiler. Loop count appears only in this concrete oracle fixture.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator, ResidualAffineBranchSystemCertificate,
    ResidualAffineBranchSystemError, ResidualAffineBranchSystemLimits,
    ResidualAffineBranchSystemOutcome, ResidualAffineBranchZeroAtomOutcome,
    ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverCompiler,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanNodeOutcome, SectorMask,
};
use symbolica::prelude::Integer;

fn sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn generated_cover(
    bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<ResidualProductLocusBooleanCoverCertificate>,
) {
    let family = sunset(&format!("residual-affine-branch-sunset-{bits}"));
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_from_bit_string(bits).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    assert_eq!(queue.work_items().len(), 1, "sector {bits}");
    let cover = Arc::new(
        ResidualProductLocusBooleanCoverCompiler::compile(
            &family,
            &context,
            queue,
            0,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap(),
    );
    (family, context, cover)
}

fn assert_row_is_annihilated(
    recognition: &rustred::ResidualAffineBranchZeroAtomRecognition,
    map: &rustred::ResidualAffineIntegerMap,
) {
    let certificate = recognition.outcome().certificate().unwrap();
    let row = certificate.row().unwrap();
    let mut constant_image = row.constant().clone();
    for position in 0..map.ambient_arity() {
        let product = &row.coefficients()[position] * map.constant(position).unwrap();
        constant_image = &constant_image + &product;
    }
    assert_eq!(constant_image, Integer::from(0));
    for column in 0..map.ambient_arity() {
        let mut linear_image = Integer::from(0);
        for position in 0..map.ambient_arity() {
            let product =
                &row.coefficients()[position] * map.linear_coefficient(position, column).unwrap();
            linear_image = &linear_image + &product;
        }
        assert_eq!(linear_image, Integer::from(0));
    }
}

fn test_values(active: bool) -> [i64; 3] {
    if active {
        [1, 2, i64::MAX]
    } else {
        [0, -1, -2]
    }
}

#[test]
fn generated_sunset_ready_terminals_compile_replay_and_preserve_the_exact_boolean_cover() {
    for bits in ["011", "101", "110", "111"] {
        let (family, context, cover) = generated_cover(bits);
        let ready_ordinals: Vec<_> = cover
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                )
            })
            .map(|node| node.ordinal())
            .collect();
        assert!(!ready_ordinals.is_empty(), "sector {bits}");

        let mut branches = Vec::new();
        for terminal_ordinal in ready_ordinals {
            let branch = ResidualAffineBranchSystemCertificate::compile(
                &family,
                &context,
                cover.clone(),
                terminal_ordinal,
                ResidualAffineBranchSystemLimits::default(),
            )
            .unwrap();
            assert!(Arc::ptr_eq(branch.source_cover(), &cover));
            branch.replay(&family, &context).unwrap();
            assert!(matches!(
                branch.outcome(),
                ResidualAffineBranchSystemOutcome::GuardedAffineMap
            ));

            let terminal = &cover.nodes()[terminal_ordinal];
            assert_eq!(
                branch
                    .zero_atom_recognitions()
                    .iter()
                    .map(|recognition| recognition.structural_locus_ordinal())
                    .collect::<Vec<_>>(),
                terminal.equal_zero_atoms()
            );
            assert_eq!(
                branch.nonzero_guard_locus_ordinals(),
                terminal.nonzero_atoms(),
                "the original nonzero guards must not be erased or weakened"
            );

            let coverage = cover.source_queue().discovery().coverage();
            let map = branch.affine_map().unwrap();
            for recognition in branch.zero_atom_recognitions() {
                let source = coverage
                    .structural_locus(recognition.structural_locus_ordinal())
                    .unwrap();
                match recognition.outcome() {
                    ResidualAffineBranchZeroAtomOutcome::Row(certificate) => {
                        assert_eq!(certificate.source(), source);
                        certificate.replay(&context).unwrap();
                        assert_row_is_annihilated(recognition, map);
                    }
                    ResidualAffineBranchZeroAtomOutcome::RedundantZeroPolynomial(certificate) => {
                        assert_eq!(certificate.source(), source);
                        assert!(certificate.source().is_zero());
                        certificate.replay(&context).unwrap();
                    }
                    unexpected => {
                        panic!("generated sunset structural atom was not affine: {unexpected:?}")
                    }
                }
            }
            branches.push(branch);
        }

        let values: Vec<_> = bits.bytes().map(|bit| test_values(bit == b'1')).collect();
        for &n0 in &values[0] {
            for &n1 in &values[1] {
                for &n2 in &values[2] {
                    let indices = [n0, n1, n2];
                    let boolean_terminal = cover
                        .ready_terminal_for_indices(&context, &indices)
                        .unwrap();
                    let matching_branches: Vec<_> = branches
                        .iter()
                        .filter(|branch| {
                            branch
                                .matches_original_boolean_terminal_for_indices(&context, &indices)
                                .unwrap()
                        })
                        .collect();
                    assert_eq!(
                        matching_branches.len(),
                        usize::from(boolean_terminal.is_some()),
                        "branch union/disjointness, sector {bits}, point {indices:?}"
                    );
                    if let Some(terminal) = boolean_terminal {
                        assert_eq!(
                            matching_branches[0].ready_terminal_ordinal(),
                            terminal.ordinal()
                        );
                        assert!(
                            matching_branches[0]
                                .guarded_affine_map_applies_at_original_indices(&context, &indices)
                                .unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn replay_accepts_an_equal_cover_allocation_and_rejects_the_wrong_scope() {
    let (family, context, cover) = generated_cover("111");
    let terminal = cover
        .nodes()
        .iter()
        .find(|node| {
            matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            )
        })
        .expect("sunset cover has a ready terminal");
    let branch = ResidualAffineBranchSystemCertificate::compile(
        &family,
        &context,
        cover.clone(),
        terminal.ordinal(),
        ResidualAffineBranchSystemLimits::default(),
    )
    .unwrap();
    assert_eq!(branch.family_fingerprint(), family.fingerprint_ref());

    let equal_cover = Arc::new((*cover).clone());
    assert!(!Arc::ptr_eq(&cover, &equal_cover));
    branch
        .replay_with_cover(&family, &context, equal_cover)
        .unwrap();

    let wrong_family = sunset("residual-affine-branch-sunset-wrong-family");
    assert!(matches!(
        branch.replay(&wrong_family, &context),
        Err(ResidualAffineBranchSystemError::WrongFamily)
    ));

    let wrong_context = ParametricCoefficientContext::try_new(
        context.base(),
        "residual-affine-branch-sunset-wrong-context",
        context.index_count(),
    )
    .unwrap();
    assert!(matches!(
        branch.replay(&family, &wrong_context),
        Err(ResidualAffineBranchSystemError::WrongContext)
    ));
}
