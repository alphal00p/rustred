//! Canonical digest and sealed-table verifier (child-job.md 3.2.3, 3.4.4; rulings R-b, R-c).

mod common;
use common::TempDir;
use rustred_campaign_contract::canon::{canonical_digest_file, canonical_line, is_volatile};
use rustred_campaign_contract::outcome::{EndKind, TableOutput};
use rustred_campaign_contract::seal::*;
use std::io::Write;
use std::path::Path;

const HEADER: &str = r#"{"type":"header","schema":"rustred.node.table.v2","family":"T162","zero_seconds":0.125,"node_options":{"seed_pose_keep":0.5}}"#;
const RULE: &str = r#"{"type":"rule","chart":"C","index":0,"target":"I(1,1,1)","rhs":[["I(1,1,0)","-1/2"],["I(0,1,1)","3"]],"seconds_search":0.701234,"lift_seconds":2.5,"witness_loci":["n1+n2-3"],"trace_rows":17}"#;
const END: &str = r#"{"type":"partial","charts":1,"rules":1,"rhs_terms":2,"seconds":3.25,"vmhwm_kb":123456}"#;

fn table_bytes(lines: &[&str]) -> Vec<u8> {
    let mut v = Vec::new();
    for l in lines {
        v.extend_from_slice(l.as_bytes());
        v.push(b'\n');
    }
    v
}

fn digest_of(lines: &[&str]) -> String {
    let mut d = rustred_campaign_contract::canon::CanonicalDigest::new();
    for l in lines {
        d.push_line(l.as_bytes()).unwrap();
    }
    d.hex()
}

/// Spans of the values of the volatile top-level members of a record (byte ranges), found by a plain search.
fn volatile_value_spans(line: &str) -> Vec<(usize, usize)> {
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    let ty = v["type"].as_str();
    let mut spans = Vec::new();
    for (k, val) in v.as_object().unwrap() {
        if is_volatile(ty, k) {
            let needle = format!("\"{k}\":{}", serde_json::to_string(val).unwrap());
            // the literal text as written (floats are written as in the constants above)
            let at = line.find(&format!("\"{k}\":")).unwrap();
            let start = at + k.len() + 3;
            let mut end = start;
            let b = line.as_bytes();
            while end < b.len() && b[end] != b',' && b[end] != b'}' {
                end += 1;
            }
            let _ = needle;
            spans.push((start, end));
        }
    }
    spans
}

#[test]
fn volatile_only_differences_give_equal_digests() {
    let a = [HEADER, RULE, END];
    let h2 = HEADER.replace("0.125", "9.875");
    let r2 = RULE.replace("0.701234", "12.000001").replace("2.5", "0.03");
    let e2 = END.replace("3.25", "1000.5").replace("123456", "7");
    let b = [h2.as_str(), r2.as_str(), e2.as_str()];
    assert_ne!(table_bytes(&a), table_bytes(&b));
    assert_eq!(digest_of(&a), digest_of(&b));
    // key order and whitespace do not matter either
    let r3 = r#"{ "witness_loci":["n1+n2-3"], "trace_rows":17, "type":"rule","chart":"C","index":0,"target":"I(1,1,1)","rhs":[["I(1,1,0)","-1/2"],["I(0,1,1)","3"]],"lift_seconds":2.5,"seconds_search":0.7}"#;
    assert_eq!(digest_of(&[HEADER, r3, END]), digest_of(&a));
}

#[test]
fn every_other_one_byte_change_changes_the_digest_or_is_rejected() {
    let base = digest_of(&[HEADER, RULE, END]);
    for (li, line) in [HEADER, RULE, END].iter().enumerate() {
        let vol = volatile_value_spans(line);
        let bytes = line.as_bytes();
        let mut changed = 0;
        for pos in 0..bytes.len() {
            if vol.iter().any(|&(s, e)| pos >= s && pos < e) {
                continue;
            }
            for rep in [b'x', b'7', b' '] {
                if bytes[pos] == rep {
                    continue;
                }
                let mut m = bytes.to_vec();
                m[pos] = rep;
                let mut lines: Vec<Vec<u8>> = [HEADER, RULE, END].iter().map(|s| s.as_bytes().to_vec()).collect();
                lines[li] = m.clone();
                let mut d = rustred_campaign_contract::canon::CanonicalDigest::new();
                let ok = lines.iter().all(|l| d.push_line(l).is_ok());
                if !ok {
                    continue; // rejected: not a valid record any more
                }
                // a space replacing a structural byte can be insignificant only where whitespace was legal AND the
                // byte was itself whitespace; there is none in these lines, so the digest must differ
                assert_ne!(d.hex(), base, "line {li} pos {pos} -> {:?} kept the digest", rep as char);
                changed += 1;
            }
        }
        assert!(changed > 0);
    }
}

#[test]
fn volatile_value_change_inside_a_key_name_is_not_volatile() {
    // changing a byte of a volatile KEY name makes it an ordinary key: digest changes
    let r = RULE.replace("\"lift_seconds\"", "\"lift_secondz\"");
    assert_ne!(digest_of(&[HEADER, &r, END]), digest_of(&[HEADER, RULE, END]));
}

fn write_table(dir: &Path, lines: &[&str]) -> (std::path::PathBuf, TableSeal) {
    let p = dir.join("F.table.jsonl");
    let mut w = RecordWriter::create(&p).unwrap();
    for l in &lines[..lines.len() - 1] {
        w.append(l.as_bytes()).unwrap();
    }
    let s = w.seal(lines[lines.len() - 1].as_bytes()).unwrap();
    (p, s)
}

fn out_of(seal: &TableSeal) -> TableOutput {
    let mut t = TableOutput { path: "out/F.table.jsonl".into(), ..Default::default() };
    seal.apply(&mut t);
    t
}

#[test]
fn writer_seal_matches_file_and_seal_file() {
    let d = TempDir::new("seal");
    let (p, s) = write_table(d.path(), &[HEADER, RULE, END]);
    let raw = std::fs::read(&p).unwrap();
    assert_eq!(raw, table_bytes(&[HEADER, RULE, END]));
    assert_eq!(s.bytes, raw.len() as u64);
    assert_eq!(s.blake3, rustred_campaign_contract::blake3_hex(&raw));
    assert_eq!((s.records, s.rules, s.end), (3, 1, EndKind::Partial));
    assert_eq!(s.canonical_blake3, digest_of(&[HEADER, RULE, END]));
    assert_eq!(canonical_digest_file(&p).unwrap(), (s.canonical_blake3.clone(), 3));
    assert_eq!(seal_file(&p).unwrap(), s);
    assert_eq!(verify_sealed(&p, &out_of(&s)), SealVerdict::Sealed);
}

#[test]
fn verifier_catches_tampering() {
    let d = TempDir::new("tamper");
    let (p, s) = write_table(d.path(), &[HEADER, RULE, END]);
    let good = std::fs::read(&p).unwrap();
    let t = out_of(&s);
    let check = |bytes: &[u8], t: &TableOutput| {
        std::fs::write(&p, bytes).unwrap();
        verify_sealed(&p, t)
    };
    // truncated table
    assert!(matches!(check(&good[..good.len() - 5], &t), SealVerdict::Unsealed(UnsealReason::SizeMismatch { .. })));
    // appended line
    let mut app = good.clone();
    app.extend_from_slice(b"{\"type\":\"rule\",\"index\":1}\n");
    assert!(matches!(check(&app, &t), SealVerdict::Unsealed(UnsealReason::SizeMismatch { .. })));
    // changed byte (same size)
    let mut ch = good.clone();
    let i = good.len() / 2;
    ch[i] = if ch[i] == b'1' { b'2' } else { b'1' };
    assert_eq!(check(&ch, &t), SealVerdict::Unsealed(UnsealReason::DigestMismatch));
    // digest mismatch in OUTCOME
    let mut t2 = t.clone();
    t2.blake3 = Some(common::D1.into());
    assert_eq!(check(&good, &t2), SealVerdict::Unsealed(UnsealReason::DigestMismatch));
    // missing end record, OUTCOME consistent with the file (bytes + digest of the file as it is)
    let no_end = table_bytes(&[HEADER, RULE]);
    let mut t3 = t.clone();
    t3.bytes = Some(no_end.len() as u64);
    t3.blake3 = Some(rustred_campaign_contract::blake3_hex(&no_end));
    t3.records = None;
    assert_eq!(check(&no_end, &t3), SealVerdict::Unsealed(UnsealReason::NoEndRecord(Some("rule".into()))));
    // torn last line, OUTCOME consistent with the bytes
    let torn = &good[..good.len() - 1];
    let mut t4 = t.clone();
    t4.bytes = Some(torn.len() as u64);
    t4.blake3 = Some(rustred_campaign_contract::blake3_hex(torn));
    assert_eq!(check(torn, &t4), SealVerdict::Unsealed(UnsealReason::TornLastLine));
    // OUTCOME says unsealed / incomplete
    let mut t5 = t.clone();
    t5.sealed = false;
    assert_eq!(check(&good, &t5), SealVerdict::Unsealed(UnsealReason::OutcomeNotSealed));
    let mut t6 = t.clone();
    t6.blake3 = None;
    assert_eq!(check(&good, &t6), SealVerdict::Unsealed(UnsealReason::OutcomeIncomplete));
    // end kind / record count disagree with OUTCOME
    let mut t7 = t.clone();
    t7.end = Some(EndKind::End);
    assert!(matches!(check(&good, &t7), SealVerdict::Unsealed(UnsealReason::EndMismatch { .. })));
    let mut t8 = t.clone();
    t8.records = Some(4);
    assert!(matches!(check(&good, &t8), SealVerdict::Unsealed(UnsealReason::RecordCountMismatch { .. })));
    // missing file
    std::fs::remove_file(&p).unwrap();
    assert!(matches!(verify_sealed(&p, &t), SealVerdict::Unsealed(UnsealReason::FileUnreadable(_))));
    // and the good bytes still verify
    assert_eq!(check(&good, &t), SealVerdict::Sealed);
}

#[test]
fn seal_refuses_non_end_record_and_invalid_records() {
    let d = TempDir::new("refuse");
    let p = d.path().join("t.jsonl");
    let mut w = RecordWriter::create(&p).unwrap();
    assert!(w.append(b"{\"a\":1,\"a\":2}").is_err());
    assert!(w.append(b"{\"a\":\n1}").is_err());
    assert_eq!(std::fs::metadata(&p).unwrap().len(), 0, "nothing written for rejected records");
    w.append(RULE.as_bytes()).unwrap();
    assert!(w.seal(RULE.as_bytes()).is_err());
}

#[test]
fn resume_from_prefix_continues_the_digests() {
    let d = TempDir::new("prefix");
    let a = d.path().join("a.jsonl");
    let mut w = RecordWriter::create(&a).unwrap();
    w.append(HEADER.as_bytes()).unwrap();
    w.append(RULE.as_bytes()).unwrap();
    let cp = w.commit().unwrap();
    // an uncommitted torn tail after the commit point
    w.raw_file().write_all(b"{\"type\":\"rule\",\"ind").unwrap();
    drop(w);
    assert_eq!(prefix_blake3(&a, cp.bytes).unwrap().as_deref(), Some(cp.prefix_blake3.as_str()));
    let b = d.path().join("b.jsonl");
    let w2 = RecordWriter::resume_from_prefix(&a, cp.bytes, &cp.prefix_blake3, &b).unwrap();
    let s = w2.seal(END.as_bytes()).unwrap();
    let c = d.path().join("c");
    std::fs::create_dir(&c).unwrap();
    let (_, s_direct) = write_table(&c, &[HEADER, RULE, END]);
    assert_eq!(s, s_direct, "resumed table == uninterrupted table");
    // wrong digest / not at a line boundary
    assert!(RecordWriter::resume_from_prefix(&a, cp.bytes, common::D0, &d.path().join("x")).is_err());
    assert!(RecordWriter::resume_from_prefix(&a, cp.bytes - 3, &cp.prefix_blake3, &d.path().join("y")).is_err());
}

#[test]
fn hundred_megabyte_line() {
    // a rule with a 110 MB witness locus (elision forbidden writes it in full, R-f): writer, sealing, verifier,
    // canonical digest all handle it; the verifier keeps only the bounded last line.
    let d = TempDir::new("big");
    let p = d.path().join("F.table.jsonl");
    let big = "n1+".repeat(110 * 1024 * 1024 / 3);
    let rule = format!(r#"{{"type":"rule","index":0,"seconds_search":0.5,"witness_loci":["{big}"]}}"#);
    drop(big);
    let mut w = RecordWriter::create(&p).unwrap();
    w.append(HEADER.as_bytes()).unwrap();
    let pos = w.append(rule.as_bytes()).unwrap();
    assert!(pos.bytes > 110_000_000);
    let s = w.seal(END.as_bytes()).unwrap();
    let t = out_of(&s);
    assert_eq!(verify_sealed(&p, &t), SealVerdict::Sealed);
    let rule2 = rule.replace("0.5", "7.25");
    assert_eq!(
        canonical_line(rule.as_bytes()).unwrap(),
        canonical_line(rule2.as_bytes()).unwrap(),
        "volatile change on a 110 MB line"
    );
    // the 110 MB line as the LAST line: not an end record, too long to be one
    let q = d.path().join("G.jsonl");
    std::fs::write(&q, format!("{HEADER}\n{rule}\n")).unwrap();
    let (n, dg) = file_blake3(&q).unwrap();
    let tq = TableOutput { path: "x".into(), bytes: Some(n), blake3: Some(dg), sealed: true, ..Default::default() };
    assert_eq!(verify_sealed(&q, &tq), SealVerdict::Unsealed(UnsealReason::LastLineTooLong));
}
