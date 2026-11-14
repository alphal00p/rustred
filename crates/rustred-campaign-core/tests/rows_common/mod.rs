//! Shared helpers of the `rows_*` tests (lane I-ROWS): the T162 family (a sha256-checked copy in tests/data/rows),
//! the F6 span cases of the frozen k3 tree (records inline in `span/records/` or read by byte offset from the
//! read-only handoff copy, sha256-checked against the case's `line_sha256`), and small result printers.
#![allow(dead_code)]

pub mod face;
#[path = "../k3_common/mod.rs"]
pub mod k3_common;
pub mod sym_t162;

use rustred_campaign_core::ibp_rows::{RowsRule, RowsVerdict};
use rustred_campaign_core::replay_base::family::Family;
use serde_json::Value;

#[allow(unused_imports)]
pub use k3_common::{HO, Oracle, oracle, sha256_hex};

/// sha256 of HO/t162-bank/family/T162-presented-P3.sym (k3 README: 91403065...56118).
pub const T162_FAMILY_SHA: &str =
    "91403065869aea5a9aee9504fa7623276a521653833bb88a395a356f83e56118";

pub fn data(name: &str) -> String {
    let p = format!("{}/tests/data/rows/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

pub fn t162_family() -> Family {
    let src = data("T162-presented-P3.sym");
    assert_eq!(
        sha256_hex(src.as_bytes()),
        T162_FAMILY_SHA,
        "T162 family copy changed"
    );
    Family::parse(&src).expect("T162 family")
}

/// The exact source line of an F6 case: the inline copy if any, else HO/<file> at byte_offset/byte_len; its sha256
/// must equal `line_sha256` either way. `None` (SKIP) if the handoff copy is not readable on this host.
pub fn f6_line(o: &Oracle, case: &Value) -> Option<String> {
    let bytes = match case["inline_record"].as_str() {
        Some(rel) => o.bytes(&format!("span/{rel}")),
        None => {
            let path = format!("{HO}/{}", case["file"].as_str().unwrap());
            let off = case["byte_offset"].as_u64().unwrap();
            let len = case["byte_len"].as_u64().unwrap() as usize;
            use std::io::{Read, Seek, SeekFrom};
            let Ok(mut f) = std::fs::File::open(&path) else {
                println!("SKIP: {path} not readable");
                return None;
            };
            f.seek(SeekFrom::Start(off)).unwrap();
            let mut b = vec![0u8; len];
            f.read_exact(&mut b).unwrap();
            b
        }
    };
    // the inline copy is the verbatim line incl. its newline: same sha256 as the source line
    assert_eq!(
        sha256_hex(&bytes),
        case["line_sha256"].as_str().unwrap(),
        "{}: source line sha256 differs",
        case["id"]
    );
    Some(String::from_utf8(bytes).unwrap())
}

/// Rule 0 of an F6 case line (export wrapper or bare record).
pub fn rule0(line: &str) -> RowsRule {
    let mut rs = RowsRule::from_json_line(line.trim_end(), 20).expect("rule line");
    assert!(!rs.is_empty(), "no rule in line");
    rs.remove(0)
}

/// One-line summary of a verdict for the test log (the report's result lines).
pub fn summary(id: &str, v: &RowsVerdict) -> String {
    let cells: Vec<String> = v
        .points
        .iter()
        .map(|p| {
            let c: Vec<String> = p
                .cells
                .iter()
                .map(|c| format!("{}/{}/{}", c.rank, c.row_columns, c.rows))
                .collect();
            format!("{:?}[{}]", p.status, c.join(" "))
        })
        .collect();
    format!(
        "{id}: {} points_passed={} sz=1e{:.0} rows={} t={:.2}s :: {}",
        v.verdict.kind(),
        v.points_passed,
        v.sz_bound_log10,
        v.cost.rows,
        v.cost.seconds,
        cells.join(" | ")
    )
}

/// Options with the family identity BOUND (re-audit A NEW-M1: the `replay` level needs a binding): the spec-digest
/// form of `TableBinding` on the family's own text.
pub fn bound(fam: &Family) -> rustred_campaign_core::ibp_rows::RowsOptions {
    rustred_campaign_core::ibp_rows::RowsOptions {
        table: Some(
            rustred_campaign_core::ibp_rows::TableBinding::from_family_digest(
                fam.squashed_digest(),
            ),
        ),
        ..rustred_campaign_core::ibp_rows::RowsOptions::default()
    }
}
