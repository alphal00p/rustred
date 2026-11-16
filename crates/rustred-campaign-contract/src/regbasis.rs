//! Kind `regular-basis` (lane wireflow (g); interfaces/regbasis.md, POLES_OBJECTIVE.md): lane regbasis' `rr_regbasis`
//! (Symbolica) as a LEGACY DIAGNOSTIC child. One job = one CLOSED node's statement: the pinned inputs are the effective
//! import spec (`import_spec`) and the three files of the statement directory (`statement` = statement.jsonl,
//! `terminals` = terminals.json, `sums` = SHA256SUMS, all in ONE directory, which the child reads as
//! `--statement-dir`). The child writes `out/regbasis/regular_basis.json` (the export object
//! `rustred.campaign.regular-basis.v1`: the regular terminal set R and the exact map) and `out/regbasis/tower.json`
//! (`rustred.campaign.regular-basis-tower.v1`, optional). The supervisor-side wrapper checks them
//! ([`check_results`]: schemas, the record bound to the PINNED statement by its sha256) and seals `out/results.jsonl`
//! (one canonical line per record + the end record). Accept mode `diagnostic_records`, rows 37-39 as kind
//! `pole-regular-at`: NEVER rule or relation evidence, never changes a verdict; a job that gives nothing readable leaves
//! the node without R (never attention, never a held node).
//!
//! Everything here is pure (bytes in, verdicts out). Generic: no node, arity or box is known here.

use crate::spec::JobSpec;
use serde_json::Value;
use std::path::Path;

/// `schema` of the export object (lane regbasis' `rustred_campaign_core::regbasis::record::RECORD_SCHEMA`).
pub const RECORD_SCHEMA: &str = "rustred.campaign.regular-basis.v1";
/// `schema` of the tower record (`rustred_campaign_core::regbasis::record::TOWER_SCHEMA`).
pub const TOWER_SCHEMA: &str = "rustred.campaign.regular-basis-tower.v1";
/// The child's output directory in `out/`.
pub const CHILD_DIR: &str = "regbasis";
/// The child's files in [`CHILD_DIR`].
pub const RECORD_FILE: &str = "regular_basis.json";
pub const TOWER_FILE: &str = "tower.json";
/// The word an OUTCOME error carries when a pinned input is not the one the spec names (row 39).
pub const INPUT_MISMATCH: &str = "REGBASIS_INPUT_MISMATCH";
/// The statement directory's file names (rr_regbasis reads exactly these).
pub const STATEMENT_FILES: [(&str, &str); 3] = [
    ("statement", "statement.jsonl"),
    ("terminals", "terminals.json"),
    ("sums", "SHA256SUMS"),
];

fn input<'a>(spec: &'a JobSpec, role: &str) -> Result<&'a crate::spec::InputRef, String> {
    spec.inputs
        .iter()
        .find(|i| i.role == role)
        .ok_or_else(|| format!("no {role} input"))
}

/// The statement directory: the ONE parent of the three statement inputs, each under its fixed file name.
pub fn statement_dir(spec: &JobSpec) -> Result<std::path::PathBuf, String> {
    let mut dir: Option<std::path::PathBuf> = None;
    for (role, name) in STATEMENT_FILES {
        let p = spec.input_path(input(spec, role)?);
        if p.file_name().and_then(|n| n.to_str()) != Some(name) {
            return Err(format!(
                "input {role} must be a file named {name}: {}",
                p.display()
            ));
        }
        let parent = p
            .parent()
            .ok_or("statement input without a directory")?
            .to_path_buf();
        match &dir {
            None => dir = Some(parent),
            Some(d) if *d == parent => {}
            Some(d) => {
                return Err(format!(
                    "the statement inputs are not in one directory ({} vs {})",
                    d.display(),
                    parent.display()
                ));
            }
        }
    }
    dir.ok_or_else(|| "no statement inputs".into())
}

/// The legacy argv: `--import-spec <in> --statement-dir <dir> --out out/regbasis --import-base /` (the spec's args, in
/// their allow-listed form, follow the fixed part).
pub fn argv(spec: &JobSpec) -> Result<Vec<String>, String> {
    let is = input(spec, "import_spec")?;
    let mut v = vec![
        "--import-spec".to_string(),
        spec.input_path(is).display().to_string(),
        "--statement-dir".into(),
        statement_dir(spec)?.display().to_string(),
        "--out".into(),
        format!("{}/out/{CHILD_DIR}", spec.run_dir),
        "--import-base".into(),
        "/".into(),
    ];
    v.extend(spec.args.iter().cloned());
    Ok(v)
}

/// The checked records: the export object bound to the statement (sha256 hex of the pinned statement bytes) and the
/// optional tower record. Returns the canonical lines to seal (record first).
pub fn check_results(
    statement_sha256: &str,
    record: &[u8],
    tower: Option<&[u8]>,
) -> Result<Vec<String>, String> {
    let v: Value = serde_json::from_slice(record).map_err(|e| format!("{RECORD_FILE}: {e}"))?;
    if v["schema"] != RECORD_SCHEMA {
        return Err(format!(
            "{RECORD_FILE}: schema {} != {RECORD_SCHEMA}",
            v["schema"]
        ));
    }
    let bound = v["closure_statement_digest"]
        .as_str()
        .map(|d| d.trim_start_matches("sha256:"));
    if bound != Some(statement_sha256) {
        return Err(format!(
            "{RECORD_FILE}: closure_statement_digest {} is not the pinned statement's {statement_sha256}",
            v["closure_statement_digest"]
        ));
    }
    let canon = |x: &Value| -> Result<String, String> {
        let b =
            crate::canon::canonical_line(x.to_string().as_bytes()).map_err(|e| format!("{e:?}"))?;
        String::from_utf8(b).map_err(|e| e.to_string())
    };
    let mut out = vec![canon(&v)?];
    if let Some(t) = tower {
        let tv: Value = serde_json::from_slice(t).map_err(|e| format!("{TOWER_FILE}: {e}"))?;
        if tv["schema"] != TOWER_SCHEMA {
            return Err(format!(
                "{TOWER_FILE}: schema {} != {TOWER_SCHEMA}",
                tv["schema"]
            ));
        }
        out.push(canon(&tv)?);
    }
    Ok(out)
}

/// The sealed results re-read (verified handle lines, end record excluded): the record (and the tower) as JSON.
pub fn records_of(lines: &[String]) -> Result<(Value, Option<Value>), String> {
    let mut rec = None;
    let mut tower = None;
    for l in lines {
        let v: Value = serde_json::from_str(l).map_err(|e| format!("results: {e}"))?;
        match v["schema"].as_str() {
            Some(RECORD_SCHEMA) if rec.is_none() => rec = Some(v),
            Some(TOWER_SCHEMA) if tower.is_none() => tower = Some(v),
            _ if v.get("type").and_then(Value::as_str) == Some("end") => {}
            s => return Err(format!("results: unexpected line of schema {s:?}")),
        }
    }
    Ok((rec.ok_or("results: no regular-basis record")?, tower))
}

/// sha256 (hex) of a file (the statement the record must be bound to).
pub fn file_sha256(p: &Path) -> Result<String, String> {
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(crate::symcert::sha256_hex(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_record_must_be_bound_to_the_pinned_statement() {
        let rec =
            json!({"schema": RECORD_SCHEMA, "closure_statement_digest": "ab12", "regular_set": []});
        let b = serde_json::to_vec(&rec).unwrap();
        let tower = serde_json::to_vec(&json!({"schema": TOWER_SCHEMA, "rows": []})).unwrap();
        let lines = check_results("ab12", &b, Some(&tower)).unwrap();
        assert_eq!(lines.len(), 2);
        let (r, t) = records_of(&lines).unwrap();
        assert_eq!(r["regular_set"], json!([]));
        assert!(t.is_some());
        assert!(
            check_results("cd34", &b, None)
                .unwrap_err()
                .contains("not the pinned")
        );
        let wrong = serde_json::to_vec(&json!({"schema": "x", "closure_statement_digest": "ab12"}))
            .unwrap();
        assert!(check_results("ab12", &wrong, None).is_err());
        let tagged = serde_json::to_vec(
            &json!({"schema": RECORD_SCHEMA, "closure_statement_digest": "sha256:ab12"}),
        )
        .unwrap();
        assert_eq!(check_results("ab12", &tagged, None).unwrap().len(), 1);
        assert!(records_of(&[]).is_err());
    }
}
