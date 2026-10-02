//! Mathematical-order identity survives generation, binary reload and replay.
use super::*;
use rustred::order::{
    CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits, OrderDescriptor,
};

fn descriptor(arity: usize) -> OrderDescriptor {
    OrderDescriptor {
        pre_support_degree_rows: vec![],
        support_weights: vec![0; arity],
        support_priority: (0..arity).collect(),
        degree_rows: vec![
            DegreeRow {
                active: vec![1; arity],
                inactive: vec![1; arity],
            },
            DegreeRow {
                active: vec![0; arity],
                inactive: vec![1; arity],
            },
        ],
        coordinate_priority: (0..arity).collect(),
        coordinate_groups: CoordinateGroups::ActiveFirst,
        active_direction: Direction::Descending,
        inactive_direction: Direction::Descending,
    }
}

fn compiled(arity: usize) -> CompiledOrder {
    CompiledOrder::compile(descriptor(arity), Limits::default()).unwrap()
}

#[test]
fn global_degree_tadpole_replays_and_refuses_sector_first_certificates() {
    let mut d = descriptor(1);
    d.pre_support_degree_rows = vec![DegreeRow {
        active: vec![1],
        inactive: vec![1],
    }];
    let expected = CompiledOrder::compile(d, Limits::default()).unwrap();
    let text = r#"{"version":1,"support_weights":[0],"support_priority":[0],
        "pre_support_degree_rows":[{"active":[1],"inactive":[1]}],
        "degree_rows":[{"active":[1],"inactive":[1]},{"active":[0],"inactive":[1]}],
        "coordinate_priority":[0],"coordinate_groups":"active-first",
        "active_direction":"descending","inactive_direction":"descending"}"#;
    let program = CandidateIntegralOrder::from_json(text).unwrap();
    assert_eq!(program, expected);
    assert!(!program.is_support_primary());
    let mut request = FamilyCandidatesRequest::new(K1);
    request.integral_order = Some(program.clone());
    let generated = family_candidates(request.clone()).unwrap();
    request.n_cores = 2;
    assert_same_program(
        generated.bundle(),
        family_candidates(request).unwrap().bundle(),
    );
    let (_, mut reducer) = load_generated_candidate_bundle::<1>(
        generated.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(reducer.ordering().program(), Some(&program));
    for power in 1..=5 {
        assert!(
            !reducer
                .reduce_unit_mass(&rustred::family::IntegralKey::try_new([power]).unwrap())
                .unwrap()
                .terms()
                .is_empty()
        );
    }
    let replayed = super::saved_program::replay(generated.bundle());
    assert!(replayed["rules"].as_u64().unwrap() > 0);
    assert_eq!(replayed["closure_claim"], false);
    // The current certified RuleCell lowering still requires a sector-first
    // witness, independently of the general replay/descent checks above.
    // Do not bypass that authority merely because this K1 example is simple.
    let error =
        certify_candidates(CandidateCertificationRequest::new(generated.bundle())).unwrap_err();
    assert!(
        error
            .message()
            .contains("requires support before physical degree")
    );
    // The bounded induction also assumes sector-first ordering even though
    // fixed-support comparison by F agrees with total excess.
    assert!(
        certify_candidates(
            CandidateCertificationRequest::new(generated.bundle()).with_max_total_excess_degree(3)
        )
        .is_err()
    );
}

#[test]
fn runtime_order_descriptor_is_bounded_strict_and_versioned() {
    let text = r#"{"version":1,"support_weights":[0],"support_priority":[0],
        "degree_rows":[{"active":[1],"inactive":[1]}],"coordinate_priority":[0],
        "coordinate_groups":"active-first","active_direction":"descending",
        "inactive_direction":"descending"}"#;
    let program = CandidateIntegralOrder::from_json(text).unwrap();
    assert_eq!(program.arity(), 1);
    assert!(program.has_total_excess_primary());
    for bad in [
        text.replace("\"version\":1", "\"version\":2"),
        text.replace("\"active\":[1]", "\"active\":[0]"),
        text.replace("\"support_priority\":[0]", "\"support_priority\":[1]"),
        text.replace("\"version\":1", "\"version\":1,\"hidden\":true"),
        text.replace("\"active-first\"", "\"unknown\""),
    ] {
        assert!(CandidateIntegralOrder::from_json(&bad).is_err(), "{bad}");
    }
    assert!(CandidateIntegralOrder::from_json(&" ".repeat(crate::MAX_INPUT_BYTES + 1)).is_err());
}

#[test]
fn programmed_order_rejects_ambiguous_request_before_search() {
    let mut request = FamilyCandidatesRequest::new(K1);
    request.integral_order = Some(compiled(2));
    assert!(
        family_candidates(request.clone())
            .unwrap_err()
            .message()
            .contains("arity")
    );
    request.integral_order = Some(compiled(1));
    request.permutation = Some(vec![0]);
    assert!(
        family_candidates(request)
            .unwrap_err()
            .message()
            .contains("permutation")
    );
}

#[test]
fn programmed_tadpole_generation_reloads_applies_and_replays_exactly() {
    let program = compiled(1);
    let expected = rustred::sector::OrderingPolicy::try_programmed(program.clone()).unwrap();
    let mut request = FamilyCandidatesRequest::new(K1);
    request.integral_order = Some(program);
    let generated = family_candidates(request.clone()).unwrap();
    request.n_cores = 2;
    let parallel = family_candidates(request).unwrap();
    assert_same_program(generated.bundle(), parallel.bundle());
    let decoded = codec::read(generated.bundle(), Default::default()).unwrap();
    let inspection =
        inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
    assert_eq!(inspection.integral_order, expected.stable_id().as_str());
    assert_eq!(
        super::super::order::saved_policy(&decoded.records).unwrap(),
        expected
    );
    assert_eq!(decoded.schema, CANDIDATE_BUNDLE_SCHEMA);
    let (_, mut reducer) = load_generated_candidate_bundle::<1>(
        generated.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(reducer.ordering(), expected);
    for power in 1..=5 {
        let target = rustred::family::IntegralKey::try_new([power]).unwrap();
        assert!(
            !reducer
                .reduce_unit_mass(&target)
                .unwrap()
                .terms()
                .is_empty()
        );
    }
    certify_candidates(CandidateCertificationRequest::new(generated.bundle())).unwrap();
    certify_candidates(
        CandidateCertificationRequest::new(generated.bundle()).with_max_total_excess_degree(3),
    )
    .unwrap();
}

#[test]
fn binary_admission_rejects_program_arity_permutation_and_unknown_order() {
    let mut request = FamilyCandidatesRequest::new(K1);
    request.integral_order = Some(compiled(1));
    let generated = family_candidates(request).unwrap();
    let original = codec::read(generated.bundle(), Default::default()).unwrap();
    let mut variants = vec![];
    let mut wrong = original.clone();
    wrong.integral_order = rustred::sector::OrderingPolicy::try_programmed(compiled(2))
        .unwrap()
        .stable_id()
        .to_string();
    variants.push(wrong);
    let mut wrong = original.clone();
    wrong.permutation = Some(vec![0]);
    variants.push(wrong);
    let mut wrong = original;
    wrong.integral_order = "unknown-order-version".into();
    variants.push(wrong);
    for wrong in variants {
        // Writer admission may reject before producing bytes. If it accepts
        // structural bytes, the untrusted structural reader must still reject.
        if let Ok(bytes) = codec::write(&wrong, Default::default()) {
            assert!(codec::read_structure(&bytes, Default::default()).is_err());
        }
    }
}

#[test]
fn programmed_checkpoint_binds_full_descriptor_and_resumes_without_search() {
    let directory = super::checkpoint::Directory::new();
    let mut request = FamilyCandidatesRequest::new(K1);
    request.integral_order = Some(compiled(1));
    request.checkpoint = Some(directory.options());
    let original = family_candidates(request.clone()).unwrap();
    request.checkpoint.as_mut().unwrap().resume = true;
    let resumed = family_candidates_with_progress(request.clone(), |event| {
        assert!(!matches!(
            event,
            crate::application::FamilyCloseProgress::Generating { .. }
        ));
    })
    .unwrap();
    assert_same_program(original.bundle(), resumed.bundle());
    let (_, loaded) =
        load_generated_candidate_checkpoint::<1>(&request, Default::default()).unwrap();
    assert_eq!(loaded.ordering().program(), request.integral_order.as_ref());
    let mut changed = descriptor(1);
    changed.active_direction = Direction::Ascending;
    request.integral_order = Some(CompiledOrder::compile(changed, Limits::default()).unwrap());
    assert!(
        family_candidates(request)
            .unwrap_err()
            .message()
            .contains("manifest differs")
    );
}
