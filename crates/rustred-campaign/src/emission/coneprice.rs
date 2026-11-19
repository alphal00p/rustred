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
//!   (`replaces.cone_sha256`, side file cone/<sha>.json); a refused verdict, or one of another old record / body, is not stamped.
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
    let (body, raw) = strip_derivation(&lines[pos])?;
    let mut d: Value = serde_json::from_str(&raw.ok_or("the record has no derivation member")?).map_err(|e| e.to_string())?;
    let refuse = |why: String| Ok((json!({"stamped": false, "reason": why}).to_string(), false));
    if d.get("replaces").is_none_or(Value::is_null) {
        return refuse("the record is not a replacement (no `replaces` member: the d = 4 guard comes first)".into());
    }
    if !c.accepted {
        return refuse(format!("the cone verdict refuses the candidate: {}", c.reason.unwrap_or_default()));
    }
    let bs = body_ids(&body)?.body_sha256;
    if !c.new_body_sha256.is_empty() && !c.new_body_sha256.contains(&bs) {
        return refuse("the cone verdict does not name this body".into());
    }
    if c.old_body_sha256.as_deref().is_some_and(|o| Some(o) != d["replaces"]["old_body_sha256"].as_str()) {
        return refuse("the cone verdict prices another old record".into());
    }
    let sha = sha256_hex(&vb);
    let side = side_dir(&out);
    super::carry_side(&side_dir(&t), &side)?;
    let p = side.join(side_rel(AREA_CONE, &sha));
    std::fs::create_dir_all(p.parent().expect("area")).map_err(|e| e.to_string())?;
    std::fs::write(&p, &vb).map_err(|e| e.to_string())?;
    d["replaces"]["cone_sha256"] = json!(sha);
    lines[pos] = attach_derivation(&body, &d)?;
    std::fs::write(&out, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    Ok((json!({"stamped": true, "table": out.display().to_string(), "cone_sha256": sha}).to_string(), true))
}
