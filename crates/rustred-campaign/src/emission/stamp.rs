//! `rr_campaign descent-stamp` (emission contract section 14, round 5): records EMITTED BEFORE section 14 (origin lift, no
//! `descent` member: e.g. the T180p2 v6 addendum pieces of rr_campaign.v10) get the descent member by the engine's own test
//! on their bytes, exactly as the emitter would have added it (the descent evidence is independent of the check, class and
//! regularity members; the body and its sha256 are unchanged). A record that is NOT LOWER, or whose test cannot run, is
//! REFUSED and nothing is written. Records that already carry the member, imports and non-rule lines are copied unchanged.
//! The side directory is carried to the output's and the output must pass table-check.
use super::emitter::{Checker, Emitter};
use rustred_campaign_core::emission::body::{attach_derivation, string_member, strip_derivation};
use rustred_campaign_core::emission::class::Policy;
use rustred_campaign_core::emission::side_dir;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Stamp `table` into `out` (see the module text). Returns (summary, written).
pub fn descent_stamp(
    table: &Path,
    out: &Path,
    node: &Path,
    work: &Path,
) -> Result<(Value, bool), String> {
    let text = std::fs::read_to_string(table).map_err(|e| format!("{}: {e}", table.display()))?;
    let header = text
        .lines()
        .find(|l| string_member(l, "type").as_deref() == Some("header"))
        .unwrap_or("")
        .to_string();
    let mut ck = Checker::new(node)?;
    ck.descent_node = Some((node.to_path_buf(), ck.sha256.clone()));
    let side_out = side_dir(out);
    let em = Emitter::new(ck, Policy::default(), &side_out, work)?;
    let (mut stamped, mut kept, mut refused) = (0u64, 0u64, vec![]);
    let mut lines = Vec::with_capacity(text.lines().count());
    for (n, l) in text.lines().enumerate() {
        if string_member(l, "type").as_deref() != Some("rule") {
            lines.push(l.to_string());
            continue;
        }
        let (body, raw) = strip_derivation(l)?;
        let mut d: Value = match raw.as_deref().map(serde_json::from_str::<Value>) {
            Some(Ok(d)) => d,
            _ => {
                lines.push(l.to_string());
                kept += 1;
                continue;
            }
        };
        if d["origin"] != "lift" || !d["descent"].is_null() {
            lines.push(l.to_string());
            kept += 1;
            continue;
        }
        let (tuple, refusal) = em.descent(&header, &body, &format!("l{n}"))?;
        match (tuple, refusal) {
            (Some((tool, out_sha, verdict, first)), None) => {
                d["descent"] = json!({"tool_sha256": tool, "output_sha256": out_sha, "verdict": verdict, "first": first});
                lines.push(attach_derivation(&body, &d)?);
                stamped += 1;
            }
            (_, Some(why)) => {
                let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                refused.push(json!({"line": n + 1, "index": v["index"], "reason": why}));
                lines.push(l.to_string());
            }
            (None, None) => refused.push(json!({"line": n + 1, "reason": "descent_not_checked"})),
        }
    }
    let mut s = json!({"schema": "rustred.campaign.descent-stamp.v1", "table": table.display().to_string(),
        "descent_node": node.display().to_string(), "stamped": stamped, "kept": kept, "refused": refused.len(),
        "refusals": refused.iter().take(50).collect::<Vec<_>>()});
    if !refused.is_empty() {
        s["verdict"] = json!("REFUSED (nothing written)");
        return Ok((s, false));
    }
    super::carry_side(&side_dir(table), &side_out)?;
    let tmp = out.with_extension("jsonl.part");
    std::fs::write(&tmp, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())?;
    let (tc, ok) = super::tablecheck::table_check(&super::tablecheck::Args {
        table: out.to_path_buf(),
        every: 1,
        work: Some(work.join("table-check")),
        ..Default::default()
    })?;
    s["table_check"] = json!({"verdict": tc["verdict"], "rules": tc["rules"], "refused": tc["refused"], "classes": tc["classes"],
                              "refusals": tc["refusals"]});
    s["verdict"] = json!(if ok {
        "STAMPED"
    } else {
        "STAMPED BUT TABLE-CHECK REFUSED"
    });
    Ok((s, ok))
}

/// CLI `descent-stamp --table T --out T2 --descent-node BIN [--work DIR]`: exit 0 stamped and accepted, 3 otherwise.
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let (mut t, mut o, mut n, mut w) = (
        None::<PathBuf>,
        None::<PathBuf>,
        None::<PathBuf>,
        None::<PathBuf>,
    );
    let mut i = 0;
    let val = |i: usize| {
        rest.get(i + 1)
            .cloned()
            .ok_or(format!("{} needs a value", rest[i]))
    };
    while i < rest.len() {
        match rest[i].as_str() {
            "--table" => t = Some(val(i)?.into()),
            "--out" => o = Some(val(i)?.into()),
            "--descent-node" => n = Some(val(i)?.into()),
            "--work" => w = Some(val(i)?.into()),
            x => return Err(format!("unknown flag {x}")),
        }
        i += 2;
    }
    let (Some(t), Some(o), Some(n)) = (t, o, n) else {
        return Err("descent-stamp --table T --out T2 --descent-node BIN [--work DIR]".into());
    };
    if t == o {
        return Err("--out must differ from --table".into());
    }
    let w = w.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("descent-stamp-{}", std::process::id()))
    });
    let (s, ok) = descent_stamp(&t, &o, &n, &w)?;
    Ok((s.to_string(), ok))
}
