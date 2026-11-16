//! Lane import5 (interfaces/import.md 18.4): `sym` records (one-term symmetry rules of `rr_node generate --symmetry`)
//! as a record class of the import, verified by the coordinator's seven steps against OUR derived group; `sym_zero`
//! refused. Constructed tables on the toy sunset (tests/import5_common).

mod import5_common;
use import5_common::*;

use import::error::TableRefusal;
use import::{ImportError, Level, RowStatus, Server};
use rustred_campaign_core::import;
use serde_json::json;

/// The orientation of the swap at a point key: (key, image) with the image strictly LOWER under the toy order.
fn lower_pair() -> ([i32; 5], [i32; 5]) {
    let cmp = comparator();
    let a = [2, 1, 1, 0, 0];
    let b = [1, 2, 1, 0, 0];
    if cmp.lower(&b, &a) { (a, b) } else { (b, a) }
}

fn fixed_of(k: &[i32]) -> Vec<Option<i32>> {
    k.iter().map(|&x| Some(x)).collect()
}

/// A correct sym record serves its key: basis `sym_record_verified`, level = min(per-key, image's): the image is a
/// declared terminal -> per-key; the image served by an imported rule (no per-key proof) -> imported.
#[test]
fn verified_sym_record_serves_at_the_lower_of_its_own_and_its_image_level() {
    let d = scratch("sym-ok");
    let (key, image) = lower_pair();
    let low = [1, 1, 1, 0, 0];
    // (a) image is a declared terminal
    let t = write_table(
        &d,
        "a.jsonl",
        &[sym_record(0, &fixed_of(&key), "+").to_string()],
    );
    let sp = spec(
        &d,
        &t,
        &[&key],
        json!({}),
        &[json!({"id":"T","kind":"terminals","keys":[image]})],
    );
    let (set, _ev, r) = run(&sp, &d);
    let rep = &set.report.sources[0];
    let s = rep.sym.as_ref().expect("sym report");
    assert_eq!((s.records, s.verified), (1, 1), "{s:?}");
    let x = row(&r, &key);
    assert_eq!(x.status, RowStatus::Served);
    assert!(matches!(x.server, Some(Server::Rule { .. })));
    assert_eq!(x.basis, Some(import::check::BASIS_SYM_RECORD_VERIFIED));
    assert_eq!(x.evidence, Some(Level::PerKey));
    assert_eq!(x.image.as_deref(), Some(&image[..]));
    assert_eq!(row(&r, &image).status, RowStatus::Terminal);
    assert!(
        !r.blocking()
            .iter()
            .any(|b| b.starts_with("descent") || b.contains("open")),
        "{:?}",
        r.blocking()
    );
    let info = set.rule(0, 0);
    assert!(
        info.sym
            .as_ref()
            .is_some_and(|s| s.verified && s.element.is_some())
    );
    // (b) image served by a rule at `imported` (no per-key proof): the sym row is imported too
    let t = write_table(
        &d,
        "b.jsonl",
        &[
            sym_record(0, &fixed_of(&key), "+").to_string(),
            point_rule(1, &image, &[&low]),
        ],
    );
    let sp = spec(
        &d,
        &t,
        &[&key],
        json!({}),
        &[json!({"id":"T","kind":"terminals","keys":[low]})],
    );
    let (_set, _ev, r) = run(&sp, &d);
    let x = row(&r, &key);
    assert_eq!(x.basis, Some(import::check::BASIS_SYM_RECORD_VERIFIED));
    assert_eq!(row(&r, &image).evidence, Some(Level::Imported));
    assert_eq!(
        x.evidence,
        Some(Level::Imported),
        "a sym row never above its image"
    );
}

/// First match: a sym record shares the index sequence with rule records; a rule BEFORE it wins, a rule AFTER it is
/// never reached for its keys.
#[test]
fn sym_records_take_their_place_in_first_match_order() {
    let d = scratch("sym-order");
    let (key, image) = lower_pair();
    let low = [1, 1, 1, 0, 0];
    let t = write_table(
        &d,
        "t.jsonl",
        &[
            sym_record(0, &fixed_of(&key), "+").to_string(),
            point_rule(1, &key, &[&low]),
        ],
    );
    let sp = spec(
        &d,
        &t,
        &[&key],
        json!({}),
        &[json!({"id":"T","kind":"terminals","keys":[image, low]})],
    );
    let (set, _ev, r) = run(&sp, &d);
    assert!(matches!(
        row(&r, &key).server,
        Some(Server::Rule { record: 0, .. })
    ));
    assert_eq!(
        set.rule(0, 1).ordinal,
        1,
        "one ordinal sequence over rule and sym records"
    );
    let t = write_table(
        &d,
        "u.jsonl",
        &[
            point_rule(0, &key, &[&low]),
            sym_record(1, &fixed_of(&key), "+").to_string(),
        ],
    );
    let sp = spec(
        &d,
        &t,
        &[&key],
        json!({}),
        &[json!({"id":"T","kind":"terminals","keys":[image, low]})],
    );
    let (_set, _ev, r) = run(&sp, &d);
    let x = row(&r, &key);
    assert!(matches!(x.server, Some(Server::Rule { record: 0, .. })));
    assert_eq!(x.basis, Some("witness_loci"));
}

/// A symbolic case (one free line slot): verified once for the whole case, serves every key of it whose image is
/// lower; where the image is NOT lower (step 7 at the key) the row has no evidence (`sym_record_not_lower`) and the
/// closure counts the descent violation.
#[test]
fn symbolic_sym_record_and_step_seven_at_the_key() {
    let d = scratch("sym-symbolic");
    let cmp = comparator();
    let (key, image) = lower_pair();
    // the case I(n0, x1, 1, 0, 0) with x1 = the second slot of `key`: image I(x1, n0, 1, 0, 0)
    let fixed = vec![None, Some(key[1]), Some(1), Some(0), Some(0)];
    let t = write_table(&d, "t.jsonl", &[sym_record(0, &fixed, "+").to_string()]);
    let sp = spec(
        &d,
        &t,
        &[&key],
        json!({}),
        &[json!({"id":"T","kind":"terminals","keys":[image]})],
    );
    let (set, _ev, r) = run(&sp, &d);
    assert_eq!(set.report.sources[0].sym.as_ref().unwrap().verified, 1);
    assert_eq!(
        row(&r, &key).basis,
        Some(import::check::BASIS_SYM_RECORD_VERIFIED)
    );
    // the reverse orientation: the record's image at the start key is HIGHER
    let fixed_rev = vec![None, Some(image[1]), Some(1), Some(0), Some(0)];
    assert!(!cmp.lower(&key, &image));
    let t = write_table(
        &d,
        "rev.jsonl",
        &[sym_record(0, &fixed_rev, "+").to_string()],
    );
    let sp = spec(
        &d,
        &t,
        &[&image],
        json!({}),
        &[json!({"id":"T","kind":"terminals","keys":[key]})],
    );
    let (set, _ev, r) = run(&sp, &d);
    assert_eq!(
        set.report.sources[0].sym.as_ref().unwrap().verified,
        1,
        "steps 1-6 hold"
    );
    let x = row(&r, &image);
    assert_eq!(x.basis, Some(import::check::BASIS_SYM_NOT_LOWER));
    assert_eq!(x.evidence, None);
    assert!(
        r.blocking()
            .iter()
            .any(|b| b.starts_with("descent_violations:")),
        "{:?}",
        r.blocking()
    );
}

/// An UNVERIFIABLE sym record never serves: every key of its case is blocked with `sym_record_unverified: <step>`,
/// and dispatch does not fall through to a later rule. Wrong sign; a wrong matrix (identity with the swap's action);
/// an action outside the derived group (D1 -> D3 kept as a line map with a numerator target): all unverified.
#[test]
fn unverifiable_sym_records_block_their_keys() {
    let d = scratch("sym-bad");
    let (key, image) = lower_pair();
    let low = [1, 1, 1, 0, 0];
    let mut wrong_matrix = sym_record(0, &fixed_of(&key), "+");
    wrong_matrix["matrix"] = json!([1, 0, 0, 0, 1, 0, 0, 0, 1]);
    let mut wrong_image = sym_record(0, &fixed_of(&key), "+");
    wrong_image["image"] = json!(kt(&key));
    for (tag, rec, step) in [
        ("sign", sym_record(0, &fixed_of(&key), "-"), "step 5"),
        ("matrix", wrong_matrix, "steps 2-3"),
        ("image", wrong_image, "step 6"),
    ] {
        let t = write_table(
            &d,
            &format!("{tag}.jsonl"),
            &[rec.to_string(), point_rule(1, &key, &[&low])],
        );
        let sp = spec(
            &d,
            &t,
            &[&key],
            json!({}),
            &[json!({"id":"T","kind":"terminals","keys":[image, low]})],
        );
        let (set, _ev, r) = run(&sp, &d);
        let s = set.report.sources[0].sym.as_ref().unwrap();
        assert_eq!((s.records, s.verified), (1, 0), "{tag}: {s:?}");
        assert!(
            s.unverified[0].1.starts_with(step),
            "{tag}: {:?}",
            s.unverified
        );
        let x = row(&r, &key);
        assert_eq!(x.status, RowStatus::Open, "{tag}");
        assert_eq!(x.open_status.as_deref(), Some("blocked"), "{tag}");
        assert!(
            x.open_reason
                .as_deref()
                .is_some_and(|m| m.starts_with(import::check::REASON_SYM_UNVERIFIED)),
            "{tag}: {:?}",
            x.open_reason
        );
        assert_eq!(
            x.open_rule,
            Some((0, 0)),
            "{tag}: no fall-through to the later rule"
        );
    }
}

/// Refused TABLES (typed): a sym record with an `exceptions` member; sign `key`; a `sym_zero` record.
#[test]
fn malformed_sym_and_sym_zero_refuse_the_table() {
    let d = scratch("sym-refuse");
    let (key, _image) = lower_pair();
    let mut with_exc = sym_record(0, &fixed_of(&key), "+");
    with_exc["exceptions"] = json!([["n0-1"]]);
    let key_sign = sym_record(0, &fixed_of(&key), "key");
    let mut zero = sym_record(0, &fixed_of(&key), "-");
    zero["type"] = json!("sym_zero");
    zero["image"] = zero["target"].clone();
    for (tag, rec, want) in [
        ("exc", with_exc, "sym_malformed"),
        ("keysign", key_sign, "sym_malformed"),
        ("zero", zero, "sym_zero"),
    ] {
        let t = write_table(
            &d,
            &format!("{tag}.jsonl"),
            &[rec.to_string(), point_rule(1, &[1, 1, 1, 0, 0], &[])],
        );
        let sp = spec(&d, &t, &[&key], json!({}), &[]);
        match try_load(&sp, &d) {
            Err(ImportError::Refused { reason, line, .. }) => {
                assert_eq!(reason.word(), want, "{tag}: {reason}");
                assert_eq!(line, 3, "{tag}");
                if tag == "zero" {
                    assert_eq!(*reason, TableRefusal::SymZero);
                }
            }
            other => panic!("{tag}: expected a refusal, got {:?}", other.map(|_| ())),
        }
    }
}

/// The sym record's `record_sha256` is the sha256 of its raw line; the statement names it as a sym server.
#[test]
fn sym_record_identity_and_statement_row() {
    let d = scratch("sym-statement");
    let (key, image) = lower_pair();
    let line = sym_record(0, &fixed_of(&key), "+").to_string();
    let t = write_table(&d, "t.jsonl", std::slice::from_ref(&line));
    let sp = spec(
        &d,
        &t,
        &[&key],
        json!({}),
        &[json!({"id":"T","kind":"terminals","keys":[image]})],
    );
    let (set, ev, r) = run(&sp, &d);
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(line.as_bytes());
    assert_eq!(set.rule(0, 0).record_sha256, h.finish_hex());
    let out = d.join("statement");
    import::write_statement(&set, &ev, &r, &out, None).unwrap();
    let st = std::fs::read_to_string(out.join("statement.jsonl")).unwrap();
    let v: serde_json::Value = st
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["key"] == json!(key))
        .unwrap();
    assert_eq!(v["server"]["sym"], json!(true));
    assert_eq!(v["server"]["image"], json!(image));
    assert_eq!(v["basis"], json!("sym_record_verified"));
}
