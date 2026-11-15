//! Lane import4 (fixes of the audit-credits findings), through the public import API on a toy node: the two-loop sunset
//! (arity 5, lines D1 D2 D3, numerators D4 D5; nothing five-loop specific) with the VERIFIED group derived from the
//! family file.
//!
//! - PROPERTY (M1): over random small nodes, a credited key's evidence level is the LOWER of the credit's own level
//!   and the level at which its image is served, for every kind of image server (table record with or without a bound
//!   per-key proof, lifted elided / witness-free record, supplied relation with a verification record only, diagnostic
//!   keys-only relation, recipe relation with or without its bound proof `Evidence::recipe_proofs`): never above.
//! - m2: an explicit family file with a NON-IDENTITY slot order (header `node_order.slot_permutation`) loads: the
//!   presented family is relabelled D1..Dn and its slot j carries the family's slot new_to_old[j].

use rustred_campaign_core::import;
use rustred_campaign_core::replay_base::family::Family;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use import::{CheckOptions, Evidence, Level, RowStatus, Server, TableSetSpec, check, load};
use rustred_campaign_core::closure::Tier;
use serde_json::{Value, json};

const N: usize = 5;
const MASK: &str = "11100";
const LOW: [i32; 5] = [1, 1, 1, 0, 0];
const FAMILY: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("import4-{tag}-{}", std::process::id()));
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

/// A POINT rule at `key`, body `rhs` (coefficient 1 each), witness loci `witness` (null = not carried).
fn point_rule(index: i64, key: &[i32], rhs: &[&[i32]], witness: Value) -> Value {
    let mut v = json!({"type":"rule","chart":MASK,"index":index,"fixed":key,"equations":[],"target":kt(key),
        "rhs": rhs.iter().map(|k| json!([kt(k), "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":witness});
    if v["witness_loci"].is_null() {
        v.as_object_mut().unwrap().remove("witness_loci");
    }
    v
}

fn write_table(dir: &Path, name: &str, header: &Value, rules: &[Value]) -> PathBuf {
    let p = dir.join(name);
    let mut s = header.to_string();
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

fn plain_header() -> Value {
    json!({"type":"header","schema":"table_dump.v1","family":"S","denominators":N,"mask":MASK,
        "family_source":FAMILY})
}

/// How the orbit's served member is served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Serve {
    /// A point record in table A; witness 0 complete, 1 elided, 2 not carried; per-key proof bound at the key.
    Table { witness: u8, proof: bool },
    /// A relation of the run's relation set.
    Relation,
}

/// The run's relation set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelKind {
    /// Supplied relations with a `verification` record only (served at `imported`, basis `supplied`).
    SuppliedVerification,
    /// Supplied keys-only DIAGNOSTIC relations (no evidence).
    SuppliedDiagnostic,
    /// A recipe with solved certified relations; per-key only with the bound proof when `recipe_proofs` is set.
    Recipe,
}

fn level_word(l: Option<Level>) -> &'static str {
    l.map_or("none", Level::word)
}

/// Lane import4 PROPERTY (audit-credits M1, binding rule): over random small nodes, no credited key's evidence level
/// exceeds the level at which its image is served; it EQUALS min(credit level, image level) (the group is verified:
/// the credit's own level is per-key), with the image level computed independently from the generator's model.
#[test]
fn credited_level_never_exceeds_its_image() {
    let iters: usize = std::env::var("IMPORT4_LEVEL_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let mut rng = Rng(0x1234_5678_9abc_def1);
    // every top key with lines in 1..=4 and zero numerators, by orbit (the sunset group permutes the three lines)
    let mut orbits: BTreeMap<[i32; 3], Vec<[i32; 5]>> = BTreeMap::new();
    for a in 1..=4 {
        for b in 1..=4 {
            for c in 1..=4 {
                let mut s = [a, b, c];
                s.sort();
                orbits.entry(s).or_default().push([a, b, c, 0, 0]);
            }
        }
    }
    let mut stats: BTreeMap<String, u64> = BTreeMap::new();
    let d = scratch("prop");
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    for it in 0..iters {
        let kind = match rng.below(3) {
            0 => RelKind::SuppliedVerification,
            1 => RelKind::SuppliedDiagnostic,
            _ => RelKind::Recipe,
        };
        // one served member per orbit (so a credit's image is that member), LOW is the terminal
        let mut served: BTreeMap<[i32; 5], Serve> = BTreeMap::new();
        for (s, members) in &orbits {
            if *s == [1, 1, 1] {
                continue;
            }
            let m = members[rng.below(members.len() as u64) as usize];
            match rng.below(8) {
                0 | 1 => {}
                2..=4 => {
                    served.insert(
                        m,
                        Serve::Table {
                            witness: rng.below(3) as u8,
                            proof: rng.chance(1, 2),
                        },
                    );
                }
                _ => {
                    served.insert(m, Serve::Relation);
                }
            }
        }
        // table A: the table records (and one unrelated record so the frame source is never empty)
        let mut a_rules = vec![point_rule(0, &[4, 4, 4, 0, 0], &[&LOW], json!([]))];
        let mut label_of: BTreeMap<[i32; 5], i64> = BTreeMap::new();
        for (k, s) in &served {
            if let Serve::Table { witness, .. } = s {
                let w = match witness {
                    0 => json!([]),
                    1 => json!(["elided"]),
                    _ => Value::Null,
                };
                let label = a_rules.len() as i64;
                label_of.insert(*k, label);
                a_rules.push(point_rule(label, k, &[&LOW], w));
            }
        }
        served.remove(&[4, 4, 4, 0, 0]);
        let a = write_table(&d, "A.table.jsonl", &plain_header(), &a_rules);
        let rel_keys: Vec<[i32; 5]> = served
            .iter()
            .filter(|(_, s)| **s == Serve::Relation)
            .map(|(k, _)| *k)
            .collect();
        // start: a random half of all keys (at least one)
        let mut start: Vec<[i32; 5]> = orbits
            .values()
            .flatten()
            .copied()
            .filter(|_| rng.chance(1, 2))
            .collect();
        if start.is_empty() {
            start.push([1, 2, 3, 0, 0]);
        }
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
            json!({"id":"G","kind":"symmetry_credit","family":{"path": fam.display().to_string()}}),
            json!({"id":"T","kind":"terminals","keys":[LOW]}),
        ];
        let mut evidence = json!({});
        let mut recipe_proofs: Option<HashSet<(String, Vec<i32>)>> = None;
        match kind {
            RelKind::SuppliedVerification | RelKind::SuppliedDiagnostic => {
                let s_rules: Vec<Value> = rel_keys
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let mut v = point_rule(i as i64, k, &[&LOW], json!([]));
                        v["dispatch"] = json!("Point");
                        v["kind"] = json!("symmetry-one-term");
                        v
                    })
                    .collect();
                let s = write_table(&d, "S.table.jsonl", &plain_header(), &s_rules);
                let mut src = json!({"id":"S","kind":"supplied_relations","class":"symmetry",
                    "path":s.display().to_string()});
                if kind == RelKind::SuppliedVerification {
                    let ver = d.join("verify.json");
                    std::fs::write(&ver, "{\"proved\":true}\n").unwrap();
                    src["verification"] = json!({"path": ver.display().to_string()});
                } else {
                    src["diagnostic"] = json!(true);
                }
                sources.push(src);
            }
            RelKind::Recipe => {
                let rp = d.join("recipe.json");
                std::fs::write(
                    &rp,
                    json!({"schema":"rustred.recipe.v1","id":"toy/r1","arity":N,"chart":MASK,
                        "domain":{"fixed":[null,null,null,0,0],"equations":[],"exceptions":[]},
                        "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1"]}},
                        "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
                    .to_string(),
                )
                .unwrap();
                let rsha = import::lines::file_sha256(&rp).unwrap();
                let solved: String = rel_keys
                    .iter()
                    .map(|k| {
                        format!("{}\n", json!({"schema":"rustred.campaign.solved-relation.v1","source":"R",
                            "recipe_sha256":rsha,"recipe_digest":"e".repeat(64),"determinism_id":"det1",
                            "binary_sha256":"b".repeat(64),"key":k,"verdict":"SOLVED","relation_digest":"aa".repeat(32),
                            "rhs":[{"key":LOW,"coef_sha256":"0".repeat(64),"mark":"top"}],
                            "proof":{"kind":"exact-weights","ok":true,"weights":3},"regular":true,
                            "results":{"path":"x","sha256":"0".repeat(64),"ordinal":0},"failure":null}))
                    })
                    .collect();
                let f = d.join("solved.jsonl");
                std::fs::write(&f, solved).unwrap();
                sources.push(json!({"id":"R","kind":"recipe","class":"recipe","path":rp.display().to_string(),
                    "sha256":rsha}));
                evidence["solved"] = json!([{"path": f.display().to_string()}]);
                evidence["expected_recipe_binary"] = json!("b".repeat(64));
                // the closing loop's hook: none (as before), or a random subset of the relations proved
                if rng.chance(3, 4) {
                    recipe_proofs = Some(
                        rel_keys
                            .iter()
                            .filter(|_| rng.chance(1, 2))
                            .map(|k| ("R".to_string(), k.to_vec()))
                            .collect(),
                    );
                }
            }
        }
        let mut spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy-sunset","frame_from":"A",
            "policies":{"elided":"lift_with_per_key_proof"},
            "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
            "sources":sources,"evidence":evidence.clone()});
        // the per-key proofs at (record, key) of the table records chosen with a proof: the record shas come from a
        // first load
        let s0 = TableSetSpec::from_json(&spec.to_string()).unwrap();
        let set0 = load(&s0, &d).unwrap();
        let mut sha_at: BTreeMap<i64, String> = BTreeMap::new();
        for r in &set0.rules[0] {
            sha_at.insert(r.label, r.record_sha256.clone());
        }
        let proofs: Vec<Value> = served
            .iter()
            .filter(|(_, s)| matches!(s, Serve::Table { proof: true, .. }))
            .map(|(k, _)| json!({"record_sha256": sha_at[&label_of[k]], "key": k, "status":"EXACT_IN_SPAN_AT_KEY"}))
            .collect();
        if !proofs.is_empty() {
            let p = d.join("proofs.json");
            std::fs::write(
                &p,
                json!({"schema":"rustred.proof-kernel.span.v1","keys":proofs}).to_string(),
            )
            .unwrap();
            evidence["proofs"] = json!([{"path": p.display().to_string()}]);
            spec["evidence"] = evidence;
        }
        let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
        let set = load(&s, &d).unwrap();
        let mut ev = Evidence::from_spec(&set, &d).unwrap();
        ev.recipe_proofs = recipe_proofs.clone();
        // the independent model of the level at which a served member is served
        let image_level = |k: &[i32]| -> Option<Level> {
            let k: [i32; 5] = k.try_into().unwrap();
            match served.get(&k) {
                Some(Serve::Table { proof, .. }) => Some(if *proof {
                    Level::PerKey
                } else {
                    Level::Imported
                }),
                Some(Serve::Relation) => match kind {
                    RelKind::SuppliedVerification => Some(Level::Imported),
                    RelKind::SuppliedDiagnostic => None,
                    RelKind::Recipe => Some(
                        if recipe_proofs
                            .as_ref()
                            .is_none_or(|p| p.contains(&("R".to_string(), k.to_vec())))
                        {
                            Level::PerKey
                        } else {
                            Level::Imported
                        },
                    ),
                },
                None => panic!("iteration {it}: a credit to the unserved key {k:?}"),
            }
        };
        for tier in [Tier::K, Tier::C] {
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
            let by_key: BTreeMap<&[i32], &import::KeyRow> =
                r.rows.iter().map(|x| (x.key.as_slice(), x)).collect();
            for x in &r.rows {
                let Some(im) = &x.image else {
                    assert!(
                        !matches!(x.server, Some(Server::Credit { .. }))
                            && x.basis != Some(import::check::BASIS_CREDIT_VIA_RELATION),
                        "iteration {it}: a credited row without its image: {x:?}"
                    );
                    continue;
                };
                assert_eq!(x.status, RowStatus::Served);
                let il = image_level(im);
                // the binding rule: min(credit level = per-key (verified group), image level); never above the image
                assert_eq!(
                    x.evidence,
                    il.map(|l| l.min(Level::PerKey)),
                    "iteration {it} {tier:?} {kind:?}: key {:?} image {im:?} served {:?}: {x:?}",
                    x.key,
                    served.get(&<[i32; 5]>::try_from(im.as_slice()).unwrap())
                );
                assert!(x.evidence <= il, "credited level above its image's");
                // and never above the importer's own row of the image, when the image is in the closure
                if let Some(y) = by_key.get(im.as_slice())
                    && y.status == RowStatus::Served
                    && y.image.is_none()
                {
                    assert!(
                        x.evidence <= y.evidence,
                        "iteration {it}: credited {x:?} above its image row {y:?}"
                    );
                    *stats.entry("image row compared".into()).or_default() += 1;
                }
                let server = match (
                    &x.server,
                    served.get(&<[i32; 5]>::try_from(im.as_slice()).unwrap()),
                ) {
                    (Some(Server::Credit { .. }), Some(Serve::Table { witness, proof })) => {
                        format!("table w{witness} proof {proof}")
                    }
                    (Some(Server::Relation { .. }), _) => format!(
                        "{kind:?}{}",
                        if kind == RelKind::Recipe {
                            if recipe_proofs.is_some() {
                                " hook"
                            } else {
                                " no hook"
                            }
                        } else {
                            ""
                        }
                    ),
                    (s, m) => panic!("iteration {it}: server {s:?} for an image served as {m:?}"),
                };
                *stats
                    .entry(format!("{tier:?} {server} -> {}", level_word(x.evidence)))
                    .or_default() += 1;
            }
        }
    }
    println!("import4 level property over {iters} nodes:");
    for (k, v) in &stats {
        println!("  {k}: {v}");
    }
    // coverage of every image-server kind and of both outcomes where both exist
    let has = |p: &str| stats.keys().any(|k| k.contains(p));
    for p in [
        "table w0 proof false -> imported",
        "table w0 proof true -> per-key",
        "table w1 proof true -> per-key",
        "table w2 proof true -> per-key",
        "SuppliedVerification -> imported",
        "SuppliedDiagnostic -> none",
        "Recipe hook -> imported",
        "Recipe hook -> per-key",
        "Recipe no hook -> per-key",
    ] {
        assert!(
            has(p),
            "the generator never produced `{p}` (stats {stats:?})"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// m2 (audit-credits, symmetry.rs `presented_family_text`): an explicit family file with a NON-IDENTITY slot order
/// (header `node_order.slot_permutation`) loads; the presented family is relabelled D1..Dn, slot j carrying the family's
/// slot new_to_old[j]; the verified group then credits a key through an image record of the presented frame.
#[test]
fn explicit_family_with_a_non_identity_slot_order_loads() {
    let d = scratch("present");
    let fam = d.join("S.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let n2o = [1usize, 2, 0, 4, 3];
    // the presented text: labels D1..D5 in order, momenta and shifts moved with their slot
    let text = import::symmetry::presented_family_text(FAMILY, &n2o).unwrap();
    let fp = Family::parse(&text).expect("the presented family parses (labels D1..Dn)");
    let fo = Family::parse(FAMILY).unwrap();
    for (j, &o) in n2o.iter().enumerate() {
        assert_eq!(fp.momenta()[j], fo.momenta()[o], "slot {j}");
    }
    assert!(text.contains("prop(D1,(k2)^2,1)") && text.contains("prop(D5,(k1+q)^2,0)"));
    // the frame header presents the slots (no family_source: the explicit family file is the only family)
    let header = json!({"type":"header","schema":"table_dump.v1","family":"S","denominators":N,"mask":MASK,
        "node_order":{"name":"presented","slot_permutation":n2o,"coordinate_priority":null,"source_order":"Input"}});
    let k = [1, 2, 3, 0, 0];
    let image = [3, 2, 1, 0, 0];
    let a = write_table(
        &d,
        "A.table.jsonl",
        &header,
        &[point_rule(0, &image, &[&LOW], json!([]))],
    );
    let st = d.join("start.jsonl");
    std::fs::write(&st, format!("{}\n", json!(k))).unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy-sunset","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","path":a.display().to_string()},
                   {"id":"G","kind":"symmetry_credit","family":{"path": fam.display().to_string()}},
                   {"id":"T","kind":"terminals","keys":[LOW]}]});
    let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&s, &d).expect("an explicit family with a non-identity presentation loads");
    assert!(!set.cmp.presentation().is_identity());
    let g = set
        .report
        .sources
        .iter()
        .find_map(|s| s.group.clone())
        .expect("the verified group");
    assert!(g.presented && g.elements > 1, "{g:?}");
    let ev = Evidence::from_spec(&set, &d).unwrap();
    let r = check(&set, &ev, &CheckOptions::default()).unwrap();
    let x = r.rows.iter().find(|x| x.key == k).unwrap();
    assert_eq!(x.status, RowStatus::Served, "{x:?}");
    assert!(matches!(x.server, Some(Server::Credit { .. })), "{x:?}");
    assert_eq!(x.image.as_deref(), Some(&image[..]));
    assert_eq!(x.evidence, Some(Level::Imported));
    let _ = std::fs::remove_dir_all(&d);
}
