//! Kind `node-basis` (line 7, coord-pole-engine.md 4 / 8; child-job.md "Line 7"): the closed record list of its sealed
//! `out/results.jsonl`. A DIAGNOSTIC kind: these records are never rule or relation evidence (accept mode
//! `diagnostic_records`, decision row 30).
//!
//! Records (in any order, the end record last):
//! - `{"type":"sample", d, prime, rank_system, rows, columns, free_columns: [..], rank_T, pivots_in_terminals: [{pivot,
//!   row}..]}`: one per (d, prime);
//! - `{"type":"dim", d, rank_T, rank_system, free_columns: [..], null_directions: [..], status: "ok"|"unresolved"}`: one
//!   per d;
//! - `{"type":"end", deficiency, verdict: "degenerate"|"regular"|"unresolved"}`.
//!
//! `d` is the canonical rational text (a string), `prime` an integer under the line-7 prime rule. When the spec names
//! `--dims` (and `--primes`), every record's d (and prime) is one of them and the records are COMPLETE: a sample for
//! every (d, prime), a dim record for every d. Generic: no arity, key form or family is assumed (keys are opaque).

use crate::arith::{Rational, check_line7_prime, parse_prime_list, parse_rational_list};
use crate::spec::{JobKind, JobSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// Verdicts of the end record.
pub const NODE_BASIS_VERDICTS: [&str; 3] = ["degenerate", "regular", "unresolved"];
/// `status` values of a `dim` record.
pub const DIM_STATUSES: [&str; 2] = ["ok", "unresolved"];

/// The spec's `--dims` / `--primes` (absent = the engine's defaults, recorded in the records).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeBasisArgs {
    pub dims: Option<Vec<Rational>>,
    pub primes: Option<Vec<u64>>,
}

impl NodeBasisArgs {
    pub fn of_spec(spec: &JobSpec) -> Result<NodeBasisArgs, String> {
        if spec.kind != JobKind::NodeBasis {
            return Err(format!("kind {:?} is not node-basis", spec.kind));
        }
        Ok(NodeBasisArgs {
            dims: spec
                .arg_value("--dims")
                .map(parse_rational_list)
                .transpose()?,
            primes: spec
                .arg_value("--primes")
                .map(parse_prime_list)
                .transpose()?,
        })
    }
}

/// What a checked node-basis results file holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeBasisSummary {
    pub samples: u64,
    pub dims: u64,
    /// The end record's verdict.
    pub verdict: String,
}

fn uint(v: &Value, m: &str) -> Result<u64, String> {
    v[m].as_u64()
        .ok_or_else(|| format!("{m} is not an unsigned integer"))
}

fn list<'a>(v: &'a Value, m: &str) -> Result<&'a Vec<Value>, String> {
    v[m].as_array().ok_or_else(|| format!("{m} is not a list"))
}

fn dim_of(v: &Value, args: &NodeBasisArgs) -> Result<Rational, String> {
    let t = v["d"].as_str().ok_or("d is not a rational text")?;
    let d = Rational::parse_canonical(t).map_err(|e| format!("d: {e}"))?;
    if args.dims.as_ref().is_some_and(|ds| !ds.contains(&d)) {
        return Err(format!("d {d} is not one of the job's --dims"));
    }
    Ok(d)
}

/// Check the canonical lines of a VERIFIED node-basis results file (end record last) against the job's arguments.
/// `Err` = not a consistent node-basis results file of this job (S false: contract violation).
pub fn check_results(lines: &[Vec<u8>], args: &NodeBasisArgs) -> Result<NodeBasisSummary, String> {
    let (end_line, recs) = lines.split_last().ok_or("results file without records")?;
    let end: Value = serde_json::from_slice(end_line).map_err(|e| format!("end record: {e}"))?;
    if end["type"] != "end" {
        return Err("the last record is not the end record".into());
    }
    let verdict = end["verdict"].as_str().unwrap_or("");
    if !NODE_BASIS_VERDICTS.contains(&verdict) {
        return Err(format!(
            "end verdict {:?} not in {NODE_BASIS_VERDICTS:?}",
            end["verdict"]
        ));
    }
    if end["deficiency"].is_null() {
        return Err("end record without deficiency".into());
    }
    let mut samples: BTreeSet<(Rational, u64)> = BTreeSet::new();
    let mut dims: BTreeSet<Rational> = BTreeSet::new();
    for (i, l) in recs.iter().enumerate() {
        let v: Value = serde_json::from_slice(l).map_err(|e| format!("record {i}: {e}"))?;
        let at = |m: String| format!("record {i}: {m}");
        match v["type"].as_str() {
            Some("sample") => {
                let d = dim_of(&v, args).map_err(at)?;
                let p = uint(&v, "prime").map_err(at)?;
                check_line7_prime(p).map_err(at)?;
                if args.primes.as_ref().is_some_and(|ps| !ps.contains(&p)) {
                    return Err(at(format!("prime {p} is not one of the job's --primes")));
                }
                let rank = uint(&v, "rank_system").map_err(at)?;
                let rows = uint(&v, "rows").map_err(at)?;
                let columns = uint(&v, "columns").map_err(at)?;
                uint(&v, "rank_T").map_err(at)?;
                if rank > rows || rank > columns {
                    return Err(at(format!(
                        "rank_system {rank} exceeds rows {rows} or columns {columns}"
                    )));
                }
                list(&v, "free_columns").map_err(at)?;
                for e in list(&v, "pivots_in_terminals").map_err(at)? {
                    if e["pivot"].is_null() || !e["row"].is_array() {
                        return Err(at("pivots_in_terminals entry without pivot / row".into()));
                    }
                }
                if !samples.insert((d, p)) {
                    return Err(at(format!("two samples at d {d}, prime {p}")));
                }
            }
            Some("dim") => {
                let d = dim_of(&v, args).map_err(at)?;
                uint(&v, "rank_T").map_err(at)?;
                uint(&v, "rank_system").map_err(at)?;
                list(&v, "free_columns").map_err(at)?;
                list(&v, "null_directions").map_err(at)?;
                let st = v["status"].as_str().unwrap_or("");
                if !DIM_STATUSES.contains(&st) {
                    return Err(at(format!(
                        "status {:?} not in {DIM_STATUSES:?}",
                        v["status"]
                    )));
                }
                if !dims.insert(d) {
                    return Err(at(format!("two dim records at d {d}")));
                }
            }
            _ => return Err(at(format!("type {} is not a node-basis record", v["type"]))),
        }
    }
    if let Some(ds) = &args.dims {
        if dims.len() != ds.len() {
            return Err(format!(
                "{} dim records for {} --dims",
                dims.len(),
                ds.len()
            ));
        }
        if let Some(ps) = &args.primes
            && samples.len() != ds.len() * ps.len()
        {
            return Err(format!(
                "{} samples for {} dims x {} primes",
                samples.len(),
                ds.len(),
                ps.len()
            ));
        }
    }
    Ok(NodeBasisSummary {
        samples: samples.len() as u64,
        dims: dims.len() as u64,
        verdict: verdict.to_string(),
    })
}
