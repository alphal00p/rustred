//! Lane bigline (interfaces/import.md 19): reading is bounded in memory PER RECORD, independent of the line length.
//! Constructed tables on the toy sunset (tests/import5_common): a rule line far longer than the per-record budget
//! (budget 4 KiB, line ~1 MiB) goes through the whole import (index, classify, check: first match on the record's
//! key, statement lines, exact facts by streaming); malformed long lines give the same typed reader error as the
//! same line read whole; the streamed reader and the whole-line reader give byte-identical indexes and records.
//! The real tables (ignored tests, run by hand): identity on loaded tables, classify-only of the tables that did not
//! load within 2 GB.

mod import5_common;
use import5_common::*;

use std::path::{Path, PathBuf};

use import::index::open_or_build_with;
use import::lines::scan_table_with;
use rustred_campaign_core::import;
use rustred_campaign_core::record::ReadLimits;
use serde_json::{Value, json};

const TOP: [i32; 5] = [3, 3, 3, 0, 0];
const BUDGET: u64 = 4096;

/// Whole-line reading, no limit (the readers as before lane bigline).
fn whole() -> ReadLimits {
    ReadLimits::unlimited()
}
/// Every line streamed, no size refusal.
fn streamed_all() -> ReadLimits {
    ReadLimits {
        record_budget: 1 << 40,
        whole_line_max: 0,
    }
}
/// The small budget of these tests.
fn tiny() -> ReadLimits {
    ReadLimits::new(BUDGET)
}

/// Distinct rhs keys strictly below TOP: dots 1..2 on D1..D3, numerator powers 0..-9 on D4, D5.
fn below(i: usize) -> [i32; 5] {
    let d = i % 8;
    let x = (i / 8) % 10;
    let y = i / 80;
    [
        1 + (d & 1) as i32,
        1 + ((d >> 1) & 1) as i32,
        1 + ((d >> 2) & 1) as i32,
        -(x as i32),
        -(y as i32),
    ]
}

/// A coefficient of about `len` bytes equal to `base` (padded with `+n1-n1` pairs: exactly `base` as a polynomial).
fn coef(base: &str, len: usize) -> String {
    let mut s = format!("({base})");
    while s.len() + 6 <= len {
        s.push_str("+n1-n1");
    }
    s
}

/// A point rule at TOP with `terms` (key, coefficient) and optional witness loci, written with the given separators.
fn big_rule(index: i64, terms: &[(String, String)], witness: &[&str]) -> String {
    json!({"type":"rule","chart":MASK,"index":index,"fixed":TOP,"equations":[],"target":kt(&TOP),
        "rhs": terms.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":witness})
    .to_string()
}

/// ~1 MiB line: 240 terms of ~4000-byte coefficients (each term below the 4 KiB budget, the key texts together too);
/// term 5 vanishes at TOP (n0 = 3), term 7 has a denominator vanishing at TOP (n2 = 3), the others are 1.
fn many_terms() -> Vec<(String, String)> {
    (0..240)
        .map(|i| {
            let c = match i {
                5 => coef("n0-3", 4000),
                7 => format!("({})/(n2-3)", coef("1", 3988)),
                _ => coef("1", 4000),
            };
            (kt(&below(i)), c)
        })
        .collect()
}

fn reader_spec(sp: &mut Value, budget: u64, stream_above: Option<u64>) {
    sp["reader"] = json!({"record_budget_bytes": budget});
    if let Some(w) = stream_above {
        sp["reader"]["stream_lines_above"] = json!(w);
    }
}

fn files_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().to_string(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

/// The index of `table` built under `a` and under `b` (in fresh cache directories): byte-identical files.
fn index_identity(table: &Path, scratch: &Path, a: ReadLimits, b: ReadLimits) {
    let ca = scratch.join("ix-a");
    let cb = scratch.join("ix-b");
    let _ = std::fs::remove_dir_all(&ca);
    let _ = std::fs::remove_dir_all(&cb);
    let (ia, _) = open_or_build_with("t", table, Some(&ca), a).unwrap();
    let (ib, _) = open_or_build_with("t", table, Some(&cb), b).unwrap();
    // equal in everything but the random id of the build
    let mut mb = ib.meta.clone();
    mb.build_id = ia.meta.build_id.clone();
    assert_eq!(ia.meta, mb, "{}", table.display());
    let (fa, fb) = (files_of(&ia.dir), files_of(&ib.dir));
    assert_eq!(fa.len(), fb.len());
    for ((na, xa), (nb, xb)) in fa.iter().zip(&fb) {
        assert_eq!(na, nb);
        if na == "index.json" {
            continue; // = the metas compared above
        }
        assert!(xa == xb, "{}: index file {na} differs", table.display());
    }
    drop((ia, ib));
    let _ = std::fs::remove_dir_all(&ca);
    let _ = std::fs::remove_dir_all(&cb);
}

/// The direct scan of `table` (rhs keys and rhs digests) under `a` and `b`: identical records field by field.
fn scan_identity(table: &Path, a: ReadLimits, b: ReadLimits) {
    for want_rhs in [false, true] {
        let ra = scan_table_with("t", table, want_rhs, &|_, _| true, a).unwrap();
        let rb = scan_table_with("t", table, want_rhs, &|_, _| true, b).unwrap();
        assert_eq!(ra.file_sha256, rb.file_sha256);
        assert_eq!(ra.records.len(), rb.records.len());
        assert_eq!(format!("{:?}", ra.kinds), format!("{:?}", rb.kinds));
        assert_eq!(ra.markers, rb.markers);
        for (x, y) in ra.records.iter().zip(&rb.records) {
            assert_eq!(format!("{x:?}"), format!("{y:?}"), "{}", table.display());
        }
    }
}

#[test]
fn a_line_far_beyond_the_budget_goes_through_the_whole_import() {
    let d = scratch("bigline-whole");
    let terms = many_terms();
    let line = big_rule(0, &terms, &[]);
    assert!(line.len() > 900_000 && line.len() as u64 > 200 * BUDGET);
    let t = write_table(&d, "t.jsonl", std::slice::from_ref(&line));
    // the record's identity is the raw line's
    let mut sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    reader_spec(&mut sp, BUDGET, None);
    let (set, ev, r) = run(&sp, &d);
    let info = &set.rules[0][0];
    assert_eq!(info.bytes, line.len() as u64);
    assert_eq!(
        info.record_sha256,
        rustred_campaign_core::closure::sha256::sha256_tag(line.as_bytes())
            .trim_start_matches("sha256:")
    );
    // first match: TOP is served by the long record, its 240 terms are reached
    let top = row(&r, &TOP);
    assert_eq!(top.status, import::RowStatus::Served, "{top:?}");
    for i in 0..240 {
        row(&r, &below(i));
    }
    // statement lines name the record by its digest
    let out = d.join("st");
    import::write_statement(&set, &ev, &r, &out, None).unwrap();
    let st = std::fs::read_to_string(out.join("statement.jsonl")).unwrap();
    let rowv: Value = st
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["key"] == json!(TOP))
        .unwrap();
    assert!(rowv.to_string().contains(&info.record_sha256), "{rowv}");
    // the same load with lines read whole (default budget) gives the same records and the same check
    let mut sp2 = spec(&d, &t, &[&TOP], json!({}), &[]);
    sp2.as_object_mut().unwrap().remove("reader");
    let (set2, _, r2) = run(&sp2, &d);
    assert_eq!(set2.rules[0][0].record_sha256, info.record_sha256);
    assert_eq!(r2.rows.len(), r.rows.len());
    assert_eq!(format!("{:?}", r2.counts()), format!("{:?}", r.counts()));
    // classify-only under the tiny budget
    let c = import::sweep::classify_table(
        &t,
        &import::sweep::ClassifyOptions {
            scratch: Some(d.join("cl")),
            reader: Some(import::spec::ReaderSpec {
                record_budget_bytes: Some(BUDGET),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert_eq!(c.outcome, "loaded", "{c:?}");
    assert_eq!((c.records, c.dispatched), (1, 1));
    // indexes and direct scans: identical whichever reader read the long line
    index_identity(&t, &d, whole(), tiny());
    index_identity(&t, &d, whole(), streamed_all());
    scan_identity(&t, whole(), tiny());
    scan_identity(&t, whole(), streamed_all());
}

#[test]
fn exact_facts_of_a_long_record_are_streamed_and_equal_the_in_memory_ones() {
    let d = scratch("bigline-facts");
    let terms = many_terms();
    for witness in [vec![], vec!["n1-3"], vec!["n1-4"]] {
        let t = write_table(&d, "t.jsonl", &[big_rule(0, &terms, &witness)]);
        let mut sp = spec(&d, &t, &[&TOP], json!({}), &[]);
        reader_spec(&mut sp, BUDGET, None);
        let small = try_load(&sp, &d).unwrap();
        let mut sp2 = spec(&d, &t, &[&TOP], json!({}), &[]);
        sp2.as_object_mut().unwrap().remove("reader");
        let big = try_load(&sp2, &d).unwrap();
        assert!(small.rules[0][0].bytes > BUDGET);
        let a = import::exact_facts::fact_verdict_line(&small, 0, 0, &TOP).unwrap();
        let b = import::exact_facts::fact_verdict_line(&big, 0, 0, &TOP).unwrap();
        assert_eq!(a, b, "witness {witness:?}");
        assert_eq!(a["verdict"], json!("irregular"));
        assert_eq!(a["zero_terms"], json!(1));
        // the record cannot be read whole under the budget: typed refusal
        let e = import::exact_facts::record_line(&small, 0, 0).unwrap_err();
        assert!(e.to_string().contains("record too large"), "{e}");
    }
    // without the pole term: regular, or hidden on the witness locus
    let mut t2 = many_terms();
    t2[7].1 = coef("1", 4000);
    for (witness, v) in [(vec![], "regular"), (vec!["n1-3"], "hidden")] {
        let t = write_table(&d, "t2.jsonl", &[big_rule(0, &t2, &witness)]);
        let mut sp = spec(&d, &t, &[&TOP], json!({}), &[]);
        reader_spec(&mut sp, BUDGET, None);
        let small = try_load(&sp, &d).unwrap();
        let mut sp2 = spec(&d, &t, &[&TOP], json!({}), &[]);
        sp2.as_object_mut().unwrap().remove("reader");
        let big = try_load(&sp2, &d).unwrap();
        let a = import::exact_facts::fact_verdict_line(&small, 0, 0, &TOP).unwrap();
        let b = import::exact_facts::fact_verdict_line(&big, 0, 0, &TOP).unwrap();
        assert_eq!(a, b);
        assert_eq!(a["verdict"], json!(v));
    }
}

#[test]
fn a_coefficient_beyond_the_budget_is_a_typed_refusal_never_a_skip() {
    let d = scratch("bigline-coef");
    let terms = vec![
        (kt(&below(0)), coef("1", 1 << 20)),
        (kt(&below(1)), "1".to_string()),
    ];
    let t = write_table(&d, "t.jsonl", &[big_rule(0, &terms, &[])]);
    let mut sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    reader_spec(&mut sp, BUDGET, None);
    // the import loads (the coefficient is not needed to classify) and the record serves TOP
    let (set, _, r) = run(&sp, &d);
    assert_eq!(row(&r, &TOP).status, import::RowStatus::Served);
    // its fact cannot be evaluated in memory: typed, naming the record and the size
    let e = import::exact_facts::fact_verdict_line(&set, 0, 0, &TOP).unwrap_err();
    match &e {
        import::ImportError::Refused { reason, .. } => match reason.as_ref() {
            import::error::TableRefusal::RecordTooLarge {
                record,
                what,
                bytes,
                budget,
            } => {
                assert_eq!(*record, Some(0));
                assert!(what.contains("rhs term 0"), "{what}");
                assert!(*bytes > 1 << 20);
                assert_eq!(*budget, BUDGET);
            }
            o => panic!("{o:?}"),
        },
        o => panic!("{o}"),
    }
    assert_eq!(import::sweep::error_word(&e), "record_too_large");
    // a record whose part without rhs exceeds the budget refuses the TABLE, typed
    let wide = json!({"type":"rule","chart":MASK,"index":0,"fixed":TOP,"equations":[],"target":kt(&TOP),
        "rhs":[[kt(&below(0)), coef("1", 3 * BUDGET as usize)]],
        "exceptions":[[coef("n0-7", 2 * BUDGET as usize)]],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let t = write_table(&d, "wide.jsonl", &[wide]);
    let mut sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    reader_spec(&mut sp, BUDGET, None);
    let e = try_load(&sp, &d).unwrap_err();
    assert_eq!(import::sweep::error_word(&e), "record_too_large", "{e}");
    assert!(e.to_string().contains("\"exceptions\""), "{e}");
}

/// A malformed-line case: its name and the line built from a coefficient text.
type Case<'a> = (&'static str, Box<dyn Fn(&str) -> String + 'a>);

/// Malformed lines: the streamed reader gives the SAME error (variant and message, byte positions included) as the
/// whole-line reader, for the line as it is and for the line with its rhs grown far beyond the budget.
#[test]
fn malformed_long_lines_give_the_small_lines_errors() {
    let d = scratch("bigline-malformed");
    let pad = coef("1", 1 << 20);
    let k = kt(&below(0));
    let head = format!(
        r#"{{"type":"rule","chart":"{MASK}","index":0,"fixed":[3,3,3,0,0],"equations":[],"target":"{}""#,
        kt(&TOP)
    );
    let tail = r#","exceptions":[],"dispatch":"Partition","witness_loci":[]}"#;
    // (name, line builder from a coefficient text)
    let cases: Vec<Case> = vec![
        (
            "truncated",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}"#)),
        ),
        (
            "truncated after a backslash",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}\"#)),
        ),
        (
            "unbalanced",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}"]{tail}"#)),
        ),
        (
            "trailing bytes",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}"]]{tail} x"#)),
        ),
        (
            "duplicate key",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}"]],"rhs":[]{tail}"#)),
        ),
        (
            "missing comma in a term",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}" "{c}"]]{tail}"#)),
        ),
        (
            "term not an array",
            Box::new(|c: &str| format!(r#"{head},"rhs":["{k}","{c}"]{tail}"#)),
        ),
        (
            "key not a string",
            Box::new(|c: &str| format!(r#"{head},"rhs":[[7,"{c}"]]{tail}"#)),
        ),
        (
            "bad escape in a key",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["\x{k}","{c}"]]{tail}"#)),
        ),
        (
            "bad separator after a term",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}"]}}{tail}"#)),
        ),
        (
            "key grammar error and a bad field: the field first",
            Box::new(|c: &str| {
                format!(r#"{head},"rhs":[["{k}" "{c}"]],"exceptions":7,"dispatch":"Partition"}}"#)
            }),
        ),
        (
            "missing colon",
            Box::new(|c: &str| format!(r#"{head},"rhs" [["{k}","{c}"]]{tail}"#)),
        ),
        (
            "end inside a term",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}","#)),
        ),
        (
            "rhs not an array",
            Box::new(|c: &str| format!(r#"{head},"rhs":{{"a":"{c}"}}{tail}"#)),
        ),
        (
            "valid (control)",
            Box::new(|c: &str| format!(r#"{head},"rhs":[["{k}","{c}"]]{tail}"#)),
        ),
    ];
    for (name, mk) in &cases {
        for (size, c) in [("small", "1"), ("long", pad.as_str())] {
            let t = write_table(&d, "m.jsonl", &[mk(c)]);
            let a = scan_table_with("t", &t, true, &|_, _| true, whole()).map(|r| r.records.len());
            for lim in [tiny(), streamed_all()] {
                let b = scan_table_with("t", &t, true, &|_, _| true, lim).map(|r| r.records.len());
                let (sa, sb) = (format!("{a:?}"), format!("{b:?}"));
                assert_eq!(sa, sb, "{name} ({size}, {lim:?})");
                // the index reader too
                let ia = open_or_build_with("t", &t, None, whole()).map(|x| x.0.meta.rule_records);
                let ib = open_or_build_with("t", &t, None, lim).map(|x| x.0.meta.rule_records);
                assert_eq!(
                    format!("{ia:?}"),
                    format!("{ib:?}"),
                    "index: {name} ({size})"
                );
            }
            if *name == "valid (control)" {
                assert!(a.is_ok());
            } else {
                assert!(a.is_err(), "{name} ({size}) loads");
            }
        }
    }
}

/// Blank and whitespace-padded long lines, a long non-rule line counted by kind, an rhs-less long line.
#[test]
fn long_non_rule_and_blank_lines() {
    let d = scratch("bigline-other");
    let blank = " ".repeat(3 * BUDGET as usize);
    let zero = json!({"type":"zero_sectors","sectors":(0..2000).map(|i| format!("s{i}")).collect::<Vec<_>>()})
        .to_string();
    assert!(zero.len() as u64 > BUDGET && (zero.len() as u64) < 8 * BUDGET);
    let rule = format!(
        "   {}   ",
        big_rule(0, &[(kt(&below(0)), coef("1", 3000))], &[])
    );
    let sym = sym_record(1, &[Some(1), Some(2), Some(1), Some(0), Some(0)], "+").to_string();
    let t = write_table(&d, "o.jsonl", &[blank, rule, sym]);
    index_identity(&t, &d, whole(), streamed_all());
    scan_identity(&t, whole(), streamed_all());
    // a long counted line under the tiny budget is refused (its compact form is the whole line) ...
    let t2 = write_table(&d, "z.jsonl", &[zero]);
    let e = scan_table_with("t", &t2, true, &|_, _| true, tiny()).unwrap_err();
    assert_eq!(import::sweep::error_word(&e), "record_too_large");
    // ... and identical when only the path changes
    index_identity(&t2, &d, whole(), streamed_all());
    scan_identity(&t2, whole(), streamed_all());
}

/// The spec's `reader`: validated, absent from the digest when absent.
#[test]
fn reader_spec_validation_and_digest() {
    let d = scratch("bigline-spec");
    let t = write_table(&d, "t.jsonl", &[point_rule(0, &TOP, &[&below(0)])]);
    let sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    let s0 = import::TableSetSpec::from_json(&sp.to_string()).unwrap();
    assert_eq!(
        s0.read_limits(),
        ReadLimits {
            record_budget: import::spec::DEFAULT_RECORD_BUDGET_BYTES,
            whole_line_max: import::spec::DEFAULT_STREAM_LINES_ABOVE
        }
    );
    assert!(!serde_json::to_string(&s0).unwrap().contains("reader"));
    let mut sp1 = sp.clone();
    reader_spec(&mut sp1, 1000, Some(2000));
    assert!(import::TableSetSpec::from_json(&sp1.to_string()).is_err());
    let mut sp2 = sp.clone();
    reader_spec(&mut sp2, 0, None);
    assert!(import::TableSetSpec::from_json(&sp2.to_string()).is_err());
    let mut sp3 = sp.clone();
    sp3["reader"] = json!({"budget": 5});
    assert!(import::TableSetSpec::from_json(&sp3.to_string()).is_err());
    let mut sp4 = sp;
    reader_spec(&mut sp4, 1 << 20, Some(0));
    let s4 = import::TableSetSpec::from_json(&sp4.to_string()).unwrap();
    assert_eq!(
        s4.read_limits(),
        ReadLimits {
            record_budget: 1 << 20,
            whole_line_max: 0
        }
    );
    assert_ne!(s4.digest(), s0.digest());
}

// ---------------------------------------------------------------------------------------------------------------
// Real tables (ignored; run by hand, one process per table under a 2 GB scope)
// ---------------------------------------------------------------------------------------------------------------

fn env_list(var: &str) -> Vec<PathBuf> {
    let p = std::env::var(var).unwrap_or_else(|_| panic!("{var}: a file listing table paths"));
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

/// IDENTITY on real tables (list in RUSTRED_BIGLINE_IDENTITY): index files byte-identical between the whole-line
/// reader (unlimited) and the streamed reader (every line streamed), and between unlimited and the default limits;
/// direct scans (rhs keys, rhs digests) identical record by record.
#[test]
#[ignore]
fn identity_of_the_readers_on_real_tables() {
    let d = scratch("bigline-identity");
    for t in env_list("RUSTRED_BIGLINE_IDENTITY") {
        let t0 = std::time::Instant::now();
        index_identity(&t, &d, whole(), streamed_all());
        index_identity(
            &t,
            &d,
            whole(),
            ReadLimits {
                record_budget: import::spec::DEFAULT_RECORD_BUDGET_BYTES,
                whole_line_max: import::spec::DEFAULT_STREAM_LINES_ABOVE,
            },
        );
        scan_identity(&t, whole(), streamed_all());
        println!(
            "IDENTITY ok {} ({} bytes, {:.1} s)",
            t.display(),
            std::fs::metadata(&t).unwrap().len(),
            t0.elapsed().as_secs_f64()
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// CLASSIFY-ONLY of ONE real table (RUSTRED_BIGLINE_TABLE) under the default reader limits: the outcome as one JSON
/// line on stdout (`BIGLINE {...}`); peak RSS is measured by the caller (systemd scope).
#[test]
#[ignore]
fn classify_one_real_table() {
    let t = PathBuf::from(std::env::var("RUSTRED_BIGLINE_TABLE").expect("RUSTRED_BIGLINE_TABLE"));
    let d = scratch("bigline-classify");
    let c = import::sweep::classify_table(
        &t,
        &import::sweep::ClassifyOptions {
            scratch: Some(d.clone()),
            accept_fewer_rules: std::env::var("RUSTRED_BIGLINE_ACCEPT_FEWER").ok(),
            ..Default::default()
        },
    );
    println!("BIGLINE {}", serde_json::to_string(&c).unwrap());
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------------------------------------------------------
// Temporary directories of the import: unique per process even with the same pid (private pid namespaces)
// ---------------------------------------------------------------------------------------------------------------

/// The load spec of `table` (start key file `start` beside it), temporary index.
fn child_spec(table: &Path, start: &Path) -> import::TableSetSpec {
    let v = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy-sunset","frame_from":"A",
        "start":{"path": start.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":table.display().to_string()}]});
    import::TableSetSpec::from_json(&v.to_string()).unwrap()
}

/// What a load of a table must show: rule records, file digest, and the record digests in order.
fn load_fingerprint(set: &import::ImportedSet) -> String {
    let shas: Vec<&str> = set.rules[0]
        .iter()
        .map(|r| r.record_sha256.as_str())
        .collect();
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(shas.join(",").as_bytes());
    format!(
        "records={} file={} recs={}",
        shas.len(),
        set.table_digests
            .values()
            .next()
            .cloned()
            .unwrap_or_default(),
        h.finish_hex()
    )
}

/// CHILD of [`two_namespaced_imports_share_one_tmpdir`] (run by it, never alone): loads RUSTRED_BIGLINE_CHILD_TABLE
/// with a temporary index under TMPDIR and prints its pid and the load's fingerprint.
#[test]
#[ignore]
fn bigline_child_import() {
    let t = PathBuf::from(std::env::var("RUSTRED_BIGLINE_CHILD_TABLE").expect("child only"));
    let start = t.with_file_name("start.jsonl");
    let base = t.parent().unwrap().to_path_buf();
    let set = import::load(&child_spec(&t, &start), &base).unwrap();
    println!(
        "CHILD pid={} {}",
        std::process::id(),
        load_fingerprint(&set)
    );
}

/// Defect found by lane repairflow2: the temporary index directory was `TMPDIR/rr-import-index-<pid>-<n>`; inside
/// private pid namespaces every run is pid 1, so two concurrent runs with the same TMPDIR shared it (one failed; one
/// could have read the other's index). Two processes, BOTH pid 1 of their own namespace, same TMPDIR, importing
/// DIFFERENT tables at the same time, three rounds: both loads are correct (fingerprint of a load in this process)
/// and nothing is left in TMPDIR. Needs `unshare` (user + pid namespaces), as the whole suite under LIGHT_PIDNS=1.
#[test]
fn two_namespaced_imports_share_one_tmpdir() {
    let d = scratch("bigline-pidns");
    let shared = d.join("tmp");
    std::fs::create_dir_all(&shared).unwrap();
    let mut expect = vec![];
    let mut tables = vec![];
    for (name, n, pad) in [("a", 3000usize, 2000usize), ("b", 2200, 3000)] {
        let td = d.join(name);
        std::fs::create_dir_all(&td).unwrap();
        let rules: Vec<String> = (0..n)
            .map(|i| {
                json!({"type":"rule","chart":MASK,"index":i,"fixed":below(i + 1),"equations":[],
                    "target":kt(&below(i + 1)),"rhs":[[kt(&below(i)), coef(&format!("{name}{i}"), pad)]],
                    "exceptions":[],"dispatch":"Partition","witness_loci":[]})
                .to_string()
            })
            .collect();
        let t = write_table(&td, "t.jsonl", &rules);
        std::fs::write(td.join("start.jsonl"), "").unwrap();
        let set = import::load(&child_spec(&t, &td.join("start.jsonl")), &td).unwrap();
        expect.push(load_fingerprint(&set));
        tables.push(t);
    }
    assert_ne!(expect[0], expect[1]);
    let exe = std::env::current_exe().unwrap();
    for round in 0..3 {
        let kids: Vec<_> = tables
            .iter()
            .map(|t| {
                std::process::Command::new("unshare")
                    .args(["-Urpf", "--mount-proc", "--"])
                    .arg(&exe)
                    .args([
                        "--ignored",
                        "--exact",
                        "bigline_child_import",
                        "--nocapture",
                        "--test-threads",
                        "1",
                    ])
                    .env("RUSTRED_BIGLINE_CHILD_TABLE", t)
                    .env("TMPDIR", &shared)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("unshare (user + pid namespaces) is needed by this test")
            })
            .collect();
        for (i, k) in kids.into_iter().enumerate() {
            let out = k.wait_with_output().unwrap();
            let so = String::from_utf8_lossy(&out.stdout);
            assert!(
                out.status.success(),
                "round {round} child {i}: {so}\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
            // (libtest prints "test bigline_child_import ... " on the same line)
            let line = so
                .lines()
                .find_map(|l| l.find("CHILD ").map(|p| &l[p..]))
                .unwrap_or_else(|| panic!("round {round} child {i}: {so}"));
            assert_eq!(line, format!("CHILD pid=1 {}", expect[i]), "round {round}");
        }
        let left: Vec<_> = std::fs::read_dir(&shared).unwrap().collect();
        assert!(left.is_empty(), "round {round}: {left:?} left in TMPDIR");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The exclusive directory: distinct names for the same pid, and an index file of another build is refused.
#[test]
fn unique_dirs_and_index_files_bound_to_their_build() {
    let d = scratch("bigline-unique");
    let a = import::index::unique_dir(&d, "x").unwrap();
    let b = import::index::unique_dir(&d, "x").unwrap();
    assert_ne!(a.path(), b.path());
    assert!(a.path().is_dir() && b.path().is_dir());
    // the guard removes the directory it created, a released one stays
    let (pa, pb) = (a.path().to_path_buf(), b.path().to_path_buf());
    drop(a);
    assert!(!pa.exists());
    let kept = b.release();
    assert_eq!(kept, pb);
    assert!(kept.is_dir());
    // two tables indexed in one cache; then the data files of one are swapped into the other's index
    let ta = write_table(&d, "ta.jsonl", &[point_rule(0, &TOP, &[&below(0)])]);
    let tb = write_table(
        &d,
        "tb.jsonl",
        &[
            point_rule(0, &TOP, &[&below(1)]),
            point_rule(1, &below(1), &[&below(2)]),
        ],
    );
    let cache = d.join("cache");
    let (ia, _) = open_or_build_with("A", &ta, Some(&cache), whole()).unwrap();
    let (ib, _) = open_or_build_with("B", &tb, Some(&cache), whole()).unwrap();
    for f in ["domains.jsonl", "bodies.jsonl"] {
        std::fs::copy(ib.dir.join(f), ia.dir.join(f)).unwrap();
    }
    let e = ia
        .scan_domains("A", &|_, _| true, &mut |_| Ok(()))
        .unwrap_err();
    assert!(matches!(e, import::ImportError::Digest { .. }), "{e}");
    let e = ia
        .scan_bodies(&|_| true, &mut |_, _, _| Ok(()))
        .unwrap_err();
    assert!(matches!(e, import::ImportError::Digest { .. }), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// L18 invariant: the import removes recursively ONLY a directory it created by its exclusive mkdir. A pre-existing
/// cache entry holding anything but index files is never removed (the load fails instead); a stale entry of index
/// files only is replaced file by file; a whole index directory of ANOTHER table of the same node (same frame, family,
/// order) put in place of this table's is refused and rebuilt (never loaded).
#[test]
fn removals_are_scoped_and_foreign_indexes_are_rebuilt() {
    let d = scratch("bigline-scoped");
    let ta = write_table(&d, "ta.jsonl", &[point_rule(0, &TOP, &[&below(0)])]);
    let tb = write_table(
        &d,
        "tb.jsonl",
        &[
            point_rule(0, &TOP, &[&below(1)]),
            point_rule(1, &below(1), &[&below(2)]),
        ],
    );
    let cache = d.join("cache");
    let entry = import::index::index_dir_for(&cache, &ta).unwrap();
    std::fs::create_dir_all(&entry).unwrap();
    std::fs::write(entry.join("index.json"), "{}").unwrap();
    std::fs::write(entry.join("precious.txt"), "not the import's").unwrap();
    let e = open_or_build_with("A", &ta, Some(&cache), whole()).unwrap_err();
    assert!(matches!(e, import::ImportError::Io { .. }), "{e}");
    assert!(
        entry.join("precious.txt").is_file(),
        "a foreign file was removed"
    );
    // nothing of the failed build is left beside the entry
    let left: Vec<String> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|x| x.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
    std::fs::remove_file(entry.join("precious.txt")).unwrap();
    // a stale entry of index files only: replaced
    let (ia, oa) = open_or_build_with("A", &ta, Some(&cache), whole()).unwrap();
    assert_eq!(oa.how, "built");
    assert_eq!(ia.meta.rule_records, 1);
    drop(ia);
    // the index of B (same node) copied over A's entry: refused by the table check, rebuilt, A's records loaded
    let (ib, _) = open_or_build_with("B", &tb, Some(&cache), whole()).unwrap();
    for f in import::index::INDEX_FILES {
        std::fs::copy(ib.dir.join(f), entry.join(f)).unwrap();
    }
    let mut sp = spec(&d, &ta, &[&TOP], json!({}), &[]);
    sp["reader"] = json!({"spot_check_records": 1000});
    let s = import::TableSetSpec::from_json(&sp.to_string()).unwrap();
    let set = import::load_with(
        &s,
        &d,
        &import::LoadOptions {
            index_dir: Some(cache.clone()),
        },
    )
    .unwrap();
    assert_eq!(set.rules[0].len(), 1);
    assert_eq!(set.report.sources[0].indexes[0].how, "built");
    let _ = std::fs::remove_dir_all(&d);
}

/// The spot check: a table changed IN PLACE where the sampled digest does not look, its mtime restored, keeps a
/// persistent index that passes size + mtime + sample; re-hashing records from the table bytes refuses it and the
/// load rebuilds the index (and loads the table as it is now).
#[test]
fn spot_check_refuses_an_index_the_table_no_longer_matches() {
    let d = scratch("bigline-spot");
    let rules: Vec<String> = (0..3000)
        .map(|i| {
            json!({"type":"rule","chart":MASK,"index":i,"fixed":below(i + 1),"equations":[],
                "target":kt(&below(i + 1)),"rhs":[[kt(&below(i)), coef(&format!("x{i}"), 3000)]],
                "exceptions":[],"dispatch":"Partition","witness_loci":[]})
            .to_string()
        })
        .collect();
    let t = write_table(&d, "t.jsonl", &rules);
    let cache = d.join("cache");
    let mut sp = spec(&d, &t, &[], json!({}), &[]);
    sp["reader"] = json!({"spot_check_records": 100000});
    let s = import::TableSetSpec::from_json(&sp.to_string()).unwrap();
    let lo = import::LoadOptions {
        index_dir: Some(cache.clone()),
    };
    let set1 = import::load_with(&s, &d, &lo).unwrap();
    assert_eq!(set1.report.sources[0].indexes[0].how, "built");
    // a byte outside every sampled block, inside a coefficient: n1 -> n2 (same length, still valid)
    let mut bytes = std::fs::read(&t).unwrap();
    let size = bytes.len() as u64;
    let blk = 1u64 << 16;
    let sampled = |o: u64| {
        (0..64u64).any(|i| {
            let s0 = i * (size - blk) / 63;
            o >= s0 && o < s0 + blk
        })
    };
    let pos = (0..bytes.len() - 2)
        .find(|&i| &bytes[i..i + 2] == b"n1" && !sampled(i as u64) && !sampled(i as u64 + 1))
        .expect("an unsampled position");
    bytes[pos + 1] = b'2';
    let mtime = std::fs::metadata(&t).unwrap().modified().unwrap();
    std::fs::write(&t, &bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&t)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let set2 = import::load_with(&s, &d, &lo).unwrap();
    assert_eq!(
        set2.report.sources[0].indexes[0].how,
        "rebuilt_after_failed_check"
    );
    let changed: Vec<usize> = (0..3000)
        .filter(|&r| set1.rules[0][r].record_sha256 != set2.rules[0][r].record_sha256)
        .collect();
    assert_eq!(changed.len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}
