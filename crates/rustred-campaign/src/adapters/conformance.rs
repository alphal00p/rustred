//! Conformance diagnosis (interfaces/adapters.md 3): WHY a finished attempt is not acceptable, named as deviations of
//! the child from the contract. Read-only and DIAGNOSTIC ONLY: nothing is ever accepted from what this module reads
//! (acceptance goes through `classify` and verified handles); each mismatch is a request to the program's owner.

use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::Outcome;
use rustred_campaign_contract::spec::JobSpec;
use rustred_campaign_contract::stop::StopSource;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One deviation from the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mismatch", rename_all = "snake_case")]
pub enum Mismatch {
    /// No OUTCOME.json where the exit code requires one.
    OutcomeMissing,
    /// OUTCOME does not parse as the contract's document (the parser's reason).
    OutcomeUnparsable { why: String },
    /// OUTCOME parses but fails `JobOutcome::check` (the reason).
    OutcomeInconsistent { why: String },
    /// OUTCOME `spec_blake3` is the PLAIN BLAKE3 of job.json, not the contract's spec digest (CONTEXT_SPEC domain).
    SpecDigestPlainBlake3,
    /// OUTCOME `spec_blake3` is neither the contract's digest nor the plain BLAKE3 of job.json.
    SpecDigestOther,
    /// `outputs.extra[role]` has no `canonical_blake3`.
    ExtraWithoutCanonicalDigest { role: String },
    /// `job.effective` present without its `blake3`.
    EffectiveWithoutDigest,
    /// `job.stop.source` outside the closed list.
    StopSourceUnknown { value: String },
    /// A budget echo that is not an integer (e.g. `budgets.wall.case_s` written as a float).
    BudgetNotInteger { field: String },
    /// An EVENTS line the contract's parser refuses (line number, reason).
    EventsUnparsable { line: u64, why: String },
    /// The program has no `version --json` line (pre-flight probe).
    NoVersionLine,
    /// The program needs a non-contract invocation (e.g. `solve-recipe --job`).
    InvocationNotContract { form: String },
    /// The results file failed the kind's record checks (the verifier's reason).
    ResultsInconsistent { why: String },
    /// A legacy child's answers carry no binding echo (e.g. rr_proof answers without `record_sha256`).
    AnswerWithoutBinding { field: String },
}

/// Diagnose a finished attempt (read-only). `spec_blake3` is the supervisor's own digest of the job.json it wrote.
pub fn diagnose(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str) -> Vec<Mismatch> {
    let mut out = Vec::new();
    match std::fs::read(layout.outcome()) {
        Err(_) => out.push(Mismatch::OutcomeMissing),
        Ok(b) => {
            let raw: Option<Value> = serde_json::from_slice(&b).ok();
            if let Some(v) = &raw {
                let j = &v["job"];
                if let Some(d) = j["spec_blake3"].as_str()
                    && d != spec_blake3
                {
                    let plain = std::fs::read(layout.job_spec())
                        .map(|x| rustred_campaign_contract::blake3_hex(&x))
                        .unwrap_or_default();
                    out.push(if d == plain {
                        Mismatch::SpecDigestPlainBlake3
                    } else {
                        Mismatch::SpecDigestOther
                    });
                }
                for x in j["outputs"]["extra"].as_array().into_iter().flatten() {
                    if x.get("canonical_blake3").is_none_or(Value::is_null) {
                        out.push(Mismatch::ExtraWithoutCanonicalDigest {
                            role: x["role"].as_str().unwrap_or("?").into(),
                        });
                    }
                }
                if j["effective"].is_object() && j["effective"].get("blake3").is_none() {
                    out.push(Mismatch::EffectiveWithoutDigest);
                }
                if let Some(s) = j["stop"]["source"].as_str()
                    && serde_json::from_value::<StopSource>(Value::String(s.into())).is_err()
                {
                    out.push(Mismatch::StopSourceUnknown { value: s.into() });
                }
                for f in ["case_s", "job_s"] {
                    let x = &j["budgets"]["wall"][f];
                    if !x.is_null() && !x.is_u64() {
                        out.push(Mismatch::BudgetNotInteger {
                            field: format!("budgets.wall.{f}"),
                        });
                    }
                }
            }
            match Outcome::from_slice(&b) {
                Err(why) => out.push(Mismatch::OutcomeUnparsable { why }),
                Ok(o) => {
                    if let Err(why) = o.job.check() {
                        out.push(Mismatch::OutcomeInconsistent { why });
                    }
                }
            }
        }
    }
    if let Ok(text) = std::fs::read(layout.events()) {
        for (i, l) in text.split(|&c| c == b'\n').enumerate() {
            if l.is_empty() {
                continue;
            }
            if let Err(why) = rustred_campaign_contract::events::EventLine::parse(l) {
                out.push(Mismatch::EventsUnparsable {
                    line: i as u64 + 1,
                    why,
                });
                break;
            }
        }
    }
    let _ = spec;
    out
}
