use super::*;
use rustred::family::IntegralKey;
use rustred::solver::CandidateReductionError;

#[test]
fn numerator_scope_is_saved_and_cold_loaded_without_clamping_positive_powers() {
    assert_eq!(FamilyCandidatesRequest::new(K1).max_numerator_rank, None);
    let mut control = FamilyCandidatesRequest::new(K3);
    control.numerical_depth = 0;
    let ordinary = family_candidates(control).unwrap();
    let (_, mut reference) = load_generated_candidate_bundle::<3>(
        ordinary.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let target = IntegralKey::try_new([3, 1, 1]).unwrap();
    let expected = reference.reduce_unit_mass(&target).unwrap();
    for rank in [0, 1, 20] {
        let mut request = FamilyCandidatesRequest::new(K3);
        request.max_numerator_rank = Some(rank);
        request.numerical_depth = 0;
        let generated = family_candidates(request).unwrap();
        let report: toml::Value = toml::from_str(generated.to_toml()).unwrap();
        assert_eq!(
            report["max_numerator_rank"].as_integer(),
            Some(i64::from(rank))
        );
        let inspected =
            inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
        assert_eq!(inspected.max_numerator_rank, Some(rank));
        assert_eq!(inspected.numerical_depth, 0);
        let (_, mut loaded) = load_generated_candidate_bundle::<3>(
            generated.bundle(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(loaded.max_numerator_rank(), Some(rank));
        assert_eq!(
            loaded.reduce_unit_mass(&target).unwrap().terms(),
            expected.terms()
        );
        // Rank counts all negative coordinates and does not bound positive dots.
        let outside = IntegralKey::try_new([-i64::from(rank) - 1, 1, 1]).unwrap();
        let before = loaded.statistics();
        assert!(matches!(
            loaded.reduce_unit_mass(&outside),
            Err(CandidateReductionError::OutsideNumeratorRank { .. })
        ));
        assert_eq!(loaded.statistics(), before);
        for total_excess in [None, Some(0), Some(30)] {
            let mut request = CandidateCertificationRequest::new(generated.bundle());
            request.max_total_excess_degree = total_excess;
            let rejected = certify_candidates(request).unwrap_err();
            assert!(
                rejected
                    .message()
                    .contains("rank-scoped candidates cannot be certified")
            );
        }
    }
    let unbounded =
        inspect_generated_candidate_bundle(ordinary.bundle(), Default::default()).unwrap();
    assert_eq!(unbounded.max_numerator_rank, None);
    assert!(!ordinary.to_toml().contains("max_numerator_rank"));
}

#[test]
fn zero_only_candidate_bundle_retains_explicit_rank_scope() {
    let mut request = FamilyCandidatesRequest::new(K1);
    request.nonpositive_indices = vec![0];
    request.max_numerator_rank = Some(0);
    let generated = family_candidates(request).unwrap();
    let inspected =
        inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
    assert_eq!(inspected.solved_sectors, 0);
    assert_eq!(inspected.max_numerator_rank, Some(0));
    let (_, mut loaded) = load_generated_candidate_bundle::<1>(
        generated.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(loaded.max_numerator_rank(), Some(0));
    assert!(
        loaded
            .reduce_unit_mass(&IntegralKey::try_new([0]).unwrap())
            .unwrap()
            .terms()
            .is_empty()
    );
    let before = loaded.statistics();
    assert!(matches!(
        loaded.reduce_unit_mass(&IntegralKey::try_new([-1]).unwrap()),
        Err(CandidateReductionError::OutsideNumeratorRank { .. })
    ));
    assert_eq!(loaded.statistics(), before);
}

#[test]
fn rank_scope_policy_rejects_malformed_shape_before_symbolica_import() {
    use rustred::persistence::{BinaryProgramKind, BinarySection, encode_program};
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    let (envelope, mut records, _) = codec::read_structure(generated.bundle(), limits).unwrap();
    records.solver_policy =
        "ordinary-source-port-numerical-depth-2-max-numerator-rank-00-v1".into();
    let program = bincode::encode_to_vec(&records, bincode::config::standard()).unwrap();
    let sections = envelope
        .sections()
        .iter()
        .map(|section| BinarySection {
            tag: section.tag,
            bytes: match section.tag {
                SectionTag::PROGRAM => &program,
                SectionTag::SYMBOLICA_STATE => b"not a native state",
                _ => section.bytes,
            },
        })
        .collect::<Vec<_>>();
    let bytes = encode_program(
        BinaryProgramKind::Candidates,
        &sections,
        limits.binary_limits(),
    )
    .unwrap();
    let error =
        load_generated_candidate_bundle::<1>(&bytes, limits, Default::default()).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Schema);
    assert!(error.message().contains("solver policy"));
}
