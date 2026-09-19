use rustred::foundry::artifact::ClosedArtifact;
use rustred::identity::ParametricIbpGenerator;
use rustred::persistence::{
    CoefficientId, CoefficientTableBuilder, DecodedCoefficientTable, EncodedCoefficientTable,
    SectionTag, inspect_program,
};
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

fn assert_same_program(left: &[u8], right: &[u8]) {
    let limits = CandidateBundleLimits::default();
    let left = codec::read(left, limits).unwrap();
    let right = codec::read(right, limits).unwrap();
    assert_eq!(left.records, right.records);
    assert_eq!(left.family, right.family);
    assert_eq!(left.coefficients.len(), right.coefficients.len());
    for index in 0..left.coefficients.len() {
        let id = CoefficientId::try_from_index(index).unwrap();
        assert_eq!(
            left.coefficients.coefficient(id).unwrap(),
            right.coefficients.coefficient(id).unwrap()
        );
    }
}

fn replace_solutions<const N: usize>(
    bundle: &mut Bundle,
    solved: &[([bool; N], rustred::solver::SectorSolution<N>)],
    limits: CandidateBundleLimits,
) {
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .unwrap();
    let mut table = CoefficientTableBuilder::new(limits.binary_limits());
    bundle.sectors = solved
        .iter()
        .map(|(sector, solution)| codec::sector_record(*sector, solution, &mut table))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    bundle.family =
        rustred::persistence::NativeFamilyRecord::from_family(&family, &mut table).unwrap();
    let table = table.finish().unwrap();
    bundle.coefficients = std::sync::Arc::new(
        DecodedCoefficientTable::import_generated(
            &table.state,
            &table.atoms,
            limits.binary_limits(),
        )
        .unwrap(),
    );
}

#[test]
fn candidate_materialization_backends_keep_small_case_bundles_identical() {
    assert_eq!("sparse".parse(), Ok(CandidateExactBackend::Sparse));
    assert_eq!(
        "semi-numerical".parse(),
        Ok(CandidateExactBackend::SemiNumerical)
    );
    assert!(
        "reconstruction-with-exact-fallback"
            .parse::<CandidateExactBackend>()
            .is_err()
    );
    for source in [K1, K3] {
        let sparse = family_candidates(FamilyCandidatesRequest::new(source)).unwrap();
        let mut request = FamilyCandidatesRequest::new(source);
        request.exact_backend = CandidateExactBackend::SemiNumerical;
        request.n_cores = 2;
        let reconstructed = family_candidates(request).unwrap();
        assert_same_program(sparse.bundle(), reconstructed.bundle());
        let report: toml::Value = toml::from_str(reconstructed.to_toml()).unwrap();
        assert_eq!(report["exact_backend"].as_str(), Some("semi-numerical"));
        certify_candidates(CandidateCertificationRequest::new(reconstructed.bundle())).unwrap();
    }
}

#[test]
fn candidate_progress_is_observational_and_never_checks_or_installs() {
    use crate::FamilyCloseProgress;
    use std::sync::Mutex;

    let quiet = family_candidates(FamilyCandidatesRequest::new(K3)).unwrap();
    let events = Mutex::new(Vec::new());
    let mut request = FamilyCandidatesRequest::new(K3);
    request.n_cores = 2;
    let observed = family_candidates_with_progress(request, |event| {
        events.lock().unwrap().push(event);
    })
    .unwrap();
    assert_same_program(quiet.bundle(), observed.bundle());
    let events = events.into_inner().unwrap();
    assert!(matches!(
        events.first(),
        Some(FamilyCloseProgress::Preparing { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(FamilyCloseProgress::Encoded { .. })
    ));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, FamilyCloseProgress::GeneratedSector { .. }))
    );
    assert!(events.iter().all(|event| matches!(
        event,
        FamilyCloseProgress::Preparing { .. }
            | FamilyCloseProgress::Prepared { .. }
            | FamilyCloseProgress::Generating { .. }
            | FamilyCloseProgress::GeneratedSector { .. }
            | FamilyCloseProgress::Encoding { .. }
            | FamilyCloseProgress::Encoded { .. }
    )));
}

#[test]
fn candidate_byte_budget_can_grow_without_changing_artifact_defaults() {
    let defaults = CandidateBundleLimits::default();
    assert_eq!(
        defaults.bundle_byte_limit(),
        crate::MAX_CLOSING_ARTIFACT_BYTES
    );
    let enlarged = CandidateBundleLimits {
        max_bundle_bytes: 512 * 1024 * 1024,
        ..defaults
    };
    assert_eq!(enlarged.bundle_byte_limit(), 512 * 1024 * 1024);
    let excessive = CandidateBundleLimits {
        max_bundle_bytes: usize::MAX,
        ..defaults
    };
    assert_eq!(excessive.bundle_byte_limit(), MAX_CANDIDATE_BUNDLE_BYTES);
    assert_eq!(crate::MAX_CLOSING_ARTIFACT_BYTES, 256 * 1024 * 1024);
}

#[test]
fn candidate_byte_budget_is_symmetric_at_the_serialized_boundary() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let defaults = CandidateBundleLimits::default();
    let bundle = codec::read(generated.bundle(), defaults).unwrap();
    let envelope = inspect_program(generated.bundle(), defaults.binary_limits()).unwrap();
    let table = EncodedCoefficientTable {
        state: envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .unwrap()
            .to_vec(),
        atoms: envelope.section(SectionTag::COEFFICIENTS).unwrap().to_vec(),
    };
    let exact = CandidateBundleLimits {
        max_bundle_bytes: generated.bundle().len(),
        ..defaults
    };
    assert_eq!(
        codec::write_records(&bundle.records, &bundle.family, &table, exact).unwrap(),
        generated.bundle()
    );
    assert!(codec::read(generated.bundle(), exact).is_ok());
    let short = CandidateBundleLimits {
        max_bundle_bytes: generated.bundle().len() - 1,
        ..defaults
    };
    assert!(codec::write_records(&bundle.records, &bundle.family, &table, short).is_err());
    assert!(codec::read(generated.bundle(), short).is_err());
}

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
    replace_solutions(&mut bundle, &solved, limits);
    assert_same_program(&codec::write(&bundle, limits).unwrap(), generated.bundle());
    let certified =
        certify_candidates(CandidateCertificationRequest::new(generated.bundle())).unwrap();
    assert_eq!(certified.schema(), CANDIDATE_CERTIFICATION_SCHEMA);
    assert_eq!(certified.status(), "generated-durable");
    let original = family_close(FamilyCloseRequest::new(source)).unwrap();
    assert!(
        crate::equivalent_generated_programs(
            certified.artifact(),
            original.artifact(),
            Default::default()
        )
        .unwrap()
    );
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
    let (family, mut candidate) = load_generated_candidate_bundle::<1>(
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
        load_generated_candidate_bundle::<3>(
            generated.bundle(),
            limits,
            ReductionLimits::default()
        )
        .is_err()
    );
    assert!(
        load_generated_candidate_bundle::<1>(
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
        load_generated_candidate_bundle::<1>(&mutated, limits, ReductionLimits::default())
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
    let (_, mut candidate) = load_generated_candidate_bundle::<3>(
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
    assert_same_program(serial.bundle(), parallel.bundle());
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
    let mut wrong_version = generated.bundle().to_vec();
    wrong_version[8..12].copy_from_slice(&999u32.to_le_bytes());
    let mut trailing = generated.bundle().to_vec();
    trailing.push(0);
    for source in [wrong_version, trailing] {
        assert_eq!(
            codec::read(&source, limits).unwrap_err().kind(),
            AppErrorKind::Schema
        );
    }
    let mut malformed = bundle.clone();
    let duplicate = malformed.sectors[0].clone();
    malformed.sectors.push(duplicate);
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
        assert!(codec::write(&bundle, limited).is_err());
    }
}

#[test]
fn saved_formula_or_source_trace_tampering_never_becomes_certified() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    let bundle = codec::read(generated.bundle(), limits).unwrap();
    for change_source in [false, true] {
        let mut altered = bundle.clone();
        if change_source {
            altered.sectors[0].rules[0].sources[0].basis_row = usize::MAX / 2;
        } else {
            let change_id = altered.sectors[0].rules[0].rhs[0].coefficient as usize;
            let family = preparation::family(K1, InputFormat::Toml).unwrap();
            let context = ParametricIbpGenerator::try_new(&family)
                .unwrap()
                .context()
                .clone();
            let mut table = CoefficientTableBuilder::new(limits.binary_limits());
            for index in 0..altered.coefficients.len() {
                let id = CoefficientId::try_from_index(index).unwrap();
                let value = altered.coefficients.coefficient(id).unwrap();
                let modified;
                let value = if index == change_id {
                    modified = value * context.integer(2).raw();
                    &modified
                } else {
                    value
                };
                assert_eq!(table.intern(value).unwrap(), id);
            }
            let table = table.finish().unwrap();
            altered.coefficients = std::sync::Arc::new(
                DecodedCoefficientTable::import_generated(
                    &table.state,
                    &table.atoms,
                    limits.binary_limits(),
                )
                .unwrap(),
            );
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
    rule.candidate.sources = vec![rustred::solver::SeedSource {
        basis_row: 2,
        seed: rustred::solver::Seed {
            integral: rule.candidate.target,
            shifts: [1, -1, 2],
        },
    }];
    rule.exceptions.branches = vec![
        vec![
            equation.clone(),
            context.index(2).unwrap().raw().numerator.clone(),
        ],
        vec![context.index(0).unwrap().raw().numerator.clone()],
    ];
    let (_, pinch) = solved
        .iter_mut()
        .find(|(sector, _)| *sector == [false, true, true])
        .unwrap();
    let negative_terminal = rustred::solver::Integral::new([
        rustred::solver::Power::new(false, -2).unwrap(),
        rustred::solver::Power::new(false, 1).unwrap(),
        rustred::solver::Power::new(false, 1).unwrap(),
    ]);
    pinch.finite_residuals.push(negative_terminal);
    bundle.permutation = Some(vec![2, 0, 1]);
    replace_solutions(&mut bundle, &solved, limits);
    let bytes = codec::write(&bundle, limits).unwrap();
    let decoded = codec::read(&bytes, limits).unwrap();
    assert_eq!(decoded.records, bundle.records);
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
    assert_eq!(solution.rules[0].candidate.sources[0].basis_row, 2);
    assert_eq!(
        solution.rules[0].candidate.sources[0].seed.shifts,
        [1, -1, 2]
    );
    assert_eq!(
        solution.rules[0].candidate.sources[0].seed.integral,
        solution.rules[0].candidate.target
    );
    assert_eq!(
        solution.rules[0]
            .candidate
            .case
            .affine()
            .unwrap()
            .equations(),
        &[equation]
    );
    assert_eq!(solution.rules[0].exceptions.branches.len(), 2);
    assert_eq!(solution.rules[0].exceptions.branches[0].len(), 2);
    assert_eq!(solution.rules[0].exceptions.branches[1].len(), 1);
    let (_, pinch) = reconstructed
        .iter()
        .find(|(sector, _)| *sector == [false, true, true])
        .unwrap();
    assert!(pinch.finite_residuals.contains(&negative_terminal));
    for equation in &solution.rules[0].exceptions.branches[0] {
        assert_eq!(
            equation.variables(),
            context.index(0).unwrap().raw().numerator.variables()
        );
    }
}

#[test]
fn native_candidate_rejects_wrong_variable_map_and_rational_guards() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    let family = preparation::family(K1, InputFormat::Toml).unwrap();
    let prepared = preparation::prepare::<1>(family, &[true], None).unwrap();
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .unwrap()
        .context()
        .clone();
    let indexed_fraction = context
        .div(&context.one(), &context.index(0).unwrap())
        .unwrap();
    for wrong_map in [true, false] {
        let mut bundle = codec::read(generated.bundle(), limits).unwrap();
        let mut table = CoefficientTableBuilder::new(limits.binary_limits());
        for index in 0..bundle.coefficients.len() {
            let id = CoefficientId::try_from_index(index).unwrap();
            assert_eq!(
                table
                    .intern(bundle.coefficients.coefficient(id).unwrap())
                    .unwrap(),
                id
            );
        }
        let bad = if wrong_map {
            table.intern(&context.base().one()).unwrap()
        } else {
            table.intern(indexed_fraction.raw()).unwrap()
        };
        if wrong_map {
            bundle.sectors[0].rules[0].rhs[0].coefficient = bad.index() as u32;
        } else {
            bundle.sectors[0].rules[0].exclusions = vec![vec![bad.index() as u32]];
        }
        let table = table.finish().unwrap();
        let bytes = codec::write_records(&bundle.records, &bundle.family, &table, limits).unwrap();
        let decoded = codec::read(&bytes, limits).unwrap();
        let error = codec::solutions::<1>(
            &decoded,
            &context,
            prepared.sources.index_variables(),
            limits,
        )
        .unwrap_err();
        assert!(error.message().contains(if wrong_map {
            "wrong indexed variable map"
        } else {
            "not a polynomial"
        }));
    }
}

#[test]
fn native_family_geometry_does_not_parse_provenance() {
    use rustred::family::IntegralKey;
    use rustred::reduction::ReductionLimits;
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let limits = CandidateBundleLimits::default();
    let mut bundle = codec::read(generated.bundle(), limits).unwrap();
    bundle.family_source = "retained provenance is not runtime family syntax".into();
    bundle.input_format = "historical-producer-description".into();
    let bytes = codec::write(&bundle, limits).unwrap();
    let (family, mut reducer) =
        load_generated_candidate_bundle::<1>(&bytes, limits, ReductionLimits::default()).unwrap();
    assert_eq!(family.fingerprint(), bundle.family_fingerprint);
    assert_eq!(
        reducer
            .reduce_unit_mass(&IntegralKey::try_new([3]).unwrap())
            .unwrap()
            .terms()
            .len(),
        1
    );
    certify_candidates(CandidateCertificationRequest::new(bytes)).unwrap();
}
