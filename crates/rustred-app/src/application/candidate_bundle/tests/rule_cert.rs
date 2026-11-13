//! bebe-cert / batch 6: `rule_certificates` on a small generated payload, and
//! planted errors (helper review y33): a changed RHS coefficient is not
//! reproduced, a dropped exception branch shows up as a hidden layer recomputed
//! from the ORIGINAL-row weights; one record per rule.

use super::super::rule_cert::{
    RULE_CERTIFICATE_SCHEMA, RULE_CERTIFICATE_STATEMENT, RuleCertificateOptions,
    RuleCertificateSummary, rule_certificates,
};
use super::*;

fn certified(bytes: &[u8]) -> (RuleCertificateSummary, Vec<serde_json::Value>) {
    let mut out = Vec::new();
    let summary = rule_certificates(
        bytes,
        Default::default(),
        &RuleCertificateOptions::default(),
        &mut out,
    )
    .unwrap();
    let lines = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    (summary, lines)
}

fn records<'a>(lines: &'a [serde_json::Value], kind: &str) -> Vec<&'a serde_json::Value> {
    lines.iter().filter(|line| line["type"] == kind).collect()
}

#[test]
fn rule_certificates_certify_a_small_payload_and_reject_planted_errors() {
    let output = family_candidates(FamilyCandidatesRequest::new(K3)).unwrap();
    let (summary, lines) = certified(output.bundle());
    assert!(summary.rules > 0);
    assert_eq!(summary.certified, summary.rules, "{summary:?}");
    assert_eq!((summary.not_reproduced, summary.failed), (0, 0));
    assert_eq!(summary.missing_records, 0);
    assert_eq!(summary.branches_hidden, 0, "{summary:?}");
    assert!(summary.complete, "{summary:?}");
    let header = records(&lines, "cert_header");
    assert_eq!(header.len(), 1);
    assert_eq!(header[0]["schema"], RULE_CERTIFICATE_SCHEMA);
    assert_eq!(header[0]["statement"], RULE_CERTIFICATE_STATEMENT);
    // exactly one record per rule, indices 0..rules
    let rules = records(&lines, "cert_rule");
    assert_eq!(rules.len(), summary.rules);
    let mut indices: Vec<u64> = rules.iter().map(|r| r["index"].as_u64().unwrap()).collect();
    indices.sort_unstable();
    assert_eq!(indices, (0..summary.rules as u64).collect::<Vec<_>>());
    assert_eq!(records(&lines, "cert_end").len(), 1);

    let bundle = codec::read(output.bundle(), Default::default()).unwrap();
    // Planted error 1: two different RHS coefficients of one rule swapped ->
    // the stored rule is no longer reproduced by its seeds; still one record
    // per rule, and the payload is not complete.
    let mut swapped = bundle.clone();
    let mut planted = false;
    'outer: for sector in &mut swapped.sectors {
        for rule in &mut sector.rules {
            if let Some(j) = (1..rule.rhs.len())
                .find(|&j| rule.rhs[j].coefficient != rule.rhs[0].coefficient)
            {
                let (a, b) = (rule.rhs[0].coefficient, rule.rhs[j].coefficient);
                rule.rhs[0].coefficient = b;
                rule.rhs[j].coefficient = a;
                planted = true;
                break 'outer;
            }
        }
    }
    assert!(planted, "K3 has a rule with two distinct RHS coefficients");
    let bytes = codec::write(&swapped, Default::default()).unwrap();
    let (mutant, lines) = certified(&bytes);
    assert_eq!(mutant.rules, summary.rules);
    assert_eq!(mutant.certified + 1, summary.certified, "{mutant:?}");
    assert_eq!(mutant.not_reproduced + mutant.failed, 1);
    assert!(!mutant.complete);
    assert_eq!(
        records(&lines, "cert_rule").len() + records(&lines, "cert_error").len(),
        summary.rules
    );

    // Planted error 2: the stored exception branches of a rule whose
    // original-row weight poles are exception branches are dropped -> the
    // recomputed layers become hidden (summarising the engine's own field would
    // not notice; y33 mutant iii).
    let with_poles: Vec<(String, u64)> = rules
        .iter()
        .filter(|r| {
            r["weight_poles"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["status"] == "exception")
        })
        .map(|r| (r["chart"].as_str().unwrap().to_owned(), r["rule"].as_u64().unwrap()))
        .collect();
    println!("K3 rules with an exception weight-pole layer: {}", with_poles.len());
    if let Some((chart, ordinal)) = with_poles.first() {
        let mut dropped = bundle.clone();
        let sector = dropped
            .sectors
            .iter_mut()
            .find(|s| {
                s.sector
                    .iter()
                    .map(|&b| if b { '1' } else { '0' })
                    .collect::<String>()
                    == *chart
            })
            .unwrap();
        sector.rules[*ordinal as usize].exclusions.clear();
        let bytes = codec::write(&dropped, Default::default()).unwrap();
        let (mutant, _) = certified(&bytes);
        assert!(mutant.branches_hidden > 0, "{mutant:?}");
        assert!(mutant.rules_with_hidden_branches >= 1);
        assert!(!mutant.complete);
    }
}
