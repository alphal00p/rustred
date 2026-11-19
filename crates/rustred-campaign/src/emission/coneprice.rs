//! `rr_campaign cone-price` and `cone-stamp` (emission contract section 17; round 6 J4): the CONE-PRICING acceptance of a
//! replacement (core `emission::cone`). Opt-in by policy (`cone_pricing`): it needs engine runs.
//! - `cone-price --old-stats A --new-stats B --entrance N [...] --out V.json`: the verdict from two owner-statistics files;
//! - `cone-price --run --binary P --old-lib L1 --old-inputs I1 [--new-lib L2] --new-inputs I2 --entrance N --out DIR [...]`:
//!   the A/B partial record pass itself, with the engine's EXISTING stop-after-phase (RUSTRED_RESUME_DIR,
//!   RUSTRED_RESUME_STOP_AFTER_PHASE = N + 3, or N + 5 with --third-level), owner statistics (RUSTRED_P_OWNER_STATS), ONE thread
//!   (`reduce-file <lib> <inputs> modular 1`; at 16 threads a stop saved nothing: notes/vk5l-physrun.PARALLEL-RECORD.md), each arm
//!   one Symbolica process (its licence slot through RR_EMISSION_SLOT_WRAPPER). The run ends with "stopped after a saved phase"
//!   (non-zero rc: expected); the statistics are flushed on that path;
//! - `cone-stamp --table T --index I --verdict V.json --out T2`: a replacement record names an ACCEPTED verdict
//!   (`replaces.cone_sha256`, side file cone/<sha>.json); a refused verdict, or one of another old record / body, is not stamped;
//! - `emit-replace ... --cone-verdict V.json` ([`apply_to_pieces`], the same checks): every piece of the replacement names the
//!   accepted verdict, or the replacement is refused (outcome `cone_refused`, the pieces kept aside in pieces.refused.jsonl).
use super::child;
use rustred_campaign_core::emission::body::{attach_derivation, body_ids, sha256_hex, string_member, strip_derivation};
use rustred_campaign_core::emission::cone::{self, Rules, Verdict};
use rustred_campaign_core::emission::{AREA_CONE, side_dir, side_rel};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// One arm of a run.
pub struct ArmRun {
    pub lib: PathBuf,
    pub inputs: PathBuf,
}

/// Run one arm; returns the owner-statistics path.
pub fn run_arm(binary: &Path, arm: &ArmRun, stop_after: i64, dir: &Path, budget_s: f64, extra_env: &[(String, String)]) -> Result<(PathBuf, Value), String> {
    std::fs::create_dir_all(dir.join("resume")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let stats = dir.join("owner_stats.json");
    let _ = std::fs::remove_file(&stats);
    let mut env: Vec<(String, String)> = vec![
        ("RUSTRED_RESUME_DIR".into(), dir.join("resume").display().to_string()),
        ("RUSTRED_RESUME_STOP_AFTER_PHASE".into(), stop_after.to_string()),
        ("RUSTRED_P_OWNER_STATS".into(), stats.display().to_string()),
        ("RAYON_NUM_THREADS".into(), "1".into()),
    ];
    env.extend(extra_env.iter().cloned());
    let args: Vec<String> = vec!["reduce-file".into(), arm.lib.display().to_string(), arm.inputs.display().to_string(), "modular".into(), "1".into()];
    let r = child::run(binary, &args, &env, dir, "arm", budget_s)?;
    let info = json!({"rc": r.code, "seconds": r.seconds, "over_budget": r.over_budget, "stderr_tail": r.stderr_tail(400),
                      "stopped_after_saved_phase": r.stderr_tail(4000).contains("stopped after a saved phase")});
    if !stats.exists() {
        return Err(format!("no owner statistics written ({info})"));
    }
    Ok((stats, info))
}

fn read_arm(p: &Path, dr: Option<i64>) -> Result<cone::Arm, String> {
    cone::parse_arm(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?, dr)
}

/// CLI `cone-price` (both modes; see the module text). Exit 0 accepted, 3 refused.
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let mut v: std::collections::BTreeMap<String, String> = Default::default();
    let mut flags: std::collections::BTreeSet<String> = Default::default();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            f @ ("--run" | "--third-level") => {
                flags.insert(f.into());
                i += 1;
            }
            f => {
                v.insert(f.trim_start_matches("--").into(), rest.get(i + 1).cloned().ok_or(format!("{f} needs a value"))?);
                i += 2;
            }
        }
    }
    let get = |k: &str| v.get(k).cloned();
    let entrance: i64 = get("entrance").ok_or("--entrance N (the Route phase where the replaced relation's children arrive)")?.parse().map_err(|e| format!("--entrance: {e}"))?;
    let mut rules = Rules::default();
    if let Some(x) = get("tolerance") {
        rules.tolerance = x.parse().map_err(|e| format!("--tolerance: {e}"))?;
    }
    if let Some(x) = get("owner-growth") {
        rules.owner_growth = x.parse().map_err(|e| format!("--owner-growth: {e}"))?;
    }
    if let Some(x) = get("min-children") {
        rules.min_children = x.parse().map_err(|e| format!("--min-children: {e}"))?;
    }
    let dr: Option<i64> = get("request-dr").map(|x| x.parse()).transpose().map_err(|e| format!("--request-dr: {e}"))?;
    let out = PathBuf::from(get("out").ok_or("--out")?);
    let (old_stats, new_stats, mut request) = if flags.contains("--run") {
        let bin = PathBuf::from(get("binary").ok_or("--binary P (p5l_library with owner statistics, e.g. diag4186)")?);
        let old = ArmRun { lib: get("old-lib").ok_or("--old-lib")?.into(), inputs: get("old-inputs").ok_or("--old-inputs")?.into() };
        let new = ArmRun { lib: get("new-lib").or(get("old-lib")).ok_or("--new-lib")?.into(), inputs: get("new-inputs").ok_or("--new-inputs")?.into() };
        let stop = entrance + if flags.contains("--third-level") { 5 } else { 3 };
        let budget: f64 = get("budget").map(|x| x.parse()).transpose().map_err(|e| format!("--budget: {e}"))?.unwrap_or(3600.0);
        let env: Vec<(String, String)> = get("env").map(|e| e.split(',').filter_map(|kv| kv.split_once('=').map(|(a, b)| (a.to_string(), b.to_string()))).collect()).unwrap_or_default();
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let (a, ia) = run_arm(&bin, &old, stop, &out.join("old"), budget, &env)?;
        let (b, ib) = run_arm(&bin, &new, stop, &out.join("new"), budget, &env)?;
        let req = json!({"binary": bin.display().to_string(), "binary_sha256": child::file_sha256(&bin)?, "stop_after_phase": stop,
                         "old": {"lib": old.lib.display().to_string(), "inputs": old.inputs.display().to_string(),
                                 "inputs_sha256": child::file_sha256(&old.inputs)?, "run": ia},
                         "new": {"lib": new.lib.display().to_string(), "inputs": new.inputs.display().to_string(),
                                 "inputs_sha256": child::file_sha256(&new.inputs)?, "run": ib}});
        (a, b, req)
    } else {
        let a = PathBuf::from(get("old-stats").ok_or("--old-stats A.json (or --run)")?);
        let b = PathBuf::from(get("new-stats").ok_or("--new-stats B.json")?);
        let req = json!({"old_stats": a.display().to_string(), "old_stats_sha256": child::file_sha256(&a)?,
                         "new_stats": b.display().to_string(), "new_stats_sha256": child::file_sha256(&b)?});
        (a, b, req)
    };
    if let Some(n) = get("note") {
        request["note"] = json!(n);
    }
    let old = read_arm(&old_stats, dr)?;
    let new = read_arm(&new_stats, Some(dr.unwrap_or(old.request_dr)))?;
    let bodies: Vec<String> = get("new-bodies").map(|s| s.split(',').map(str::to_string).collect()).unwrap_or_default();
    let verdict = cone::price(&old, &new, entrance, &rules, request, get("old-body"), bodies)?;
    let text = serde_json::to_string_pretty(&verdict).map_err(|e| e.to_string())?;
    let vp = if flags.contains("--run") { out.join("cone-verdict.json") } else { out.clone() };
    if let Some(d) = vp.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&vp, &text).map_err(|e| format!("{}: {e}", vp.display()))?;
    Ok((json!({"verdict": vp.display().to_string(), "sha256": sha256_hex(text.as_bytes()), "accepted": verdict.accepted,
               "reason": verdict.reason, "entrance_ratio_old_over_new": verdict.entrance_ratio_old_over_new,
               "second_apply_ratio_old_over_new": verdict.second_apply_ratio_old_over_new, "levels": verdict.levels}).to_string(),
        verdict.accepted))
}

/// Why `line` (a replacement record with its derivation) may NOT name the verdict `c`; `None` = it may.
pub fn check_line(line: &str, c: &Verdict) -> Result<Option<String>, String> {
    let (body, raw) = strip_derivation(line)?;
    let d: Value = serde_json::from_str(&raw.ok_or("the record has no derivation member")?).map_err(|e| e.to_string())?;
    if d.get("replaces").is_none_or(Value::is_null) {
        return Ok(Some("the record is not a replacement (no `replaces` member: emit it through emit-replace or descent-stamp first)".into()));
    }
    if !c.accepted {
        return Ok(Some(format!("the cone verdict refuses the candidate: {}", c.reason.clone().unwrap_or_default())));
    }
    let bs = body_ids(&body)?.body_sha256;
    if !c.new_body_sha256.is_empty() && !c.new_body_sha256.contains(&bs) {
        return Ok(Some("the cone verdict does not name this body".into()));
    }
    if c.old_body_sha256.as_deref().is_some_and(|o| Some(o) != d["replaces"]["old_body_sha256"].as_str()) {
        return Ok(Some("the cone verdict prices another old record".into()));
    }
    Ok(None)
}

/// `line` with `replaces.cone_sha256 = sha` (call after [`check_line`] said None).
pub fn stamp_line(line: &str, sha: &str) -> Result<String, String> {
    let (body, raw) = strip_derivation(line)?;
    let mut d: Value = serde_json::from_str(&raw.ok_or("the record has no derivation member")?).map_err(|e| e.to_string())?;
    d["replaces"]["cone_sha256"] = json!(sha);
    attach_derivation(&body, &d)
}

/// Write the verdict bytes as the side file `cone/<sha256>.json` under `side`; returns the sha256.
pub fn write_side(side: &Path, vb: &[u8]) -> Result<String, String> {
    let sha = sha256_hex(vb);
    let p = side.join(side_rel(AREA_CONE, &sha));
    std::fs::create_dir_all(p.parent().expect("area")).map_err(|e| e.to_string())?;
    std::fs::write(&p, vb).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(sha)
}

/// What a cone verdict did to the pieces of one replacement.
#[derive(Debug)]
pub enum Applied {
    /// every piece names the verdict (`replaces.cone_sha256`); the verdict side file is written
    Stamped { lines: Vec<String>, sha256: String },
    /// the verdict refuses, or does not belong to these pieces (the first piece that may not name it): nothing written
    Refused { reason: String, piece: usize, sha256: String },
}

/// emit-replace `--cone-verdict` (contract section 17): ALL pieces of the replacement must be allowed to name the verdict
/// ([`check_line`]: an accepted verdict, of this old record, naming each piece's body when it names bodies); then each is
/// stamped and the verdict written under `side`. Otherwise nothing is written and the replacement is refused.
pub fn apply_to_pieces(lines: &[String], vb: &[u8], side: &Path) -> Result<Applied, String> {
    let c: Verdict = serde_json::from_slice(vb).map_err(|e| format!("cone verdict: {e}"))?;
    let sha = sha256_hex(vb);
    if lines.is_empty() {
        return Err("no pieces to stamp".into());
    }
    for (i, l) in lines.iter().enumerate() {
        if let Some(reason) = check_line(l, &c)? {
            return Ok(Applied::Refused { reason, piece: i, sha256: sha });
        }
    }
    let stamped = lines.iter().map(|l| stamp_line(l, &sha)).collect::<Result<Vec<_>, _>>()?;
    write_side(side, vb)?;
    Ok(Applied::Stamped { lines: stamped, sha256: sha })
}

/// CLI `cone-stamp --table T --index I --verdict V.json --out T2`: exit 0 stamped, 3 refused (nothing written).
pub fn stamp_cmd(rest: &[String]) -> Result<(String, bool), String> {
    let val = |k: &str| -> Result<String, String> {
        rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1)).cloned().ok_or(format!("{k} needed"))
    };
    let (t, out) = (PathBuf::from(val("--table")?), PathBuf::from(val("--out")?));
    let index: i64 = val("--index")?.parse().map_err(|e| format!("--index: {e}"))?;
    let vb = std::fs::read(val("--verdict")?).map_err(|e| e.to_string())?;
    let c: Verdict = serde_json::from_slice(&vb).map_err(|e| format!("verdict: {e}"))?;
    let text = std::fs::read_to_string(&t).map_err(|e| format!("{}: {e}", t.display()))?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let pos = lines
        .iter()
        .position(|l| string_member(l, "type").as_deref() == Some("rule") && serde_json::from_str::<Value>(&strip_derivation(l).map(|x| x.0).unwrap_or_default()).ok().and_then(|v| v["index"].as_i64()) == Some(index))
        .ok_or(format!("no rule record with index {index}"))?;
    if let Some(why) = check_line(&lines[pos], &c)? {
        return Ok((json!({"stamped": false, "reason": why}).to_string(), false));
    }
    let side = side_dir(&out);
    super::carry_side(&side_dir(&t), &side)?;
    let sha = write_side(&side, &vb)?;
    lines[pos] = stamp_line(&lines[pos], &sha)?;
    std::fs::write(&out, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    Ok((json!({"stamped": true, "table": out.display().to_string(), "cone_sha256": sha}).to_string(), true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(target: i64, old: &str) -> String {
        let body = format!(r#"{{"type":"rule","index":7,"target":[{target},1],"rhs":[[[0,1],"1/(d-4)"]]}}"#);
        attach_derivation(&body, &json!({"class": {"word": "whole domain"}, "replaces": {"old_body_sha256": old, "regularity_sha256": "r"}}))
            .unwrap()
    }

    fn verdict(accepted: bool, old: &str, bodies: Vec<String>) -> Vec<u8> {
        let v = Verdict {
            schema: "rustred.emission.cone-price.v1".into(),
            old_body_sha256: Some(old.into()),
            new_body_sha256: bodies,
            request: json!({}),
            entrance_phase: 4,
            phases_read: vec![4, 5, 6, 7],
            levels: vec![],
            entrance_ratio_old_over_new: Some(2.0),
            second_apply_ratio_old_over_new: Some(if accepted { 1.5 } else { 0.98 }),
            owners_grown: vec![],
            second_apply_shells: Value::Null,
            second_apply_owners: vec![],
            values_note: String::new(),
            third_level: None,
            rules: Rules::default(),
            accepted,
            reason: (!accepted).then(|| "cone_worse: more children at the second Apply".to_string()),
            rank_key: None,
        };
        serde_json::to_vec(&v).unwrap()
    }

    fn body_sha(l: &str) -> String {
        body_ids(&strip_derivation(l).unwrap().0).unwrap().body_sha256
    }

    #[test]
    fn emit_replace_cone_verdict_stamps_every_piece_or_refuses_the_replacement() {
        let side = std::env::temp_dir().join(format!("coneprice-apply-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&side);
        let lines = vec![piece(1, "OLD"), piece(2, "OLD")];
        let both: Vec<String> = lines.iter().map(|l| body_sha(l)).collect();
        // accepted, of this old record, naming both bodies: every piece stamped, the side file written once
        let vb = verdict(true, "OLD", both.clone());
        match apply_to_pieces(&lines, &vb, &side).unwrap() {
            Applied::Stamped { lines: st, sha256 } => {
                assert_eq!(sha256, sha256_hex(&vb));
                assert_eq!(st.len(), 2);
                for (a, b) in st.iter().zip(&lines) {
                    let d: Value = serde_json::from_str(&strip_derivation(a).unwrap().1.unwrap()).unwrap();
                    assert_eq!(d["replaces"]["cone_sha256"], json!(sha256));
                    assert_eq!(d["replaces"]["regularity_sha256"], json!("r"), "the regularity member is kept");
                    assert_eq!(body_sha(a), body_sha(b), "the body is unchanged");
                }
                assert_eq!(std::fs::read(side.join(side_rel(AREA_CONE, &sha256))).unwrap(), vb);
            }
            other => panic!("{other:?}"),
        }
        let side2 = side.join("refusals");
        let refused = |vb: Vec<u8>, ls: &[String]| match apply_to_pieces(ls, &vb, &side2).unwrap() {
            Applied::Refused { reason, piece, .. } => (reason, piece),
            other => panic!("{other:?}"),
        };
        // a verdict naming only the first body refuses at the second piece
        let (r, p) = refused(verdict(true, "OLD", vec![both[0].clone()]), &lines);
        assert_eq!((r.as_str(), p), ("the cone verdict does not name this body", 1));
        // a refusing verdict
        let (r, p) = refused(verdict(false, "OLD", both.clone()), &lines);
        assert!(r.starts_with("the cone verdict refuses the candidate: cone_worse") && p == 0, "{r}");
        // a verdict pricing another old record
        let (r, _) = refused(verdict(true, "OTHER", both.clone()), &lines);
        assert_eq!(r, "the cone verdict prices another old record");
        // a piece that is not a replacement (no `replaces`)
        let plain = attach_derivation(r#"{"type":"rule","index":7,"target":[1,1],"rhs":[]}"#, &json!({"class": {}})).unwrap();
        let (r, _) = refused(verdict(true, "OLD", vec![]), &[plain]);
        assert!(r.starts_with("the record is not a replacement"), "{r}");
        // nothing written by a refusal
        assert!(!side2.exists());
        let _ = std::fs::remove_dir_all(&side);
    }
}
