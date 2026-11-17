//! Lane sizegen (TRICKS 31 / 35): the effective derivation of a generated rule, from the producing job's own files.

use rustred_campaign_core::economy::derivation::*;

const EVENTS: &str = r#"{"v":1,"seq":0,"t":0.0,"kind":"job_started"}
{"v":1,"seq":5,"t":10.0,"kind":"case_started","ordinal":0,"case":{"equations":[],"fixed":[1,null],"frame":"PRESENTED"}}
{"v":1,"seq":9,"t":20.0,"kind":"engine","event":{"RelationRetry":{"case":{"equations":[],"fixed":[1,null]},"source_order":"input+symbolic_first"}}}
{"v":1,"seq":10,"t":30.0,"kind":"time_decision","what":"screen_sampled","case_ordinal":0,"rule":null,"limit_s":1800.0,"measured_s":115.9,"taken":"sampled reconstruction"}
{"v":1,"seq":11,"t":40.0,"kind":"rule_committed","index":0,"line_offset":10,"line_bytes":99,"canon_blake3":"abc","rhs":3,"trace_rows":5,"monomials":7}
{"v":1,"seq":12,"t":40.1,"kind":"case_finished","ordinal":0,"result":"rule","rule":0}
{"v":1,"seq":13,"t":41.0,"kind":"case_started","ordinal":1,"case":{"equations":[],"fixed":[2,null],"frame":"PRESENTED"}}
{"v":1,"seq":14,"t":50.0,"kind":"rule_committed","index":1,"canon_blake3":"def","line_bytes":80,"monomials":2}
{"v":1,"seq":15,"t":50.1,"kind":"case_finished","ordinal":1,"result":"rule","rule":1}
"#;

#[test]
fn derivation_records_the_retry_the_pose_and_a_replayable_schedule() {
    let stderr = "EVENT FixedDotStatic {}\nEVENT 15.5s SeedPose chosen=sym1-rev+|spired cost=1.8e4 default_cost=4.0e5 probes=5\n";
    let table = vec![
        r#"{"type":"header","numerical_depth":2,"node_options":{"seed_schedule":"Spired","max_depth":3}}"#.to_string(),
        r#"{"type":"rule","index":0,"target":"I(1,n1)","seeds":["I(1,n1)"],"seed_setting":{"max_depth":3},"source_order":"input","witness_loci":["-1+n1"],"witness_poles":1}"#.to_string(),
        r#"{"type":"rule","index":1,"target":"I(2,n1)","seeds":["I(2,n1)","I(1,n1)"],"source_order":"input","witness_loci":[]}"#.to_string(),
    ];
    let spec = r#"{"program":{"name":"rr_node","blake3":"b3"},"inputs":[{"role":"cases","path":"/x"}],"args":["--order","id","--seed-schedule","spired"]}"#;
    let d = rule_derivations(&JobFiles {
        node: "T1",
        item: "T1.im.0.rung",
        job: "T1.im.0.rung-t0",
        attempt: 1,
        job_spec: spec,
        events: EVENTS,
        stderr,
        table_lines: &table,
        engine_path: "/bin/rr_node",
        engine_sha256: "e5",
    })
    .unwrap();
    assert_eq!(d.len(), 2);
    // rule 0: reached on the symbolic-first retry (the record itself says source_order "input")
    assert_eq!(
        d[0]["seed_schedule"]["effective"], "spired",
        "a relation retry does not change the seed schedule"
    );
    assert_eq!(d[0]["lift_rung"], "symbolic_first");
    assert_eq!(
        d[0]["attempt_that_produced"],
        "retry input+symbolic_first (relation retry #1)"
    );
    assert_eq!(d[0]["relation_retries"][0], "input+symbolic_first");
    assert_eq!(
        d[0]["pose"]["chosen"], "sym1-rev+|spired",
        "the stderr pose inside the case window"
    );
    assert_eq!(d[0]["rule"]["committed"]["canon_blake3"], "abc");
    assert_eq!(d[0]["case"]["case"]["fixed"][0], 1);
    assert_eq!(d[0]["loci"]["witness_loci"][0], "-1+n1");
    assert_eq!(d[0]["time_decisions"][0]["taken"], "sampled reconstruction");
    let replay: Vec<&str> = d[0]["replay_args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(
        replay,
        vec!["--order", "id", "--seed-schedule", "spired"],
        "the producing job's own args (deterministic replay)"
    );
    // rule 1: first attempt, the header's schedule, no pose (none in its window)
    assert_eq!(d[1]["seed_schedule"]["effective"], "spired");
    assert_eq!(d[1]["attempt_that_produced"], "first attempt");
    assert_eq!(d[1]["lift_rung"], "default");
    assert!(d[1]["pose"].is_null());
    assert_ne!(d[0]["seeds_sha256"], d[1]["seeds_sha256"]);
    assert_eq!(d[0]["engine"]["sha256"], "e5");
    assert_eq!(d[0]["schema"], SCHEMA);
}
