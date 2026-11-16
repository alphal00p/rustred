//! Lane wireflow (closeloop.md 25, item h): pure tests (no process). The node settings `ship_gate` and `pole_profiles`
//! are omitted from the node's serialisation at their defaults (config digests unchanged), parse and validate; the
//! closing loop's choice annotation takes the engine's per-key pole order when `pole_profiles` is on.

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
    assert!(!s.contains("ship_gate") && !s.contains("pole_profiles"), "{s}");
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
