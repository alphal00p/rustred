//! Complete checkpoint consumption uses the same uncertified reducer; these
//! small external family inputs never become production dispatch conditions.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use rustred::family::IntegralKey;
use rustred::persistence::{BinaryProgramKind, BinarySection, encode_program};
use rustred::solver::CandidateReductionError;

use super::super::checkpoint::{CheckpointManifest, CheckpointStore};
use super::checkpoint::Directory;
use super::*;

fn files(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn save(directory: &Directory, request: &mut FamilyCandidatesRequest, bundle: &Bundle) {
    let manifest = CheckpointManifest::for_request(
        request,
        &bundle.family_fingerprint,
        &bundle.root_sector,
        bundle
            .sectors
            .iter()
            .map(|sector| sector.sector.clone())
            .collect(),
    )
    .unwrap();
    let mut options = directory.options();
    let store = CheckpointStore::open(&options, manifest, request.bundle_limits).unwrap();
    for (ordinal, sector) in bundle.sectors.iter().enumerate() {
        let mut shard = bundle.clone();
        shard.sectors = vec![sector.clone()];
        store
            .publish(
                ordinal,
                &codec::write(&shard, request.bundle_limits).unwrap(),
            )
            .unwrap();
    }
    drop(store);
    options.resume = true;
    request.checkpoint = Some(options);
}

#[test]
fn complete_checkpoint_loader_matches_monolith_without_writes_or_workers() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(1);
    request.permutation = Some(vec![2, 0, 1]);
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    request.checkpoint = Some(directory.options());
    let generated = family_candidates(request.clone()).unwrap();
    request.checkpoint.as_mut().unwrap().resume = true;
    // Loading does not create an executor or invoke generation: generation
    // rejects this unused worker count while read-only loading accepts it.
    request.n_cores = 0;
    let path = &request.checkpoint.as_ref().unwrap().directory;
    let before = files(path);
    let (family, mut sharded) =
        load_generated_candidate_checkpoint::<3>(&request, Default::default()).unwrap();
    let (_, mut monolithic) = load_generated_candidate_bundle::<3>(
        generated.bundle(),
        request.bundle_limits,
        Default::default(),
    )
    .unwrap();
    assert_eq!(family.fingerprint(), monolithic.family_fingerprint());
    assert_eq!(sharded.ordering(), monolithic.ordering());
    assert_eq!(sharded.terminals(), monolithic.terminals());
    assert_eq!(sharded.max_numerator_rank(), Some(1));
    let targets = [[3, 1, 1], [0, 3, 1], [-1, 2, 1], [0, 0, 1]]
        .into_iter()
        .map(|powers| IntegralKey::try_new(powers).unwrap())
        .collect::<Vec<_>>();
    let actual = sharded
        .trace_targets(targets.clone(), Default::default())
        .unwrap();
    assert_eq!(
        actual,
        monolithic
            .trace_targets(targets.clone(), Default::default())
            .unwrap()
    );
    assert!(actual.uncovered().is_empty());
    for target in targets {
        assert_eq!(
            sharded.reduce_unit_mass(&target).unwrap().terms(),
            monolithic.reduce_unit_mass(&target).unwrap().terms()
        );
    }
    assert_eq!(before, files(path));
}

#[test]
fn complete_checkpoint_loader_rejects_identity_changes_without_writing() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(0);
    request.checkpoint = Some(directory.options());
    family_candidates(request.clone()).unwrap();
    request.checkpoint.as_mut().unwrap().resume = true;
    let path = request.checkpoint.as_ref().unwrap().directory.clone();
    let before = files(&path);
    let mut variants = Vec::new();
    let mut changed = request.clone();
    changed.max_numerator_rank = Some(1);
    variants.push(("rank", changed));
    let mut changed = request.clone();
    changed.nonpositive_indices = vec![0];
    variants.push(("root", changed));
    let mut changed = request.clone();
    changed.permutation = Some(vec![2, 0, 1]);
    variants.push(("ordering", changed));
    let mut changed = request.clone();
    changed.exact_backend = CandidateExactBackend::SparseFactorized;
    variants.push(("backend", changed));
    let mut changed = request.clone();
    changed.numerical_depth = 1;
    variants.push(("depth", changed));
    let mut changed = request.clone();
    changed.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    variants.push(("finite policy", changed));
    let mut changed = request.clone();
    changed.source.push('\n');
    variants.push(("source", changed));
    for (field, changed) in variants {
        let error =
            load_generated_candidate_checkpoint::<3>(&changed, Default::default()).unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Input, "{field}: {error}");
        // Narrowing the root shrinks its expected ordinal range, so existing
        // store admission rejects an out-of-range filename before reaching the
        // later whole-manifest comparison. Every other mutation keeps ordinals.
        let expected = if field == "root" {
            "unexpected checkpoint file"
        } else {
            "manifest differs"
        };
        assert!(error.message().contains(expected), "{field}: {error}");
        assert_eq!(before, files(&path));
    }
    assert!(
        load_generated_candidate_checkpoint::<1>(&request, Default::default())
            .unwrap_err()
            .message()
            .contains("arity mismatch")
    );
    assert_eq!(before, files(&path));
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.join("checkpoint.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(
        load_generated_candidate_checkpoint::<3>(&request, Default::default())
            .unwrap_err()
            .message()
            .contains("active owner")
    );
    assert_eq!(before, files(&path));

    drop(lock);
    // A structural count header is inspected without native import. Inflating
    // it in every local dictionary demonstrates that cumulative coefficient
    // count is not merely reset to the per-shard allowance.
    for (name, bytes) in &before {
        if !name.ends_with(".rrbin") {
            continue;
        }
        let (envelope, _, _) = codec::read_structure(bytes, request.bundle_limits).unwrap();
        let mut table = envelope.section(SectionTag::COEFFICIENTS).unwrap().to_vec();
        table[..8].copy_from_slice(&100_000_u64.to_le_bytes());
        let sections = envelope
            .sections()
            .iter()
            .map(|section| BinarySection {
                tag: section.tag,
                bytes: if section.tag == SectionTag::COEFFICIENTS {
                    &table
                } else {
                    section.bytes
                },
            })
            .collect::<Vec<_>>();
        fs::write(
            path.join(name),
            encode_program(
                BinaryProgramKind::Candidates,
                &sections,
                request.bundle_limits.binary_limits(),
            )
            .unwrap(),
        )
        .unwrap();
    }
    let before = files(&path);
    let mut counts = request.clone();
    counts.bundle_limits.max_collection_entries = 150_000;
    let error = load_generated_candidate_checkpoint::<3>(&counts, Default::default()).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Limit);
    assert!(error.message().contains("aggregate coefficient-entry"));
    assert_eq!(before, files(&path));
}

#[test]
fn complete_checkpoint_loader_requires_existing_complete_uncorrupted_directory() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    assert!(load_generated_candidate_checkpoint::<3>(&request, Default::default()).is_err());
    request.checkpoint = Some(directory.options());
    assert!(
        load_generated_candidate_checkpoint::<3>(&request, Default::default())
            .unwrap_err()
            .message()
            .contains("explicit resume")
    );
    assert!(!request.checkpoint.as_ref().unwrap().directory.exists());
    request.checkpoint.as_mut().unwrap().resume = true;
    assert!(load_generated_candidate_checkpoint::<3>(&request, Default::default()).is_err());
    assert!(!request.checkpoint.as_ref().unwrap().directory.exists());
    request.checkpoint.as_mut().unwrap().resume = false;
    family_candidates(request.clone()).unwrap();
    request.checkpoint.as_mut().unwrap().resume = true;
    let path = request.checkpoint.as_ref().unwrap().directory.clone();
    let shard = path.join("sector-0.rrbin");
    let saved = fs::read(&shard).unwrap();
    fs::remove_file(&shard).unwrap();
    let before = files(&path);
    assert!(
        load_generated_candidate_checkpoint::<3>(&request, Default::default())
            .unwrap_err()
            .message()
            .contains("1 missing, first ordinal 0")
    );
    assert_eq!(before, files(&path));
    fs::write(&shard, &saved[..saved.len() - 1]).unwrap();
    let before = files(&path);
    assert!(load_generated_candidate_checkpoint::<3>(&request, Default::default()).is_err());
    assert_eq!(before, files(&path));
    fs::write(&shard, saved).unwrap();
    fs::remove_file(path.join("checkpoint.lock")).unwrap();
    let before = files(&path);
    assert!(load_generated_candidate_checkpoint::<3>(&request, Default::default()).is_err());
    assert_eq!(before, files(&path));
}

#[test]
fn complete_checkpoint_loader_preserves_zero_only_explicit_rank() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K1);
    request.nonpositive_indices = vec![0];
    request.max_numerator_rank = Some(0);
    request.checkpoint = Some(directory.options());
    family_candidates(request.clone()).unwrap();
    request.checkpoint.as_mut().unwrap().resume = true;
    let (_, mut reducer) =
        load_generated_candidate_checkpoint::<1>(&request, Default::default()).unwrap();
    assert_eq!(reducer.max_numerator_rank(), Some(0));
    assert!(
        reducer
            .reduce_unit_mass(&IntegralKey::try_new([0]).unwrap())
            .unwrap()
            .terms()
            .is_empty()
    );
    assert!(matches!(
        reducer.reduce_unit_mass(&IntegralKey::try_new([-1]).unwrap()),
        Err(CandidateReductionError::OutsideNumeratorRank { .. })
    ));
}

#[test]
fn complete_checkpoint_loader_preflights_aggregate_budgets_before_native_import() {
    let directory = Directory::new();
    let mut request = FamilyCandidatesRequest::new(K3);
    request.numerical_depth = 0;
    let generated = family_candidates(request.clone()).unwrap();
    let mut bundle = codec::read(generated.bundle(), request.bundle_limits).unwrap();
    // Inflate structural entries without algebra or numerical work. These are
    // deliberately repeated saved residual records for an ingress-only test.
    for sector in &mut bundle.sectors {
        sector.rules.clear();
        sector.finite_residuals = vec![
            IntegralRecord {
                symbolic: vec![false; 3],
                values: sector.sector.iter().map(|&x| i16::from(x)).collect()
            };
            100
        ];
    }
    save(&directory, &mut request, &bundle);
    let path = request.checkpoint.as_ref().unwrap().directory.clone();
    let first = path.join("sector-0.rrbin");
    let bytes = fs::read(&first).unwrap();
    let (envelope, _, _) = codec::read_structure(&bytes, request.bundle_limits).unwrap();
    let sections = envelope
        .sections()
        .iter()
        .map(|section| BinarySection {
            tag: section.tag,
            bytes: if section.tag == SectionTag::SYMBOLICA_STATE {
                b"invalid native state"
            } else {
                section.bytes
            },
        })
        .collect::<Vec<_>>();
    fs::write(
        &first,
        encode_program(
            BinaryProgramKind::Candidates,
            &sections,
            request.bundle_limits.binary_limits(),
        )
        .unwrap(),
    )
    .unwrap();
    let before = files(&path);
    let mut structural = request.clone();
    structural.bundle_limits.max_collection_entries = 200;
    let error =
        load_generated_candidate_checkpoint::<3>(&structural, Default::default()).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Limit);
    assert!(error.message().contains("aggregate collection-entry"));
    // Each local coefficient table fits; only their sum exceeds this budget.
    let local_max = before
        .iter()
        .filter(|(name, _)| name.ends_with(".rrbin"))
        .map(|(_, bytes)| {
            codec::read_structure(bytes, request.bundle_limits)
                .unwrap()
                .0
                .section(SectionTag::COEFFICIENTS)
                .unwrap()
                .len()
        })
        .max()
        .unwrap();
    let mut coefficients = request.clone();
    coefficients.bundle_limits.max_total_coefficient_bytes = local_max;
    let error =
        load_generated_candidate_checkpoint::<3>(&coefficients, Default::default()).unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Limit);
    assert!(error.message().contains("aggregate coefficient-table"));
    let mut disk = request.clone();
    disk.checkpoint.as_mut().unwrap().max_total_bytes = 1;
    assert_eq!(
        load_generated_candidate_checkpoint::<3>(&disk, Default::default())
            .unwrap_err()
            .kind(),
        AppErrorKind::Limit
    );
    let mut shard = request.clone();
    shard.bundle_limits.max_bundle_bytes = 1;
    assert_eq!(
        load_generated_candidate_checkpoint::<3>(&shard, Default::default())
            .unwrap_err()
            .kind(),
        AppErrorKind::Limit
    );
    assert_eq!(before, files(&path));
}

#[test]
fn complete_checkpoint_loader_keeps_above_rank_children_internal_and_gaps_uncovered() {
    // Synthetic, explicitly uncertified routing fixtures test the application
    // boundary, not an asserted IBP relation. No topology is dispatched on.
    for retain_child in [true, false] {
        let directory = Directory::new();
        let mut request = FamilyCandidatesRequest::new(K3);
        request.max_numerator_rank = Some(0);
        request.numerical_depth = 0;
        let generated = family_candidates(request.clone()).unwrap();
        let mut bundle = codec::read(generated.bundle(), request.bundle_limits).unwrap();
        let context =
            ParametricIbpGenerator::try_new(&preparation::family(K3, InputFormat::Toml).unwrap())
                .unwrap()
                .context()
                .clone();
        let one = (0..bundle.coefficients.len())
            .find(|&i| {
                let coefficient = bundle
                    .coefficients
                    .coefficient(CoefficientId::try_from_index(i).unwrap())
                    .unwrap();
                coefficient == context.one().raw()
                    && coefficient.numerator.variables()
                        == context.one().raw().numerator.variables()
            })
            .unwrap() as u32;
        let root = IntegralRecord {
            symbolic: vec![false; 3],
            values: vec![1, 1, 1],
        };
        let child = IntegralRecord {
            symbolic: vec![false; 3],
            values: vec![-3, 1, 1],
        };
        for sector in &mut bundle.sectors {
            sector.rules.clear();
            sector.finite_residuals.clear();
            if sector.sector == [true; 3] {
                sector.rules.push(RuleRecord {
                    case: CaseRecord {
                        kind: "coordinate".into(),
                        fixed_axes: vec![0, 1, 2],
                        fixed_values: vec![1, 1, 1],
                        equations: vec![],
                    },
                    target: root.clone(),
                    rhs: vec![TermRecord {
                        integral: child.clone(),
                        coefficient: one,
                    }],
                    sources: vec![],
                    exclusions: vec![],
                });
            } else if retain_child && sector.sector == [false, true, true] {
                sector.finite_residuals.push(child.clone());
            }
        }
        save(&directory, &mut request, &bundle);
        let bytes = codec::write(&bundle, request.bundle_limits).unwrap();
        let (_, mut monolithic) =
            load_generated_candidate_bundle::<3>(&bytes, request.bundle_limits, Default::default())
                .unwrap();
        let (_, mut sharded) =
            load_generated_candidate_checkpoint::<3>(&request, Default::default()).unwrap();
        let root = IntegralKey::try_new([1, 1, 1]).unwrap();
        let child = IntegralKey::try_new([-3, 1, 1]).unwrap();
        let report = sharded
            .trace_targets([root.clone()], Default::default())
            .unwrap();
        assert_eq!(
            report,
            monolithic
                .trace_targets([root.clone()], Default::default())
                .unwrap()
        );
        assert_eq!(report.max_negative_index_degree(), 3);
        if retain_child {
            assert_eq!(
                report.declared_terminals(),
                &BTreeSet::from([child.clone()])
            );
            assert!(report.uncovered().is_empty());
            assert_eq!(
                sharded.reduce_unit_mass(&root).unwrap().terms(),
                monolithic.reduce_unit_mass(&root).unwrap().terms()
            );
            assert!(sharded.terminals().contains(&child));
        } else {
            assert_eq!(report.uncovered(), &BTreeSet::from([child.clone()]));
            assert!(
                matches!(sharded.reduce_unit_mass(&root), Err(CandidateReductionError::Uncovered { target }) if target == child)
            );
        }
        assert!(matches!(
            sharded.reduce_unit_mass(&child),
            Err(CandidateReductionError::OutsideNumeratorRank { .. })
        ));
    }
}
