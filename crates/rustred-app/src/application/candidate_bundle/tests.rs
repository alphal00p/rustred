use rustred::foundry::artifact::ClosedArtifact;
use rustred::identity::ParametricIbpGenerator;
use rustred::solver::{AffineCase, AffineIntersection, CoordinateCase};

use crate::application::{AppErrorKind, FamilyCloseRequest, InputFormat, family_close};

use super::{codec, model::*, preparation, *};

const K1: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "saved_candidate_k1"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;

const K3: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "saved_candidate_k3"
loop_momenta = ["q1", "q2"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P1"
expression = "q1^2-1"
[[family.denominators]]
id = "P2"
expression = "q2^2-1"
[[family.denominators]]
id = "P3"
expression = "(q1-q2)^2-1"
[target]
powers = [1, 1, 1]
"#;

fn roundtrip<const N: usize>(source: &str) {
    let generated = family_candidates(FamilyCandidatesRequest::new(source)).unwrap();
    assert_eq!(generated.status(), "uncertified-candidates");
    assert_eq!(generated.schema(), FAMILY_CANDIDATES_SCHEMA);
    assert!(ClosedArtifact::decode_durable(generated.bundle()).is_err());
    let limits = CandidateBundleLimits::default();
    let mut bundle = codec::read(generated.bundle(), limits).unwrap();
    let family = preparation::family(source, InputFormat::Toml).unwrap();
    let prepared = preparation::prepare::<N>(family, &bundle.root_sector, None).unwrap();
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .unwrap()
        .context()
        .clone();
    let solved = codec::solutions::<N>(
        &bundle,
        &context,
        prepared.sources.index_variables(),
        limits,
    )
    .unwrap();
    assert!(
        solved
            .iter()
            .flat_map(|(_, solution)| &solution.rules)
            .any(|rule| !rule.candidate.sources.is_empty())
    );
    bundle.sectors = solved
        .iter()
        .map(|(sector, solution)| codec::sector_record(*sector, solution))
        .collect();
    assert_eq!(codec::write(&bundle, limits).unwrap(), generated.bundle());
    let certified =
        certify_candidates(CandidateCertificationRequest::new(generated.bundle())).unwrap();
    assert_eq!(certified.schema(), CANDIDATE_CERTIFICATION_SCHEMA);
    assert_eq!(certified.status(), "generated-durable");
    let original = family_close(FamilyCloseRequest::new(source)).unwrap();
    assert_eq!(certified.artifact(), original.artifact());
    // Cold replay is a distinct, explicit test operation, not hidden in certify.
    ClosedArtifact::decode_durable(certified.artifact()).unwrap();
}

#[test]
fn k1_saved_candidates_roundtrip_and_certify_without_resolving() {
    roundtrip::<1>(K1);
}

#[test]
fn saved_candidate_loader_applies_without_search_or_artifact_promotion() {
    use rustred::family::IntegralKey;
    use rustred::foundry::artifact::derive_one_loop_unit_mass_tadpole;
    use rustred::reduction::{Reducer, ReductionLimits};

    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let (family, mut candidate) = load_candidate_bundle::<1>(
        generated.bundle(),
        CandidateBundleLimits::default(),
        ReductionLimits::default(),
    )
    .unwrap();
    assert_eq!(family.fingerprint(), candidate.family_fingerprint());
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut certified = Reducer::new(&artifact).unwrap();
    for power in [0, 1, 2, 7] {
        let key = IntegralKey::try_new([power]).unwrap();
        assert_eq!(
            candidate.reduce_unit_mass(&key).unwrap().terms(),
            certified.reduce_unit_mass(&key).unwrap().terms()
        );
    }
    assert!(candidate.statistics().rule_applications() > 0);
    assert!(ClosedArtifact::decode_durable(generated.bundle()).is_err());
}

#[test]
fn candidate_loader_rejects_wrong_arity_family_binding_and_ingress_budget() {
    use rustred::reduction::ReductionLimits;

    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    assert!(
        load_candidate_bundle::<3>(generated.bundle(), limits, ReductionLimits::default()).is_err()
    );
    assert!(
        load_candidate_bundle::<1>(
            generated.bundle(),
            CandidateBundleLimits {
                max_bundle_bytes: 1,
                ..limits
            },
            ReductionLimits::default()
        )
        .is_err()
    );
    let mut bundle = codec::read(generated.bundle(), limits).unwrap();
    bundle.family_fingerprint.push_str("-mismatch");
    let mutated = codec::write(&bundle, limits).unwrap();
    assert_eq!(
        load_candidate_bundle::<1>(&mutated, limits, ReductionLimits::default())
            .unwrap_err()
            .kind(),
        AppErrorKind::Input
    );
}

#[test]
fn candidate_loader_preserves_nonpositive_root_and_saved_coordinate_priority() {
    use rustred::family::IntegralKey;
    use rustred::reduction::ReductionLimits;

    let mut request = FamilyCandidatesRequest::new(K3);
    request.permutation = Some(vec![2, 0, 1]);
    request.nonpositive_indices = vec![0];
    let generated = family_candidates(request).unwrap();
    let (_, mut candidate) = load_candidate_bundle::<3>(
        generated.bundle(),
        CandidateBundleLimits::default(),
        ReductionLimits::default(),
    )
    .unwrap();
    assert!(
        !candidate
            .reduce_unit_mass(&IntegralKey::try_new([0, 1, 1]).unwrap())
            .unwrap()
            .terms()
            .is_empty()
    );
    assert!(
        candidate
            .reduce_unit_mass(&IntegralKey::try_new([1, 1, 1]).unwrap())
            .is_err()
    );
}

#[test]
fn k3_saved_candidates_roundtrip_and_certify_without_resolving() {
    roundtrip::<3>(K3);
}

#[test]
fn solve_only_accepts_symbolic_mass_but_certification_keeps_its_existing_admission() {
    let source = K1.replace("q^2-1", "q^2-m");
    let generated = family_candidates(FamilyCandidatesRequest::new(&source)).unwrap();
    assert_eq!(generated.status(), "uncertified-candidates");
    let error =
        certify_candidates(CandidateCertificationRequest::new(generated.bundle())).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Input);
}

#[test]
fn rank_scoped_certification_rejects_without_unbounded_fallback() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let bounded = CandidateCertificationRequest::new(generated.bundle())
        .with_max_negative_index_degree(MAX_RANK_SCOPED_CERTIFICATION_DEGREE);
    let error = certify_candidates(bounded).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Execution);
    assert!(error.message().contains("successor-closed entry scope"));

    let over_limit = CandidateCertificationRequest::new(generated.bundle())
        .with_max_negative_index_degree(MAX_RANK_SCOPED_CERTIFICATION_DEGREE + 1);
    let error = certify_candidates(over_limit).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Input);
    assert!(error.message().contains("supported rank-scoped limit"));
}

#[test]
fn bundles_are_deterministic_across_workers_and_explicit_root_permutation_roundtrip() {
    let serial = family_candidates(FamilyCandidatesRequest::new(K3)).unwrap();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.n_cores = 2;
    let parallel = family_candidates(request).unwrap();
    assert_eq!(serial.bundle(), parallel.bundle());
    let mut request = FamilyCandidatesRequest::new(K3);
    request.permutation = Some(vec![2, 0, 1]);
    request.nonpositive_indices = vec![0];
    let generated = family_candidates(request).unwrap();
    let bundle = codec::read(generated.bundle(), CandidateBundleLimits::default()).unwrap();
    assert_eq!(bundle.root_sector, [false, true, true]);
    assert_eq!(bundle.permutation, Some(vec![2, 0, 1]));
    assert!(bundle.sectors.iter().all(|sector| !sector.sector[0]));
    certify_candidates(CandidateCertificationRequest::new(generated.bundle())).unwrap();
}

#[test]
fn strict_schema_shapes_and_input_limits_reject_before_native_reconstruction() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    let bundle = codec::read(generated.bundle(), limits).unwrap();
    for source in [
        String::from_utf8(generated.bundle().to_vec())
            .unwrap()
            .replacen(".v1", ".v999", 1),
        format!(
            "unexpected = true\n{}",
            std::str::from_utf8(generated.bundle()).unwrap()
        ),
    ] {
        assert_eq!(
            codec::read(source.as_bytes(), limits).unwrap_err().kind(),
            AppErrorKind::Schema
        );
    }
    let mut malformed = bundle.clone();
    malformed.sectors.push(malformed.sectors[0].clone());
    assert!(codec::write(&malformed, limits).is_err());
    let mut malformed = bundle.clone();
    malformed.sectors[0].rules[0].target.values.clear();
    assert!(codec::write(&malformed, limits).is_err());
    for limited in [
        CandidateBundleLimits {
            max_bundle_bytes: 0,
            ..limits
        },
        CandidateBundleLimits {
            max_collection_entries: 0,
            ..limits
        },
        CandidateBundleLimits {
            max_coefficient_bytes: 0,
            ..limits
        },
        CandidateBundleLimits {
            max_total_coefficient_bytes: 0,
            ..limits
        },
    ] {
        assert_eq!(
            codec::read(generated.bundle(), limited).unwrap_err().kind(),
            AppErrorKind::Limit
        );
    }
}

#[test]
fn saved_formula_or_source_trace_tampering_never_becomes_certified() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    let bundle = codec::read(generated.bundle(), limits).unwrap();
    for change_source in [false, true] {
        let mut altered = bundle.clone();
        let rule = &mut altered.sectors[0].rules[0];
        if change_source {
            rule.sources[0].basis_row = usize::MAX / 2;
        } else {
            rule.rhs[0].coefficient = format!("2*({})", rule.rhs[0].coefficient);
        }
        let bytes = codec::write(&altered, limits).unwrap();
        assert!(certify_candidates(CandidateCertificationRequest::new(bytes)).is_err());
    }
    let mut missing = bundle;
    missing.sectors.clear();
    assert!(
        certify_candidates(CandidateCertificationRequest::new(
            codec::write(&missing, limits).unwrap()
        ))
        .is_err()
    );
}

#[test]
fn native_namespaces_affine_faces_and_whole_exclusions_survive_codec() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K3)).unwrap();
    let limits = CandidateBundleLimits::default();
    let mut bundle = codec::read(generated.bundle(), limits).unwrap();
    let family = preparation::family(K3, InputFormat::Toml).unwrap();
    let prepared = preparation::prepare::<3>(family, &[true; 3], None).unwrap();
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .unwrap()
        .context()
        .clone();
    let mut solved = codec::solutions::<3>(
        &bundle,
        &context,
        prepared.sources.index_variables(),
        limits,
    )
    .unwrap();
    let (_, solution) = solved
        .iter_mut()
        .find(|(sector, _)| *sector == [true; 3])
        .unwrap();
    let equation = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap()
        .raw()
        .numerator
        .clone();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::new([None, None, Some(1)]).unwrap(),
        &[equation.clone()],
        prepared.sources.index_variables(),
        &[true; 3],
    )
    .unwrap() else {
        panic!("expected an affine fixture");
    };
    let rule = &mut solution.rules[0];
    rule.candidate.target = case.face().integral();
    rule.candidate.case = case.into();
    rule.candidate.rhs.clear();
    rule.candidate.sources.clear();
    rule.exceptions.branches = vec![vec![
        equation,
        context.index(2).unwrap().raw().numerator.clone(),
    ]];
    bundle.sectors = solved
        .iter()
        .map(|(sector, solution)| codec::sector_record(*sector, solution))
        .collect();
    let bytes = codec::write(&bundle, limits).unwrap();
    let decoded = codec::read(&bytes, limits).unwrap();
    let reconstructed = codec::solutions::<3>(
        &decoded,
        &context,
        prepared.sources.index_variables(),
        limits,
    )
    .unwrap();
    let (_, solution) = reconstructed
        .iter()
        .find(|(sector, _)| *sector == [true; 3])
        .unwrap();
    assert!(solution.rules[0].candidate.case.affine().is_some());
    assert_eq!(
        solution.rules[0].candidate.case.fixed(),
        &[None, None, Some(1)]
    );
    assert_eq!(solution.rules[0].exceptions.branches.len(), 1);
    assert_eq!(solution.rules[0].exceptions.branches[0].len(), 2);
    for equation in &solution.rules[0].exceptions.branches[0] {
        assert_eq!(
            equation.variables(),
            context.index(0).unwrap().raw().numerator.variables()
        );
    }
}
