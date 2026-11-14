//! `job.effective.blake3` (child-job.md 3.2.4; contract owner's ruling 2026-11-14, lane engfix4): BLAKE3 in the
//! `CONTEXT_EFFECTIVE` domain over the canonical line of exactly {subcommand, args, node_options, order, env}.

use rustred_campaign_contract::outcome::{
    CONTEXT_EFFECTIVE, EFFECTIVE_DIGEST_VERSION, Effective, LICENCE_PRESENT,
};
use serde_json::json;

fn sample() -> Effective {
    Effective {
        subcommand: "solve-recipe".into(),
        args: vec!["--order".into(), "id".into()],
        node_options: json!({"threads": 1, "proof": true, "relation_rule": "m1_first", "key_budget_s": null,
            "max_trace_rows": null, "recipe_digest": "abc", "determinism_id": "d"}),
        order: json!({"name": "id"}),
        env: [("SYMBOLICA_LICENSE".to_string(), LICENCE_PRESENT.to_string())]
            .into_iter()
            .collect(),
        blake3: String::new(),
    }
}

#[test]
fn effective_digest_is_the_domain_separated_digest_of_the_five_members() {
    assert_eq!(EFFECTIVE_DIGEST_VERSION, 2);
    let e = sample();
    let d = e.digest().unwrap();
    // independent recomputation: canonical line (sorted keys, no whitespace) of the five members, derive-key context
    let canonical = r#"{"args":["--order","id"],"env":{"SYMBOLICA_LICENSE":"<present>"},"node_options":{"determinism_id":"d","key_budget_s":null,"max_trace_rows":null,"proof":true,"recipe_digest":"abc","relation_rule":"m1_first","threads":1},"order":{"name":"id"},"subcommand":"solve-recipe"}"#;
    let mut h = blake3::Hasher::new_derive_key(CONTEXT_EFFECTIVE);
    h.update(canonical.as_bytes());
    assert_eq!(d, h.finalize().to_hex().to_string());
    // pinned vector (engine routes compute the same value through Effective::sealed)
    assert_eq!(
        d,
        "04d5bb45173ff273bc02b20999b0c851f668226c22b5abafc519a2c27bb05012"
    );
    // `blake3` itself is not hashed; member order and whitespace do not matter
    let mut e2 = e.clone();
    e2.blake3 = "f".repeat(64);
    assert_eq!(e2.digest().unwrap(), d);
    let sealed = e.clone().sealed().unwrap();
    assert!(sealed.verify());
    assert_eq!(sealed.blake3, d);
    // not the plain BLAKE3 of the canonical JSON (version 1 reading of the verb) and not the canonical line of the
    // struct with "blake3":"" (version 1 reading of generate)
    assert_ne!(
        d,
        rustred_campaign_contract::blake3_hex(canonical.as_bytes())
    );
    let mut changed = e.clone();
    changed.args.push("--threads".into());
    assert_ne!(changed.digest().unwrap(), d);
    assert!(!e2.verify());
}
