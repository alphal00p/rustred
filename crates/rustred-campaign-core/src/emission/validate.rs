//! Record-level validation of `rr_campaign table-check` (emission contract section 9). PURE: side files are read through
//! a lookup closure `(area, sha256) -> bytes`. A record is REFUSED when it lacks the `derivation` member, when its
//! `body_sha256` is not that of its own bytes, when index != position, when a referenced side file is missing or has
//! another sha256, when its stated stages differ from the stored checker output (or from a re-run), when the checker
//! output was made on another relation / domain / certificate, or when its stated class is not the one `class_of`
//! derives from the stored evidence.
use super::body::{body_ids, sha256_hex, strip_derivation};
use super::class::{
    CheckSummary, ClassWord, ConeEvidence, Evidence, Kind, Origin, SampleEvidence, class_of,
};
use super::legacy::verify_row;
use super::{AREA_CERT, AREA_CHECK, AREA_EVID, AREA_KEYS, AREA_POLICY, AREA_PROV, SCHEMA};
use serde_json::Value;

/// The verdict on one record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordVerdict {
    pub position: u64,
    pub index: i64,
    pub problems: Vec<String>,
    pub class: Option<ClassWord>,
    pub kind: Option<Kind>,
    /// the stored checker output line (for a re-run comparison by the caller)
    pub check_output: Option<String>,
    pub certificate_sha256: Option<String>,
    pub body_sha256: Option<String>,
}

impl RecordVerdict {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Validate one rule line at file `position` (among the rule records of its chart). `side(area, sha)` returns the side
/// file's bytes. `alias_class(index)` returns the class of an earlier record (alias kind).
pub fn check_record(
    line: &str,
    position: u64,
    side: &dyn Fn(&str, &str) -> Option<Vec<u8>>,
    alias_class: &dyn Fn(i64) -> Option<ClassWord>,
) -> RecordVerdict {
    let mut v = RecordVerdict {
        position,
        index: -1,
        problems: vec![],
        class: None,
        kind: None,
        check_output: None,
        certificate_sha256: None,
        body_sha256: None,
    };
    let rec: Value = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            v.problems.push(format!("not JSON: {e}"));
            return v;
        }
    };
    v.index = rec.get("index").and_then(Value::as_i64).unwrap_or(-1);
    if v.index != position as i64 {
        v.problems
            .push(format!("index {} != position {position}", v.index));
    }
    let (body, raw) = match strip_derivation(line) {
        Ok(x) => x,
        Err(e) => {
            v.problems.push(format!("record scan: {e}"));
            return v;
        }
    };
    let Some(raw) = raw else {
        v.problems.push("no derivation member".into());
        return v;
    };
    let d: Value = match serde_json::from_str(&raw) {
        Ok(d) => d,
        Err(e) => {
            v.problems.push(format!("derivation not JSON: {e}"));
            return v;
        }
    };
    if d["schema"].as_str() != Some(SCHEMA) {
        v.problems
            .push(format!("derivation schema {}", d["schema"]));
        return v;
    }
    let ids = match body_ids(&body) {
        Ok(i) => i,
        Err(e) => {
            v.problems.push(format!("body: {e}"));
            return v;
        }
    };
    v.body_sha256 = Some(ids.body_sha256.clone());
    if d["body_sha256"].as_str() != Some(ids.body_sha256.as_str()) {
        v.problems.push(format!(
            "body_sha256 {} is not the body's {}",
            d["body_sha256"], ids.body_sha256
        ));
    }
    let Some(kind) = d["kind"].as_str().and_then(Kind::from_word) else {
        v.problems.push(format!("kind {}", d["kind"]));
        return v;
    };
    v.kind = Some(kind);
    let origin = match d["origin"].as_str() {
        Some("lift") => Origin::Lift,
        Some("import") => Origin::Import,
        o => {
            v.problems.push(format!("origin {o:?}"));
            return v;
        }
    };
    let fetch =
        |area: &str, sha: &str, what: &str, problems: &mut Vec<String>| -> Option<Vec<u8>> {
            match side(area, sha) {
                None => {
                    problems.push(format!("{what} {sha} missing beside the table"));
                    None
                }
                Some(b) if sha256_hex(&b) != sha => {
                    problems.push(format!("{what} {sha}: the file has another sha256"));
                    None
                }
                Some(b) => Some(b),
            }
        };
    if let Some(p) = d["producer"].as_object()
        && let Some(sha) = p.get("provenance_sha256").and_then(Value::as_str)
    {
        fetch(AREA_PROV, sha, "provenance", &mut v.problems);
    }
    if let Some(sha) = d["policy"]["sha256"].as_str() {
        fetch(AREA_POLICY, sha, "policy", &mut v.problems);
    }
    let cert_sha = d["certificate"]["sha256"].as_str().map(str::to_string);
    if let Some(sha) = &cert_sha {
        fetch(AREA_CERT, sha, "certificate", &mut v.problems);
        v.certificate_sha256 = Some(sha.clone());
    }
    let mut summary: Option<CheckSummary> = None;
    if let Some(out_sha) = d["check"]["output_sha256"].as_str()
        && let Some(b) = fetch(AREA_CHECK, out_sha, "check output", &mut v.problems)
    {
        let text = String::from_utf8_lossy(&b).trim().to_string();
        match CheckSummary::parse(&text) {
            Ok(s) => {
                if s.relation_sha256 != ids.relation_sha256 {
                    v.problems.push(
                        "the check output was made on another relation (target + rhs)".into(),
                    );
                }
                if s.domain_sha256 != ids.domain_sha256 {
                    v.problems
                        .push("the check output was made on another domain".into());
                }
                if cert_sha.as_deref() != Some(s.cert_sha256.as_str()) {
                    v.problems
                        .push("the check output was made with another certificate".into());
                }
                if d["check"]["stages"] != s.record_json() {
                    v.problems.push(format!(
                        "stated stages {} differ from the stored check output {}",
                        d["check"]["stages"],
                        s.record_json()
                    ));
                }
                summary = Some(s);
            }
            Err(e) => v.problems.push(format!("check output: {e}")),
        }
        v.check_output = Some(text);
    }
    let cone = serde_json::from_value::<ConeEvidence>(d["class"]["cone"].clone()).ok();
    let sample = serde_json::from_value::<SampleEvidence>(d["class"]["sample"].clone()).ok();
    // legacy import (contract section 12): the bound verdict row is re-read and re-verified against THIS body; the
    // per-key proof lines must have been made on this body's bytes; the proven key lists must be beside the table
    let mut legacy_proof = None;
    let legacy = &d["class"]["legacy"];
    if origin == Origin::Import && legacy.is_object() {
        if let Some(sha) = legacy["proof_object"]["row_sha256"].as_str()
            && let Some(b) = fetch(AREA_EVID, sha, "bound proof object", &mut v.problems)
        {
            match serde_json::from_slice::<Value>(&b) {
                Ok(row) => match verify_row(&row, &ids.relation_sha256, &ids.domain_sha256) {
                    Ok(()) => legacy_proof = Some(true),
                    Err(e) => {
                        v.problems.push(e);
                        legacy_proof = Some(false);
                    }
                },
                Err(e) => v.problems.push(format!("bound proof object: {e}")),
            }
        }
        if let Some(ls) = legacy["proofs_line_sha256"].as_str()
            && ls != sha256_hex(body.as_bytes())
        {
            v.problems
                .push("the per-key proof lines were made on another line than this body".into());
        }
        let keyed = [
            cone.as_ref().map(|c| (c.keys_sha256.clone(), c.proven)),
            sample.as_ref().map(|s| (s.keys_sha256.clone(), s.proven)),
        ];
        for (sha, proven) in keyed.into_iter().flatten() {
            if let Some(b) = fetch(AREA_KEYS, &sha, "proven key list", &mut v.problems) {
                let n = b.split(|&c| c == b'\n').filter(|l| !l.is_empty()).count() as u64;
                if n != proven {
                    v.problems
                        .push(format!("proven key list has {n} keys, the record states {proven}"));
                }
            }
        }
    }
    let ev = Evidence {
        cone,
        sample,
        exact_at_key: d["class"]["exact_at_key"].as_bool(),
        alias_target: d["class"]["alias_of"].as_i64().and_then(alias_class),
        sealed: d["class"]["sealed"].as_bool(),
        legacy_proof,
    };
    let derived = class_of(kind, origin, summary.as_ref(), &ev);
    let stated = d["class"]["word"].as_str().and_then(ClassWord::from_word);
    match stated {
        None => v
            .problems
            .push(format!("class word {}", d["class"]["word"])),
        Some(c) if c != derived.class => v.problems.push(format!(
            "stated class '{}' but the stored evidence gives '{}'",
            c.word(),
            derived.class.word()
        )),
        Some(_) => {}
    }
    v.class = Some(derived.class);
    v
}

/// Compare a RE-RUN of the checker on the record's body with the stored output: the stage verdicts must agree.
pub fn compare_rerun(stored: &str, rerun: &str) -> Result<(), String> {
    let a = CheckSummary::parse(stored)?;
    let b = CheckSummary::parse(rerun)?;
    if a.record_json() != b.record_json()
        || a.relation_sha256 != b.relation_sha256
        || a.domain_sha256 != b.domain_sha256
        || a.cert_sha256 != b.cert_sha256
    {
        return Err(format!(
            "re-run differs: stored {} vs re-run {}",
            a.record_json(),
            b.record_json()
        ));
    }
    Ok(())
}
