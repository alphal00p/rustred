//! D2 per terminal set (lane poles, B2; pole-diagnostics.md 1.5, 3, 6.3, addenda 2, 3, 5; POLES_OBJECTIVE.md 2(B);
//! TRICKS 16 / 17): the SMALL RECORD other lanes read, and the node-basis records of the engine route.
//!
//! - [`TerminalSetRecord`] (`rustred.campaign.pole-basis.v1`): (terminal set digest, d0, `regular` | `degenerate(order)` |
//!   `unresolved`, evidence). The verdict of record comes from the EXACT master map onto a reference set (route X,
//!   algebra crate `valuation::basis::check_master_map`); the engine route E (rank at a d0 sample against generic
//!   samples) is attached as a SCREEN that proves nothing alone (the node-local system can lose rank at d0 itself).
//! - [`node_basis_from_symmasters`]: the ADAPTER (replaceable) from `rr_node symmasters` OUT.json to the contract's
//!   node-basis records (`sample` / `dim` / `end`, crates/rustred-campaign-contract/src/node_basis.rs): the frozen engine
//!   has no node-basis job kind (DEVIATION in WS/notes/implementer.poles.md). Members the legacy verb does not report
//!   are named (`rank_system_rows: "ibp"`: the rank of the IBP rows; the rank after the symmetry rows is not printed).
//! - [`screen_of_node_basis`]: rank_T at d0 against the generic dims from node-basis records (any source).
//!
//! A probe or a d0 sample is never a pole verdict (TRICKS 17). Generic (L8): keys are opaque texts.

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const POLE_BASIS_SCHEMA: &str = "rustred.campaign.pole-basis.v1";

/// Canonical rational text of `n/d` texts as symmasters prints them (`"4/1"` -> `"4"`).
pub fn canonical_rational_text(t: &str) -> Result<String, String> {
    crate::poles::params::canonical_dimension(t).map_err(|e| e.0)
}

/// sha256 of the canonical JSON of the terminal list (order kept: the set is given in the caller's order).
pub fn terminal_set_digest(terminals: &[String]) -> String {
    let t = crate::schema::canon::canonical_json(&json!(terminals), false).expect("strings only");
    crate::replay_base::sha256::sha256_hex(t.as_bytes())
}

/// The small record of a terminal-set check.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalSetRecord {
    pub node: String,
    pub d0: String,
    /// `found` (the closure's), `published`, `regular`, or a comparison label.
    pub label: String,
    pub terminals: Vec<String>,
    /// The reference set R of the map (route X); empty when only the screen ran.
    pub reference: Vec<String>,
    /// `regular` / `degenerate` / `unresolved`.
    pub verdict: String,
    /// Degenerate: the largest pole order a key regular on R has on T (M^-1).
    pub order: Option<u64>,
    /// Route X details (`MapCheck::to_json` of the algebra crate) or null.
    pub route_x: Value,
    /// Route E screen (rank_T at d0 / generic) or null.
    pub screen: Value,
    /// Where the map / records came from: `{source, sha256, band, ...}` (data).
    pub evidence: Value,
}

impl TerminalSetRecord {
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "schema": POLE_BASIS_SCHEMA,
            "node": self.node,
            "d0": self.d0,
            "label": self.label,
            "terminal_set": self.terminals,
            "terminal_set_sha256": terminal_set_digest(&self.terminals),
            "reference_set": self.reference,
            "verdict": self.verdict,
            "order": self.order,
            "route_x": self.route_x,
            "screen": self.screen,
            "screen_meaning": "rank of T at a d0 sample against generic samples: proves nothing alone (the system can lose rank at d0 itself); the verdict is the exact map's",
            "evidence": self.evidence,
        });
        let t = crate::schema::canon::canonical_json(&v, true).unwrap_or_else(|_| v.to_string());
        v["record_digest"] = json!(crate::replay_base::sha256::sha256_hex(t.as_bytes()));
        v
    }
}

fn texts(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// node-basis records (`sample` per (d, p), `dim` per d, `end`) from a `rr_node symmasters` report run with the
/// terminals as `--masters` and `--prefer` (so they are the LOWEST columns). `terminals` = their key texts as symmasters
/// prints them; `d0` = the canonical text of the sampling dimension; the other dims are the generic points.
pub fn node_basis_from_symmasters(
    doc: &Value,
    terminals: &[String],
    d0: &str,
) -> Result<Vec<Value>, String> {
    let tset: BTreeSet<&str> = terminals.iter().map(String::as_str).collect();
    let samples = doc["samples"]
        .as_array()
        .ok_or("symmasters report without samples")?;
    // exact maps per d (CRT + rational reconstruction over the primes, by symmasters)
    let mut exact: BTreeMap<String, &Value> = BTreeMap::new();
    for e in doc["exact"]
        .as_array()
        .ok_or("symmasters report without exact maps")?
    {
        let d = canonical_rational_text(e["d"].as_str().ok_or("exact entry without d")?)?;
        exact.insert(d, &e["masters"]);
    }
    let mut out: Vec<Value> = vec![];
    let mut per_d_idx: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for s in samples {
        let d = canonical_rational_text(s["d"].as_str().ok_or("sample without d")?)?;
        let p: u64 = s["p"]
            .as_str()
            .and_then(|x| x.parse().ok())
            .ok_or("sample without prime")?;
        let free = texts(&s["VG_M"]);
        let rank_t = free.iter().filter(|k| tset.contains(k.as_str())).count();
        let pivots: Vec<Value> = terminals
            .iter()
            .filter(|t| !free.contains(t))
            .map(|t| {
                // the exact map of a pivoted terminal: an EMPTY map = it reduces to 0 (a valid null direction e_t);
                // an ABSENT map (no reconstruction) = unknown
                let row = exact
                    .get(&d)
                    .and_then(|m| m[t.as_str()].as_object())
                    .map(|o| o.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>());
                let known = row.is_some();
                json!({"pivot": t, "row": row.unwrap_or_default(), "row_known": known,
                    "row_field": "Q (symmasters exact map at this d)"})
            })
            .collect();
        let rows = s["rows_ibp"].as_u64().unwrap_or(0) + s["rows_sym"].as_u64().unwrap_or(0);
        out.push(json!({"type": "sample", "d": d, "prime": p,
            "rank_system": s["pivots_ibp_T"], "rank_system_rows": "ibp",
            "rows": rows, "columns": s["cols"], "free_columns": free, "rank_T": rank_t,
            "pivots_in_terminals": pivots,
            "unconstrained_inner": s["unconstrained_inner"], "leaks": s["leaks"], "legacy": "rr_node symmasters"}));
        per_d_idx.entry(d).or_default().push(out.len() - 1);
    }
    let mut dims = vec![];
    for (d, idx) in &per_d_idx {
        let ss: Vec<&Value> = idx.iter().map(|&i| &out[i]).collect();
        let same = ss
            .iter()
            .all(|s| s["free_columns"] == ss[0]["free_columns"]);
        let mut nulls = vec![];
        for pv in ss[0]["pivots_in_terminals"]
            .as_array()
            .into_iter()
            .flatten()
        {
            // pivot t = sum_k c_k k (k in T, lower): direction e_t - sum c_k e_k on the terminal order
            let t = pv["pivot"].as_str().unwrap_or("");
            let mut dir: Vec<Value> = vec![json!([t, "1"])];
            for e in pv["row"].as_array().into_iter().flatten() {
                let k = e[0].as_str().unwrap_or("");
                if k == t {
                    continue;
                }
                let c = e[1].as_str().map(|c| {
                    if let Some(x) = c.strip_prefix('-') {
                        x.to_string()
                    } else {
                        format!("-{c}")
                    }
                });
                dir.push(json!([k, c]));
            }
            nulls.push(Value::Array(dir));
        }
        let complete = ss[0]["pivots_in_terminals"]
            .as_array()
            .is_some_and(|a| a.iter().all(|p| p["row_known"] == true));
        dims.push(json!({"type": "dim", "d": d, "rank_T": ss[0]["rank_T"], "rank_system": ss[0]["rank_system"],
            "free_columns": ss[0]["free_columns"], "null_directions": nulls,
            "status": if same && complete { "ok" } else { "unresolved" }}));
    }
    let r0 = dims
        .iter()
        .find(|x| x["d"] == d0)
        .and_then(|x| x["rank_T"].as_u64());
    let rg = dims
        .iter()
        .filter(|x| x["d"] != d0)
        .filter_map(|x| x["rank_T"].as_u64())
        .collect::<BTreeSet<_>>();
    let unresolved = dims.iter().any(|x| x["status"] != "ok") || rg.len() != 1 || r0.is_none();
    let (deficiency, verdict) = match (r0, rg.iter().next()) {
        (Some(a), Some(&g)) if !unresolved => (
            json!(g.saturating_sub(a)),
            if g > a { "degenerate" } else { "regular" },
        ),
        // the contract's end record always carries a number: the rank loss read from the first sample of each dim
        // (largest generic rank_T against the d0 rank_T, 0 when either is missing), with the verdict `unresolved`
        _ => (
            json!(match (r0, rg.iter().next_back()) {
                (Some(a), Some(&g)) => g.saturating_sub(a),
                _ => 0,
            }),
            "unresolved",
        ),
    };
    out.extend(dims);
    out.push(json!({"type": "end", "deficiency": deficiency, "verdict": verdict,
        "meaning": "screen: rank_T at the d0 sample against the generic samples (never a pole verdict)"}));
    Ok(out)
}

/// The screen from node-basis records (any source): `{rank_T_d0, rank_T_generic, deficiency, verdict, free_columns_d0,
/// free_columns_generic, null_directions_d0}`.
pub fn screen_of_node_basis(records: &[Value], d0: &str) -> Value {
    let dims: Vec<&Value> = records.iter().filter(|r| r["type"] == "dim").collect();
    let at0 = dims.iter().find(|r| r["d"] == d0);
    let generic: Vec<&&Value> = dims.iter().filter(|r| r["d"] != d0).collect();
    let end = records.iter().find(|r| r["type"] == "end");
    json!({
        "rank_T_d0": at0.map(|r| r["rank_T"].clone()),
        "rank_T_generic": generic.iter().map(|r| r["rank_T"].clone()).collect::<Vec<_>>(),
        "free_columns_d0": at0.map(|r| r["free_columns"].as_array().map_or(0, Vec::len)),
        "free_columns_generic": generic.iter().map(|r| r["free_columns"].as_array().map_or(0, Vec::len)).collect::<Vec<_>>(),
        "null_directions_d0": at0.map(|r| r["null_directions"].clone()),
        "deficiency": end.map(|e| e["deficiency"].clone()),
        "verdict": end.map(|e| e["verdict"].clone()),
    })
}
