//! Shared helpers of the rustred-campaign-algebra tests: the licence gate (tests SKIP without the Symbolica licence:
//! unlicensed Symbolica processes of one Unix user abort each other host-wide), the core crate's test data, the
//! frozen k3 tree (helpers of the core tests, included by path) and the campaign fixtures.
#![allow(dead_code)]

#[path = "../../../rustred-campaign-core/tests/k3_common/mod.rs"]
pub mod k3_common;

#[allow(unused_imports)]
pub use k3_common::{HO, Oracle, gunzip, oracle, sha256_hex};
use rustred_campaign_core::ibp_rows::RowsRule;
use serde_json::Value;

/// The core crate's rows test data (family copies, tables).
pub fn core_data(name: &str) -> String {
    let p = format!(
        "{}/../rustred-campaign-core/tests/data/rows/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// The frozen campaign fixtures (RUSTRED_CAMPAIGN_FIXTURES or the default path).
pub fn campaign_fixtures() -> String {
    std::env::var("RUSTRED_CAMPAIGN_FIXTURES").unwrap_or_else(|_| {
        "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/fixtures/campaign-20261114".into()
    })
}

/// `true` when the Symbolica licence is available; otherwise prints a SKIP line.
pub fn licensed_or_skip(test: &str) -> bool {
    if rustred_campaign_algebra::licensed() {
        return true;
    }
    println!(
        "SKIP {test}: SYMBOLICA_LICENSE absent or invalid (source WS/ops/env.sh first; unlicensed Symbolica runs abort each other host-wide)"
    );
    false
}

/// The exact source line of an F6 span case (inline copy or HO byte range; sha256-checked). `None` = not readable.
pub fn f6_line(o: &Oracle, case: &Value) -> Option<String> {
    let bytes = match case["inline_record"].as_str() {
        Some(rel) => o.bytes(&format!("span/{rel}")),
        None => {
            let path = format!("{HO}/{}", case["file"].as_str().unwrap());
            let off = case["byte_offset"].as_u64().unwrap();
            let len = case["byte_len"].as_u64().unwrap() as usize;
            use std::io::{Read, Seek, SeekFrom};
            let Ok(mut f) = std::fs::File::open(&path) else {
                crate::inputs::missing(format_args!("SKIP: {path} not readable"));
                return None;
            };
            f.seek(SeekFrom::Start(off)).unwrap();
            let mut b = vec![0u8; len];
            f.read_exact(&mut b).unwrap();
            b
        }
    };
    assert_eq!(sha256_hex(&bytes), case["line_sha256"].as_str().unwrap());
    Some(String::from_utf8(bytes).unwrap())
}

/// Rule 0 of a rule line (export wrapper or bare record).
pub fn rule0(line: &str, arity: usize) -> RowsRule {
    let mut rs = RowsRule::from_json_line(line.trim_end(), arity).expect("rule line");
    assert!(!rs.is_empty(), "no rule in line");
    rs.remove(0)
}

/// The rules of a JSONL table (header skipped) or a gzipped one.
pub fn table_rules(path: &str, arity: usize, max: usize) -> (Option<Value>, Vec<RowsRule>) {
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let bytes = if path.ends_with(".gz") {
        gunzip(&raw)
    } else {
        raw
    };
    let text = String::from_utf8(bytes).unwrap();
    let mut header = None;
    let mut out = Vec::new();
    for l in text.lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        if v["type"] == "header" {
            header = Some(v);
            continue;
        }
        if v["type"] != "rule" && v.get("rules").is_none() {
            continue;
        }
        if let Ok(rs) = RowsRule::from_json_line(l, arity) {
            out.extend(rs);
        }
        if out.len() >= max {
            out.truncate(max);
            break;
        }
    }
    (header, out)
}
