//! lane symfix (audit r2-sym F1 / F2 / F3 / F4 / F8; interfaces/import.md 21): a `supplied_relations` record is served
//! ONLY when the import verifies it AT LOAD. Toy node: the two-loop sunset (arity 5, lines D1 D2 D3, numerators D4 D5;
//! nothing five-loop specific, L8), group DERIVED from the frame header's family.
//!
//! - a plain relation named with a `verification` file only: re-derived from the group -> per-key `symmetry_relation`;
//! - an honest composite + its verification line: re-verified by the same backend -> `imported`
//!   `symmetry_composite_same_backend` (certified at required `imported`, NOT at `per-key`);
//! - the same composite with a CHECKED kernel certificate -> per-key `symmetry_relation_kernel_certified`;
//! - forgeries: a changed coefficient with the honest line, a certificate of another relation / kernel / family / class,
//!   an unsealed certificate file (load error), a two-row line without a certificate, a repeated rhs key (load error).

use rustred_campaign_core::import::supplied_verify::{
    CERT_SCHEMA, family_sha256, relation_canonical_text, relation_sha256,
};
use rustred_campaign_core::import::{
    self, CheckOptions, Evidence, Level, RowStatus, TableSetSpec, check, load,
};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::repairflow::composite::{
    COMPOSITE_SCHEMA, Catalogue, ComposeOptions, QField, RuleUse, Service, compose_image, image_of,
    import_lower, rows_by_element,
};
use rustred_campaign_core::replay_base::algebra::Q;
use rustred_campaign_core::replay_base::symrows::VerifiedGroup;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const N: usize = 5;
const MASK: &str = "11100";
const LOW: [i32; 5] = [1, 1, 1, 0, 0];
const FAMILY: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";
const KERNEL: &str = "abababababababababababababababababababababababababababababababab";

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("symfix-{tag}-{}", std::process::id()));
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

fn q_text(q: &Q) -> String {
    if q.d == 1 {
        q.n.to_string()
    } else {
        format!("{}/{}", q.n, q.d)
    }
}

fn point_rule(index: i64, key: &[i32], rhs: &[(Vec<i32>, String)]) -> Value {
    json!({"type":"rule","chart":MASK,"index":index,"fixed":key,"equations":[],"target":kt(key),
        "rhs": rhs.iter().map(|(k, c)| json!([kt(k), c])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
}

fn write_table(dir: &Path, name: &str, rules: &[Value]) -> PathBuf {
    let p = dir.join(name);
    let mut s =
        json!({"type":"header","schema":"table_dump.v1","family":"S","denominators":N,"mask":MASK,
        "family_source":FAMILY})
        .to_string();
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

fn supplied_table(d: &Path, rels: &[(Vec<i32>, Vec<(Vec<i32>, String)>, &str)]) -> PathBuf {
    let rules: Vec<Value> = rels
        .iter()
        .enumerate()
        .map(|(i, (k, rhs, kind))| {
            let mut r = point_rule(i as i64, k, rhs);
            r["dispatch"] = json!("Point");
            r["kind"] = json!(kind);
            r
        })
        .collect();
    write_table(d, "S.table.jsonl", &rules)
}

fn load_toy(
    d: &Path,
    rules: &[Value],
    start: &[Vec<i32>],
    extra: Option<Value>,
    required: &str,
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
        "required_evidence": required,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},"sources":sources});
    load(&TableSetSpec::from_json(&spec.to_string()).unwrap(), d)
}

fn row_of(
    set: &import::ImportedSet,
    d: &Path,
    k: &[i32],
) -> (RowStatus, Option<Level>, String, Level) {
    let ev = Evidence::from_spec(set, d).unwrap();
    let r = check(
        set,
        &ev,
        &CheckOptions {
            threads: 1,
            tier: Some(rustred_campaign_core::closure::Tier::K),
            ..Default::default()
        },
    )
    .unwrap();
    let row = r.rows.iter().find(|x| x.key == k).expect("row");
    (
        row.status,
        row.evidence,
        row.basis.unwrap_or("").to_string(),
        r.required,
    )
}

fn group_of(d: &Path, set: &import::ImportedSet) -> (VerifiedGroup, String) {
    let gs: import::symmetry::GroupSpec = serde_json::from_value(json!({})).unwrap();
    let (g, rep) = import::symmetry::load_verified_group(
        &gs,
        d,
        set.frame.mask(),
        set.cmp.presentation().new_to_old(),
        Some(FAMILY),
    )
    .unwrap();
    (g, rep.digest)
}

/// Every (key, element, image) of the toy box whose image passes `want`.
fn search(
    set: &import::ImportedSet,
    g: &VerifiedGroup,
    want: impl Fn(&[(Vec<i32>, Q)], &[i32]) -> bool,
) -> Vec<(Vec<i32>, usize, Vec<(Vec<i32>, Q)>)> {
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
                                out.push((key.clone(), *e, im));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

struct OneRule {
    term: Vec<i32>,
    rule: RuleUse,
}

impl<'a> Catalogue<QField<'a, Independent>> for OneRule {
    fn serve(&self, _: &QField<'a, Independent>, term: &[i32]) -> Result<Service<Q>, String> {
        if term == self.term.as_slice() {
            Ok(Service::Rule {
                rule: self.rule.clone(),
                rhs: vec![(LOW.to_vec(), Q::ONE)],
            })
        } else {
            Ok(Service::Unserved("not the one rule's term".into()))
        }
    }
}

/// The toy composite: key, element, image, the catalogue term t (a point rule I(t) = I(LOW) in table A), the composite
/// rhs texts, the catalogue rule (record sha256), the group digest. None when the toy node has no such image.
struct Toy {
    key: Vec<i32>,
    element: usize,
    image: Vec<(Vec<i32>, Q)>,
    t: Vec<i32>,
    rhs: Vec<(Vec<i32>, String)>,
    record_sha256: String,
    group_digest: String,
}

fn toy_composite(d: &Path) -> Option<Toy> {
    let set0 = load_toy(d, &[], &[LOW.to_vec()], None, "imported").unwrap();
    let (g0, _) = group_of(d, &set0);
    let lines = set0.frame.mask().to_vec();
    let cmp0 = |a: &[i32], b: &[i32]| set0.cmp.cmp(a, b);
    let nonlower = |im: &[(Vec<i32>, Q)], key: &[i32]| -> Vec<Vec<i32>> {
        im.iter()
            .filter(|(t, _)| !import_lower(&lines, t, key, &cmp0))
            .map(|(t, _)| t.clone())
            .collect()
    };
    let found = search(&set0, &g0, |im, key| {
        let nl = nonlower(im, key);
        im.len() >= 2
            && nl.len() == 1
            && im.iter().all(|(t, _)| t.as_slice() != key)
            && nl[0].iter().zip(&lines).all(|(&x, &l)| !(!l && x > 0))
    });
    let (key, element, image) = found.into_iter().next()?;
    let t = nonlower(&image, &key).remove(0);
    drop(set0);
    let set = load_toy(
        d,
        &[point_rule(1, &t, &[(LOW.to_vec(), "1".into())])],
        &[LOW.to_vec()],
        None,
        "imported",
    )
    .unwrap();
    let (_, group_digest) = group_of(d, &set);
    let ri = set.rules[0].iter().position(|r| r.label == 1).unwrap();
    let info = set.rules[0][ri].clone();
    let rule = RuleUse {
        table: 0,
        record: ri,
        source: info.source.clone(),
        label: info.label,
        record_sha256: info.record_sha256.clone(),
    };
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let lines = set.frame.mask().to_vec();
    let lower = |k: &[i32], tt: &[i32]| import_lower(&lines, k, tt, &cmp);
    let c = compose_image(
        &QField(&Independent),
        &OneRule {
            term: t.clone(),
            rule,
        },
        &image,
        &key,
        &lower,
        &cmp,
        &ComposeOptions::default(),
    )
    .unwrap()
    .expect("the honest composite over Q");
    Some(Toy {
        rhs: c.rhs.iter().map(|(k, q)| (k.clone(), q_text(q))).collect(),
        key,
        element,
        image,
        t,
        record_sha256: info.record_sha256,
        group_digest,
    })
}

fn verification_line(toy: &Toy, kind: Option<&str>) -> Value {
    let mut v = json!({"schema": COMPOSITE_SCHEMA, "target": kt(&toy.key), "group_digest": toy.group_digest,
        "element": toy.element,
        "image": toy.image.iter().map(|(k, q)| json!([kt(k), q_text(q)])).collect::<Vec<_>>(),
        "replacements": [{"term": kt(&toy.t), "depth": 0, "source": "A", "label": 1,
            "record_sha256": toy.record_sha256}],
        "key_coefficient": null, "dropped_zero": [], "rhs_terms": toy.rhs.len(), "plain": false,
        "producer": "symfix test"});
    if let Some(k) = kind {
        v["kind"] = json!(k);
    }
    v
}

fn certificate_file(
    d: &Path,
    name: &str,
    key: &[i32],
    rhs: &[(Vec<i32>, String)],
    class: &str,
    kernel: &str,
    family: &str,
    end: bool,
) -> Value {
    let p = d.join(name);
    let mut s = json!({"schema": CERT_SCHEMA, "relation_sha256": relation_sha256(key, rhs).unwrap(),
        "key": key, "class": class,
        "verdict": if class == "certified" {"EXACT_IN_SPAN_AT_KEY"} else {"EXACT_NOT_IN_SPAN_AT_KEY"},
        "family_sha256": family, "kernel_sha256": kernel, "group_digest": null})
    .to_string();
    s.push('\n');
    if end {
        s.push_str(&json!({"type":"end","records":1,"request_blake3": "0".repeat(64)}).to_string());
        s.push('\n');
    }
    std::fs::write(&p, &s).unwrap();
    json!({"path": p.display().to_string(), "sha256": sha256(s.as_bytes())})
}

/// The toy composite in a supplied file + its verification line (+ optional certificates): the row of its key.
#[allow(clippy::type_complexity)]
fn composite_row(
    tag: &str,
    rhs_of: impl Fn(&Toy) -> Vec<(Vec<i32>, String)>,
    kind: Option<&str>,
    with_verification: bool,
    certs: Option<(Vec<Value>, Vec<String>)>,
    required: &str,
) -> Option<(
    Result<(RowStatus, Option<Level>, String, Level), String>,
    Toy,
    Vec<(Vec<i32>, String)>,
)> {
    let d = scratch(tag);
    let toy = toy_composite(&d)?;
    let rhs = rhs_of(&toy);
    let s = supplied_table(&d, &[(toy.key.clone(), rhs.clone(), "symmetry-composite")]);
    let mut src = json!({"id":"rf:symmetry-composite","kind":"supplied_relations","class":"symmetry-composite",
        "path": s.display().to_string(), "sha256": sha256(&std::fs::read(&s).unwrap())});
    if with_verification {
        let v = d.join("S.verification.jsonl");
        let text = format!("{}\n", verification_line(&toy, kind));
        std::fs::write(&v, &text).unwrap();
        src["verification"] =
            json!({"path": v.display().to_string(), "sha256": sha256(text.as_bytes())});
    }
    if let Some((files, kernels)) = certs {
        src["kernel_certificates"] = json!({"files": files, "kernels": kernels});
    }
    let set = load_toy(
        &d,
        &[point_rule(1, &toy.t, &[(LOW.to_vec(), "1".into())])],
        &[toy.key.clone()],
        Some(src),
        required,
    );
    let r = set
        .map(|set| row_of(&set, &d, &toy.key))
        .map_err(|e| e.to_string());
    Some((r, toy, rhs))
}

fn served_at(r: &(RowStatus, Option<Level>, String, Level)) -> Option<(Level, String)> {
    (r.0 == RowStatus::Served)
        .then(|| (r.1.unwrap_or(Level::Imported), r.2.clone()))
        .filter(|_| r.1.is_some())
}

#[test]
fn an_honest_composite_is_served_at_imported_by_the_same_backend_only() {
    let Some((r, _, _)) = composite_row("honest", |t| t.rhs.clone(), None, true, None, "imported")
    else {
        eprintln!("no single-non-lower-term image on the toy node: not exercised");
        return;
    };
    let r = r.expect("load");
    assert_eq!(
        served_at(&r),
        Some((Level::Imported, "symmetry_composite_same_backend".into())),
        "{r:?}"
    );
    // at per-key the same row is served BELOW the required level (the per-key spec blocks on it)
    let (r2, _, _) =
        composite_row("honest-pk", |t| t.rhs.clone(), None, true, None, "per-key").unwrap();
    let r2 = r2.expect("load");
    assert_eq!(r2.3, Level::PerKey);
    assert!(r2.1.is_some_and(|l| l < r2.3), "{r2:?}");
}

#[test]
fn a_forged_composite_with_the_honest_line_is_not_served() {
    let Some((r, _, _)) = composite_row(
        "forged",
        |t| {
            let mut f = t.rhs.clone();
            f[0].1 = if f[0].1 == "7" { "8" } else { "7" }.to_string();
            f
        },
        None,
        true,
        None,
        "imported",
    ) else {
        return;
    };
    let r = r.expect("load");
    assert_eq!(r.0, RowStatus::Open, "{r:?}");
    // a duplicated term hiding a missing one (F2) is refused at parse
    let (r, _, _) = composite_row(
        "dup",
        |t| {
            let mut f = t.rhs.clone();
            let last = f.len() - 1;
            f[last] = f[0].clone();
            f
        },
        None,
        true,
        None,
        "imported",
    )
    .unwrap();
    let e = r.expect_err("a repeated rhs key must be refused at load");
    assert!(e.contains("repeated"), "{e}");
}

#[test]
fn a_checked_kernel_certificate_lifts_the_composite_to_per_key() {
    let fam = family_sha256(FAMILY);
    let Some((r, toy, rhs)) =
        composite_row("cert", |t| t.rhs.clone(), None, false, None, "per-key")
    else {
        return;
    };
    // no route at all: open
    assert_eq!(r.expect("load").0, RowStatus::Open);
    let mk = |tag: &str,
              class: &str,
              kernel: &str,
              family: &str,
              end: bool,
              other: bool,
              allowed: &str| {
        let dd = scratch(&format!("certfile-{tag}"));
        let mut rel = rhs.clone();
        if other {
            rel[0].1 = format!("({})*2", rel[0].1);
        }
        let f = certificate_file(
            &dd,
            "results.jsonl",
            &toy.key,
            &rel,
            class,
            kernel,
            family,
            end,
        );
        composite_row(
            &format!("cert-{tag}"),
            |t| t.rhs.clone(),
            None,
            false,
            Some((vec![f], vec![allowed.to_string()])),
            "per-key",
        )
        .unwrap()
        .0
    };
    let ok = mk("ok", "certified", KERNEL, &fam, true, false, KERNEL).expect("load");
    assert_eq!(
        served_at(&ok),
        Some((Level::PerKey, "symmetry_relation_kernel_certified".into())),
        "{ok:?}"
    );
    for (tag, class, kernel, family, other, allowed) in [
        (
            "refused",
            "kernel_refused",
            KERNEL,
            fam.as_str(),
            false,
            KERNEL,
        ),
        (
            "kernel",
            "certified",
            KERNEL,
            fam.as_str(),
            false,
            &"cd".repeat(32)[..],
        ),
        (
            "family",
            "certified",
            KERNEL,
            &"ef".repeat(32)[..],
            false,
            KERNEL,
        ),
        (
            "other-relation",
            "certified",
            KERNEL,
            fam.as_str(),
            true,
            KERNEL,
        ),
    ] {
        let r = mk(tag, class, kernel, family, true, other, allowed).expect("load");
        assert_eq!(r.0, RowStatus::Open, "{tag}: {r:?}");
    }
    // an unsealed certificate file (no end record: e.g. a journal) refuses the load
    let e = mk("unsealed", "certified", KERNEL, &fam, false, false, KERNEL)
        .expect_err("an unsealed certificate stream must refuse the load");
    assert!(e.contains("sealed"), "{e}");
}

#[test]
fn a_two_row_line_needs_a_kernel_certificate() {
    let Some((r, toy, rhs)) = composite_row(
        "tworow",
        |t| t.rhs.clone(),
        Some("two_row"),
        true,
        None,
        "imported",
    ) else {
        return;
    };
    assert_eq!(r.expect("load").0, RowStatus::Open);
    let dd = scratch("tworow-cert");
    let f = certificate_file(
        &dd,
        "results.jsonl",
        &toy.key,
        &rhs,
        "certified",
        KERNEL,
        &family_sha256(FAMILY),
        true,
    );
    let (r, _, _) = composite_row(
        "tworow-c",
        |t| t.rhs.clone(),
        Some("two_row"),
        true,
        Some((vec![f], vec![KERNEL.to_string()])),
        "imported",
    )
    .unwrap();
    let r = r.expect("load");
    assert_eq!(
        served_at(&r),
        Some((Level::PerKey, "symmetry_relation_kernel_certified".into())),
        "{r:?}"
    );
}

#[test]
fn a_plain_relation_with_a_verification_file_is_rederived_per_key() {
    let d = scratch("plain");
    let set0 = load_toy(&d, &[], &[LOW.to_vec()], None, "imported").unwrap();
    let (g, _) = group_of(&d, &set0);
    let lines = set0.frame.mask().to_vec();
    let cmp = |a: &[i32], b: &[i32]| set0.cmp.cmp(a, b);
    let found = search(&set0, &g, |im, key| {
        im.len() >= 2 && im.iter().all(|(t, _)| import_lower(&lines, t, key, &cmp))
    });
    let Some((key, _, image)) = found.into_iter().next() else {
        return;
    };
    drop(set0);
    let rhs: Vec<(Vec<i32>, String)> = image.iter().map(|(k, q)| (k.clone(), q_text(q))).collect();
    let s = supplied_table(&d, &[(key.clone(), rhs, "symmetry-composite")]);
    let v = d.join("empty.verification.jsonl");
    std::fs::write(&v, b"").unwrap();
    let src = json!({"id":"rf","kind":"supplied_relations","class":"symmetry-composite",
        "path": s.display().to_string(), "sha256": sha256(&std::fs::read(&s).unwrap()),
        "verification": {"path": v.display().to_string(), "sha256": sha256(b"")}});
    let set = load_toy(&d, &[], &[key.clone()], Some(src), "per-key").unwrap();
    let r = row_of(&set, &d, &key);
    assert_eq!(
        served_at(&r),
        Some((Level::PerKey, "symmetry_relation".into())),
        "{r:?}"
    );
}

#[test]
fn the_relation_digest_is_order_free_and_refuses_repeats() {
    let a = vec![
        (vec![1, 1, 1, 0, 0], "2".to_string()),
        (vec![1, 0, 1, 0, 0], "-1/2".to_string()),
    ];
    let b = vec![a[1].clone(), a[0].clone()];
    let k = [2, 1, 1, 0, 0];
    assert_eq!(relation_sha256(&k, &a), relation_sha256(&k, &b));
    assert_eq!(
        relation_canonical_text(&k, &a).unwrap(),
        r#"{"key":[2,1,1,0,0],"rhs":[[[1,0,1,0,0],"-1/2"],[[1,1,1,0,0],"2"]],"schema":"rustred.campaign.symrows-relation.v1"}"#
    );
    assert!(relation_sha256(&k, &[a[0].clone(), a[0].clone()]).is_err());
}
