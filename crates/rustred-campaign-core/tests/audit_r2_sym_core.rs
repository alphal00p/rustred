//! AUDIT r2-sym (independent auditor of round 2's SUPPLIED relations, 2026-11-15; notes/implementer.audit-r2-sym.md).
//! Core level, through the public API, on a toy node: the two-loop sunset (arity 5, lines D1 D2 D3, numerators D4 D5;
//! nothing five-loop specific, L8) with its group DERIVED from the family file.
//!
//! FINDINGS reproduced here (each test is `#[ignore]`d while the defect stands and FAILS when run with `--ignored`;
//! the fix flips it to green and the `#[ignore]` goes):
//! - F1 `a_verification_only_supplied_relation_is_not_certified_unread`: a `supplied_relations` source that names a
//!   `verification` file is served EXACT whatever that file holds (import/load.rs: `None if ver.is_some() =>
//!   VERDICT_EXACT`); a FALSE relation, even one whose coefficient text is not a number, is served at `imported` and
//!   counts as CERTIFIED at the default `required_evidence` (`imported`). This is exactly how the repair flow feeds
//!   the symmetry composites into a node (closeloop/repair.rs `with_composites`).
//! - F2 `verify_shipped_refuses_a_duplicated_term_that_hides_a_missing_one`: `composite_verify::verify_shipped`
//!   compares the NUMBER of terms and looks every SHIPPED term up in the recomposed relation, but never checks that the
//!   shipped terms are distinct: a relation that repeats one term (same coefficient) in place of another passes.
//!
//! Forgeries REFUSED today (green; they guard the checks that work): a changed coefficient, the true relation of
//! another key, another element, another element's image, a dropped term, a term that is not lower, an extra
//! zero-coefficient term, a coefficient with a pole, cited records on a plain relation, a composite citing another
//! record or another term, and the import's own `verify` mode (group re-derivation) refusing the F1 relation.

use rustred_campaign_core::import::{
    self, CheckOptions, Evidence, Level, RowStatus, TableSetSpec, check, load,
};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::repairflow::composite::{
    Catalogue, ComposeOptions, QField, RuleUse, Service, compose_image, image_of, import_lower,
    rows_by_element,
};
use rustred_campaign_core::repairflow::composite_verify::{ModCatalogue, ModField, verify_shipped};
use rustred_campaign_core::replay_base::algebra::Q;
use rustred_campaign_core::replay_base::symrows::VerifiedGroup;

use serde_json::{Value, json};
use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const N: usize = 5;
const MASK: &str = "11100";
const LOW: [i32; 5] = [1, 1, 1, 0, 0];
const FAMILY: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";
/// The supervisor's default d values (adapters::symcomp::DEFAULT_D_VALUES; data).
const DS: [u64; 3] = [2_000_029, 9_999_991, 314_159_257];

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("audit-r2-sym-{tag}-{}", std::process::id()));
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

fn sha256(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// A POINT rule at `key` with body `rhs` (key, coefficient text).
fn point_rule(index: i64, key: &[i32], rhs: &[(Vec<i32>, String)]) -> Value {
    json!({"type":"rule","chart":MASK,"index":index,"fixed":key,"equations":[],"target":kt(key),
        "rhs": rhs.iter().map(|(k, c)| json!([kt(k), c])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
}

fn header() -> Value {
    json!({"type":"header","schema":"table_dump.v1","family":"S","denominators":N,"mask":MASK,
        "family_source":FAMILY})
}

fn write_table(dir: &Path, name: &str, rules: &[Value]) -> PathBuf {
    let p = dir.join(name);
    let mut s = header().to_string();
    s.push('\n');
    s.push_str(&json!({"type":"chart","chart":MASK,"guards":["n>=1","n>=1","n>=1","n<=0","n<=0"],"zero":false}).to_string());
    s.push('\n');
    for r in rules {
        s.push_str(&r.to_string());
        s.push('\n');
    }
    s.push_str(r#"{"type":"end","charts":1}"#);
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    p
}

/// The toy node's import: frame table A (an unrelated record + `rules`), terminal LOW, optional extra source.
fn toy_set(
    d: &Path,
    rules: &[Value],
    start: &[Vec<i32>],
    extra: Option<Value>,
) -> import::ImportedSet {
    toy_set_result(d, rules, start, extra).unwrap()
}

/// [`toy_set`] returning the load's result (lane symfix: a malformed supplied record is refused at load).
fn toy_set_result(
    d: &Path,
    rules: &[Value],
    start: &[Vec<i32>],
    extra: Option<Value>,
) -> Result<import::ImportedSet, import::ImportError> {
    let mut all = vec![point_rule(
        0,
        &[4, 4, 4, 0, 0],
        &[(LOW.to_vec(), "1".into())],
    )];
    for (i, r) in rules.iter().enumerate() {
        let mut r = r.clone();
        r["index"] = json!(i as i64 + 1);
        all.push(r);
    }
    let a = write_table(d, "A.table.jsonl", &all);
    let st = d.join("start.jsonl");
    std::fs::write(
        &st,
        start
            .iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let mut sources = vec![
        json!({"id":"A","kind":"engine_table","path":a.display().to_string()}),
        json!({"id":"T","kind":"terminals","keys":[LOW]}),
    ];
    if let Some(x) = extra {
        sources.push(x);
    }
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy-sunset","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},"sources":sources});
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    load(&s, d)
}

fn group_of(d: &Path, set: &import::ImportedSet) -> VerifiedGroup {
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let gs: import::symmetry::GroupSpec =
        serde_json::from_value(json!({"family": {"path": fam.display().to_string()}})).unwrap();
    import::symmetry::load_verified_group(
        &gs,
        d,
        set.frame.mask(),
        set.cmp.presentation().new_to_old(),
        Some(FAMILY),
    )
    .unwrap()
    .0
}

fn record_line(set: &import::ImportedSet, t: usize, r: usize) -> Result<Vec<u8>, String> {
    let info = &set.rules[t][r];
    let mut f = std::fs::File::open(&info.path).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(info.offset))
        .map_err(|e| e.to_string())?;
    let mut line = vec![];
    std::io::BufReader::new(f)
        .read_until(b'\n', &mut line)
        .map_err(|e| e.to_string())?;
    while matches!(line.last(), Some(b'\n' | b'\r')) {
        line.pop();
    }
    Ok(line)
}

fn q_text(q: &Q) -> String {
    if q.d == 1 {
        q.n.to_string()
    } else {
        format!("{}/{}", q.n, q.d)
    }
}

// ------------------------------------------------------------------------------------------------------------ F1

/// F1 setup: FALSE relations in a supplied file (a top key = 7 x the terminal; with `bad_coefficient` also another =
/// "1/0" x the terminal); `with` = the source's checking field (`verification` or `verify`). lane symfix: the
/// malformed coefficient "1/0" is now REFUSED at load (see `a_malformed_coefficient_is_refused_at_parse`), so the
/// served-row tests use the "7" relation alone.
fn f1_spec(tag: &str, with: &str, bad_coefficient: bool) -> (PathBuf, Value, Vec<Vec<i32>>) {
    let d = scratch(tag);
    let k1 = vec![2, 1, 1, 0, 0];
    let k2 = vec![1, 1, 3, 0, 0];
    let mut rels = vec![point_rule(0, &k1, &[(LOW.to_vec(), "7".into())])];
    let mut keys = vec![k1];
    if bad_coefficient {
        rels.push(point_rule(1, &k2, &[(LOW.to_vec(), "1/0".into())]));
        keys.push(k2);
    }
    let rules = rels
        .iter()
        .map(|r| {
            let mut r = r.clone();
            r["dispatch"] = json!("Point");
            r["kind"] = json!("symmetry-composite");
            r
        })
        .collect::<Vec<_>>();
    let s = write_table(&d, "S.table.jsonl", &rules);
    let mut src = json!({"id":"rf:symmetry-composite","kind":"supplied_relations","class":"symmetry-composite",
        "path": s.display().to_string(), "sha256": sha256(&std::fs::read(&s).unwrap())});
    if with == "verification" {
        // the verification file's CONTENT is irrelevant to the import: an empty file, pinned as the repair flow pins
        let v = d.join("verification.jsonl");
        std::fs::write(&v, b"").unwrap();
        src["verification"] = json!({"path": v.display().to_string(), "sha256": sha256(b"")});
    } else {
        let fam = d.join("S.sym");
        std::fs::write(&fam, FAMILY).unwrap();
        src["verify"] = json!({"family": {"path": fam.display().to_string()}});
    }
    (d, src, keys)
}

fn f1_check(tag: &str, with: &str) -> (import::ImportedSet, import::CheckResult, Vec<Vec<i32>>) {
    let (d, src, keys) = f1_spec(tag, with, false);
    let set = toy_set(&d, &[], &keys, Some(src));
    let ev = Evidence::from_spec(&set, &d).unwrap();
    let r = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 1,
            tier: Some(rustred_campaign_core::closure::Tier::K),
            ..Default::default()
        },
    )
    .unwrap();
    (set, r, keys)
}

fn certified(r: &import::CheckResult, k: &[i32]) -> Option<(RowStatus, Option<Level>, String)> {
    let row = r.rows.iter().find(|x| x.key == k)?;
    Some((
        row.status,
        row.evidence,
        row.basis.unwrap_or("").to_string(),
    ))
}

#[test]
fn a_verification_only_supplied_relation_is_not_certified_unread() {
    let (set, r, keys) = f1_check("f1", "verification");
    assert_eq!(r.required, Level::Imported, "the default required evidence");
    // lane symfix: the empty verification file is READ: no line of the key -> unverified, typed
    let rep = set
        .report
        .sources
        .iter()
        .find(|s| s.relations > 0)
        .expect("the supplied source's report");
    assert_eq!(rep.relations_not_verified.len(), keys.len(), "{rep:?}");
    assert!(
        rep.relations_not_verified
            .iter()
            .all(|(_, why)| why.starts_with("verification_line_missing")),
        "{:?}",
        rep.relations_not_verified
    );
    for k in &keys {
        let (st, lv, basis) = certified(&r, k).expect("row");
        let at_required = st == RowStatus::Served && lv.is_some_and(|l| l >= r.required);
        assert!(
            !at_required,
            "F1: the FALSE supplied relation at {k:?} is served at {lv:?} (basis {basis}) = certified at the default required evidence; the verification file was never read (it is empty)"
        );
    }
}

/// Control: the SAME false relations through the import's own `verify` mode (re-derivation from the verified group)
/// are refused (fail closed): the import CAN check; the `verification` path does not.
#[test]
fn the_import_verify_mode_refuses_the_same_false_relations() {
    let (set, r, keys) = f1_check("f1v", "verify");
    let rep = set
        .report
        .sources
        .iter()
        .find(|s| s.relations > 0)
        .expect("the supplied source's report");
    assert_eq!(rep.relations_verified, 0);
    assert_eq!(rep.relations_not_verified.len(), keys.len());
    for k in &keys {
        let (st, lv, _) = certified(&r, k).expect("row");
        assert!(
            !(st == RowStatus::Served && lv.is_some_and(|l| l >= r.required)),
            "{k:?}: {st:?} {lv:?}"
        );
    }
}

/// lane symfix (F1, "1/0"): a malformed coefficient text is REFUSED AT PARSE (load error, typed), whatever checking
/// field the source carries.
#[test]
fn a_malformed_coefficient_is_refused_at_parse() {
    for with in ["verification", "verify"] {
        let (d, src, keys) = f1_spec(&format!("f1p-{with}"), with, true);
        let e = toy_set_result(&d, &[], &keys, Some(src))
            .expect_err("a supplied record with the coefficient 1/0 must be refused at load");
        let m = e.to_string();
        assert!(m.contains("vanishes identically"), "{with}: {m}");
    }
}

// ------------------------------------------------------------------------------------------------------------ F2 +

/// A plain multi-term symmetry relation of the toy node: (key, element, image), every image term lower than the key.
struct Plain {
    key: Vec<i32>,
    element: usize,
    image: Vec<(Vec<i32>, Q)>,
}

fn search(
    set: &import::ImportedSet,
    g: &VerifiedGroup,
    want: impl Fn(&[(Vec<i32>, Q)], &[i32]) -> bool,
) -> Vec<Plain> {
    let lines = set.frame.mask().to_vec();
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let mut out = vec![];
    for a in 1..=3 {
        for b in 1..=3 {
            for c in 1..=3 {
                for n4 in -2..=0 {
                    for n5 in -2..=0 {
                        let key = vec![a, b, c, n4, n5];
                        let Ok(by) = rows_by_element(&Independent, g, &key, 1 << 12) else {
                            continue;
                        };
                        for (e, rows) in &by {
                            if let Ok(Some(im)) = image_of(&Independent, rows, &key, &cmp)
                                && want(&im, &key)
                            {
                                out.push(Plain {
                                    key: key.clone(),
                                    element: *e,
                                    image: im,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = lines;
    out
}

struct Ctx {
    set: import::ImportedSet,
    group: VerifiedGroup,
    lines: Vec<bool>,
}

impl Ctx {
    fn verify(
        &self,
        key: &[i32],
        element: usize,
        image: &[(Vec<i32>, Q)],
        records: &[(Vec<i32>, String)],
        rhs: &[(Vec<i32>, String)],
    ) -> Result<(), String> {
        let field = ModField::engine(&Independent, &DS);
        let line_of = |t: usize, r: usize| record_line(&self.set, t, r);
        let mcat = ModCatalogue {
            set: &self.set,
            line_of: &line_of,
            field: &field,
        };
        let cmp = |a: &[i32], b: &[i32]| self.set.cmp.cmp(a, b);
        let lower = |k: &[i32], t: &[i32]| import_lower(&self.lines, k, t, &cmp);
        verify_shipped(
            &Independent,
            &field,
            &mcat,
            &self.group,
            key,
            element,
            image,
            records,
            rhs,
            &lower,
            &cmp,
            1 << 12,
            1,
        )
    }
}

fn plain_ctx(tag: &str) -> (Ctx, Vec<Plain>) {
    let d = scratch(tag);
    let set = toy_set(&d, &[], &[LOW.to_vec()], None);
    let group = group_of(&d, &set);
    let lines = set.frame.mask().to_vec();
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let found = search(&set, &group, |im, key| {
        im.len() >= 2 && im.iter().all(|(t, _)| import_lower(&lines, t, key, &cmp))
    });
    (Ctx { set, group, lines }, found)
}

fn texts(im: &[(Vec<i32>, Q)]) -> Vec<(Vec<i32>, String)> {
    im.iter().map(|(k, q)| (k.clone(), q_text(q))).collect()
}

#[test]
fn verify_shipped_accepts_the_honest_plain_relation_and_refuses_forgeries() {
    let (cx, found) = plain_ctx("plain");
    assert!(
        found.len() >= 2,
        "the toy node must give at least two plain multi-term symmetry relations (found {})",
        found.len()
    );
    let p = &found[0];
    let rhs = texts(&p.image);
    cx.verify(&p.key, p.element, &p.image, &[], &rhs)
        .expect("the honest plain relation");
    let refuse = |what: &str, r: Result<(), String>| {
        assert!(r.is_err(), "forgery '{what}' was ACCEPTED");
    };
    // (a) one coefficient changed
    let mut f = rhs.clone();
    f[0].1 = format!("({})+1", f[0].1);
    refuse(
        "coefficient + 1",
        cx.verify(&p.key, p.element, &p.image, &[], &f),
    );
    // (b) the true relation of ANOTHER key shipped for this key (image and rhs of found[j], j with another key)
    if let Some(o) = found.iter().find(|o| o.key != p.key) {
        refuse(
            "another key's relation",
            cx.verify(&p.key, o.element, &o.image, &[], &texts(&o.image)),
        );
        refuse(
            "another key's rhs under this key's image",
            cx.verify(&p.key, p.element, &p.image, &[], &texts(&o.image)),
        );
    }
    // (c) another element claimed (with this image), and another element's image (same key) when there is one
    for e in 0..cx.group_len() {
        if e == p.element {
            continue;
        }
        let r = cx.verify(&p.key, e, &p.image, &[], &rhs);
        if r.is_ok() {
            // acceptable only when that element's image at the key IS this image (then the relation is true)
            let same = found
                .iter()
                .any(|o| o.key == p.key && o.element == e && o.image == p.image);
            assert!(
                same,
                "element {e} accepted with element {}'s image",
                p.element
            );
        }
    }
    // (d) a term dropped (a nonzero coefficient silently assumed zero)
    refuse(
        "a dropped term",
        cx.verify(&p.key, p.element, &p.image, &[], &rhs[1..]),
    );
    // (e) a term that is not lower (the key itself; a higher key)
    let mut f = rhs.clone();
    f.push((p.key.clone(), "1".into()));
    refuse(
        "the key on its own rhs",
        cx.verify(&p.key, p.element, &p.image, &[], &f),
    );
    // (f) an extra term with coefficient zero
    let mut f = rhs.clone();
    f.push((LOW.to_vec(), "0".into()));
    if !rhs.iter().any(|(k, _)| k == &LOW.to_vec()) {
        refuse(
            "an extra zero term",
            cx.verify(&p.key, p.element, &p.image, &[], &f),
        );
    }
    // (g) a coefficient with a pole (at every d) and one with a pole at the key's d value is not expressible here:
    //     a coefficient text that is not a number
    let mut f = rhs.clone();
    f[0].1 = "1/0".into();
    refuse(
        "coefficient 1/0",
        cx.verify(&p.key, p.element, &p.image, &[], &f),
    );
    let mut f = rhs.clone();
    f[0].1 = format!("{}*(d-d+1)/(d-d)", f[0].1);
    refuse(
        "coefficient with an identically vanishing denominator",
        cx.verify(&p.key, p.element, &p.image, &[], &f),
    );
    // (h) catalogue records cited on a plain relation
    let rec = cx.set.rules[0][0].record_sha256.clone();
    refuse(
        "a cited record on a plain relation",
        cx.verify(
            &p.key,
            p.element,
            &p.image,
            &[(p.image[0].0.clone(), rec)],
            &rhs,
        ),
    );
    // (i) a forged image (one image coefficient changed, shipped consistently)
    let mut im = p.image.clone();
    im[0].1 = Q {
        n: im[0].1.n + im[0].1.d,
        d: im[0].1.d,
    };
    refuse(
        "a forged image shipped consistently",
        cx.verify(&p.key, p.element, &im, &[], &texts(&im)),
    );
}

impl Ctx {
    fn group_len(&self) -> usize {
        let by = rows_by_element(&Independent, &self.group, &LOW, 1 << 12).unwrap_or_default();
        by.keys().max().map_or(0, |m| m + 1).max(8)
    }
}

#[test]
fn verify_shipped_refuses_a_duplicated_term_that_hides_a_missing_one() {
    let (cx, found) = plain_ctx("dup");
    let p = found.first().expect("a plain multi-term relation");
    let rhs = texts(&p.image);
    // drop the LAST term and repeat the FIRST (same coefficient): same length, every shipped term is in the
    // recomposed relation with its coefficient; the shipped relation is NOT the identity (a term is missing, one is
    // doubled)
    let mut f = rhs.clone();
    let last = f.len() - 1;
    f[last] = f[0].clone();
    assert!(
        cx.verify(&p.key, p.element, &p.image, &[], &f).is_err(),
        "F2: verify_shipped ACCEPTED the forged relation {f:?} for the true one {rhs:?} at {:?} (element {})",
        p.key,
        p.element
    );
}

// ------------------------------------------------------------------------------------------------ composite forgeries

/// A catalogue serving exactly ONE term over Q (for the honest composite's coefficient texts).
struct OneRule {
    term: Vec<i32>,
    rule: RuleUse,
    rhs: Vec<(Vec<i32>, Q)>,
}

impl<'a> Catalogue<QField<'a, Independent>> for OneRule {
    fn serve(&self, _: &QField<'a, Independent>, term: &[i32]) -> Result<Service<Q>, String> {
        if term == self.term.as_slice() {
            Ok(Service::Rule {
                rule: self.rule.clone(),
                rhs: self.rhs.clone(),
            })
        } else {
            Ok(Service::Unserved("not the one rule's term".into()))
        }
    }
}

#[test]
fn verify_shipped_composite_binds_the_cited_record_and_term() {
    // an image with exactly ONE term that is not lower (t != key); a catalogue point rule at t (I(t) = I(LOW)): the
    // composite = image with t replaced. The catalogue rule's TRUTH is the table's (verify_shipped takes the table as
    // data); what is checked is that the shipped relation is the composition through the FIRST MATCH at t.
    let d = scratch("comp");
    let set0 = toy_set(&d, &[], &[LOW.to_vec()], None);
    let g0 = group_of(&d, &set0);
    let lines = set0.frame.mask().to_vec();
    let cmp0 = |a: &[i32], b: &[i32]| set0.cmp.cmp(a, b);
    let found = search(&set0, &g0, |im, key| {
        im.len() >= 2
            && im
                .iter()
                .filter(|(t, _)| !import_lower(&lines, t, key, &cmp0))
                .count()
                == 1
            && im.iter().all(|(t, _)| t.as_slice() != key)
            && im
                .iter()
                .filter(|(t, _)| !import_lower(&lines, t, key, &cmp0))
                .all(|(t, _)| t.iter().zip(&lines).all(|(&x, &l)| !(!l && x > 0)))
    });
    let Some(p) = found.first() else {
        eprintln!(
            "no single-non-lower-term image on the toy node: composite forgeries not exercised"
        );
        return;
    };
    let t = p
        .image
        .iter()
        .find(|(t, _)| !import_lower(&lines, t, &p.key, &cmp0))
        .unwrap()
        .0
        .clone();
    drop(set0);
    let set = toy_set(
        &d,
        &[point_rule(1, &t, &[(LOW.to_vec(), "1".into())])],
        &[LOW.to_vec()],
        None,
    );
    let group = group_of(&d, &set);
    let lines = set.frame.mask().to_vec();
    let info = set.rules[0]
        .iter()
        .find(|r| r.label == 1)
        .expect("the catalogue record")
        .clone();
    let rule = RuleUse {
        table: 0,
        record: set.rules[0].iter().position(|r| r.label == 1).unwrap(),
        source: info.source.clone(),
        label: info.label,
        record_sha256: info.record_sha256.clone(),
    };
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let lower = |k: &[i32], tt: &[i32]| import_lower(&lines, k, tt, &cmp);
    let qf = QField(&Independent);
    let cat = OneRule {
        term: t.clone(),
        rule: rule.clone(),
        rhs: vec![(LOW.to_vec(), Q::ONE)],
    };
    let c = compose_image(
        &qf,
        &cat,
        &p.image,
        &p.key,
        &lower,
        &cmp,
        &ComposeOptions::default(),
    )
    .unwrap()
    .expect("the honest composite over Q");
    let rhs: Vec<(Vec<i32>, String)> = c.rhs.iter().map(|(k, q)| (k.clone(), q_text(q))).collect();
    let cx = Ctx {
        set,
        group,
        lines: lines.clone(),
    };
    let recs = vec![(t.clone(), rule.record_sha256.clone())];
    cx.verify(&p.key, p.element, &p.image, &recs, &rhs)
        .expect("the honest composite");
    // another record cited for the same term (the unrelated record of the table)
    let other = cx.set.rules[0]
        .iter()
        .find(|r| r.label == 0)
        .unwrap()
        .record_sha256
        .clone();
    assert!(
        cx.verify(&p.key, p.element, &p.image, &[(t.clone(), other)], &rhs)
            .is_err(),
        "a composite citing another record was ACCEPTED"
    );
    // the right record cited at another term
    assert!(
        cx.verify(
            &p.key,
            p.element,
            &p.image,
            &[(LOW.to_vec(), rule.record_sha256.clone())],
            &rhs
        )
        .is_err(),
        "a composite citing the record at another term was ACCEPTED"
    );
    // the image alone shipped (the replacement silently skipped: t is not lower)
    assert!(
        cx.verify(&p.key, p.element, &p.image, &recs, &texts(&p.image))
            .is_err(),
        "the uncomposed image was ACCEPTED as the composite"
    );
    // no record cited
    assert!(
        cx.verify(&p.key, p.element, &p.image, &[], &rhs).is_err(),
        "a composite without its cited record was ACCEPTED"
    );
}
