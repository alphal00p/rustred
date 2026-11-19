//! Record-level validation of `rr_campaign table-check` (emission contract section 9). PURE: side files are read through
//! a lookup closure `(area, sha256) -> bytes`. A record is REFUSED when it lacks the `derivation` member, when its
//! `body_sha256` is not that of its own bytes, when index != position, when a referenced side file is missing or has
//! another sha256, when its stated stages differ from the stored checker output (or from a re-run), when the checker
//! output was made on another relation / domain / certificate, or when its stated class is not the one `class_of`
//! derives from the stored evidence.
use super::body::{body_ids, sha256_hex, strip_derivation};
use super::class::{
    CheckSummary, ClassWord, ConeEvidence, Evidence, Kind, Origin, SampleEvidence, class_of,
    span_exact,
};
use super::legacy::verify_row;
use super::{
    AREA_CERT, AREA_CHECK, AREA_CONE, AREA_DESCENT, AREA_EVID, AREA_KEYS, AREA_POLICY, AREA_PROV,
    AREA_REGULARITY, AREA_SYMROWS, SCHEMA,
};
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
    /// REPORTED facts that refuse nothing (the d = 4 regularity verdict since rule 21)
    pub notes: Vec<String>,
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
        notes: vec![],
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
    let mut policy: Option<super::class::Policy> = None;
    if let Some(sha) = d["policy"]["sha256"].as_str()
        && let Some(b) = fetch(AREA_POLICY, sha, "policy", &mut v.problems)
    {
        policy = serde_json::from_slice(&b).ok();
    }
    let cert_sha = d["certificate"]["sha256"].as_str().map(str::to_string);
    if let Some(sha) = &cert_sha {
        fetch(AREA_CERT, sha, "certificate", &mut v.problems);
        v.certificate_sha256 = Some(sha.clone());
    }
    let mut summary: Option<CheckSummary> = None;
    // POINT / SYMMETRY: the exactness at the record's key comes ONLY from a stored instance-span output bound to this
    // body (a stated `exact_at_key` is never read)
    let point = matches!(kind, Kind::Point | Kind::Symmetry);
    let mut exact_at_key: Option<bool> = None;
    if point {
        if let Some(out_sha) = d["check"]["output_sha256"].as_str()
            && let Some(b) = fetch(AREA_CHECK, out_sha, "check output", &mut v.problems)
        {
            let text = String::from_utf8_lossy(&b).trim().to_string();
            match span_exact(&text, &body) {
                Ok(x) => {
                    exact_at_key = Some(x);
                    let stored = serde_json::from_str::<Value>(&text)
                        .ok()
                        .and_then(|o| o["verdict"].as_str().map(str::to_string));
                    if d["check"]["verdict"].as_str() != stored.as_deref() {
                        v.problems.push(format!(
                            "stated span verdict {} differs from the stored check output {stored:?}",
                            d["check"]["verdict"]
                        ));
                    }
                }
                Err(e) => v.problems.push(e),
            }
            // a SYMMETRY RELATION (contract section 15): proven on the expanded symmetry row alone, its exact certificate
            // made on these record bytes, the request of the record's own element at its key
            if origin == Origin::Lift && let Some(e) = super::symrows::element(&body) {
                let s = &d["check"]["symrows"];
                match (s["request_sha256"].as_str(), s["group_sha256"].as_str(), cert_sha.as_deref()) {
                    (Some(rq), Some(g), Some(cs)) => {
                        let req = fetch(AREA_SYMROWS, rq, "symrows request", &mut v.problems);
                        let _ = fetch(AREA_SYMROWS, g, "symmetry group", &mut v.problems);
                        let cert = fetch(AREA_CERT, cs, "symmetry-relation certificate", &mut v.problems);
                        if let (Some(req), Some(cert)) = (req, cert)
                            && let Err(x) = super::symrows::verify(&text, &cert, &req, &body)
                        {
                            v.problems.push(format!("symmetry relation (element {e}): {x}"));
                        }
                        if s["element"].as_u64() != Some(e) {
                            v.problems.push(format!("the check names element {}, the record element {e}", s["element"]));
                        }
                    }
                    _ => v.problems.push(format!(
                        "a symmetry relation (element {e}) without its symrows check and certificate (contract section 15)"
                    )),
                }
            }
            v.check_output = Some(text);
        }
    } else if let Some(out_sha) = d["check"]["output_sha256"].as_str()
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
    // A REPLACEMENT (a record the ladder / emit-replace wrote, `ladder` member, or one stating `replaces`) MAY name its d = 4
    // regularity verdict (contract section 13). Since the user ruling of 2026-11-19 (LANE_RULES rule 21) it is a REPORTED
    // NUMBER, tie-break only: a missing, "worse" or v1 verdict refuses nothing; what it says goes into the record's notes
    let is_replacement = !d["ladder"].is_null() || d.get("replaces").is_some_and(|x| !x.is_null());
    if is_replacement && origin == Origin::Lift {
        match d["replaces"]["regularity_sha256"].as_str() {
            None => v.notes.push("regularity d4: not reported (no verdict named)".into()),
            Some(sha) => match side(AREA_REGULARITY, sha) {
                Some(b) if sha256_hex(&b) == sha => match serde_json::from_slice::<Value>(&b) {
                    Ok(r) => {
                        let rule = if r["schema"].as_str() == Some(super::regularity::SCHEMA_V1) { " (v1 verdict, old rule)" } else { "" };
                        let c = &r["counts"];
                        v.notes.push(if r["same_relation"] == true {
                            format!("regularity d4: same relation{rule}")
                        } else {
                            format!(
                                "regularity d4 (reported{rule}): {} keys compared, {} worse ({} regular -> singular), {} better ({} singular -> regular)",
                                c["keys"], c["worse"], c["regular_to_singular"], c["better"], c["singular_to_regular"]
                            )
                        });
                        let mine = r["new_body_sha256"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(ids.body_sha256.as_str())));
                        if !mine || r["old_body_sha256"] != d["replaces"]["old_body_sha256"] {
                            v.notes.push("regularity d4: the named verdict is of another body / old record (reported only)".into());
                        }
                    }
                    Err(e) => v.notes.push(format!("regularity d4: verdict unreadable ({e})")),
                },
                _ => v.notes.push(format!("regularity d4: verdict {sha} missing beside the table (reported only)")),
            },
        }
    }
    // DESCENT (contract section 14): every EMITTED record (origin lift; the lifted path only assumed descent) names the
    // engine's descent output on this record; the output must be of this one record (1 rule, as many terms as its rhs),
    // say every term LOWER, and agree with the stated verdict. An exact, regular record that ascends is refused.
    if origin == Origin::Lift {
        match d["descent"]["output_sha256"].as_str() {
            None => v
                .problems
                .push("an emitted record without its descent verdict (contract section 14)".into()),
            Some(sha) => {
                if let Some(b) = fetch(AREA_DESCENT, sha, "descent output", &mut v.problems) {
                    let out = String::from_utf8_lossy(&b);
                    match super::descent::verify(&out, &body) {
                        Ok(lower) => {
                            let word = if lower { "LOWER" } else { "NOT_LOWER" };
                            if d["descent"]["verdict"].as_str() != Some(word) {
                                v.problems.push(format!(
                                    "stated descent verdict {} but the stored output says {word}",
                                    d["descent"]["verdict"]
                                ));
                            }
                            if !lower {
                                v.problems.push(format!(
                                    "not_lower: a right-hand-side term is not lower than its target ({})",
                                    super::descent::parse(&out).ok().and_then(|x| x.first).unwrap_or_default()
                                ));
                            }
                        }
                        Err(e) => v.problems.push(format!("descent output: {e}")),
                    }
                }
            }
        }
    }
    // CONE PRICING (contract section 17, opt-in by policy): a replacement names an ACCEPTED cone verdict of its own old record
    // and body
    if is_replacement && origin == Origin::Lift && policy.as_ref().is_some_and(|p| p.cone_pricing) {
        match d["replaces"]["cone_sha256"].as_str() {
            None => v.problems.push("a replacement without its cone-pricing verdict (contract section 17; the policy asks for it)".into()),
            Some(sha) => {
                if let Some(b) = fetch(AREA_CONE, sha, "cone verdict", &mut v.problems) {
                    match serde_json::from_slice::<super::cone::Verdict>(&b) {
                        Ok(c) => {
                            if c.schema != super::cone::SCHEMA {
                                v.problems.push(format!("cone verdict schema {}", c.schema));
                            }
                            if !c.accepted {
                                v.problems.push(format!("the cone verdict refuses the candidate: {}", c.reason.unwrap_or_default()));
                            }
                            if c.old_body_sha256.as_deref().is_some_and(|o| Some(o) != d["replaces"]["old_body_sha256"].as_str()) {
                                v.problems.push("the cone verdict prices another old record".into());
                            }
                            if !c.new_body_sha256.is_empty() && !c.new_body_sha256.iter().any(|x| x == &ids.body_sha256) {
                                v.problems.push("the cone verdict does not name this body".into());
                            }
                        }
                        Err(e) => v.problems.push(format!("cone verdict: {e}")),
                    }
                }
            }
        }
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
        exact_at_key,
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
