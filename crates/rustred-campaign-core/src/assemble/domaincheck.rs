//! `dispatch_equal` (plan 2.8): the core dispatcher over an assembled table FILE and the engine's matcher on the same
//! file (`rr_node domaincheck`, interfaces/domaincheck.md) agree on every key of a key set. The library writes the key
//! file and parses the engine's OUT.jsonl; running the binary is the caller's (a child job, or a test oracle).

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::parse_key_text;
use crate::cover::{Dispatch, Dispatcher, Table};
use crate::domain::DomainOptions;
use crate::key::Key;
use crate::record::{ReadOptions, RhsMode, read_table_file};

/// The core's view of an assembled table: one-table dispatcher plus the declared terminals (`residual` records).
pub struct CoreView {
    pub disp: Dispatcher,
    pub terminals: BTreeSet<Key>,
}

/// Load an assembled table the way the core does (records in file order; terminals from `residual` records).
pub fn core_view(path: &Path) -> Result<CoreView, String> {
    let tf = read_table_file(path, ReadOptions { rhs: RhsMode::Skip }).map_err(|e| e.to_string())?;
    let t = Table::from_table_file("assembled", path.display().to_string(), &tf, DomainOptions::default())
        .map_err(|e| e.to_string())?;
    if t.skipped_count() > 0 {
        return Err("assembled table holds non-Partition records: positions would differ (refused)".into());
    }
    let disp = Dispatcher::new(vec![t], vec![0]).map_err(|e| e.to_string())?;
    let mut terminals = BTreeSet::new();
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    for line in text.lines() {
        if !line.contains("\"residual\"") {
            continue;
        }
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if v.get("type").and_then(Value::as_str) == Some("residual") {
            let k = v
                .get("key")
                .and_then(Value::as_str)
                .and_then(parse_key_text)
                .ok_or("residual record without a readable key")?;
            terminals.insert(k);
        }
    }
    Ok(CoreView { disp, terminals })
}

/// The key file of `rr_node domaincheck --keys FILE` (one `a,b,..` per line).
pub fn key_file(keys: &[Key]) -> String {
    let mut s = String::new();
    for k in keys {
        s.push_str(&k.iter().map(i32::to_string).collect::<Vec<_>>().join(","));
        s.push('\n');
    }
    s
}

/// One disagreement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Mismatch {
    pub key: Key,
    pub core: String,
    pub engine: String,
}

/// Result of the comparison.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DispatchEqual {
    pub keys: u64,
    pub agree: u64,
    pub rule: u64,
    pub terminal: u64,
    pub none: u64,
    /// Core Blocked where the engine selects the same rule (the engine does not test poles / elision).
    pub core_blocked: u64,
    pub mismatches: Vec<Mismatch>,
}

impl DispatchEqual {
    /// Equal with `core_blocked` allowed only when `allow_core_blocked`.
    pub fn equal(&self, allow_core_blocked: bool) -> bool {
        self.mismatches.is_empty() && (allow_core_blocked || self.core_blocked == 0) && self.agree == self.keys
    }
}

/// Compare the core view with the engine's OUT.jsonl (keys in the same order as given to the engine).
pub fn compare(core: &CoreView, keys: &[Key], engine_out: &str) -> Result<DispatchEqual, String> {
    let mut lines = Vec::new();
    for l in engine_out.lines() {
        let v: Value = serde_json::from_str(l).map_err(|e| e.to_string())?;
        if v.get("type").and_then(Value::as_str) == Some("key") {
            lines.push(v);
        }
    }
    if lines.len() != keys.len() {
        return Err(format!("engine answered {} keys, {} given", lines.len(), keys.len()));
    }
    let mut r = DispatchEqual::default();
    for (k, v) in keys.iter().zip(&lines) {
        r.keys += 1;
        let ek = v.get("key").and_then(Value::as_str).and_then(parse_key_text);
        if ek.as_ref() != Some(k) {
            return Err(format!("engine key {ek:?} differs from {k:?}"));
        }
        let decision = v.get("decision").and_then(Value::as_str).unwrap_or("");
        let sel = v
            .get("selected")
            .and_then(|s| s.get("rule"))
            .and_then(Value::as_u64)
            .map(|x| x as usize);
        let core_term = core.terminals.contains(k);
        let d = core.disp.first_match(k).map_err(|e| e.to_string())?;
        let (ok, cs, blocked) = match (&d, decision) {
            _ if core_term => (decision == "master", "terminal".to_string(), false),
            (Dispatch::Covered(rr), "rule") => (sel == Some(rr.record), format!("rule {}", rr.record), false),
            (Dispatch::Uncovered, "none") => (true, "none".into(), false),
            (Dispatch::Blocked { at, .. }, "rule") if sel == Some(at.record) => {
                (true, format!("blocked {}", at.record), true)
            }
            (Dispatch::OutsideChart { .. }, "lower" | "zero") => (true, "outside chart".into(), false),
            (other, _) => (false, format!("{other:?}"), false),
        };
        if ok {
            r.agree += 1;
            if blocked {
                r.core_blocked += 1;
            } else if core_term {
                r.terminal += 1;
            } else if decision == "rule" {
                r.rule += 1;
            } else if decision == "none" {
                r.none += 1;
            }
        } else {
            r.mismatches.push(Mismatch {
                key: k.clone(),
                core: cs,
                engine: format!("{decision} {sel:?}"),
            });
        }
    }
    Ok(r)
}
