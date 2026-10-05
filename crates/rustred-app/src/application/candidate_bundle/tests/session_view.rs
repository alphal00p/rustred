use super::*;
use std::{sync::Arc, time::Duration};

#[test]
fn generated_policy_is_retained_and_file_ingress_uses_explicit_bounds() {
    let mut request = FamilyCandidatesRequest::new(K1);
    request.bundle_limits.max_collection_entries = 10_000_000;
    let result = family_candidates(request.clone()).unwrap();
    assert_eq!(result.bundle_limits(), request.bundle_limits);
    let view = result.artifact().unwrap();
    assert_eq!(
        view.metadata().unwrap()["transport_limits"]["bundle_max_entries"],
        10_000_000
    );
    assert_eq!(view.metadata().unwrap()["decoded_coefficients"], 0);
    let mut restricted = result.clone();
    restricted.bundle_limits.max_collection_entries = 1;
    assert!(
        restricted
            .artifact()
            .err()
            .unwrap()
            .message()
            .contains("aggregate collection-entry")
    );
    let directory = super::checkpoint::Directory::new();
    let path = directory.0.join("candidate.rrbin");
    std::fs::write(&path, result.bundle()).unwrap();
    let loaded = CandidateArtifact::open_file(&path, result.bundle_limits()).unwrap();
    assert_eq!(loaded.metadata().unwrap(), view.metadata().unwrap());
    let mut short = result.bundle_limits();
    short.max_bundle_bytes = result.bundle().len() - 1;
    let error = CandidateArtifact::open_file(path, short).err().unwrap();
    assert!(error.message().contains("exceeds byte limit"));
}

#[test]
fn native_terminal_normalization_is_family_bound_paged_and_structural_only() {
    use rustred::{
        family::IntegralKey, reduction::terminal_normalization::TerminalNormalizationPlan,
    };
    let generated = family_candidates(FamilyCandidatesRequest::new(K3)).unwrap();
    let mut bundle = codec::read(generated.bundle(), Default::default()).unwrap();
    // Three equal independent-product routings, represented in different
    // sectors of one complete native family. No supplied algebraic weights.
    let powers = [[1, 1, 0], [1, 0, 1], [0, 1, 1]];
    let template = bundle.sectors[0].clone();
    bundle.sectors = powers
        .iter()
        .map(|p| {
            let mut sector = template.clone();
            sector.sector = p.iter().map(|&n| n > 0).collect();
            sector.rules.clear();
            sector.finite_residuals = vec![model::IntegralRecord {
                symbolic: vec![false; 3],
                values: p.to_vec(),
            }];
            sector
        })
        .collect();
    let bytes = codec::write(&bundle, Default::default()).unwrap();
    let view = CandidateArtifact::open(&bytes, Default::default()).unwrap();
    let family = preparation::family(K3, InputFormat::Auto).unwrap();
    let normalized = view
        .normalize_terminals(&family, Default::default())
        .unwrap();
    let meta = normalized.metadata().unwrap();
    assert_eq!(meta["raw_terminal_records"], 3);
    assert_eq!(meta["unique_raw_terminals"], 3);
    assert_eq!(meta["canonical_terminals"], 1);
    assert_eq!(meta["closure_claim"], false);
    assert_eq!(meta["master_minimality_claim"], false);
    assert_eq!(normalized.relations(0, 1).unwrap().items.len(), 1);
    assert_eq!(normalized.terminals(0, 1).unwrap().items.len(), 1);
    assert!(normalized.relations(0, 1001).is_err());
    assert!(normalized.relation(3, 1000).is_err());
    assert!(normalized.relation(0, 1).is_err());
    let relation = normalized.relation(0, 1000).unwrap();
    normalized
        .coefficient(
            relation["rhs"][0]["coefficient_id"].as_u64().unwrap() as usize,
            1000,
        )
        .unwrap();
    let raw = powers
        .iter()
        .map(|p| IntegralKey::try_new(p.iter().copied().map(i64::from)).unwrap())
        .collect();
    let rebuilt = TerminalNormalizationPlan::decode_generated(
        &normalized.sidecar().unwrap(),
        &family,
        &raw,
        super::super::order::saved_policy(&bundle.records).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(rebuilt.canonical_terminals().len(), 1);
    assert_eq!(view.metadata().unwrap()["decoded_coefficients"], 0);
    let other = preparation::family(K1, InputFormat::Auto).unwrap();
    assert!(
        view.normalize_terminals(&other, Default::default())
            .err()
            .unwrap()
            .message()
            .contains("family does not match")
    );
    assert!(
        view.normalize_terminals(
            &family,
            CandidateTerminalNormalizationLimits {
                max_terminals: 2,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn native_session_matches_sync_and_lazy_pages_do_not_decode_coefficients() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<CandidateArtifact>();
    send_sync::<CandidateGenerationSession>();
    let baseline = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let (session, job) =
        CandidateGenerationSession::prepare(FamilyCandidatesRequest::new(K1), 1).unwrap();
    assert!(!session.done().unwrap());
    assert!(session.result().is_err());
    job.run();
    assert!(session.wait(Some(Duration::ZERO)).unwrap());
    let events = session.poll_events(1, Duration::ZERO).unwrap();
    assert_eq!(events.snapshot.state, CandidateGenerationState::Completed);
    assert!(events.dropped_events > 0);
    assert_eq!(events.events.len(), 1);
    let result = session.result().unwrap();
    assert_same_program(baseline.bundle(), result.bundle());
    let view = CandidateArtifact::open(result.bundle(), Default::default()).unwrap();
    assert_eq!(view.metadata().unwrap()["decoded_coefficients"], 0);
    let sectors = view.sectors(0, 1).unwrap();
    assert_eq!(sectors.items.len(), 1);
    let rules = view.rules(0, 0, 1).unwrap();
    assert!(!rules.items.is_empty());
    view.terminals(0, 0, 1).unwrap();
    let rule = view.rule(0, 0, 1_000_000).unwrap();
    assert_eq!(view.metadata().unwrap()["decoded_coefficients"], 0);
    let id = rule.rhs[0].coefficient_id as usize;
    let first = view.coefficient(id, 1_000_000).unwrap();
    let second = view.coefficient(id, 1_000_000).unwrap();
    assert_eq!(first.numerator, second.numerator);
    assert_eq!(view.metadata().unwrap()["decoded_coefficients"], 1);
    assert!(view.rules(usize::MAX, 0, 1).is_err());
    assert!(view.sectors(0, 1001).is_err());
    assert!(view.rule(0, 0, 1).is_err());
    assert!(view.coefficient(usize::MAX, 100).is_err());
    // The lazy decoder is precisely the existing native table decoder.
    let old = inspect_generated_candidate_program(
        result.bundle(),
        Default::default(),
        CandidateProgramInspectionOptions {
            include_rhs_coefficients: true,
            ..Default::default()
        },
    )
    .unwrap();
    let old = old
        .coefficients
        .iter()
        .find(|c| c.id as usize == id)
        .unwrap();
    assert_eq!(first.numerator_terms, old.numerator_terms);
    assert_eq!(first.denominator_terms, old.denominator_terms);
}

#[test]
fn typed_family_generation_keeps_native_family_without_source_parse() {
    let family = Arc::new(preparation::family(K1, InputFormat::Auto).unwrap());
    let prepared = preparation::prepare_shared::<1>(family.clone(), &[true], None).unwrap();
    assert!(Arc::ptr_eq(&prepared.family, &family));
    drop(prepared);
    let (session, job) = CandidateGenerationSession::from_family(
        family.clone(),
        FamilyCandidatesRequest::new("deliberately not TOML"),
        8,
    )
    .unwrap();
    job.run();
    let result = session.result().unwrap();
    let loaded = codec::read(result.bundle(), Default::default()).unwrap();
    assert_eq!(loaded.family_fingerprint, family.fingerprint());
    assert!(loaded.family_source.starts_with("native-family:"));
    // Structural source labels are never used to reconstruct the native family.
    certify_candidates(CandidateCertificationRequest::new(result.bundle())).unwrap();
}

#[test]
fn cancellation_and_abandoned_job_do_not_claim_completion() {
    let (session, job) =
        CandidateGenerationSession::prepare(FamilyCandidatesRequest::new(K1), 2).unwrap();
    session.cancel().unwrap();
    assert!(!session.done().unwrap());
    job.run();
    assert!(session.done().unwrap());
    assert_eq!(
        session.snapshot().unwrap().state,
        CandidateGenerationState::Cancelled
    );
    assert!(session.result().is_err());
    let (session, job) =
        CandidateGenerationSession::prepare(FamilyCandidatesRequest::new(K1), 2).unwrap();
    drop(job);
    assert_eq!(
        session.snapshot().unwrap().state,
        CandidateGenerationState::Failed
    );
    assert!(session.result().is_err());
    assert!(CandidateGenerationSession::prepare(FamilyCandidatesRequest::new(K1), 0).is_err());
}

#[cfg(not(feature = "reconstruction"))]
#[test]
fn unavailable_reconstruction_is_explicit_not_a_sparse_fallback() {
    assert!("semi-numerical".parse::<CandidateExactBackend>().is_err());
    let mut request = FamilyCandidatesRequest::new(K1);
    request.exact_backend = CandidateExactBackend::SemiNumerical;
    assert!(
        family_candidates(request)
            .unwrap_err()
            .message()
            .contains("reconstruction")
    );
}
