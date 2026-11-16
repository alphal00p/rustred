//! Lane wireflow (closeloop.md 25 / 26, items h, b): pure tests (no process). The node settings `ship_gate`,
//! `pole_profiles` and `facts_frontier` are omitted from the node's serialisation at their defaults (config digests
//! unchanged), parse and validate; the closing loop's choice annotation takes the engine's per-key pole order when
//! `pole_profiles` is on.

use rustred_campaign::closeloop::accept::solved_choice;
use rustred_campaign::closeloop::config::CloseLoopConfig;
use rustred_campaign_core::poles::PoleObjective;

fn cfg(extra: &str) -> String {
    format!(
        r#"{{"schema":"rustred.campaign.closeloop.v1","nodes":[{{"node":"toy","import_spec":"/x.json",
        "solve":{{"program":"p","family":"/f","order":"o","relation_rule":"first_admissible","max_keys_per_job":1,
        "threads":1,"cap_bytes":1,"expected_peak_bytes":1,"heartbeat_s":1,"stop_grace_s":1}}{extra}}}]}}"#
    )
}

#[test]
fn ship_gate_and_pole_profiles_are_omitted_at_their_defaults() {
    let d = CloseLoopConfig::from_json(&cfg("")).unwrap();
    let s = serde_json::to_string(&d.nodes[0]).unwrap();
    assert!(
        !s.contains("ship_gate") && !s.contains("pole_profiles"),
        "{s}"
    );
    assert_eq!(d.nodes[0].ship_gate.mode, "report");
    assert!(!d.nodes[0].pole_profiles);
    let e = CloseLoopConfig::from_json(&cfg(
        r#","ship_gate":{"mode":"enforce","disk":false},"pole_profiles":true"#,
    ))
    .unwrap();
    assert_eq!(e.nodes[0].ship_gate.mode, "enforce");
    assert!(!e.nodes[0].ship_gate.disk);
    assert!(e.nodes[0].pole_profiles);
    assert!(CloseLoopConfig::from_json(&cfg(r#","ship_gate":{"modus":"off"}"#)).is_err());
}

#[test]
fn the_choice_uses_the_engine_pole_order_when_given() {
    let o = PoleObjective::default();
    let none = solved_choice(&o, "rel", 7, None);
    let reg = solved_choice(&o, "rel", 7, Some(0));
    let sing = solved_choice(&o, "rel", 7, Some(1));
    assert_eq!(none["chosen"], "rel");
    assert_eq!(reg["chosen"], "rel");
    // without a profile the annotation says so; with one it records the order it used
    assert_ne!(none, reg, "{none} vs {reg}");
    assert_ne!(reg, sing, "{reg} vs {sing}");
    assert!(none.to_string().contains("none"), "{none}");
}

#[test]
fn facts_frontier_is_omitted_when_absent_and_validated_when_given() {
    let d = CloseLoopConfig::from_json(&cfg("")).unwrap();
    assert!(d.nodes[0].facts_frontier.is_none());
    assert!(
        !serde_json::to_string(&d.nodes[0])
            .unwrap()
            .contains("facts_frontier")
    );
    let proof = r#","exact_tier":true,"proof":{"program":"rr_proof","family":"/f"}"#;
    let on = CloseLoopConfig::from_json(&cfg(&format!(
        r#"{proof},"facts_frontier":{{"max_pairs":100000,"max_jobs":40}}"#
    )))
    .unwrap();
    let f = on.nodes[0].facts_frontier.as_ref().unwrap();
    assert_eq!(
        (
            f.max_pairs,
            f.max_jobs,
            f.max_keys_per_job,
            f.prefetch_outside
        ),
        (100000, 40, 500000, true)
    );
    // without the exact tier, with too large chunks, with a misspelt field, against keys_only_first false: refused
    assert!(
        CloseLoopConfig::from_json(&cfg(r#","facts_frontier":{"max_pairs":1,"max_jobs":1}"#))
            .is_err()
    );
    assert!(
        CloseLoopConfig::from_json(&cfg(&format!(
            r#"{proof},"facts_frontier":{{"max_pairs":1,"max_jobs":1,"max_keys_per_job":500001}}"#
        )))
        .is_err()
    );
    assert!(
        CloseLoopConfig::from_json(&cfg(&format!(
            r#"{proof},"facts_frontier":{{"max_pairs":1,"max_jobs":1,"prefetch":false}}"#
        )))
        .is_err()
    );
    assert!(CloseLoopConfig::from_json(&cfg(&format!(
        r#"{proof},"scale":{{"keys_only_first":false}},"facts_frontier":{{"max_pairs":1,"max_jobs":1}}"#
    )))
    .is_err());
}
