use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;

use super::ParametricReplayEvidence;

#[test]
fn anchored_evidence_retains_real_counts_anchor_and_pivots() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let rule = &artifact.rules()[0];
    assert!(matches!(
        rule.replay_evidence(),
        ParametricReplayEvidence::Anchored { .. }
    ));
    assert!(rule.replay().unwrap().source_rows_used() > 0);
    assert_eq!(
        rule.anchor(),
        Some(rule.concrete_replay().unwrap().anchor())
    );
    assert!(rule.pivot_guard().is_some());
    assert!(!rule.elimination_pivot_guards().is_empty());
    assert!(rule.replay_evidence().combined_original_domain().is_none());
}

#[test]
fn combined_evidence_never_inherits_an_anchored_fixtures_history() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut rule = artifact.rules()[0].clone();
    assert!(rule.pivot_guard().is_some());
    // Only evidence is replaced: stale internal anchored data must not be
    // observable as a combined producer's elimination transcript.
    rule.replace_replay_with_uncertified_combined_domain_for_test();
    assert!(rule.replay().is_none());
    assert!(rule.concrete_replay().is_none());
    assert!(rule.anchor().is_none());
    assert!(rule.pivot_guard().is_none());
    assert!(rule.elimination_pivot_guards().is_empty());
    let domain = rule.replay_evidence().combined_original_domain().unwrap();
    assert_eq!(domain.sector(), rule.sector());
    assert!(domain.fixed_restrictions().is_empty());
    assert_eq!(domain.application.len(), 1);
    assert_eq!(domain.application[0].lower(), [0]);
    assert_eq!(domain.application[0].upper(), [None]);
    assert!(domain.affine_exclusions().is_empty());
    assert_eq!(rule.clone().replay_evidence(), rule.replay_evidence());
}
