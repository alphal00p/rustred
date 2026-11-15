//! Independent audit (lane audit-credits, 2026-11-14) of the IMAGE LIFT and of the EVIDENCE LEVEL of symmetry credits
//! as changed by lane import3 (closure.md 17, interfaces/import.md 15), through the public import API on a toy node:
//! a two-loop sunset family (arity 5, lines D1 D2 D3, numerators D4 D5; nothing five-loop specific) with the VERIFIED
//! group derived from the family file.
//!
//! `ok_*` tests pin behaviour that holds. The reproduction of finding M1 (failed on ac/import3 c56203c9) passes since
//! lane import4 and is no longer ignored.

use rustred_campaign_core::import;

use std::path::{Path, PathBuf};

use import::{CheckOptions, Evidence, Level, RowStatus, TableSetSpec, check, load};
use rustred_campaign_core::closure::Tier;
use serde_json::{Value, json};

/// lane closeloop4 (audit-closeloop M1 / m4): the spec text with its evidence PINNED (each file's sha256, a
/// SHA256SUMS manifest beside a directory) and the family of its tables named (spec evidence is read only so).
#[allow(dead_code)]
fn pin_ev(spec: &serde_json::Value) -> String {
    use std::io::BufRead;
    let mut v = spec.clone();
    let fam = v["sources"].as_array().and_then(|a| {
        a.iter().find_map(|s| {
            let p = s["path"].as_str()?;
            let f = std::fs::File::open(p).ok()?;
            let mut l = String::new();
            std::io::BufReader::new(f).read_line(&mut l).ok()?;
            let h: serde_json::Value = serde_json::from_str(l.trim()).ok()?;
            h["family"].as_str().map(str::to_string)
        })
    });
    let sha = |p: &std::path::Path| rustred_campaign_core::import::lines::file_sha256(p).unwrap();
    if let Some(ev) = v.get_mut("evidence").and_then(|e| e.as_object_mut()) {
        for k in ["facts", "proofs", "solved"] {
            if let Some(a) = ev.get_mut(k).and_then(|x| x.as_array_mut()) {
                for e in a {
                    let p = std::path::PathBuf::from(e["path"].as_str().unwrap_or(""));
                    if p.is_file() && e.get("sha256").is_none() {
                        e["sha256"] = serde_json::json!(sha(&p));
                    }
                    if p.is_dir() && e.get("manifest").is_none() {
                        let mut names: Vec<String> = std::fs::read_dir(&p)
                            .unwrap()
                            .flatten()
                            .map(|x| x.file_name().to_string_lossy().into_owned())
                            .filter(|n| n.ends_with(".json") || n.ends_with(".jsonl"))
                            .collect();
                        names.sort();
                        let body: String = names
                            .iter()
                            .map(|n| format!("{}  {n}\n", sha(&p.join(n))))
                            .collect();
                        let m = std::env::temp_dir().join(format!(
                            "pin-ev-{}-{}.SHA256SUMS",
                            std::process::id(),
                            rustred_campaign_core::closure::sha256::sha256_tag(
                                p.display().to_string().as_bytes()
                            )
                        ));
                        std::fs::write(&m, body).unwrap();
                        e["manifest"] =
                            serde_json::json!({"path": m.display().to_string(), "sha256": sha(&m)});
                    }
                    if e.get("family").is_none()
                        && let Some(f) = &fam
                    {
                        e["family"] = serde_json::json!(f);
                    }
                }
            }
        }
    }
    v.to_string()
}

const N: usize = 5;
const MASK: &str = "11100";
const FAMILY: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("audit-credits-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn kt(k: &[i32]) -> String {
    format!(
        "I({})",
        k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    )
}

/// A POINT rule at `key` (every slot fixed), body `rhs` (coefficient 1 each), witness loci `witness`.
fn point_rule(index: i64, key: &[i32], rhs: &[&[i32]], witness: Value) -> String {
    let mut v = json!({"type":"rule","chart":MASK,"index":index,"fixed":key,"equations":[],"target":kt(key),
        "rhs": rhs.iter().map(|k| json!([kt(k), "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":witness});
    if v["witness_loci"].is_null() {
        // witness loci NOT CARRIED (field absent)
        v.as_object_mut().unwrap().remove("witness_loci");
    }
    v.to_string()
}

fn header() -> String {
    json!({"type":"header","schema":"table_dump.v1","family":"S","denominators":N,"mask":MASK,
        "family_source":FAMILY})
    .to_string()
}

fn write_table(dir: &Path, name: &str, rules: &[String]) -> PathBuf {
    let p = dir.join(name);
    let mut s = header();
    s.push('\n');
    s.push_str(&json!({"type":"chart","chart":MASK,"guards":["n>=1","n>=1","n>=1","n<=0","n<=0"],"zero":false}).to_string());
    s.push('\n');
    for r in rules {
        s.push_str(r);
        s.push('\n');
    }
    s.push_str(r#"{"type":"end","charts":1}"#);
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    p
}

fn write_keys(dir: &Path, keys: &[&[i32]]) -> PathBuf {
    let p = dir.join("start.jsonl");
    let s: String = keys.iter().map(|k| format!("{}\n", json!(k))).collect();
    std::fs::write(&p, s).unwrap();
    p
}

fn run(
    spec: &Value,
    dir: &Path,
    tier: Tier,
) -> (import::ImportedSet, Evidence, import::CheckResult) {
    let s = TableSetSpec::from_json(&pin_ev(spec)).unwrap();
    let set = load(&s, dir).unwrap();
    let ev = Evidence::from_spec(&set, dir).unwrap();
    let r = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 2,
            tier: Some(tier),
            ..Default::default()
        },
    )
    .unwrap();
    (set, ev, r)
}

fn row<'a>(r: &'a import::CheckResult, k: &[i32]) -> &'a import::KeyRow {
    r.rows
        .iter()
        .find(|x| x.key == k)
        .unwrap_or_else(|| panic!("key {k:?} not reached"))
}

const K: [i32; 5] = [1, 2, 3, 0, 0];
const LOW: [i32; 5] = [1, 1, 1, 0, 0];

/// The distinct images of K under the sunset's line permutations (every non-identity element moves (1,2,3)).
fn images() -> Vec<[i32; 5]> {
    let v = [1, 2, 3];
    let mut out = vec![];
    for p in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        out.push([v[p[0]], v[p[1]], v[p[2]], 0, 0]);
    }
    out
}

fn base_spec(dir: &Path, table: &Path, fam: &Path, start: &[&[i32]]) -> Value {
    let st = write_keys(dir, start);
    json!({"schema":"rustred.campaign.import-spec.v1","node":"toy-sunset","frame_from":"A",
        "policies":{"elided":"lift_with_per_key_proof"},
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":table.display().to_string()},
                   {"id":"G","kind":"symmetry_credit","family":{"path": fam.display().to_string()}},
                   {"id":"T","kind":"terminals","keys":[LOW]}]})
}

fn proof_file(dir: &Path, name: &str, lines: &[(String, [i32; 5])]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(
        &p,
        json!({"schema":"rustred.proof-kernel.span.v1","keys":
            lines.iter().map(|(sha, k)| json!({"record_sha256": sha, "key": k, "status":"EXACT_IN_SPAN_AT_KEY"})).collect::<Vec<_>>()})
        .to_string(),
    )
    .unwrap();
    p
}

/// IMAGE LIFT through the public import, both tiers: every image of K is a POINT record with ELIDED witness loci
/// (first match there), K itself has no record. (i) no proofs: K open; (ii) proofs bound to (image record, image):
/// K credited at level per-key; (iii) proofs bound to (image record, K) (wrong key): open; (iv) proofs bound to a
/// SECOND record covering the image (a complete-witness record after the elided first match): open (only the first
/// match's proof lifts); (v) a regularity fact alone at (image record, image): open.
#[test]
fn ok_image_lift_only_by_the_first_match_proof_at_the_image() {
    let d = scratch("lift");
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let imgs = images();
    let mut rules = vec![];
    for (i, im) in imgs.iter().enumerate() {
        rules.push(point_rule(i as i64, im, &[&LOW], json!(["elided"])));
    }
    // second records at the same images, complete witness (never first match)
    for (i, im) in imgs.iter().enumerate() {
        rules.push(point_rule(100 + i as i64, im, &[&LOW], json!([])));
    }
    let a = write_table(&d, "A.table.jsonl", &rules);
    let spec = base_spec(&d, &a, &fam, &[&K]);
    for tier in [Tier::K, Tier::C] {
        // (i) no proofs
        let (set, _ev, r) = run(&spec, &d, tier);
        assert!(set.credits.as_ref().unwrap().is_verified());
        let x = row(&r, &K);
        assert_eq!(x.status, RowStatus::Open, "{tier:?} (i): {x:?}");
        let sha_first: Vec<String> = (0..imgs.len())
            .map(|i| set.rules[0][i].record_sha256.clone())
            .collect();
        let sha_second: Vec<String> = (0..imgs.len())
            .map(|i| set.rules[0][imgs.len() + i].record_sha256.clone())
            .collect();
        // (ii) proofs at (first record, image)
        let p = proof_file(
            &d,
            "p2.json",
            &imgs
                .iter()
                .zip(&sha_first)
                .map(|(im, s)| (s.clone(), *im))
                .collect::<Vec<_>>(),
        );
        let mut s2 = spec.clone();
        s2["evidence"] = json!({"proofs":[{"path": p.display().to_string()}]});
        let (_set, ev, r) = run(&s2, &d, tier);
        assert_eq!(ev.counts.proofs_per_key, imgs.len());
        let x = row(&r, &K);
        assert_eq!(
            (x.status, x.evidence, x.basis),
            (
                RowStatus::Served,
                Some(Level::PerKey),
                Some(import::check::BASIS_SYMMETRY_CREDIT_VERIFIED)
            ),
            "{tier:?} (ii)"
        );
        // (iii) proofs at (first record, K): wrong key
        let p = proof_file(
            &d,
            "p3.json",
            &sha_first.iter().map(|s| (s.clone(), K)).collect::<Vec<_>>(),
        );
        let mut s3 = spec.clone();
        s3["evidence"] = json!({"proofs":[{"path": p.display().to_string()}]});
        let (_set, _ev, r) = run(&s3, &d, tier);
        assert_eq!(row(&r, &K).status, RowStatus::Open, "{tier:?} (iii)");
        // (iv) proofs bound to the SECOND record at the image
        let p = proof_file(
            &d,
            "p4.json",
            &imgs
                .iter()
                .zip(&sha_second)
                .map(|(im, s)| (s.clone(), *im))
                .collect::<Vec<_>>(),
        );
        let mut s4 = spec.clone();
        s4["evidence"] = json!({"proofs":[{"path": p.display().to_string()}]});
        let (_set, _ev, r) = run(&s4, &d, tier);
        assert_eq!(row(&r, &K).status, RowStatus::Open, "{tier:?} (iv)");
        // (v) regularity facts at (first record, image), no proof
        let f = d.join("facts.jsonl");
        let lines: String = imgs
            .iter()
            .zip(&sha_first)
            .enumerate()
            .map(|(i, (im, s))| {
                format!(
                    "{}\n",
                    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":s,"rule":i,"key":im,
                        "in_case":true,"chart_ok":true,"branch":null,"regular":"regular","irregular_terms":[],"hidden_loci":[],
                        "rhs":[{"term":0,"key":LOW,"coef":"1"}],"zero_test":"exact","error":null})
                )
            })
            .collect();
        std::fs::write(&f, lines).unwrap();
        let mut s5 = spec.clone();
        s5["evidence"] = json!({"facts":[{"path": f.display().to_string()}]});
        let (_set, _ev, r) = run(&s5, &d, tier);
        assert_eq!(row(&r, &K).status, RowStatus::Open, "{tier:?} (v)");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// EVIDENCE LEVEL of a verified credit whose image is a complete-witness record: `imported` without a per-key proof at
/// (image record, image), `per-key` with one; never above the image's own level (the image row, reached as a start
/// key, has the same level).
#[test]
fn ok_credit_level_equals_the_image_level_for_table_images() {
    let d = scratch("level");
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let imgs = images();
    let rules: Vec<String> = imgs
        .iter()
        .enumerate()
        .map(|(i, im)| point_rule(i as i64, im, &[&LOW], json!([])))
        .collect();
    let a = write_table(&d, "A.table.jsonl", &rules);
    let mut start: Vec<&[i32]> = vec![&K];
    for im in &imgs {
        start.push(im);
    }
    let spec = base_spec(&d, &a, &fam, &start);
    let (set, _ev, r) = run(&spec, &d, Tier::K);
    let x = row(&r, &K);
    assert_eq!(
        (x.status, x.evidence, x.basis),
        (
            RowStatus::Served,
            Some(Level::Imported),
            Some(import::check::BASIS_SYMMETRY_CREDIT_VERIFIED)
        )
    );
    let shas: Vec<String> = (0..imgs.len())
        .map(|i| set.rules[0][i].record_sha256.clone())
        .collect();
    let p = proof_file(
        &d,
        "p.json",
        &imgs
            .iter()
            .zip(&shas)
            .map(|(im, s)| (s.clone(), *im))
            .collect::<Vec<_>>(),
    );
    let mut s2 = spec.clone();
    s2["evidence"] = json!({"proofs":[{"path": p.display().to_string()}]});
    let (_set, _ev, r) = run(&s2, &d, Tier::K);
    let x = row(&r, &K);
    assert_eq!(
        (x.status, x.evidence),
        (RowStatus::Served, Some(Level::PerKey))
    );
    for im in &imgs {
        assert_eq!(row(&r, im).evidence, Some(Level::PerKey), "image {im:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// FINDING M1 (level escalation, new code of import3 in import/check.rs, branch `symmetry_credit_via_relation`), FIXED
/// by lane import4: a verified credit whose image is served by a SUPPLIED relation that is NOT re-derived (it carries
/// only a `verification` record, so its own key is served at level `imported`, basis `supplied`) was reported at level
/// `per-key`, ABOVE the level of its image. Binding rule: a credited key's level is the LOWER of the credit's own level
/// and the level at which its image is served (`imported` here); a credit never raises a level.
#[test]
fn credit_via_unverified_supplied_relation_is_not_above_its_image() {
    let d = scratch("viarel");
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    // table A: one unrelated rule (frame source); no record at K or its images
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[point_rule(0, &[3, 3, 3, 0, 0], &[&LOW], json!([]))],
    );
    // supplied relations at EVERY image of K: I(image) = I(1,1,1,0,0), with a verification record only
    let imgs = images();
    let s_rules: Vec<String> = imgs
        .iter()
        .enumerate()
        .map(|(i, im)| {
            let mut v: Value =
                serde_json::from_str(&point_rule(i as i64, im, &[&LOW], json!([]))).unwrap();
            v["dispatch"] = json!("Point");
            v["kind"] = json!("symmetry-one-term");
            v.to_string()
        })
        .collect();
    let s = write_table(&d, "S.table.jsonl", &s_rules);
    let ver = d.join("verify.json");
    std::fs::write(&ver, "{\"proved\":true}\n").unwrap();
    let mut start: Vec<&[i32]> = vec![&K];
    for im in &imgs {
        start.push(im);
    }
    let mut spec = base_spec(&d, &a, &fam, &start);
    spec["sources"].as_array_mut().unwrap().push(
        json!({"id":"sym","kind":"supplied_relations","class":"symmetry","path":s.display().to_string(),
               "verification":{"path": ver.display().to_string()},"hand_name":{"template":"{source}#{index}"}}),
    );
    let (_set, _ev, r) = run(&spec, &d, Tier::K);
    // lane symfix (audit r2-sym F1, import.md 21): a supplied relation whose `verification` file the import cannot
    // use (here not a composite verification line, and the relation not re-derivable from the group) is NOT verified at
    // load: no EXACT bit, the image is NOT served (it was served at `imported`, basis `supplied`, before the fix)
    for im in &imgs {
        let y = row(&r, im);
        assert!(
            !(y.status == RowStatus::Served && y.evidence.is_some()),
            "image {im:?}: an unverified supplied relation served: {y:?}"
        );
    }
    // the M1 rule (a credit never raises a level) on the new semantics: the image has no level, the credit has none
    let x = row(&r, &K);
    println!("credited key row: {x:?}");
    assert!(
        !(x.status == RowStatus::Served && x.evidence.is_some()),
        "the credit is reported above its image's (absent) level: {x:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// NEVER CHAINED with the image lift: the image of K is ITSELF uncovered (no record) and only creditable through its
/// own image, which is a lifted elided record with its proof: K must stay open (Tier C), the image is credited.
#[test]
fn ok_lifted_images_never_chain() {
    let d = scratch("chain");
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    // records only at the images of images that are NOT images of K... the sunset group is S3 on the lines: every
    // permutation of (1,2,3) is an image of K, so use a key with a repeated value: K2 = (1,1,2), images (1,2,1),
    // (2,1,1); put a record only at (2,1,1) and start from K2 and (1,2,1): (1,2,1) -> (2,1,1) one step; K2 -> (2,1,1)
    // one step as well (the group is the full S3), so chaining cannot be observed on S3 with distinct records.
    // Instead: elided record at (2,1,1) WITHOUT proof, complete record at (1,2,1): K2's elements reach both; the
    // credit must go to (1,2,1) (covered) and never to (2,1,1) through (1,2,1).
    let k2 = [1, 1, 2, 0, 0];
    let a = write_table(
        &d,
        "A.table.jsonl",
        &[
            point_rule(0, &[2, 1, 1, 0, 0], &[&LOW], json!(["elided"])),
            point_rule(1, &[1, 2, 1, 0, 0], &[&LOW], json!([])),
        ],
    );
    let spec = base_spec(&d, &a, &fam, &[&k2]);
    let (_set, _ev, r) = run(&spec, &d, Tier::C);
    let x = row(&r, &k2);
    assert_eq!(x.status, RowStatus::Served, "{x:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A REGULARITY FACT never lifts an IMAGE (closure.md 17, re-audit decision kept): the images of K are point records
/// WITHOUT witness loci (not carried); with lift facts (regular, in case) at (image record, image) the IMAGES
/// themselves are served (witness lift, Tier C) when reached directly, but K is NOT credited through them; with a
/// per-key proof at (image record, image) it is.
#[test]
fn ok_regularity_fact_lifts_the_image_key_but_never_a_credit_through_it() {
    let d = scratch("factlift");
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let imgs = images();
    let rules: Vec<String> = imgs
        .iter()
        .enumerate()
        .map(|(i, im)| point_rule(i as i64, im, &[&LOW], Value::Null))
        .collect();
    let a = write_table(&d, "A.table.jsonl", &rules);
    let mut start: Vec<&[i32]> = vec![&K];
    for im in &imgs {
        start.push(im);
    }
    let spec = base_spec(&d, &a, &fam, &start);
    let (set, _ev, _r) = run(&spec, &d, Tier::C);
    let shas: Vec<String> = (0..imgs.len())
        .map(|i| set.rules[0][i].record_sha256.clone())
        .collect();
    let f = d.join("facts.jsonl");
    let lines: String = imgs
        .iter()
        .zip(&shas)
        .enumerate()
        .map(|(i, (im, s))| {
            format!(
                "{}\n",
                json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":s,"rule":i,"key":im,
                    "in_case":true,"chart_ok":true,"branch":null,"regular":"regular","irregular_terms":[],"hidden_loci":[],
                    "rhs":[{"term":0,"key":LOW,"coef":"1"}],"zero_test":"exact","error":null})
            )
        })
        .collect();
    std::fs::write(&f, lines).unwrap();
    let mut s2 = spec.clone();
    s2["evidence"] = json!({"facts":[{"path": f.display().to_string()}]});
    let (_set, _ev, r) = run(&s2, &d, Tier::C);
    for im in &imgs {
        let y = row(&r, im);
        assert_eq!(
            y.status,
            RowStatus::Served,
            "image {im:?} lifted by its fact: {y:?}"
        );
    }
    let x = row(&r, &K);
    assert_eq!(
        x.status,
        RowStatus::Open,
        "credited through a fact-lifted image: {x:?}"
    );
    // with per-key proofs at the images: credited, level per-key
    let p = proof_file(
        &d,
        "p.json",
        &imgs
            .iter()
            .zip(&shas)
            .map(|(im, s)| (s.clone(), *im))
            .collect::<Vec<_>>(),
    );
    let mut s3 = s2.clone();
    s3["evidence"]["proofs"] = json!([{"path": p.display().to_string()}]);
    let (_set, _ev, r) = run(&s3, &d, Tier::C);
    let x = row(&r, &K);
    assert_eq!(
        (x.status, x.evidence),
        (RowStatus::Served, Some(Level::PerKey)),
        "{x:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
