//! Batch 6: `rule_certificates` (`rustred p-certify`) as public API on a small generated payload, and the bundle
//! guard stamp. Integration target on purpose: the rustred-app lib-test target needs > 40 GB to compile.
use rustred::propagator::custom::BUNDLE_GUARD_TAG;
use rustred_app::{
    CandidateBundleLimits, FamilyCandidatesRequest, RULE_CERTIFICATE_SCHEMA,
    RULE_CERTIFICATE_STATEMENT, RuleCertificateOptions, family_candidates, rule_certificates,
};
use serde_json::Value;

const K3: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "saved_candidate_k3"
loop_momenta = ["q1", "q2"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P1"
expression = "q1^2-1"
[[family.denominators]]
id = "P2"
expression = "q2^2-1"
[[family.denominators]]
id = "P3"
expression = "(q1-q2)^2-1"
[target]
powers = [1, 1, 1]
"#;

#[test]
fn a_generated_payload_is_guard_stamped_and_completely_certified() {
    let output = family_candidates(FamilyCandidatesRequest::new(K3)).unwrap();
    let mut out = Vec::new();
    let summary = rule_certificates(
        output.bundle(),
        CandidateBundleLimits::default(),
        &RuleCertificateOptions::default(),
        &mut out,
    )
    .unwrap();
    assert!(summary.rules > 0);
    assert_eq!(summary.certified, summary.rules, "{summary:?}");
    assert_eq!(
        (
            summary.not_reproduced,
            summary.failed,
            summary.missing_records
        ),
        (0, 0, 0)
    );
    assert_eq!(summary.branches_hidden, 0);
    assert!(summary.complete, "{summary:?}");
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let header = &lines[0];
    assert_eq!(header["type"], "cert_header");
    assert_eq!(header["schema"], RULE_CERTIFICATE_SCHEMA);
    assert_eq!(header["statement"], RULE_CERTIFICATE_STATEMENT);
    // the ruling: a batch-6 generation carries the guard version in its solver policy
    let policy = header["solver_policy"].as_str().unwrap();
    assert!(policy.ends_with(BUNDLE_GUARD_TAG), "{policy}");
    // exactly one certified record per rule, indices 0..rules
    let mut indices: Vec<u64> = lines
        .iter()
        .filter(|line| line["type"] == "cert_rule")
        .map(|line| line["index"].as_u64().unwrap())
        .collect();
    indices.sort_unstable();
    assert_eq!(indices, (0..summary.rules as u64).collect::<Vec<_>>());
    assert_eq!(lines.iter().filter(|l| l["type"] == "cert_end").count(), 1);
}
