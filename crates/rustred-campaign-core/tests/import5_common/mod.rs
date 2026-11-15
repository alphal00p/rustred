//! Shared helpers of the lane-import5 tests (constructed tables on a toy node: the two-loop sunset, arity 5, lines
//! D1 D2 D3, numerators D4 D5; nothing five-loop specific). Used by tests/import_*.rs of lane import5.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use import::{CheckOptions, Evidence, TableSetSpec, check, load};
use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import;
use serde_json::{Value, json};

pub const N: usize = 5;
pub const MASK: &str = "11100";
pub const FAMILY: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

pub fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("import5-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn kt(k: &[i32]) -> String {
    format!(
        "I({})",
        k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    )
}

pub fn header_value() -> Value {
    json!({"type":"header","schema":"table_dump.v1","family":"S","denominators":N,"mask":MASK,
        "order_name":"legacy","family_source":FAMILY})
}

pub fn chart_line() -> String {
    json!({"type":"chart","chart":MASK,"guards":["n>=1","n>=1","n>=1","n<=0","n<=0"],"zero":false})
        .to_string()
}

/// A POINT rule at `key` (every slot fixed), body `rhs` (coefficient 1 each), complete (empty) witness loci.
pub fn point_rule(index: i64, key: &[i32], rhs: &[&[i32]]) -> String {
    json!({"type":"rule","chart":MASK,"index":index,"fixed":key,"equations":[],"target":kt(key),
        "rhs": rhs.iter().map(|k| json!([kt(k), "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

/// The sunset's k1 <-> k2 swap: D1 <-> D2, D3 fixed, D4 <-> D5; T over (k1, k2, q), row-vector convention.
pub const SWAP_MATRIX: [i64; 9] = [0, 1, 0, 1, 0, 0, 0, 0, 1];

/// A `sym` record (engine layout) for the swap on the case `fixed`.
pub fn sym_record(index: i64, fixed: &[Option<i32>], sign: &str) -> Value {
    let pat: Vec<String> = fixed
        .iter()
        .enumerate()
        .map(|(i, f)| f.map_or(format!("n{i}"), |v| v.to_string()))
        .collect();
    let perm = [1usize, 0, 2, 4, 3];
    let mut img = vec!["0".to_string(); N];
    for i in 0..N {
        img[perm[i]] = pat[i].clone();
    }
    json!({"type":"sym","chart":MASK,"index":index,"fixed":fixed,"equations":[],
        "target":format!("I({})",pat.join(",")),"image":format!("I({})",img.join(",")),"element":0,
        "action":[[1,false],[0,false],[2,false],[4,false],[3,false]],"matrix":SWAP_MATRIX,"sign":sign,
        "numerical": fixed.iter().all(Option::is_some)})
}

/// Write a table: header (optionally modified), chart, the given lines, then `tail` lines (default: an end record).
pub fn write_table_with(
    dir: &Path,
    name: &str,
    header: &Value,
    lines: &[String],
    tail: Option<&[String]>,
) -> PathBuf {
    let p = dir.join(name);
    let mut s = header.to_string();
    s.push('\n');
    s.push_str(&chart_line());
    s.push('\n');
    for r in lines {
        s.push_str(r);
        s.push('\n');
    }
    match tail {
        Some(t) => {
            for l in t {
                s.push_str(l);
                s.push('\n');
            }
        }
        None => {
            s.push_str(r#"{"type":"end","charts":1}"#);
            s.push('\n');
        }
    }
    std::fs::write(&p, s).unwrap();
    p
}

pub fn write_table(dir: &Path, name: &str, lines: &[String]) -> PathBuf {
    write_table_with(dir, name, &header_value(), lines, None)
}

pub fn write_keys(dir: &Path, keys: &[&[i32]]) -> PathBuf {
    let p = dir.join("start.jsonl");
    let s: String = keys.iter().map(|k| format!("{}\n", json!(k))).collect();
    std::fs::write(&p, s).unwrap();
    p
}

/// A one-table spec (plus extra sources).
pub fn spec(
    dir: &Path,
    table: &Path,
    start: &[&[i32]],
    extra_src: Value,
    extra: &[Value],
) -> Value {
    let st = write_keys(dir, start);
    let mut src = json!({"id":"A","kind":"engine_table","path":table.display().to_string()});
    if let Some(o) = extra_src.as_object() {
        for (k, v) in o {
            src[k] = v.clone();
        }
    }
    let mut sources = vec![src];
    sources.extend(extra.iter().cloned());
    json!({"schema":"rustred.campaign.import-spec.v1","node":"toy-sunset","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":sources})
}

pub fn try_load(spec: &Value, dir: &Path) -> Result<import::ImportedSet, import::ImportError> {
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    load(&s, dir)
}

pub fn run(spec: &Value, dir: &Path) -> (import::ImportedSet, Evidence, import::CheckResult) {
    let set = try_load(spec, dir).unwrap();
    let ev = Evidence::from_spec(&set, dir).unwrap();
    let r = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 2,
            tier: Some(Tier::K),
            ..Default::default()
        },
    )
    .unwrap();
    (set, ev, r)
}

pub fn row<'a>(r: &'a import::CheckResult, k: &[i32]) -> &'a import::KeyRow {
    r.rows
        .iter()
        .find(|x| x.key == k)
        .unwrap_or_else(|| panic!("key {k:?} not reached"))
}

/// The node comparator of the toy header.
pub fn comparator() -> rustred_campaign_core::order::NodeComparator {
    rustred_campaign_core::order::NodeComparator::from_header(&header_value()).unwrap()
}

/// int7 (coordinator's ruling 2026-11-15): a NAMED list of tables without an `end` record (schema
/// `rustred.campaign.tables-no-end.v1`) listing `tables`; returns the override text `"<list path> sha256:<hex>"`.
#[allow(dead_code)]
pub fn no_end_list(dir: &Path, name: &str, tables: &[&Path]) -> String {
    let v = serde_json::json!({"schema": rustred_campaign_core::import::lines::NO_END_LIST_SCHEMA,
        "tables": tables.iter().map(|t| serde_json::json!({"node": "toy", "path": t.display().to_string(),
            "origin": "test writer omits end"})).collect::<Vec<_>>()});
    let p = dir.join(name);
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let sha = rustred_campaign_core::import::lines::file_sha256(&p).unwrap();
    format!("{} sha256:{sha}", p.display())
}
