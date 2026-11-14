//! Kind `proof-instantiate` (legacy-wrapped `rr_proof instantiate`, coord-ec2-instantiate.md 2d / 3): what an answer
//! line of the sealed `out/results.jsonl` may be USED for (line 7, child-job.md "Line 7").
//!
//! `--elided-loci refuse` (the default when the spec's args do not name it) keeps ruling R-f: a record with elided
//! witness loci is never instantiated, so an answer that carries an elision marker (`elided_cover`,
//! `witness_loci_elided`) is a contract violation. `--elided-loci facts`: such answers are EXACT COEFFICIENT FACTS of the
//! stored record text at the key and NOTHING ELSE: the kernel's own hidden-pole test is lost with the elided text, so
//! they are never regularity evidence and never carry `regular: "regular"` (nor `true`); `elided_cover` is one of
//! `guards_v2`, `excepted`, `none` (the ENGINE's guard invariant, recorded, not re-derived). A fact answer
//! (`rustred.campaign.fact.v1`) about a (rule, key) whose instantiate line is elided is demoted the same way.
//! Regularity at such a key comes only from the key's own bound per-key proof (instance-span / instance-exact).

use crate::spec::ElidedLoci;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// `schema` of an instantiate answer line.
pub const INSTANTIATE_SCHEMA: &str = "rustred.proof-kernel.instantiate.v1";
/// `schema` of a closure fact answer line (request form (B)).
pub const FACT_SCHEMA: &str = "rustred.campaign.fact.v1";
/// Values of `elided_cover` (coord-ec2-instantiate.md 2d).
pub const ELIDED_COVERS: [&str; 3] = ["guards_v2", "excepted", "none"];

/// What one answer line may be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerUse {
    /// A complete record's answer: its `regular` value (and the coefficient facts) may serve as evidence at the key.
    RegularityAndFacts,
    /// Exact coefficient facts only (an elided record in facts mode; or a decided non-regular answer); NEVER regularity
    /// evidence.
    CoefficientFactsOnly,
    /// No evidence (an `error` answer; a regularity fact of an elided record).
    Nothing,
}

fn elided(v: &Value) -> bool {
    !v["elided_cover"].is_null() || !v["witness_loci_elided"].is_null()
}

fn says_regular(x: &Value) -> bool {
    *x == Value::Bool(true) || x.as_str() == Some("regular")
}

/// The use of ONE instantiate answer line (schema [`INSTANTIATE_SCHEMA`]) under the job's elided-loci mode. `Err` =
/// contract violation (the classifier treats the attempt's output as not verified).
pub fn instantiate_answer_use(v: &Value, mode: ElidedLoci) -> Result<AnswerUse, String> {
    if v["schema"] != INSTANTIATE_SCHEMA {
        return Err(format!(
            "schema {} is not {INSTANTIATE_SCHEMA}",
            v["schema"]
        ));
    }
    if elided(v) {
        if mode == ElidedLoci::Refuse {
            return Err("an answer about a record with elided witness loci under --elided-loci refuse (R-f)".into());
        }
        if says_regular(&v["regular"]) {
            return Err("an elided record's answer claims regular (facts mode answers coefficient facts only)".into());
        }
        match v["elided_cover"].as_str() {
            Some(c) if ELIDED_COVERS.contains(&c) => {}
            _ => {
                return Err(format!(
                    "elided_cover {} is not one of {ELIDED_COVERS:?}",
                    v["elided_cover"]
                ));
            }
        }
        if !v["witness_loci_elided"].is_null() && v["witness_loci_elided"].as_u64().is_none() {
            return Err("witness_loci_elided is not a count".into());
        }
        return Ok(if v["error"].is_null() {
            AnswerUse::CoefficientFactsOnly
        } else {
            AnswerUse::Nothing
        });
    }
    if !v["error"].is_null() {
        return Ok(AnswerUse::Nothing);
    }
    Ok(if v["regular"].as_str() == Some("regular") {
        AnswerUse::RegularityAndFacts
    } else {
        AnswerUse::CoefficientFactsOnly
    })
}

/// The uses of the answer lines of a VERIFIED results file (canonical lines, END RECORD LAST) of a proof-instantiate
/// job under `mode`: instantiate lines by [`instantiate_answer_use`]; fact lines ([`FACT_SCHEMA`]) about a (rule, key)
/// whose instantiate line is elided (or that carry a marker themselves) are coefficient facts only, and such a fact
/// answering `regular` is a violation; any other line is a violation (fail closed).
pub fn answer_uses(lines: &[Vec<u8>], mode: ElidedLoci) -> Result<Vec<AnswerUse>, String> {
    let (_, answers) = lines.split_last().ok_or("results file without records")?;
    let mut parsed = Vec::with_capacity(answers.len());
    let mut elided_at: BTreeSet<(i64, String)> = BTreeSet::new();
    for (i, l) in answers.iter().enumerate() {
        let v: Value = serde_json::from_slice(l).map_err(|e| format!("answer {i}: {e}"))?;
        if v["schema"] == INSTANTIATE_SCHEMA
            && elided(&v)
            && let Some(r) = v["rule"].as_i64()
        {
            elided_at.insert((r, v["key"].to_string()));
        }
        parsed.push(v);
    }
    let mut out = Vec::with_capacity(parsed.len());
    for (i, v) in parsed.iter().enumerate() {
        let at = |m: String| format!("answer {i}: {m}");
        let u = if v["schema"] == INSTANTIATE_SCHEMA {
            instantiate_answer_use(v, mode).map_err(at)?
        } else if v["schema"] == FACT_SCHEMA {
            let about_elided = elided(v)
                || v["label"]
                    .as_i64()
                    .is_some_and(|r| elided_at.contains(&(r, v["key"].to_string())));
            if about_elided && mode == ElidedLoci::Refuse {
                return Err(at(
                    "a fact about an elided record under --elided-loci refuse (R-f)".into(),
                ));
            }
            match (about_elided, v["kind"].as_str()) {
                (_, _) if !v["error"].is_null() => AnswerUse::Nothing,
                (true, Some("regularity")) if says_regular(&v["value"]) => {
                    return Err(at(
                        "a regularity fact of an elided record claims regular".into()
                    ));
                }
                (true, Some("regularity")) => AnswerUse::Nothing,
                (true, Some("coefficient")) => AnswerUse::CoefficientFactsOnly,
                (false, Some("regularity")) if says_regular(&v["value"]) => {
                    AnswerUse::RegularityAndFacts
                }
                (false, Some("regularity" | "coefficient")) => AnswerUse::CoefficientFactsOnly,
                (_, k) => return Err(at(format!("fact kind {k:?}"))),
            }
        } else {
            return Err(at(format!("schema {}", v["schema"])));
        };
        out.push(u);
    }
    Ok(out)
}
