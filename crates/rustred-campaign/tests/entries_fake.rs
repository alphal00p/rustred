//! Lane gencheck round 5 H0 GAP-2 + GAP-3 (emission contract section 16): RELATION ENTRIES from emitted records, with
//! `rr_fake_child` standing in for the checker and the engine's descent test. GAP-2: the d = 4 acceptance against ONE KEY
//! of (a) a stored relation entry (the one the engine would serve) or (b) the table rule that serves the key; worse =
//! refused, nothing written. GAP-3: the writer (a relation root with MANIFEST precedence over_table; each entry carries its
//! emitted record) and the entry-check that refuses an entry without its emitted record, its regularity verdict, its
//! descent verdict, or with a pole order that is not its relation's.
use rustred_campaign::emission::entries::{
    ENTRY_SCHEMA, WriteArgs, entry_check, entry_digest, node_dir, write_entries,
};
use rustred_campaign::emission::gate::{GateKind, gate_beside};
use rustred_campaign_core::emission::body::{attach_derivation, strip_derivation};
use rustred_campaign_core::emission::side_dir;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");
const IDENT: &str = "toy-node-identity-0123456789abcdef";

fn tmp(tag: &str) -> PathBuf {
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let d = PathBuf::from(base).join(format!("entries-fake-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn header() -> Value {
    json!({"type":"header","schema":"table_dump.v1","family":"TOY","mask":"110","denominators":3,
           "node_order":{"name":"id","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"}})
}

fn point(i: usize, key: [i64; 3], coef: &str) -> Value {
    let t = |k: [i64; 3]| format!("I({},{},{})", k[0], k[1], k[2]);
    json!({"type":"rule","chart":"110","index":i,"case":format!("Coordinate(P{i})"),"fixed":key,"equations":[],
           "target":t(key),"rhs":[[t([key[0]-1, key[1], key[2]]), coef]],"exceptions":[],"dispatch":"Partition","witness_loci":[]})
}

/// The emitted table of `recs` (the in-flow gate with the fake checker and the fake descent test).
fn emitted(dir: &Path, recs: &[Value]) -> PathBuf {
    let mut plain = format!("{}\n", header());
    for r in recs {
        plain.push_str(&format!("{r}\n"));
    }
    let es: rustred_campaign_core::emission::Settings = serde_json::from_value(
        json!({"checker": FAKE, "recover_rows": false, "descent_node": FAKE}),
    )
    .unwrap();
    let em = dir.join("new.emitted.table.jsonl");
    gate_beside(&es, &plain, &em, GateKind::Point, &[], true).unwrap();
    em
}

fn old_entry(key: [i64; 3], coef: &str, level: &str, pole: i64) -> String {
    let t = |k: [i64; 3]| format!("I({},{},{})", k[0], k[1], k[2]);
    let body = json!({"type":"relation","key":key,"target":t(key),"verdict":"SOLVED","all_lower":true,"proof":{"ok":true,"kind":"exact-weights"},
                      "rhs":[[t([key[0]-1, key[1], key[2]]), coef, "top"]]});
    json!({"schema":ENTRY_SCHEMA,"node":"TOY","node_identity":IDENT,"key_frame":"table","key":key,
           "entry_digest":entry_digest(&body).unwrap(),"body":body,"evidence":{"level":level},
           "diagnostics":{"pole_order_at":{"4":pole}}})
    .to_string()
}

fn args(dir: &Path, emitted: &Path) -> WriteArgs {
    WriteArgs {
        emitted: emitted.to_path_buf(),
        node: "TOY".into(),
        node_identity: IDENT.into(),
        out: dir.join("root"),
        revision: "test".into(),
        work: dir.join("work"),
        ..Default::default()
    }
}

#[test]
fn gap2a_more_regular_than_the_stored_entry_is_written_over_table_and_entry_check_accepts() {
    let d = tmp("a");
    let em = emitted(&d, &[point(0, [1, 2, 0], "1")]);
    // two stored entries at the key: the engine serves the per-key one (pole 1); an "imported" one ranks lower
    let olds = d.join("old.jsonl");
    std::fs::write(
        &olds,
        format!(
            "{}\n{}\n",
            old_entry([1, 2, 0], "3/(d-4)", "imported", 1),
            old_entry([1, 2, 0], "1/(d-4)", "per-key", 1)
        ),
    )
    .unwrap();
    let mut a = args(&d, &em);
    a.old_entries = vec![olds];
    let (s, ok) = write_entries(&a).unwrap();
    eprintln!("{s:#}");
    assert!(ok, "{s:#}");
    assert_eq!(s["regularity"][0]["old_pole"], 1);
    assert_eq!(s["regularity"][0]["new_pole"], 0);
    assert_eq!(s["regularity"][0]["old"]["entries_at_key"], 2);
    let dir = node_dir(&d.join("root"), "TOY", IDENT);
    let m: Value =
        serde_json::from_slice(&std::fs::read(dir.join("MANIFEST.json")).unwrap()).unwrap();
    assert_eq!(m["entries"]["precedence"], "over_table");
    assert_eq!(m["entries"]["over_table_keys"], json!([[1, 2, 0]]));
    let (c, ok) = entry_check(&dir).unwrap();
    assert!(ok, "{c:#}");
    // the index carries the engine's rank (per-key evidence 2, -pole 0, -1 term): a missing rank would be (0, 0, 0) and lose to
    // a later per-key layer at the key
    let ix: Value = serde_json::from_str(
        std::fs::read_to_string(dir.join("index.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ix["rank"], json!([2, 0, -1]));
    // the entry names the per-key old entry (1/(d-4)), not the imported one
    let e: Value = serde_json::from_str(
        std::fs::read_to_string(dir.join("entries-emitted.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let rec = e["body"]["emitted_record"].as_str().unwrap();
    let der: Value = serde_json::from_str(&strip_derivation(rec).unwrap().1.unwrap()).unwrap();
    assert!(der["replaces"]["regularity_sha256"].is_string());
    assert_eq!(der["descent"]["verdict"], "LOWER");
    assert_eq!(e["diagnostics"]["pole_order_at"]["4"], 0);
    assert_eq!(
        e["body"]["rhs"][0][2], "sub",
        "I(0,2,0) leaves line 0: a sub-sector term"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// RULE 21 (user ruling 2026-11-19): an entry WORSE at d = 4 than the old object at its key is WRITTEN, the regression
/// REPORTED (old pole 0 -> new pole 1); a key without an old object is written too ("not compared").
/// N2 of the gate R-C read (rule 21): a d = 4 measurement that FAILS (here the stored entry's coefficient cannot be parsed)
/// is reported "not measured" with its error in the verdict side file, never an abort; the record is written and
/// entry-check accepts it.
#[test]
fn a_failed_measurement_is_reported_not_measured_and_never_aborts() {
    let d = tmp("notmeasured");
    let em = emitted(&d, &[point(0, [1, 2, 0], "1")]);
    let olds = d.join("old.jsonl");
    std::fs::write(
        &olds,
        format!("{}\n", old_entry([1, 2, 0], "1/(", "per-key", 0)),
    )
    .unwrap();
    let mut a = args(&d, &em);
    a.old_entries = vec![olds];
    let (s, ok) = write_entries(&a).unwrap();
    assert!(ok, "{s:#}");
    assert!(s["regularity"][0]["not_worse"].is_null(), "{s:#}");
    let sha = s["regularity"][0]["regularity_sha256"].as_str().unwrap();
    let vf = side_dir(&em).join(rustred_campaign_core::emission::side_rel(
        rustred_campaign_core::emission::AREA_REGULARITY,
        sha,
    ));
    let v: Value = serde_json::from_slice(&std::fs::read(&vf).unwrap()).unwrap();
    assert_eq!(v["not_measured"], true, "{v:#}");
    assert!(v["error"].as_str().is_some_and(|e| !e.is_empty()), "{v:#}");
    assert!(v["not_worse"].is_null(), "{v:#}");
    assert!(
        entry_check(&node_dir(&d.join("root"), "TOY", IDENT))
            .unwrap()
            .1
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn gap2_worse_than_the_old_object_is_written_and_reported_and_a_new_key_is_written() {
    let d = tmp("worse");
    let em = emitted(&d, &[point(0, [1, 2, 0], "1/(d-4)")]);
    let olds = d.join("old.jsonl");
    std::fs::write(
        &olds,
        format!("{}\n", old_entry([1, 2, 0], "1", "per-key", 0)),
    )
    .unwrap();
    let mut a = args(&d, &em);
    a.old_entries = vec![olds];
    let (s, ok) = write_entries(&a).unwrap();
    assert!(ok, "{s:#}");
    assert_eq!(
        (
            s["regularity"][0]["old_pole"].as_u64(),
            s["regularity"][0]["new_pole"].as_u64()
        ),
        (Some(0), Some(1)),
        "{s:#}"
    );
    assert_eq!(s["regularity"][0]["not_worse"], false);
    assert!(
        entry_check(&node_dir(&d.join("root"), "TOY", IDENT))
            .unwrap()
            .1
    );
    // no old object at the key: written, "not compared"
    let d2 = tmp("newkey");
    let em2 = emitted(&d2, &[point(0, [1, 2, 0], "1")]);
    let (s, ok) = write_entries(&args(&d2, &em2)).unwrap();
    assert!(ok, "{s:#}");
    assert!(
        s["regularity"][0]["regularity"]
            .as_str()
            .unwrap()
            .contains("not compared"),
        "{s:#}"
    );
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}

#[test]
fn gap2b_one_key_of_the_first_match_table_rule() {
    let d = tmp("b");
    // the old table: a parametric rule with a d = 4 pole everywhere
    let old = d.join("old.table.jsonl");
    let mut r = json!({"type":"rule","chart":"110","index":0,"case":"Coordinate(A)","fixed":[null,null,null],"equations":[],
        "target":"I(n0,n1,n2)","rhs":[["I(n0-1,n1,n2)","1/(d-4)"]],"exceptions":[],"dispatch":"Partition","witness_loci":[]});
    std::fs::write(
        &old,
        format!(
            "{}\n{r}\n{}\n",
            header(),
            json!({"type":"end","charts":1,"rules":1})
        ),
    )
    .unwrap();
    let em = emitted(&d, &[point(0, [1, 2, 0], "1")]);
    let mut a = args(&d, &em);
    a.old_table = Some(old.clone());
    let (s, ok) = write_entries(&a).unwrap();
    assert!(ok, "{s:#}");
    assert_eq!(s["regularity"][0]["old"]["kind"], "table rule at one key");
    assert_eq!(s["regularity"][0]["old"]["rule"], 0);
    // the same rule made regular: the new relation with a pole is WRITTEN there, its regression reported (rule 21)
    r["rhs"] = json!([["I(n0-1,n1,n2)", "1"]]);
    std::fs::write(
        &old,
        format!(
            "{}\n{r}\n{}\n",
            header(),
            json!({"type":"end","charts":1,"rules":1})
        ),
    )
    .unwrap();
    let d2 = tmp("b2");
    let em2 = emitted(&d2, &[point(0, [1, 2, 0], "1/(d-4)")]);
    let mut a2 = args(&d2, &em2);
    a2.old_table = Some(old);
    let (s, ok) = write_entries(&a2).unwrap();
    assert!(ok && s["regularity"][0]["not_worse"] == false, "{s:#}");
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}

#[test]
fn gap3_entry_check_refuses_entries_without_their_members() {
    let d = tmp("check");
    let em = emitted(&d, &[point(0, [1, 2, 0], "1")]);
    let olds = d.join("old.jsonl");
    std::fs::write(
        &olds,
        format!("{}\n", old_entry([1, 2, 0], "1/(d-4)", "per-key", 1)),
    )
    .unwrap();
    let mut a = args(&d, &em);
    a.old_entries = vec![olds];
    assert!(write_entries(&a).unwrap().1);
    let dir = node_dir(&d.join("root"), "TOY", IDENT);
    let good = std::fs::read_to_string(dir.join("entries-emitted.jsonl")).unwrap();
    let e0: Value = serde_json::from_str(good.lines().next().unwrap()).unwrap();
    // a variant: edit the entry, recompute its digest (an honest-looking entry), check it in a copy of the directory
    let check = |tag: &str, edit: &dyn Fn(&mut Value)| -> Value {
        let mut e = e0.clone();
        edit(&mut e);
        e["entry_digest"] = json!(entry_digest(&e["body"]).unwrap());
        let c = d.join(format!("variant-{tag}"));
        std::fs::create_dir_all(&c).unwrap();
        std::fs::copy(dir.join("MANIFEST.json"), c.join("MANIFEST.json")).unwrap();
        let f = c.join("entries-emitted.jsonl");
        std::fs::write(&f, format!("{e}\n")).unwrap();
        rustred_campaign::emission::carry_side(
            &side_dir(&dir.join("entries-emitted.jsonl")),
            &side_dir(&f),
        )
        .unwrap();
        let (s, ok) = entry_check(&c).unwrap();
        assert!(!ok, "{tag}: {s:#}");
        s
    };
    let s = check("no-record", &|e| {
        e["body"].as_object_mut().unwrap().remove("emitted_record");
    });
    assert!(
        s.to_string().contains("without its emitted record"),
        "{s:#}"
    );
    // rule 21: an entry WITHOUT its regularity verdict is accepted (the verdict is a reported number)
    {
        let mut e = e0.clone();
        with_der(&mut e, &|dv| {
            dv.as_object_mut().unwrap().remove("replaces");
        });
        e["entry_digest"] = json!(entry_digest(&e["body"]).unwrap());
        let c = d.join("variant-no-regularity");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::copy(dir.join("MANIFEST.json"), c.join("MANIFEST.json")).unwrap();
        let f = c.join("entries-emitted.jsonl");
        std::fs::write(&f, format!("{e}\n")).unwrap();
        rustred_campaign::emission::carry_side(
            &side_dir(&dir.join("entries-emitted.jsonl")),
            &side_dir(&f),
        )
        .unwrap();
        let (s, ok) = entry_check(&c).unwrap();
        assert!(ok, "{s:#}");
    }
    let s = check("no-descent", &|e| {
        with_der(e, &|dv| dv["descent"] = Value::Null)
    });
    assert!(s.to_string().contains("descent"), "{s:#}");
    let s = check("pole", &|e| {
        e["diagnostics"]["pole_order_at"]["4"] = json!(1);
    });
    assert!(s.to_string().contains("pole order"), "{s:#}");
    let s = check("rhs", &|e| {
        e["body"]["rhs"][0][1] = json!("2");
    });
    assert!(s.to_string().contains("rhs"), "{s:#}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Edit the derivation of an entry's emitted record.
fn with_der(e: &mut Value, f: &dyn Fn(&mut Value)) {
    let rec = e["body"]["emitted_record"].as_str().unwrap().to_string();
    let (b, dv) = strip_derivation(&rec).unwrap();
    let mut dv: Value = serde_json::from_str(&dv.unwrap()).unwrap();
    f(&mut dv);
    e["body"]["emitted_record"] = json!(attach_derivation(&b, &dv).unwrap());
}
