use super::*;
use rustred::family::IntegralKey;
use rustred::persistence::{BinaryProgramKind, BinarySection, encode_program};

#[test]
fn finite_depth_is_saved_inspectable_and_cold_applicable_across_backends_and_workers() {
    assert_eq!(FamilyCandidatesRequest::new(K1).numerical_depth, 2);
    for depth in [0, 1, 2] {
        let mut request = FamilyCandidatesRequest::new(K3);
        request.numerical_depth = depth;
        let baseline = family_candidates(request.clone()).unwrap();
        let report: toml::Value = toml::from_str(baseline.to_toml()).unwrap();
        assert_eq!(
            report["numerical_depth"].as_integer(),
            Some(i64::from(depth))
        );
        let inspection =
            inspect_generated_candidate_bundle(baseline.bundle(), Default::default()).unwrap();
        assert_eq!(inspection.numerical_depth, depth);
        let bundle = codec::read(baseline.bundle(), Default::default()).unwrap();
        assert_eq!(bundle.solver_policy, super::super::policy::encode(depth));
        if depth == 2 {
            assert_same_program(
                baseline.bundle(),
                family_candidates(FamilyCandidatesRequest::new(K3))
                    .unwrap()
                    .bundle(),
            );
        }
        let (_, mut reducer) = load_generated_candidate_bundle::<3>(
            baseline.bundle(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let target = IntegralKey::try_new([2, 1, 1]).unwrap();
        let expected = reducer.reduce_unit_mass(&target).unwrap();
        for (backend, workers) in [
            (CandidateExactBackend::Sparse, 2),
            (CandidateExactBackend::SparseFactorized, 1),
            (CandidateExactBackend::SparseFactorized, 2),
            (CandidateExactBackend::SparseTargetOnlyFactorized, 1),
            (CandidateExactBackend::SparseTargetOnlyFactorized, 2),
        ] {
            request.exact_backend = backend;
            request.n_cores = workers;
            let result = family_candidates(request.clone()).unwrap();
            assert_same_program(baseline.bundle(), result.bundle());
            let (_, mut loaded) = load_generated_candidate_bundle::<3>(
                result.bundle(),
                Default::default(),
                Default::default(),
            )
            .unwrap();
            let actual = loaded.reduce_unit_mass(&target).unwrap();
            assert_eq!(actual.target(), expected.target());
            assert_eq!(actual.terms(), expected.terms());
            for (key, coefficient) in actual.terms() {
                assert_eq!(
                    coefficient.numerator.variables(),
                    expected.terms()[key].numerator.variables()
                );
                assert_eq!(
                    coefficient.denominator.variables(),
                    expected.terms()[key].denominator.variables()
                );
            }
        }
    }
}

#[test]
fn unknown_policy_fails_before_native_state_import() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    let (envelope, mut records, _) = codec::read_structure(generated.bundle(), limits).unwrap();
    records.solver_policy = "ordinary-source-port-numerical-depth-00-v1".into();
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
