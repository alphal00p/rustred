//! Lane import5, item 5: PERMANENT READER TESTS against the two reader defects the coordinator found on its own side
//! (prefix readers that lost records): (a) records whose keys are in another order (T211 min, rule index 2894: the
//! table has 943 rule records); (b) records written WITH SPACES (`{"type": "rule", ...}`, Python json.dumps): the
//! minimised T065 (every rule line spaced) and T062 (header / zero_sectors / chart / residual lines spaced). The import's
//! reader is a tokenising scanner (white space skipped before every token, the kind taken from the parsed `type`
//! member), so it must be immune: proved on the real files (read-only; `#[ignore]`d, paths overridable by env) and on
//! small constructed lines (not ignored). (c) The LOAD SWEEP: every table path of the ledger loaded by the import in
//! classify-only mode (no closure): the list of refused tables and why.
//!
//! `record_sha256` is the sha256 of the RAW line: a compact re-encoding of a spaced file has other digests BY DESIGN
//! (one record, one digest: the bytes as stored); everything else must be equal.
//!
//! Env: RUSTRED_T211_MIN, RUSTRED_SPACED_TABLES (comma-separated), RUSTRED_IMPORT_SCRATCH (scratch dir),
//! RUSTRED_LEDGER_TABLES (file: one table path per line), RUSTRED_SWEEP_MAX_BYTES (default 5e9), RUSTRED_SWEEP_THREADS.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use import::lines::{RawRecord, scan_table_each};
use rustred_campaign_core::import;
use rustred_campaign_core::record::{ReadOptions, RecordReader, RhsMode, TableLine};
use serde_json::{Value, json};

const MIN: &str = "/common/build-cache/vhirschi/p_integral_dev/simplify-tables/min";

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("import5-readers-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The `type` member of a line, read by serde (a full JSON parse; other members ignored without a tree).
fn serde_kind(line: &[u8]) -> String {
    #[derive(serde::Deserialize)]
    struct K {
        #[serde(rename = "type")]
        t: String,
    }
    serde_json::from_slice::<K>(line).unwrap().t
}

fn scan_all(path: &Path) -> (import::lines::RawTable, Vec<RawRecord>) {
    let mut recs = vec![];
    let raw = scan_table_each("t", path, true, &|_, _| true, &mut |r| {
        recs.push(r);
        Ok(())
    })
    .unwrap();
    (raw, recs)
}

// ---------------------------------------------------------------------------------------------------- constructed

/// Keys in another order and spaces anywhere: the same record, field by field.
#[test]
fn spaced_and_reordered_rule_lines_read_identically() {
    let compact = br#"{"type":"rule","chart":"110","index":3,"fixed":[null,1,null],"equations":["n0=n2"],"target":"I(n0,1,n2)","rhs":[["I(n0-1,1,n2)","n0/(n2-1)"],["I(n0,1,n2-1)","1"]],"exceptions":[["-1+n0"]],"dispatch":"Partition","witness_loci":[]}"#;
    let spaced = br#"{"type": "rule", "chart": "110", "index": 3, "fixed": [null, 1, null], "equations": ["n0=n2"], "target": "I(n0,1,n2)", "rhs": [["I(n0-1,1,n2)", "n0/(n2-1)"], ["I(n0,1,n2-1)", "1"]], "exceptions": [["-1+n0"]], "dispatch": "Partition", "witness_loci": []}"#;
    let reordered = br#"{ "index" : 3 , "rhs":[ ["I(n0-1,1,n2)","n0/(n2-1)"] , ["I(n0,1,n2-1)","1"] ],"chart":"110","fixed":[null,1,null],"witness_loci":[],"equations":["n0=n2"],"exceptions":[["-1+n0"]],"target":"I(n0,1,n2)","dispatch":"Partition","type" : "rule" }"#;
    let read = |l: &[u8]| match RecordReader::new(
        l,
        ReadOptions {
            rhs: RhsMode::KeysOnly,
        },
    )
    .next()
    {
        Some(Ok(TableLine::Rule(r))) => *r,
        other => panic!("not a rule: {other:?}"),
    };
    let a = read(compact);
    assert_eq!(read(spaced), a);
    assert_eq!(read(reordered), a);
    assert_eq!(a.rhs_keys.as_ref().unwrap().len(), 2);
    // a spaced non-rule line is classified by its parsed kind
    let res = br#"{"type": "residual", "chart": "110", "key": "I(1,1,0)"}"#;
    assert!(matches!(
        RecordReader::new(&res[..], ReadOptions::default()).next(),
        Some(Ok(TableLine::Other { kind })) if kind == "residual"
    ));
}

/// A whole constructed table written spaced and key-reordered loads with the same records, kinds and markers as its
/// compact form (the same check the real-file tests run).
#[test]
fn constructed_spaced_table_equals_its_compact_form() {
    let d = scratch("constructed");
    let hdr = json!({"type":"header","schema":"table_dump.v1","denominators":3,"mask":"110","order_name":"legacy"});
    let lines = vec![
        hdr.clone(),
        json!({"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}),
        json!({"type":"rule","chart":"110","index":0,"fixed":[2,1,0],"equations":[],"target":"I(2,1,0)",
               "rhs":[["I(1,1,0)","d-4"]],"exceptions":[],"dispatch":"Partition","witness_loci":[]}),
        json!({"type":"residual","chart":"110","key":"I(1,1,0)"}),
        json!({"type":"composite_assembly_complete","complete":true,"rules":1,"residuals":1,"expected_LP":1}),
    ];
    let spaced = d.join("spaced.jsonl");
    let mut f = std::fs::File::create(&spaced).unwrap();
    for v in &lines {
        // json.dumps style + the type member LAST for the rule line
        let mut t = serde_json::to_string_pretty(v).unwrap().replace('\n', " ");
        if v["type"] == "rule" {
            let mut o = v.as_object().unwrap().clone();
            o.remove("type");
            let mut s = serde_json::to_string(&o).unwrap();
            s.pop();
            t = format!("{s}, \"type\": \"rule\"}}");
        }
        writeln!(f, "{t}").unwrap();
    }
    drop(f);
    let compact = compact_copy(&spaced, &d.join("compact.jsonl"));
    compare_tables(&spaced, &compact, 3);
}

// ---------------------------------------------------------------------------------------------------- helpers

/// The compact re-encoding of a table: every line parsed with serde_json and re-serialised compactly (keys sorted).
fn compact_copy(src: &Path, dst: &Path) -> PathBuf {
    let rd = BufReader::with_capacity(1 << 20, std::fs::File::open(src).unwrap());
    let mut w = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(dst).unwrap());
    for l in rd.split(b'\n') {
        let l = l.unwrap();
        if l.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let v: Value = serde_json::from_slice(&l).unwrap();
        w.write_all(serde_json::to_string(&v).unwrap().as_bytes())
            .unwrap();
        w.write_all(b"\n").unwrap();
    }
    w.flush().unwrap();
    dst.to_path_buf()
}

/// Both files scanned by the import's line scanner: equal kinds, header, markers, rule records field by field (rhs
/// keys included), sha256 different where the bytes differ. Returns (rule records, residual records).
fn compare_tables(a: &Path, b: &Path, arity: usize) -> (usize, usize) {
    let (ra, va) = scan_all(a);
    let (rb, vb) = scan_all(b);
    assert_eq!(ra.kinds, rb.kinds, "line kinds");
    assert_eq!(ra.header.raw, rb.header.raw, "header");
    assert_eq!(ra.header.denominators, Some(arity));
    assert_eq!(ra.markers, rb.markers, "markers / residual keys");
    assert_eq!((ra.rule_lines, ra.sym_lines), (rb.rule_lines, rb.sym_lines));
    assert_eq!(va.len(), vb.len(), "rule records");
    let mut differ = 0usize;
    for (x, y) in va.iter().zip(&vb) {
        assert_eq!(x.ordinal, y.ordinal);
        assert_eq!(x.line_no, y.line_no);
        assert_eq!(x.rec, y.rec, "record at line {}", x.line_no);
        assert_eq!(x.domain_bounds, y.domain_bounds);
        assert_eq!(x.origin, y.origin);
        assert_eq!(x.origin_index, y.origin_index);
        if x.bytes != y.bytes {
            assert_ne!(
                x.sha256, y.sha256,
                "different bytes, different record_sha256"
            );
            differ += 1;
        }
    }
    println!(
        "{}: {} rule records ({} re-encoded with other bytes: record_sha256 differs by design), {} residual records, kinds {:?}",
        a.display(),
        va.len(),
        differ,
        ra.markers.residuals.len(),
        ra.kinds
    );
    (va.len(), ra.markers.residuals.len())
}

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

// ---------------------------------------------------------------------------------------------------- real files

/// (a) T211 minimised: 943 rule records, among them index 2894 (keys in another order); the scanner's count equals an
/// independent serde count of the `type` member of every line.
#[test]
#[ignore = "real table (2.5 GB, read-only): RUSTRED_T211_MIN"]
fn t211_min_has_943_rule_records() {
    let p = env_path("RUSTRED_T211_MIN", &format!("{MIN}/T211/F.table.jsonl"));
    if !p.is_file() {
        println!("SKIP: {} missing", p.display());
        return;
    }
    let mut serde_rules = 0usize;
    let rd = BufReader::with_capacity(1 << 22, std::fs::File::open(&p).unwrap());
    for l in rd.split(b'\n') {
        let l = l.unwrap();
        if !l.iter().all(u8::is_ascii_whitespace) && serde_kind(&l) == "rule" {
            serde_rules += 1;
        }
    }
    let mut indices = vec![];
    let raw = scan_table_each("T211", &p, false, &|_, _| true, &mut |r| {
        indices.push(r.rec.index);
        Ok(())
    })
    .unwrap();
    println!(
        "T211 min: scanner {} rule records, serde {serde_rules}; kinds {:?}",
        indices.len(),
        raw.kinds
    );
    assert_eq!(indices.len(), 943);
    assert_eq!(serde_rules, 943);
    assert_eq!(raw.rule_lines, 943);
    assert!(indices.contains(&Some(2894)));
}

/// (b) Spaced tables: T065 minimised (every rule line spaced) and T062 minimised (header / zero_sectors / chart /
/// residual lines spaced), or the list in RUSTRED_SPACED_TABLES: equal to their compact re-encoding.
#[test]
#[ignore = "real tables (read-only; compact copies in the scratch dir): RUSTRED_SPACED_TABLES"]
fn spaced_real_tables_equal_their_compact_reencoding() {
    let list: Vec<PathBuf> = match std::env::var("RUSTRED_SPACED_TABLES") {
        Ok(s) => s
            .split(',')
            .filter(|x| !x.is_empty())
            .map(PathBuf::from)
            .collect(),
        Err(_) => vec![
            PathBuf::from(format!("{MIN}/T065/F.table.jsonl")),
            PathBuf::from(format!("{MIN}/T062/F.table.jsonl")),
        ],
    };
    let d = scratch("spaced");
    for p in &list {
        if !p.is_file() {
            println!("SKIP: {} missing", p.display());
            continue;
        }
        // the file is really spaced somewhere
        let rd = BufReader::new(std::fs::File::open(p).unwrap());
        let spaced = rd
            .split(b'\n')
            .filter_map(Result::ok)
            .filter(|l| l.starts_with(b"{\"type\": "))
            .count();
        assert!(spaced > 0, "{}: no spaced line", p.display());
        let c = compact_copy(p, &d.join("compact.jsonl"));
        let arity = scan_all(p).0.header.denominators.unwrap();
        let (rules, residuals) = compare_tables(p, &c, arity);
        println!(
            "{}: {spaced} spaced lines; {rules} rules; {residuals} residuals",
            p.display()
        );
        assert!(rules > 0);
        std::fs::remove_file(&c).unwrap();
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) The LOAD SWEEP: every table path of the ledger (RUSTRED_LEDGER_TABLES: one path per line) loaded by the import
/// in classify-only mode; files above RUSTRED_SWEEP_MAX_BYTES (default 5e9) skipped and listed. A table refused only
/// for a marker's `rules` count is loaded a second time with `marker_rules.accept_fewer` to report whether that is
/// its only obstacle. Report: sweep.jsonl + sweep-summary.json in the scratch dir.
#[test]
#[ignore = "the ledger's table paths (read-only; ~160 GB): RUSTRED_LEDGER_TABLES"]
fn load_sweep_of_the_ledger_tables() {
    let list = env_path(
        "RUSTRED_LEDGER_TABLES",
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/import5/ledger_tables.txt",
    );
    let Ok(text) = std::fs::read_to_string(&list) else {
        println!("SKIP: {} missing", list.display());
        return;
    };
    let max: u64 = std::env::var("RUSTRED_SWEEP_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map_or(5_000_000_000, |x| x as u64);
    let threads: usize = std::env::var("RUSTRED_SWEEP_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let paths: Vec<PathBuf> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect();
    let d = scratch("sweep");
    let out = Mutex::new(std::io::BufWriter::new(
        std::fs::File::create(d.join("sweep.jsonl")).unwrap(),
    ));
    let results: Mutex<Vec<Value>> = Mutex::new(vec![]);
    // small files in parallel, the big ones (> 1 GB) one at a time afterwards (memory)
    let (small, big): (Vec<&PathBuf>, Vec<&PathBuf>) = paths
        .iter()
        .partition(|p| std::fs::metadata(p).map_or(0, |m| m.len()) <= 1_000_000_000);
    let classify_one = |p: &PathBuf| {
        let bytes = std::fs::metadata(p).map_or(0, |m| m.len());
        let v = if bytes > max {
            json!({"path": p.display().to_string(), "bytes": bytes, "outcome": "skipped_size"})
        } else {
            let tmp = d.join(format!("t-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
            let opts = import::sweep::ClassifyOptions {
                scratch: Some(tmp.clone()),
                index_dir: Some(tmp.join("idx")),
                accept_fewer_rules: None,
            };
            let c = import::sweep::classify_table(p, &opts);
            let mut v = serde_json::to_value(&c).unwrap();
            if c.reason.as_deref().is_some_and(|r| r.ends_with(":rules")) {
                let c2 = import::sweep::classify_table(
                    p,
                    &import::sweep::ClassifyOptions {
                        accept_fewer_rules: Some("sweep: minimised table".into()),
                        ..opts.clone()
                    },
                );
                v["with_accept_fewer"] =
                    json!({"outcome": c2.outcome, "reason": c2.reason, "detail": c2.detail});
            }
            let _ = std::fs::remove_dir_all(&tmp);
            v
        };
        writeln!(out.lock().unwrap(), "{v}").unwrap();
        results.lock().unwrap().push(v);
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(p) = small.get(i) else { break };
                    classify_one(p);
                }
            });
        }
    });
    for p in &big {
        classify_one(p);
    }
    out.lock().unwrap().flush().unwrap();
    let res = results.into_inner().unwrap();
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut refused = vec![];
    for v in &res {
        let o = v["outcome"].as_str().unwrap_or("").to_string();
        let key = match v["reason"].as_str() {
            Some(r) => format!("{o}:{r}"),
            None => o.clone(),
        };
        *by.entry(key).or_default() += 1;
        if let Some(k) = v["kinds"].as_object() {
            for (kk, n) in k {
                *kinds.entry(kk.clone()).or_default() += n.as_u64().unwrap_or(0) as usize;
            }
        }
        if o != "loaded" {
            refused.push(json!({"path": v["path"], "outcome": o, "reason": v["reason"], "detail": v["detail"],
                "bytes": v["bytes"], "with_accept_fewer": v.get("with_accept_fewer")}));
        }
    }
    let summary = json!({"tables": res.len(), "by_outcome": by, "line_kinds_of_loaded": kinds, "not_loaded": refused});
    std::fs::write(
        d.join("sweep-summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("SWEEP {} tables: {by:?}", res.len());
    println!("SWEEP report: {}", d.display());
    assert_eq!(res.len(), paths.len());
}
