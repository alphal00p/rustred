use super::*;
use crate::{
    CandidateCertificationRequest, CandidateProgramInspectionOptions,
    inspect_generated_candidate_program,
};

fn marked() -> (Vec<u8>, CheckedPriorityOwnerExport) {
    let (bytes, family) = base();
    let (_, proposal) = request(&family);
    let export = encode_checked_priority_owner_with_policy::<1>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece,
    )
    .unwrap();
    (bytes, export)
}

#[test]
fn checked_after_baseline_export_roundtrips_policy_suffix_terminals_and_native_priority() {
    let (bytes, export) = marked();
    assert_eq!(
        export.dispatch_policy(),
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece
    );
    let old = codec::read(&bytes, Default::default()).unwrap();
    let marked = codec::read(export.bytes(), Default::default()).unwrap();
    assert_eq!(
        marked.schema,
        super::super::super::model::DISPATCH_CANDIDATE_BUNDLE_SCHEMA
    );
    assert_eq!(
        &marked.sectors[0].rules[1..],
        old.sectors[0].rules.as_slice()
    );
    assert_eq!(
        marked.sectors[0].finite_residuals,
        old.sectors[0].finite_residuals
    );
    let report = inspect_generated_candidate_program(
        export.bytes(),
        Default::default(),
        CandidateProgramInspectionOptions::default(),
    )
    .unwrap();
    assert_eq!(
        report.sectors[0].rules[0].dispatch_policy,
        Some("AfterBaselinePartitionWholePiece")
    );
    assert!(
        report.sectors[0].rules[1..]
            .iter()
            .all(|r| r.dispatch_policy.is_none())
    );
    let programs = load(export.bytes());
    assert_eq!(
        disposition(&programs, 2),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    assert_eq!(
        disposition(&programs, 30),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 0 }
    );
    assert_eq!(disposition(&programs, 1), disposition(&load(&bytes), 1));
    let family = marked
        .family
        .to_family(
            &marked.coefficients,
            CandidateBundleLimits::default().family_limits(),
            CandidateBundleLimits::default().binary_limits(),
        )
        .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let indices = [generator.context().base().one().get_variables().len()];
    let solutions =
        codec::solutions::<1>(&marked, generator.context(), &indices, Default::default()).unwrap();
    assert_eq!(
        solutions[0].1.rules[0].dispatch_policy,
        RuleDispatchPolicy::AfterBaselinePartitionWholePiece
    );
    let mut table = CoefficientTableBuilder::new(CandidateBundleLimits::default().binary_limits());
    assert!(
        codec::sector_record([true], &solutions[0].1, &mut table)
            .unwrap_err()
            .to_string()
            .contains("dispatch policy")
    );
    assert!(
        crate::certify_candidates(CandidateCertificationRequest::new(export.bytes()))
            .unwrap_err()
            .to_string()
            .contains("dispatch policy")
    );
    // Old explicit/default export remains the same v2 encoding and JSON shape.
    let (_, proposal) = request(&family);
    let ordinary = encode_checked_priority_owner::<1>(
        &bytes,
        proposal,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut unmarked = marked;
    unmarked.records.rule_dispatch.clear();
    unmarked.records.schema = super::super::super::model::CANDIDATE_BUNDLE_SCHEMA.into();
    let unmarked_bytes = codec::write(&unmarked, Default::default()).unwrap();
    assert_eq!(unmarked_bytes, ordinary.bytes());
    assert_ne!(blake3::hash(export.bytes()), blake3::hash(&unmarked_bytes));
}

#[test]
fn policy_payload_is_cold_reinspected_and_policy_only_change_rejects_resume_and_cold_binding() {
    use crate::{
        OwnerDomainMatchRequest, OwnerDomainWalkCheckpointOptions,
        OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest, OwnerDomainWalkSchedulingPolicy,
        OwnerDomainWalkVerifyOptions, OwnerDomainWalkVerifyReferenceLevers,
        OwnerDomainWalkVerifyReinspect, owner_domain_walk_verify_closure,
        owner_domain_walk_with_progress,
    };
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = Directory(std::env::temp_dir().join(format!(
        "rustred-policy-binding-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&directory.0).unwrap();
    let (_, export) = marked();
    let mut bundle = codec::read(export.bytes(), Default::default()).unwrap();
    let owner_path = directory.0.join("owner.rrbin");
    std::fs::write(&owner_path, export.bytes()).unwrap();
    let fingerprint = bundle.family_fingerprint.clone();
    let selection = |bytes: usize| {
        serde_json::json!({"family_fingerprint":fingerprint,
        "owners":[{"path":"owner.rrbin","bytes":bytes,"mask":"1"}],"initial_frontier_routes":[]})
        .to_string()
    };
    let queries = serde_json::json!({"schema":"rustred.owner-domain-queries.json.v2","queries":[
        {"id":"n2","owner":"1","lower":[1],"upper":[1],"max_numerator_rank":0}]})
    .to_string();
    let mut matching = OwnerDomainMatchRequest::new(selection(export.bytes().len()), queries);
    matching.owner_base = directory.0.clone();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(8).unwrap(),
    };
    request.workers = 1;
    let checkpoint = directory.0.join("checkpoint");
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&checkpoint));
    let result =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(result.document["recursive_worklist_exhausted"], true);
    let mut verify = OwnerDomainWalkVerifyOptions::new(&checkpoint);
    verify.require_closure = true;
    verify.reinspect = OwnerDomainWalkVerifyReinspect::All;
    verify.reference_levers = OwnerDomainWalkVerifyReferenceLevers::Off;
    let report =
        owner_domain_walk_verify_closure(&request, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(
        report["roots_independently_verified"],
        report["roots_total"]
    );
    // No algebra/source/terminal/case changes: move the one policy marker to
    // the next saved rule. Its same-size encoding preserves the exact request
    // and manifests, so owner-payload binding itself must reject this change.
    let old_records = bundle.records.clone();
    assert!(bundle.sectors[0].rules.len() > 1);
    bundle.records.rule_dispatch[0].rule = 1;
    assert_eq!(bundle.sectors, old_records.sectors);
    let changed = codec::write(&bundle, Default::default()).unwrap();
    assert_eq!(changed.len(), export.bytes().len());
    assert_ne!(changed, export.bytes());
    std::fs::write(&owner_path, &changed).unwrap();
    let report =
        owner_domain_walk_verify_closure(&request, &verify, &AtomicBool::new(false), |_| {})
            .unwrap();
    assert_eq!(report["verdict"], "FAIL", "{report}");
    assert!(
        report["violations_by_class"]["binding"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    assert!(report["violations"].as_array().unwrap().iter().any(|v| {
        v.as_str()
            .is_some_and(|s| s.contains("owner payload digests differ"))
    }));
    request.checkpoint.as_mut().unwrap().resume = true;
    assert!(owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).is_err());
}
