//! Lane wireflow (interfaces/repairflow.md 20 / 21): the core half of the wiring. Pure tests (no process).
//! - every new policy setting is OMITTED from the canonical policy at its default (existing digests unchanged), and an
//!   explicitly written default gives the same digest as an absent one;
//! - the mixed-shell VARIANT steps (`mixed_shell[@up][+SHELL]*`): grammar, extra shells, up centres, a plain step's
//!   recipe unchanged;
//! - `composite_verify::verify_composed_image` is steps (b)-(d) of `verify_shipped` (its own tests stay in the module).

use rustred_campaign_core::repairflow::policy::{PolicyFile, RepairPolicy};
use rustred_campaign_core::repairflow::recipes::{
    MIXED_SHELL, RecipeContext, face_recipe, mixed_shell_variant,
};
use rustred_campaign_core::termmin::steps::{mixed_shell_offsets, mixed_shell_offsets_with};
use rustred_campaign_core::worklist::items::Face;

fn file(rp: serde_json::Value) -> RepairPolicy {
    PolicyFile::parse(
        &serde_json::json!({"schema": "rustred.campaign.repairflow.v1", "repair_policy": rp, "nodes": {}})
            .to_string(),
    )
    .unwrap()
    .effective(None)
    .unwrap()
}

#[test]
fn new_settings_at_their_defaults_leave_the_canonical_policy_and_its_digest_unchanged() {
    let d = RepairPolicy::default();
    let c = d.canonical();
    for w in ["composite_jobs", "tworow", "pieces"] {
        assert!(!c.contains(w), "{w} in the default canonical policy: {c}");
    }
    let explicit = file(serde_json::json!({
        "route0": {"composite_jobs": {"enabled": false, "max_rounds": 4, "program": "rr_rf_symflow", "threads": 1,
                                      "max_attempts": 2, "wall_s": 14400, "kernel": null}},
        "tworow": {"enabled": false}, "pieces": {"plan": false}}));
    assert_eq!(explicit.digest(), d.digest());
    assert_eq!(file(serde_json::json!({})).digest(), d.digest());
    // switched on: part of the digest
    let on = file(serde_json::json!({"route0": {"composite_jobs": {"enabled": true}}, "pieces": {"plan": true}}));
    assert_ne!(on.digest(), d.digest());
    assert!(on.canonical().contains("composite_jobs") && on.canonical().contains("pieces"));
}

#[test]
fn composite_jobs_settings_are_validated() {
    let bad = |rp: serde_json::Value| {
        PolicyFile::parse(
            &serde_json::json!({"schema": "rustred.campaign.repairflow.v1", "repair_policy": rp, "nodes": {}})
                .to_string(),
        )
        .is_err()
    };
    assert!(bad(serde_json::json!({"route0": {"composite_jobs": {"enabled": true, "max_rounds": 0}}})));
    assert!(bad(serde_json::json!({"route0": {"composite_jobs": {"enabled": true,
        "kernel": {"program": "rr_proof", "sha256": "xyz"}}}})));
    assert!(bad(serde_json::json!({"route0": {"composite_jobs": {"enabled": true,
        "kernel": {"program": "rr_proof", "sha256": "a".repeat(64), "census": "lee"}}}})));
    assert!(bad(serde_json::json!({"tworow": {"enabled": true}})));
    assert!(!bad(serde_json::json!({"route0": {"composite_jobs": {"enabled": true,
        "kernel": {"program": "rr_proof", "sha256": "a".repeat(64)}}}, "tworow": {"enabled": true}})));
    assert!(bad(serde_json::json!({"recipes": {"escalation": ["mixed_shell@down"]}})));
    assert!(bad(serde_json::json!({"recipes": {"escalation": ["mixed_shell+S9x"]}})));
    assert!(!bad(serde_json::json!({"recipes": {"escalation": ["mixed_shell@up+S2+S2m1", "mixed_shell+S2"]}})));
}

#[test]
fn mixed_shell_variants_add_up_centres_and_shells() {
    assert_eq!(mixed_shell_variant(MIXED_SHELL), Some(Ok((false, vec![]))));
    assert_eq!(
        mixed_shell_variant("mixed_shell@up+S2+S2m1"),
        Some(Ok((true, vec!["S2".into(), "S2m1".into()])))
    );
    assert!(mixed_shell_variant("d2_clast_fon_mM1C").is_none());
    assert!(matches!(mixed_shell_variant("mixed_shellx"), Some(Err(_))));
    // up centres: each dotted line one up, each off-floor numerator one up (raw + 1)
    let lines = [true, true, true, false, false];
    let key = [2, 1, 3, -1, 0];
    let shells: Vec<String> = vec!["S0".into()];
    let plain = mixed_shell_offsets(&key, &lines, &shells).unwrap();
    let up = mixed_shell_offsets_with(&key, &lines, &shells, true).unwrap();
    assert_eq!(plain, mixed_shell_offsets_with(&key, &lines, &shells, false).unwrap());
    assert!(up.starts_with(&plain));
    for c in [[1, 0, 0, 0, 0], [0, 0, 1, 0, 0], [0, 0, 0, 1, 0]] {
        assert!(up.contains(&c.to_vec()), "{c:?} not in {up:?}");
        assert!(!plain.contains(&c.to_vec()));
    }
    assert!(!up.contains(&vec![0, 1, 0, 0, 0]), "a line at its floor gets no up centre");
    // face recipes: the plain step is unchanged by the variant code; the variant names itself and its up centres
    let face = Face::new(vec![0, 2], vec![3]);
    let ctx = RecipeContext {
        node: "N",
        node_order: "presented",
        policy_digest: "p",
        mixed_shells: vec!["S0".into(), "S1".into()],
    };
    let p = face_recipe(&face, MIXED_SHELL, &lines, &[0, 1, 2], &[3, 4], &ctx).unwrap();
    assert_eq!(p.id, "rf/d0.2_n3/mixed_shell");
    assert!(p.provenance.as_ref().unwrap().get("up_centres").is_none());
    let v = face_recipe(&face, "mixed_shell@up+S2", &lines, &[0, 1, 2], &[3, 4], &ctx).unwrap();
    assert_eq!(v.id, "rf/d0.2_n3/mixed_shell@up+S2");
    assert_eq!(v.provenance.as_ref().unwrap()["up_centres"], true);
    assert_eq!(
        v.provenance.as_ref().unwrap()["shells_harder_unit"],
        serde_json::json!(["S0", "S1", "S2"])
    );
    assert!(v.seeds.centres.len() > p.seeds.centres.len());
}
