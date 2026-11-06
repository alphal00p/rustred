//! Eps families through the application paths (owner: M2-T7a rr-eps).
//!
//! An eps family (lines with powers `n + j eps`, power shifts `j (4-d)/2`) is
//! admitted only by the p-only path
//! (`IntegralFamily::shifted_unit_scale_kinematics`, used by the p library).
//! Every existing application consumer keeps refusing it with a typed error
//! (plan D10, critique-eng MF5): a T001c-shaped family (the 2L kite with eps
//! on two lines) still generates, since generation has no admission gate,
//! but its programs are refused when a bundle or a checkpoint is loaded,
//! when a routed campaign prepares its owners, and when the family is
//! closed.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rustred::solver::CandidateFamilyAdmission;
use rustred_app::{
    AppError, CandidateCheckpointOptions, FamilyCandidatesRequest, FamilyCloseRequest,
    RoutedCampaignRequest, family_candidates, family_close, inspect_generated_candidate_bundle,
    load_generated_candidate_bundle, load_generated_candidate_bundle_with_admission,
    load_generated_candidate_checkpoint, routed_campaign_with_progress,
};

/// `rules_study/knockout/families/T001c.sym` at `q.q = gram`.
fn t001c(gram: i64) -> String {
    format!(
        "I(name(T001c),loops(k1,k2),externals(q),dimension(d),\
         prop(D1,(k1)^2,1),prop(D2,(-k1-k2)^2,1),prop(D3,(k2)^2,1),\
         prop(D4,(k1+k2-q)^2,1),prop(D5,(-k2+q)^2,1),gram(q,q,{gram}),\
         power_shift(D1,2-d/2),power_shift(D5,2-d/2))"
    )
}

/// The same kite without eps lines: the control of every refusal below.
fn kite(gram: i64) -> String {
    t001c(gram)
        .replace(",power_shift(D1,2-d/2),power_shift(D5,2-d/2)", "")
        .replace("T001c", "kite")
}

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-p-library-eps-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Generate only the top chart (all five lines positive), which closes for
/// the one-sided solver; the other charts are M2-T7b's.
fn top_request(source: &str) -> FamilyCandidatesRequest {
    let mut request = FamilyCandidatesRequest::new(source);
    request.selected_sectors = Some(vec![vec![true; 5]]);
    request
}

/// A typed refusal of the shifted family by the unit-scale admission
/// (`CandidateReductionError::UnsupportedFamily` or the closing predicate).
fn assert_refused(error: AppError, consumer: &str) {
    let message = error.to_string();
    eprintln!("{consumer}: {message}");
    assert!(
        message.contains("unsupported candidate family")
            || message.contains("no installed closure verifier"),
        "{consumer}: {message}"
    );
}

#[test]
fn shifted_candidate_bundles_are_refused_at_load() {
    for gram in [1, -1] {
        let generated = family_candidates(top_request(&t001c(gram))).unwrap();
        let error = load_generated_candidate_bundle::<5>(
            generated.bundle(),
            Default::default(),
            Default::default(),
        )
        .err()
        .unwrap();
        assert_refused(error, "candidate bundle load");
        for admission in [
            CandidateFamilyAdmission::UnitScalePropagator,
            CandidateFamilyAdmission::UnitMassVacuum,
            CandidateFamilyAdmission::ParametricVacuum,
        ] {
            let error = load_generated_candidate_bundle_with_admission::<5>(
                generated.bundle(),
                Default::default(),
                Default::default(),
                admission,
            )
            .err()
            .unwrap();
            assert_refused(error, &format!("candidate bundle load under {admission:?}"));
        }
        // Control: the unshifted kite loads under the propagator admission.
        let control = family_candidates(top_request(&kite(gram))).unwrap();
        let (_, reducer) = load_generated_candidate_bundle::<5>(
            control.bundle(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            reducer.admission(),
            CandidateFamilyAdmission::UnitScalePropagator
        );
    }
}

#[test]
fn shifted_checkpoints_are_refused_at_load() {
    for (source, shifted) in [(t001c(-1), true), (kite(-1), false)] {
        let directory = Directory::new();
        let mut request = top_request(&source);
        request.checkpoint = Some(CandidateCheckpointOptions::new(directory.0.join("ckpt")));
        family_candidates(request.clone()).unwrap();
        request.checkpoint.as_mut().unwrap().resume = true;
        let loaded = load_generated_candidate_checkpoint::<5>(&request, Default::default());
        match (loaded, shifted) {
            (Err(error), true) => assert_refused(error, "candidate checkpoint load"),
            (Ok(_), false) => {}
            (Err(error), false) => panic!("control checkpoint: {error}"),
            (Ok(_), true) => panic!("a shifted checkpoint was admitted"),
        }
    }
}

#[test]
fn shifted_owner_programs_are_refused_by_the_routed_campaign() {
    let directory = Directory::new();
    let generated = family_candidates(top_request(&t001c(-1))).unwrap();
    let inspection =
        inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
    std::fs::write(directory.0.join("owner.rrbin"), generated.bundle()).unwrap();
    let selection = serde_json::json!({
        "family_fingerprint": inspection.family_fingerprint,
        "owners": [{"path": "owner.rrbin", "bytes": generated.bundle().len(), "mask": "11111"}],
        "initial_frontier_routes": [],
    });
    let mut request = RoutedCampaignRequest::new(selection.to_string(), "1,1,1,1,1\n".into());
    request.owner_base = directory.0.clone();
    let error = routed_campaign_with_progress(request, &AtomicBool::new(false), |_| {})
        .err()
        .unwrap();
    assert_refused(error, "routed campaign prepare");
}

#[test]
fn shifted_families_cannot_be_closed() {
    for gram in [1, -1] {
        let error = family_close(FamilyCloseRequest::new(t001c(gram)))
            .err()
            .unwrap();
        assert_refused(error, "family close");
    }
}
