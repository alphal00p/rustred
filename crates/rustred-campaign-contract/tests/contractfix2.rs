//! Lane contractfix2 (2026-11-14): regression tests of the fixes of the re-audit's minor findings that have no
//! repro in the audit files (ac2-m9, the verified handle for ac2-M1) and extra edges of ac2-m3 / m4 / m6 / m7.

mod common;
use common::*;
use rustred_campaign_contract::canon::{self, CanonError};
use rustred_campaign_contract::classify::{Accept, Action, ROWS};
use rustred_campaign_contract::events::EventLine;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::resume::parse_cases_line;
use rustred_campaign_contract::seal::{
    self, RecordWriter, StreamDigest, TABLE_END_TYPES, UnsealReason, VerifiedHandle,
};
use rustred_campaign_contract::{OutcomeKind, blake3_hex, spec};

const HEADER: &str = r#"{"type":"header","family_file":"/abs/F.sym","family_blake3":"1111111111111111111111111111111111111111111111111111111111111111","zero_seconds":0.25}"#;

fn rule(i: u64) -> String {
    format!(
        r#"{{"type":"rule","chart":"11","index":{i},"case":"c{i}","target":"I({i})","rhs":[["I(0)","-1/2"]],"seconds_search":0.5}}"#
    )
}

fn run_with_table(tag: &str, lines: &[String]) -> (TempDir, RunLayout) {
    let d = TempDir::new(tag);
    let root = d.path().join("J.a1");
    std::fs::create_dir_all(root.join("out")).unwrap();
    let layout = RunLayout::new(&root);
    let mut w = RecordWriter::create(&layout.table()).unwrap();
    for l in lines {
        w.append(l.as_bytes()).unwrap();
    }
    w.seal(br#"{"type":"end","rules":2,"seconds":1.0}"#)
        .unwrap();
    (d, layout)
}

fn honest_lines() -> Vec<String> {
    vec![HEADER.to_string(), rule(0), rule(1)]
}

/// ac2-m9: row 3 rests on the child's own `per_case` claims, bound to no sealed record: it must never close the
/// item by itself (kind `no_rule`, accept nothing, action "confirm by coverage, else next rung").
#[test]
fn cf2_row3_never_closes_an_item_on_unverified_claims() {
    let r = ROWS.iter().find(|r| r.row == 3).unwrap();
    assert_eq!(
        (r.kind, r.accept, r.action),
        (
            OutcomeKind::NoRule,
            Accept::Nothing,
            Action::ConfirmCoverageElseNextRung
        )
    );
    // the only rows of kind `complete` are the verified ones: row 1, and (line 7) rows 30 / 31 of the diagnostic jobs
    // (sealed + their closed record lists checked), which accept diagnostic records only, never rules or relations
    let complete: Vec<u8> = ROWS
        .iter()
        .filter(|r| r.kind == OutcomeKind::Complete)
        .map(|r| r.row)
        .collect();
    // lane coreseed: row 37 (kind pole-regular-at, answers sealed and checked) gives diagnostic records only
    assert_eq!(complete, vec![1, 30, 31, 37]);
    for r in ROWS
        .iter()
        .filter(|r| r.row == 30 || r.row == 31 || r.row == 37)
    {
        assert_eq!(r.accept, Accept::DiagnosticRecords);
    }
}

/// The verified handle (ac2-M1 consumer API): the canonical lines come from the verified bytes, all or nothing.
#[test]
fn cf2_handle_returns_exactly_the_verified_lines() {
    let lines = honest_lines();
    let (_d, l) = run_with_table("cf2-h", &lines);
    let (v, h) =
        seal::verify_stream_handle(&l.root, "out/F.table.jsonl", &TABLE_END_TYPES).unwrap();
    assert_eq!(
        (h.dev(), h.ino(), h.bytes(), h.blake3()),
        (v.dev, v.ino, v.bytes, v.blake3.as_str())
    );
    let raw = h.read_verified().unwrap();
    assert_eq!(raw, std::fs::read(l.table()).unwrap());
    let got = h.canonical_lines().unwrap();
    assert_eq!(got.len() as u64, v.records);
    assert_eq!(got[1], canon::canonical_line(rule(0).as_bytes()).unwrap());
    // a re-opened handle on the unchanged file agrees
    let h2 = VerifiedHandle::reopen(&l.root, &v).unwrap();
    assert_eq!(h2.canonical_lines().unwrap(), got);
}

#[test]
fn cf2_handle_refuses_a_file_changed_in_place() {
    let (_d, l) = run_with_table("cf2-inplace", &honest_lines());
    let (v, h) =
        seal::verify_stream_handle(&l.root, "out/F.table.jsonl", &TABLE_END_TYPES).unwrap();
    let orig = std::fs::read(l.table()).unwrap();
    // same inode, same size, one byte changed (rules I(0) -> I(9))
    let mut b = orig.clone();
    let at = b.windows(4).position(|w| w == b"I(0)").unwrap();
    b[at + 2] = b'9';
    std::fs::OpenOptions::new()
        .write(true)
        .open(l.table())
        .and_then(|mut f| std::io::Write::write_all(&mut f, &b))
        .unwrap();
    assert_eq!(h.read_verified(), Err(UnsealReason::DigestMismatch));
    assert_eq!(h.canonical_lines(), Err(UnsealReason::DigestMismatch));
    // garbage appended: size differs
    let mut g = orig.clone();
    g.extend_from_slice(b"{\"type\":\"rule\",\"index\":99}\n");
    std::fs::OpenOptions::new()
        .write(true)
        .open(l.table())
        .and_then(|mut f| std::io::Write::write_all(&mut f, &g))
        .unwrap();
    assert!(matches!(
        h.read_verified(),
        Err(UnsealReason::SizeMismatch { .. })
    ));
    // restored: readable again (the figures, not the moment, decide)
    std::fs::write(l.table(), &orig).unwrap();
    assert_eq!(h.read_verified().unwrap(), orig);
    let _ = v;
}

#[test]
fn cf2_handle_keeps_the_verified_inode_when_the_path_is_replaced() {
    let (_d, l) = run_with_table("cf2-replace", &honest_lines());
    let (v, h) =
        seal::verify_stream_handle(&l.root, "out/F.table.jsonl", &TABLE_END_TYPES).unwrap();
    let orig = std::fs::read(l.table()).unwrap();
    // another perfectly sealed table renamed over the layout path
    let other = l.out_dir().join("other.jsonl");
    let mut w = RecordWriter::create(&other).unwrap();
    for i in 0..5 {
        w.append(rule(i).as_bytes()).unwrap();
    }
    w.seal(br#"{"type":"end","rules":5}"#).unwrap();
    std::fs::rename(&other, l.table()).unwrap();
    // the kept handle still reads the verified bytes
    assert_eq!(h.read_verified().unwrap(), orig);
    // a re-open refuses the replacement
    assert_eq!(
        VerifiedHandle::reopen(&l.root, &v).unwrap_err(),
        UnsealReason::InodeChanged
    );
}

/// ac2-m6: a header off line 1, or a second header, is refused by the verifier and by the engine-side digest.
#[test]
fn cf2_header_only_on_line_one() {
    for (tag, lines, at) in [
        ("cf2-h2", vec![rule(0), HEADER.to_string(), rule(1)], 2u64),
        (
            "cf2-hh",
            vec![HEADER.to_string(), rule(0), HEADER.to_string()],
            3,
        ),
    ] {
        let d = TempDir::new(tag);
        let root = d.path().join("J.a1");
        std::fs::create_dir_all(root.join("out")).unwrap();
        let mut b = Vec::new();
        for l in &lines {
            b.extend_from_slice(l.as_bytes());
            b.push(b'\n');
        }
        b.extend_from_slice(b"{\"type\":\"end\"}\n");
        std::fs::write(root.join("out/F.table.jsonl"), &b).unwrap();
        assert_eq!(
            seal::verify_stream(&root, "out/F.table.jsonl", &TABLE_END_TYPES).unwrap_err(),
            UnsealReason::HeaderNotFirst { line: at },
            "{tag}"
        );
        let mut sd = StreamDigest::new(&TABLE_END_TYPES);
        sd.feed(&b);
        assert!(sd.failed().is_some() && sd.seal().is_none(), "{tag}");
    }
}

/// ac2-m3 edges: a FIFO at the layout path is refused without blocking; a symlinked directory is refused.
#[test]
fn cf2_open_in_run_fifo_and_symlinked_dir() {
    let (_d, l) = run_with_table("cf2-fifo", &honest_lines());
    std::fs::remove_file(l.table()).unwrap();
    let st = std::process::Command::new("mkfifo")
        .arg(l.table())
        .status()
        .unwrap();
    assert!(st.success());
    let t0 = std::time::Instant::now();
    assert!(matches!(
        seal::open_in_run(&l.root, "out/F.table.jsonl"),
        Err(UnsealReason::NotRegularFile(_))
    ));
    assert!(t0.elapsed() < std::time::Duration::from_secs(5));
    // out/ a symlink to a real directory holding a sealed table
    let (_d2, other) = run_with_table("cf2-other", &honest_lines());
    std::fs::remove_file(l.table()).unwrap();
    std::fs::remove_dir(l.out_dir()).unwrap();
    std::os::unix::fs::symlink(other.out_dir(), l.out_dir()).unwrap();
    assert!(matches!(
        seal::verify_stream(&l.root, "out/F.table.jsonl", &TABLE_END_TYPES),
        Err(UnsealReason::NotRegularFile(_))
    ));
    // the run directory itself may be reached through a symlink (it is the supervisor's own path)
    let lnk = other.root.parent().unwrap().join("via-link");
    std::os::unix::fs::symlink(&other.root, &lnk).unwrap();
    assert!(seal::verify_stream(&lnk, "out/F.table.jsonl", &TABLE_END_TYPES).is_ok());
}

/// ac2-m4: every contract document reader refuses duplicate keys at any depth.
#[test]
fn cf2_duplicate_keys_refused_by_every_reader() {
    assert!(matches!(
        canon::check_document(b"{\n  \"a\": {\"b\": 1, \"b\": 2}\n}\n"),
        Err(CanonError::DuplicateKey(_))
    ));
    assert!(canon::check_document(b"{\n  \"a\": {\"b\": 1}\n}\n").is_ok());
    let ev = br#"{"v":1,"seq":1,"utc":"x","event":"x","x":1,"x":2}"#;
    assert!(EventLine::parse(ev).unwrap_err().contains("duplicate"));
    let case = r#"{"frame":"PRESENTED","fixed":[1],"fixed":[2]}"#;
    assert!(parse_cases_line(case).unwrap_err().contains("duplicate"));
    let s = sample_spec("/w/jobs/J.a1");
    let good = String::from_utf8(s.to_json_line()).unwrap();
    let bad = good.replacen('{', "{\"job_id\":\"other\",", 1);
    assert!(spec::JobSpec::from_json(bad.as_bytes()).is_err());
}

/// ac2-m7: the three domain-separated digest kinds and the raw file digest differ on one preimage; the raw file digest
/// stays the plain BLAKE3 (b3sum) of the bytes.
#[test]
fn cf2_digest_domains() {
    let b = b"{\"a\":1}\n";
    let raw = blake3_hex(b);
    assert_eq!(raw, blake3::hash(b).to_hex().to_string());
    let rec = canon::canonical_line_blake3(b"{\"a\":1}").unwrap();
    let mut d = canon::CanonicalDigest::new();
    d.push_line(b"{\"a\":1}").unwrap();
    let st = d.hex();
    let sp = spec::spec_blake3(b);
    let all = [&raw, &rec, &st, &sp];
    for i in 0..4 {
        for j in i + 1..4 {
            assert_ne!(all[i], all[j], "{i} {j}");
        }
    }
    assert_eq!(rec, canon::record_digest_of_canonical(b"{\"a\":1}"));
    let mut h = blake3::Hasher::new_derive_key(spec::CONTEXT_SPEC);
    h.update(b);
    assert_eq!(sp, h.finalize().to_hex().to_string());
}
