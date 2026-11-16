//! Line 7 (lane contract-line7): specs and effective objects that use NONE of the line-7 arguments / options have the
//! digests they had before line 7. The pinned values were computed by THIS file against the sources of campaign
//! 0ceb9e79 (= engine-line7 727b6cc3, before line 7) and are asserted unchanged after it. Uses only APIs that existed
//! before line 7.

mod common;
use common::*;
use rustred_campaign_contract::outcome::{Effective, LICENCE_PRESENT};
use rustred_campaign_contract::spec::{Budgets, InputRef, JobKind, JobSpec, spec_blake3};
use serde_json::json;

fn digests() -> Vec<(&'static str, String)> {
    let gen_spec = sample_spec("/w/jobs/G.a1");
    let mut sr = sample_spec("/w/jobs/S.a1");
    sr.kind = JobKind::SolveRecipe;
    sr.program.features_required = vec![];
    sr.budgets = Budgets::default();
    sr.args = ["--order", "id", "--threads", "4", "--max-trace-rows", "9"]
        .iter()
        .map(|a| a.to_string())
        .collect();
    sr.inputs = ["family", "recipe", "keys"]
        .iter()
        .map(|r| InputRef {
            role: (*r).into(),
            path: format!("/abs/{r}"),
            blake3: D1.into(),
            upto: None,
        })
        .collect();
    let golden = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/job_spec.generate.json"),
    )
    .unwrap();
    let reparsed = JobSpec::from_json(&golden).unwrap();
    let eff = |subcommand: &str, node_options: serde_json::Value| Effective {
        subcommand: subcommand.into(),
        args: vec!["--order".into(), "id".into()],
        node_options,
        order: json!({"name": "id"}),
        env: [("SYMBOLICA_LICENSE".to_string(), LICENCE_PRESENT.to_string())]
            .into_iter()
            .collect(),
        blake3: String::new(),
    };
    vec![
        ("generate spec", spec_blake3(&gen_spec.to_json_line())),
        ("solve-recipe spec", spec_blake3(&sr.to_json_line())),
        ("golden generate spec bytes", spec_blake3(&golden)),
        ("golden generate spec reserialised", spec_blake3(&reparsed.to_json_line())),
        (
            "generate effective",
            eff("generate", json!({"symmetry_rows": true, "max_box": 3, "seed_centre_offset": null}))
                .digest()
                .unwrap(),
        ),
        (
            "solve-recipe effective",
            eff("solve-recipe", json!({"threads": 1, "proof": true, "relation_rule": "m1_first",
                "key_budget_s": null, "max_trace_rows": null, "recipe_digest": "abc", "determinism_id": "d"}))
            .digest()
            .unwrap(),
        ),
    ]
}

#[test]
fn digests_without_line7_arguments_are_unchanged() {
    let got = digests();
    for (what, d) in &got {
        eprintln!("PIN {what}: {d}");
    }
    let pinned: [&str; 6] = PINNED;
    for ((what, d), p) in got.iter().zip(pinned) {
        assert_eq!(d, p, "{what}");
    }
}

// computed at 727b6cc3 (before line 7), 2026-11-14, by this file
const PINNED: [&str; 6] = [
    "65f43b77aa6f257d7d69c848bf93e89df89bb4c4ae8fd40f1f213b3ffd2f6cc9",
    "d9d75fd3dd6e8af98f2e302927f973b3ed43a2ee0ca8d5e5509576e69c3c1a26",
    "8b1f407d9d7e1d823ec8986ab813734c70097241db03713480d2832fd614727d",
    "2abcc1393f6a84bc4caeb37993689710252865efea7e5dce2bee310d0224b32e",
    "ebcde386367141d365fba9691a5d93a2c185d4b9cb257a2345e76cf5d59003ba",
    "04d5bb45173ff273bc02b20999b0c851f668226c22b5abafc519a2c27bb05012",
];
