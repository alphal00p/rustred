use super::*;
use std::{sync::Arc, time::Duration};

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
