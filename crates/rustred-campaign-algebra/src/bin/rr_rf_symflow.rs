//! `rr_rf_symflow`: the repair flow's SYMMETRY step of a node on Symbolica (lane repairflow; interfaces/repairflow.md
//! 17; proposed contract kind `symmetry-composite`, notes/implementer.repairflow.md "CONTRACT KIND"): composite descent
//! per open key (TRICKS 1 + 3 + 8), every acceptable choice per key, the shipped relations and the interim gate.
//!
//! ```text
//! rr_rf_symflow run --request REQUEST.json --out DIR          (the contract form)
//! rr_rf_symflow --spec IMPORT_SPEC --out DIR [--base DIR] [--keys KEYS.jsonl] [--threads N] [--policy POLICY.json]
//!               [--node NAME] [--limit N]                     (the hand form: builds the same request, unpinned)
//! rr_rf_symflow version
//! ```
//! REQUEST (`rustred.campaign.symmetry-composite.request.v1`): node, import spec / keys / policy as `{path, sha256}`
//! (every given sha256 is verified before use: a mismatch is a refusal), import base, threads, limit. Without `keys` the
//! open (`uncovered`) keys of the spec's own import check are taken.
//! OUT (sealed: each file written as `.part`, then renamed; `SHA256SUMS` over them; `result.json` LAST): `choices.jsonl`
//! (the per-key record of ALL acceptable choices, `composite::KeyChoices`), `keys.jsonl` (per key: every element's
//! outcome), `composite.table.jsonl` (the SHIPPED relations: a `supplied_relations` point table, exact coefficients),
//! `composite.verification.jsonl` (bindings: group digest, element, image, record_sha256 of every rule used),
//! `report.json` (per face, gate). Deterministic: the same request gives byte-identical sealed files for any thread
//! count (results in key order; no clock in them; timings go to the unsealed `timing.json`): resume = re-run.
//! Exit: 0 done (sealed); 2 refusal (usage, input, digest, licence: one `{"verdict":"ERROR","error":..}` line on stdout,
//! nothing sealed). A signal / OOM leaves no `result.json` (the wrapper treats it as a resource failure).
//! The Symbolica licence must be in the environment (`source WS/ops/env.sh`).

#[path = "../compose_sym.rs"]
mod compose_sym;

use compose_sym::{SetCatalogue, SymQd};
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::closure::sha256::Sha256;
use rustred_campaign_core::import::check::{CheckOptions, RowStatus};
use rustred_campaign_core::import::{Evidence, TableSetSpec};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::repairflow::composite::{
    ComposeAlgebra, ComposeOptions, ElementOutcome, KeyChoices, KeyComposites, compose_all,
    import_lower, rows_by_element, shipped_record, verification_record,
};
use rustred_campaign_core::repairflow::policy::PolicyFile;
use rustred_campaign_core::repairflow::symflow;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Schema of a request.
const REQUEST_SCHEMA: &str = "rustred.campaign.symmetry-composite.request.v1";
/// Schema of `result.json`.
const RESULT_SCHEMA: &str = "rustred.campaign.symmetry-composite.result.v1";

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// Read a pinned input: the bytes, refused when a stated sha256 differs.
fn pinned(v: &Value, role: &str) -> Option<(PathBuf, Vec<u8>, String)> {
    if v.is_null() {
        return None;
    }
    let p = PathBuf::from(
        v["path"]
            .as_str()
            .unwrap_or_else(|| die(&format!("{role}: path"))),
    );
    let b = std::fs::read(&p).unwrap_or_else(|e| die(&format!("{role} {}: {e}", p.display())));
    let got = sha256_hex(&b);
    if let Some(want) = v["sha256"].as_str()
        && !want
            .trim_start_matches("sha256:")
            .eq_ignore_ascii_case(&got)
    {
        die(&format!(
            "{role} {}: sha256 {got} differs from the request's {want}",
            p.display()
        ));
    }
    Some((p, b, got))
}

struct Sealer {
    out: PathBuf,
    sums: Vec<(String, String, usize)>,
}

impl Sealer {
    fn write(&mut self, name: &str, bytes: &[u8]) {
        let part = self.out.join(format!("{name}.part"));
        std::fs::write(&part, bytes).unwrap_or_else(|e| die(&format!("{}: {e}", part.display())));
        std::fs::rename(&part, self.out.join(name)).unwrap_or_else(|e| die(&e.to_string()));
        let lines = bytes.iter().filter(|&&c| c == b'\n').count();
        self.sums.push((name.to_string(), sha256_hex(bytes), lines));
    }
}

fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

/// Keys-only relations of a table file (point records: `fixed` + rhs key texts) as key -> rhs keys.
fn extra_relations(path: &str, arity: usize) -> HashMap<Vec<i32>, Vec<Vec<i32>>> {
    let mut m = HashMap::new();
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| die(&format!("{path}: {e}")));
    for l in text.lines() {
        let v: Value = match serde_json::from_str(l) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["type"] != "rule" {
            continue;
        }
        let k: Vec<i32> = serde_json::from_value(v["fixed"].clone()).unwrap_or_default();
        let rhs: Vec<Vec<i32>> = v["rhs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| {
                rustred_campaign_core::import::lines::parse_key_text(t[0].as_str()?, arity)
            })
            .collect();
        if k.len() == arity {
            m.insert(k, rhs);
        }
    }
    m
}

/// `rr_rf_symflow layers --spec S --keys K1 --cap C --out DIR [--threads N] [--witness strict|diagnostic]
/// [--policy P] [--node N] [--extra-relations FILE]... [--max-layers M]`: the LAYERED exploration (TRICKS 10): the box
/// closure is walked once (its served keys are LEAVES); layer k = composites at its keys, then a keys-only bounded walk
/// from them (keys with dots + rank > C are counted, not expanded; earlier layers' served keys are leaves; shipped
/// composites and the extra relations, e.g. solved core relations, serve the keys no table rule serves); its new
/// uncovered keys with dots + rank <= C are layer k+1. Records per layer in DIR/layers.jsonl; DIR/L<k>.choices.jsonl;
/// the shipped composites of every layer in DIR/composite.table.jsonl + verification; the interim gate at the end.
fn run_layers(args: &[String]) {
    let out = PathBuf::from(arg(args, "--out").unwrap_or_else(|| die("--out")));
    let spec_path = arg(args, "--spec").unwrap_or_else(|| die("--spec"));
    let keys_path = arg(args, "--keys").unwrap_or_else(|| die("--keys"));
    let cap: i64 = arg(args, "--cap")
        .map(|s| s.parse().unwrap_or_else(|_| die("--cap")))
        .unwrap_or(10);
    let threads: usize = arg(args, "--threads")
        .map(|s| s.parse().unwrap_or(1))
        .unwrap_or(1)
        .max(1);
    let max_layers: usize = arg(args, "--max-layers")
        .map(|s| s.parse().unwrap_or(20))
        .unwrap_or(20);
    let witness = arg(args, "--witness").unwrap_or_else(|| "strict".into());
    let node = arg(args, "--node");
    let policy: rustred_campaign_core::repairflow::RepairPolicy = match arg(args, "--policy") {
        Some(p) => {
            PolicyFile::parse(&std::fs::read_to_string(&p).unwrap_or_else(|e| die(&e.to_string())))
                .and_then(|f| f.effective(node.as_deref()))
                .unwrap_or_else(|e| die(&e))
        }
        None => Default::default(),
    };
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    let mut sv: Value = serde_json::from_str(
        &std::fs::read_to_string(&spec_path).unwrap_or_else(|e| die(&e.to_string())),
    )
    .unwrap_or_else(|e| die(&e.to_string()));
    match witness.as_str() {
        "strict" => {}
        "diagnostic" => sv["policies"]["witness_check"] = json!("off_diagnostic"),
        w => die(&format!("witness {w:?}")),
    }
    let base = PathBuf::from("/");
    let alg = SymbolicaAlgebra::new().unwrap_or_else(|e| die(&e));
    let t0 = Instant::now();
    let spec = TableSetSpec::from_json(&sv.to_string())
        .unwrap_or_else(|e| die(&format!("import spec: {e}")));
    let set = rustred_campaign_core::import::load(&spec, &base)
        .unwrap_or_else(|e| die(&format!("import load: {e}")));
    let lines = set.frame.mask().to_vec();
    let arity = lines.len();
    let dr = |k: &[i32]| -> i64 {
        k.iter()
            .zip(&lines)
            .map(|(&x, &l)| {
                if l {
                    if x >= 1 { (x - 1) as i64 } else { (-x) as i64 }
                } else {
                    (x.abs()) as i64
                }
            })
            .sum()
    };
    let fpath = symflow::frame_table_path(&set).unwrap_or_else(|| die("frame_from: no table file"));
    let header: Value = {
        use std::io::BufRead;
        let f = std::fs::File::open(&fpath).unwrap_or_else(|e| die(&format!("{fpath}: {e}")));
        let mut l = String::new();
        std::io::BufReader::new(f)
            .read_line(&mut l)
            .unwrap_or_else(|e| die(&e.to_string()));
        serde_json::from_str(l.trim_end()).unwrap_or_else(|e| die(&format!("frame header: {e}")))
    };
    let gs: rustred_campaign_core::import::symmetry::GroupSpec = match &policy.route0.family {
        Some(f) => serde_json::from_value(json!({"family": {"path": f}})),
        None => serde_json::from_value(json!({})),
    }
    .unwrap_or_else(|e| die(&e.to_string()));
    let (group, grep) = rustred_campaign_core::import::symmetry::load_verified_group(
        &gs,
        &base,
        &lines,
        set.cmp.presentation().new_to_old(),
        header["family_source"].as_str(),
    )
    .unwrap_or_else(|e| die(&format!("verified group: {e}")));
    let qd = SymQd::new().unwrap_or_else(|e| die(&e));
    let cat = SetCatalogue::new(&set, alg, policy.composite.max_record_bytes);
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let lower = |k: &[i32], t: &[i32]| import_lower(&lines, k, t, &cmp);
    let opts = ComposeOptions {
        max_depth: policy.composite.max_depth,
        max_terms: policy.composite.max_terms,
        max_replacements: policy.composite.max_replacements,
    };
    let max_row_terms = policy.route0.max_terms;
    // TRICKS 8 / 1x8 (lane tworow): two-row combinations at the keys with no single-element choice, then the composite
    // on the near misses; the family's zero-sector predicate (Lee) as tworow asks
    let fam = rustred_campaign_core::replay_base::family::Family::parse(
        header["family_source"].as_str().unwrap_or(""),
    )
    .ok();
    let zero_fn = |k: &[i32]| -> bool {
        fam.as_ref()
            .is_some_and(|f| f.zero_key(&k.iter().map(|&x| x as i64).collect::<Vec<i64>>()))
    };
    let tpol = rustred_campaign_core::tworow::TwoRowPolicy::default();
    // extra keys-only relations (e.g. solved core relations)
    let mut extra: HashMap<Vec<i32>, Vec<Vec<i32>>> = HashMap::new();
    let mut extra_given = 0usize;
    for (i, a) in args.iter().enumerate() {
        if a == "--extra-relations" {
            let m = extra_relations(
                args.get(i + 1).unwrap_or_else(|| die("--extra-relations")),
                arity,
            );
            extra_given += m.len();
            extra.extend(m);
        }
    }
    let core_extra: std::collections::HashSet<Vec<i32>> = extra.keys().cloned().collect();
    // the box closure: walked once (no bound), its served keys are LEAVES
    let layer1: Vec<Vec<i32>> = std::fs::read_to_string(&keys_path)
        .unwrap_or_else(|e| die(&e.to_string()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| die(&e.to_string())))
        .collect();
    let l1set: std::collections::HashSet<&Vec<i32>> = layer1.iter().collect();
    let box_starts: Vec<Vec<i32>> = set.start.iter().filter(|k| !l1set.contains(k)).collect();
    let empty: std::collections::HashSet<Vec<i32>> = Default::default();
    let bw = symflow::bounded_walk(&set, &box_starts, &HashMap::new(), &empty, &|_| true);
    let mut leaves = bw.served.clone();
    let mut lf =
        std::fs::File::create(out.join("layers.jsonl")).unwrap_or_else(|e| die(&e.to_string()));
    use std::io::Write as _;
    writeln!(lf, "{}", json!({"layer": 0, "what": "box closure (leaves)", "witness": witness, "starts": box_starts.len(),
        "served": bw.served.len(), "uncovered": bw.uncovered.len(), "blocked": bw.blocked.len(),
        "positive_numerator_terms": bw.positive_numerator_terms, "subsector_terms": bw.subsector_terms,
        "extra_relations_given": extra_given, "seconds": t0.elapsed().as_secs_f64(), "vm_hwm_kb": vm_hwm_kb()})).ok();
    let chart = set.frame.mask_string();
    let mut table =
        json!({"type": "header", "schema": "table_dump.v1", "family": header["family"].clone(),
        "producer": "rr_rf_symflow layers", "kind": "symmetry-composite"})
        .to_string();
    table.push('\n');
    let mut ver = String::new();
    let mut accepted: Vec<symflow::KeyService> = vec![];
    let mut layer_keys_seen: std::collections::HashSet<Vec<i32>> = layer1.iter().cloned().collect();
    let mut keys: Vec<Vec<i32>> = layer1.iter().filter(|k| dr(k) <= cap).cloned().collect();
    let mut k_layer = 1usize;
    while !keys.is_empty() && k_layer <= max_layers {
        let tl = Instant::now();
        type R = Result<KeyComposites<compose_sym::Rd>, String>;
        let results: Vec<(Vec<i32>, R)> = {
            let chunk = keys.len().div_ceil(threads).max(1);
            std::thread::scope(|s| {
                let hs: Vec<_> = keys
                    .chunks(chunk)
                    .map(|ks| {
                        let (qd, cat, group) = (&qd, &cat, &group);
                        let (cmp, lower) = (&cmp, &lower);
                        s.spawn(move || {
                            ks.iter()
                                .map(|k| {
                                    let r = rows_by_element(&Independent, group, k, max_row_terms)
                                        .map(|by| {
                                            compose_all(
                                                &Independent,
                                                qd,
                                                cat,
                                                &by,
                                                k,
                                                lower,
                                                cmp,
                                                &opts,
                                            )
                                        });
                                    (k.clone(), r)
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                hs.into_iter()
                    .flat_map(|h| h.join().expect("worker"))
                    .collect()
            })
        };
        let (mut plain, mut comp, mut core_open, mut core_solved, mut over) =
            (0u64, 0u64, 0u64, 0u64, 0u64);
        let mut by_face: BTreeMap<String, [u64; 4]> = BTreeMap::new();
        let mut fc = String::new();
        for (k, r) in &results {
            let ch = match r {
                Err(e) => KeyChoices::failed(k, &lines, e),
                Ok(kc) => KeyChoices::of_composites(k, kc, &lines),
            };
            let e = by_face.entry(ch.face.clone()).or_default();
            match r {
                Err(_) => over += 1,
                Ok(kc) => match kc.best() {
                    Some(b) => {
                        if b.is_plain() {
                            plain += 1;
                            e[0] += 1
                        } else {
                            comp += 1;
                            e[1] += 1
                        }
                        extra.insert(k.clone(), b.rhs.iter().map(|(q, _)| q.clone()).collect());
                        let idx = accepted.len();
                        table.push_str(&shipped_record(&qd, k, b, &chart, idx).to_string());
                        table.push('\n');
                        ver.push_str(
                            &verification_record(
                                &qd,
                                k,
                                b,
                                &grep.digest,
                                "rr_rf_symflow/symbolica",
                            )
                            .to_string(),
                        );
                        ver.push('\n');
                        accepted.push(symflow::KeyService::from_composite(k, b, &ch.face));
                    }
                    None => {
                        if core_extra.contains(k) {
                            core_solved += 1;
                            e[3] += 1
                        } else {
                            core_open += 1;
                            e[2] += 1
                        }
                    }
                },
            }
            fc.push_str(&serde_json::to_string(&ch).expect("json"));
            fc.push('\n');
        }
        // two-row combinations and near-miss composites at the keys with no single-element choice
        let core_keys: Vec<Vec<i32>> = results
            .iter()
            .filter(|(k, r)| matches!(r, Ok(kc) if kc.best().is_none()) && !core_extra.contains(k))
            .map(|(k, _)| k.clone())
            .collect();
        let tr_results: Vec<rustred_campaign_core::tworow::KeyResult> = {
            let chunk = core_keys.len().div_ceil(threads).max(1);
            std::thread::scope(|s| {
                let hs: Vec<_> = core_keys
                    .chunks(chunk)
                    .map(|ks| {
                        let (group, tpol, cmp, zero_fn) = (&group, &tpol, &cmp, &zero_fn);
                        s.spawn(move || {
                            rustred_campaign_core::tworow::two_row_relations(
                                &Independent,
                                ks,
                                group,
                                cmp,
                                tpol,
                                Some(zero_fn as &dyn Fn(&[i32]) -> bool),
                            )
                        })
                    })
                    .collect();
                hs.into_iter()
                    .flat_map(|h| h.join().expect("worker"))
                    .collect()
            })
        };
        let (mut two_row, mut near_comp, mut tr_over) = (0u64, 0u64, 0u64);
        for tr in &tr_results {
            let k = &tr.key;
            let face = rustred_campaign_core::worklist::items::Face::of_key(k, &lines)
                .map(|f| f.label())
                .unwrap_or_default();
            let found: Option<(
                rustred_campaign_core::repairflow::composite::Composite<compose_sym::Rd>,
                Value,
                &str,
            )> = match &tr.outcome {
                rustred_campaign_core::tworow::KeyOutcome::Served { relations }
                    if !relations.is_empty() =>
                {
                    let c = &relations[0];
                    let rhs: Vec<(Vec<i32>, compose_sym::Rd)> = c
                        .rhs
                        .iter()
                        .map(|(q, a)| (q.clone(), qd.from_q(*a).unwrap_or_else(|e| die(&e))))
                        .collect();
                    Some((
                        rustred_campaign_core::repairflow::composite::Composite {
                            element: c.rows.first().map(|r| r.spec.element).unwrap_or(0),
                            rhs,
                            image: c.rhs.clone(),
                            replacements: vec![],
                            key_coefficient: None,
                            dropped_zero: vec![],
                        },
                        c.to_json(),
                        "two_row",
                    ))
                }
                rustred_campaign_core::tworow::KeyOutcome::OverCap { .. } => {
                    tr_over += 1;
                    None
                }
                _ => match rustred_campaign_core::tworow::composite::with_catalogue_rule(
                    &qd,
                    &cat,
                    &tr.near_misses,
                    &lower,
                    &cmp,
                    &opts,
                ) {
                    Ok(rustred_campaign_core::tworow::composite::CompositeStep::Accepted {
                        near,
                        composite,
                    }) => Some((composite, near.comb.to_json(), "two_row_composite")),
                    _ => None,
                },
            };
            if let Some((c, comb, kind)) = found {
                if kind == "two_row" {
                    two_row += 1
                } else {
                    near_comp += 1
                }
                core_open = core_open.saturating_sub(1);
                if let Some(e) = by_face.get_mut(&face) {
                    e[2] = e[2].saturating_sub(1);
                    e[1] += 1;
                }
                extra.insert(k.clone(), c.rhs.iter().map(|(q, _)| q.clone()).collect());
                let idx = accepted.len();
                let mut rec = shipped_record(&qd, k, &c, &chart, idx);
                rec["kind"] = json!(format!("symmetry-{kind}"));
                table.push_str(&rec.to_string());
                table.push('\n');
                let mut vline =
                    verification_record(&qd, k, &c, &grep.digest, "rr_rf_symflow/symbolica");
                vline["combination"] = comb;
                vline["kind"] = json!(kind);
                ver.push_str(&vline.to_string());
                ver.push('\n');
                let mut ks = symflow::KeyService::from_composite(k, &c, &face);
                ks.kind = kind.into();
                accepted.push(ks);
            }
        }
        std::fs::write(out.join(format!("L{k_layer}.choices.jsonl")), &fc).ok();
        let w = symflow::bounded_walk(&set, &keys, &extra, &leaves, &|q: &[i32]| dr(q) <= cap);
        let mut blocked: BTreeMap<String, u64> = BTreeMap::new();
        for (_, why) in &w.blocked {
            let c = why.split(['{', '(', ' ']).next().unwrap_or("").to_string();
            *blocked.entry(c).or_default() += 1;
        }
        let mut next: Vec<Vec<i32>> = vec![];
        for q in &w.uncovered {
            if dr(q) <= cap && !layer_keys_seen.contains(q) {
                layer_keys_seen.insert(q.clone());
                next.push(q.clone());
            }
        }
        leaves.extend(w.served.iter().cloned());
        writeln!(lf, "{}", json!({"layer": k_layer, "keys": keys.len(), "served_plain": plain, "served_composite": comp,
            "to_core": core_open, "core_solved_given": core_solved, "over_cap": over,
            "two_row": two_row, "two_row_composite": near_comp, "two_row_over_cap": tr_over,
            "walk": {"served_by_table": w.served_by_table, "served_by_extra": w.served_by_extra,
                "uncovered": w.uncovered.len(), "blocked": blocked, "beyond_bound": w.beyond, "leaf_hits": w.leaf_hits,
                "positive_numerator_terms": w.positive_numerator_terms, "subsector_terms": w.subsector_terms,
                "terminals": w.terminals, "no_body": w.no_body},
            "next_layer_keys": next.len(), "leaves_total": leaves.len(),
            "faces": by_face.iter().map(|(f, v)| (f.clone(), json!({"plain": v[0], "composite": v[1], "core": v[2], "core_solved_given": v[3]}))).collect::<serde_json::Map<String, Value>>(),
            "seconds": tl.elapsed().as_secs_f64(), "vm_hwm_kb": vm_hwm_kb()})).ok();
        let nk: String = next
            .iter()
            .map(|q| serde_json::to_string(q).expect("json") + "\n")
            .collect();
        std::fs::write(out.join(format!("L{}.keys.jsonl", k_layer + 1)), nk).ok();
        keys = next;
        k_layer += 1;
    }
    std::fs::write(out.join("composite.table.jsonl"), &table).ok();
    std::fs::write(out.join("composite.verification.jsonl"), &ver).ok();
    let gate = symflow::gate(&accepted, &|a: &[i32], b: &[i32]| lower(a, b));
    writeln!(lf, "{}", json!({"done": true, "layers": k_layer - 1, "shipped": accepted.len(), "gate": gate,
        "group": grep.digest, "witness": witness, "cap": cap, "seconds": t0.elapsed().as_secs_f64(), "vm_hwm_kb": vm_hwm_kb()})).ok();
}

/// wireflow (b): one key's shipped two-row form (the relation as a composite without / with catalogue replacements,
/// the combination it comes from, the verification `kind`).
struct TwoRowShip {
    composite: rustred_campaign_core::repairflow::composite::Composite<compose_sym::Rd>,
    comb: rustred_campaign_core::tworow::Combination,
    kind: &'static str,
}

/// The independent re-check of a two-row form: the combination recomputed by the Symbolica SECOND ROUTE
/// (`tworow_check::second_route`: every row from the verified slot images, compared term by term); a two_row form must
/// equal it and descend; a two_row_composite repeats the catalogue composition of the near miss mod p
/// (`composite_verify::verify_composed_image`, engine primes x three d values) and compares the shipped terms.
#[allow(clippy::too_many_arguments)]
fn check_two_row(
    t: &TwoRowShip,
    k: &[i32],
    group: &rustred_campaign_core::replay_base::symrows::VerifiedGroup,
    cmp: &dyn Fn(&[i32], &[i32]) -> std::cmp::Ordering,
    lower: &dyn Fn(&[i32], &[i32]) -> bool,
    qd: &SymQd,
    mf: &rustred_campaign_core::repairflow::composite_verify::ModField<'_, Independent>,
    mcat: &rustred_campaign_core::repairflow::composite_verify::ModCatalogue<'_, '_, Independent>,
    max_row_terms: usize,
    zero_fn: &dyn Fn(&[i32]) -> bool,
    max_depth: usize,
) -> Result<(), String> {
    let sr = rustred_campaign_algebra::tworow_check::second_route(
        group,
        &t.comb,
        cmp,
        max_row_terms,
        Some(zero_fn),
    )?;
    if !sr.equal {
        return Err(format!("second route differs: {:?}", sr.difference));
    }
    let rhs: Vec<(Vec<i32>, String)> =
        t.composite.rhs.iter().map(|(q, v)| (q.clone(), qd.text(v))).collect();
    if t.kind == "two_row" {
        if sr.not_lower.is_some() || t.composite.rhs.iter().any(|(q, _)| !lower(q, k)) {
            return Err("a two_row term is not lower than the key".into());
        }
        return Ok(());
    }
    let records: Vec<(Vec<i32>, String)> = t
        .composite
        .replacements
        .iter()
        .map(|x| (x.term.clone(), x.rule.record_sha256.clone()))
        .collect();
    let depth = t
        .composite
        .replacements
        .iter()
        .map(|x| x.depth + 1)
        .max()
        .unwrap_or(0)
        .max(max_depth);
    rustred_campaign_core::repairflow::composite_verify::verify_composed_image(
        mf, mcat, k, &t.comb.rhs, &records, &rhs, lower, cmp, depth,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("version") {
        println!(
            "{}",
            json!({"tool": "rr_rf_symflow", "request_schema": REQUEST_SCHEMA, "result_schema": RESULT_SCHEMA})
        );
        return;
    }
    if args.get(1).map(String::as_str) == Some("layers") {
        run_layers(&args);
        return;
    }
    if args.get(1).map(String::as_str) == Some("regions") {
        // post-pass on an earlier run: rr_rf_symflow regions --choices FILE --out FILE [--policy P] [--node N]
        let cf = arg(&args, "--choices").unwrap_or_else(|| die("--choices"));
        let recs: Vec<KeyChoices> = std::fs::read_to_string(&cf)
            .unwrap_or_else(|e| die(&format!("{cf}: {e}")))
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| die(&format!("{cf}: {e}"))))
            .collect();
        let pol = match arg(&args, "--policy") {
            Some(p) => PolicyFile::parse(
                &std::fs::read_to_string(&p).unwrap_or_else(|e| die(&e.to_string())),
            )
            .and_then(|f| f.effective(arg(&args, "--node").as_deref()))
            .unwrap_or_else(|e| die(&e)),
            None => Default::default(),
        };
        let v = symflow::fit_face_regions(&recs, &pol.regions);
        let o = arg(&args, "--out").unwrap_or_else(|| die("--out"));
        std::fs::write(&o, serde_json::to_string_pretty(&v).expect("json") + "\n")
            .unwrap_or_else(|e| die(&e.to_string()));
        return;
    }
    let out = PathBuf::from(arg(&args, "--out").unwrap_or_else(|| die("--out")));
    // the request: given (contract form) or built from the flags (hand form, inputs unpinned)
    let (request, request_bytes): (Value, Vec<u8>) = if args.get(1).map(String::as_str)
        == Some("run")
    {
        let p = arg(&args, "--request").unwrap_or_else(|| die("--request"));
        let b = std::fs::read(&p).unwrap_or_else(|e| die(&format!("{p}: {e}")));
        let v: Value = serde_json::from_slice(&b).unwrap_or_else(|e| die(&format!("{p}: {e}")));
        (v, b)
    } else {
        let file = |n: &str| arg(&args, n).map(|p| json!({"path": p}));
        let v = json!({"schema": REQUEST_SCHEMA, "node": arg(&args, "--node"),
                "import_spec": file("--spec").unwrap_or_else(|| die("--spec")),
                "import_base": arg(&args, "--base").unwrap_or_else(|| "/".into()),
                "keys": file("--keys"), "policy": file("--policy"),
                "threads": arg(&args, "--threads").map(|s| s.parse::<u64>().unwrap_or_else(|_| die("--threads"))),
                "limit": arg(&args, "--limit").map(|s| s.parse::<u64>().unwrap_or_else(|_| die("--limit"))),
                "witness": arg(&args, "--witness"), "round": arg(&args, "--round")});
        let b = serde_json::to_vec_pretty(&v).expect("json");
        (v, b)
    };
    if request["schema"] != REQUEST_SCHEMA {
        die(&format!("request schema must be {REQUEST_SCHEMA}"));
    }
    let node = request["node"].as_str().map(str::to_string);
    let threads = request["threads"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = request["limit"].as_u64().map(|n| n as usize);
    let base = PathBuf::from(request["import_base"].as_str().unwrap_or("/"));
    let (spec_path, spec_bytes, spec_sha) =
        pinned(&request["import_spec"], "import_spec").unwrap_or_else(|| die("import_spec"));
    let keys_in = pinned(&request["keys"], "keys");
    let policy_in = pinned(&request["policy"], "policy");
    let policy = match &policy_in {
        Some((p, b, _)) => PolicyFile::parse(&String::from_utf8_lossy(b))
            .and_then(|f| f.effective(node.as_deref()))
            .unwrap_or_else(|e| die(&format!("policy {}: {e}", p.display()))),
        None => Default::default(),
    };
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let _ = std::fs::remove_file(out.join("result.json"));
    let alg = SymbolicaAlgebra::new().unwrap_or_else(|e| die(&e));
    let t0 = Instant::now();
    // `witness`: "strict" (default) or "diagnostic" (EXPLORATION: the import's witness_check off_diagnostic: records with
    // elided / absent witness loci serve without their per-key proofs; every record of the run is labelled diagnostic,
    // nothing is certified)
    let witness = request["witness"].as_str().unwrap_or("strict").to_string();
    let spec_bytes: Vec<u8> = match witness.as_str() {
        "strict" => spec_bytes,
        "diagnostic" => {
            let mut v: Value =
                serde_json::from_slice(&spec_bytes).unwrap_or_else(|e| die(&e.to_string()));
            v["policies"]["witness_check"] = json!("off_diagnostic");
            serde_json::to_vec_pretty(&v).expect("json")
        }
        w => die(&format!("witness {w:?}: strict | diagnostic")),
    };
    let round_tag = request["round"].as_str().unwrap_or("r1").to_string();
    let spec = TableSetSpec::from_json(&String::from_utf8_lossy(&spec_bytes))
        .unwrap_or_else(|e| die(&format!("import spec: {e}")));
    let set = rustred_campaign_core::import::load(&spec, &base)
        .unwrap_or_else(|e| die(&format!("import load: {e}")));
    let t_load = t0.elapsed().as_secs_f64();
    let lines = set.frame.mask().to_vec();
    // the open keys: given, or the import check's `uncovered` keys (check order)
    let (keys, check_info) = match &keys_in {
        Some((p, b, _)) => {
            let ks: Vec<Vec<i32>> = String::from_utf8_lossy(b)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    serde_json::from_str(l)
                        .unwrap_or_else(|e| die(&format!("{}: {e}", p.display())))
                })
                .collect();
            (ks, json!({"keys_file": p}))
        }
        None => {
            let ev =
                Evidence::from_spec(&set, &base).unwrap_or_else(|e| die(&format!("evidence: {e}")));
            let rows = out.join("check.rows");
            let res = rustred_campaign_core::import::check::check_streamed(
                &set,
                &ev,
                &CheckOptions {
                    threads,
                    tier: None,
                    max_rounds: None,
                },
                &rows,
            )
            .unwrap_or_else(|e| die(&format!("import check: {e}")));
            let _ = std::fs::remove_file(&rows);
            let mut open = vec![];
            let mut other: BTreeMap<String, u64> = BTreeMap::new();
            for r in &res.rows {
                if r.status != RowStatus::Open {
                    continue;
                }
                let s = r.open_status.clone().unwrap_or_else(|| "open".into());
                if s == "uncovered" {
                    open.push(r.key.clone());
                } else {
                    *other.entry(s).or_default() += 1;
                }
            }
            (
                open,
                json!({"closure_keys": res.summary.keys_total, "closure_open": res.summary.open_total,
                "open_other": other, "spec_digest": res.spec_digest}),
            )
        }
    };
    let keys: Vec<Vec<i32>> = match limit {
        Some(n) => keys.into_iter().take(n).collect(),
        None => keys,
    };
    let t_check = t0.elapsed().as_secs_f64();
    // the verified group (derived from the frame header's family, or the policy's family file)
    let fpath = symflow::frame_table_path(&set).unwrap_or_else(|| die("frame_from: no table file"));
    let fpath = if Path::new(&fpath).is_absolute() {
        PathBuf::from(&fpath)
    } else {
        base.join(&fpath)
    };
    let header: Value = {
        use std::io::BufRead;
        let f = std::fs::File::open(&fpath)
            .unwrap_or_else(|e| die(&format!("{}: {e}", fpath.display())));
        let mut l = String::new();
        std::io::BufReader::new(f)
            .read_line(&mut l)
            .unwrap_or_else(|e| die(&e.to_string()));
        serde_json::from_str(l.trim_end()).unwrap_or_else(|e| die(&format!("frame header: {e}")))
    };
    let gs: rustred_campaign_core::import::symmetry::GroupSpec = match &policy.route0.family {
        Some(f) => serde_json::from_value(json!({"family": {"path": f}})),
        None => serde_json::from_value(json!({})),
    }
    .unwrap_or_else(|e| die(&e.to_string()));
    let (group, grep) = rustred_campaign_core::import::symmetry::load_verified_group(
        &gs,
        &base,
        &lines,
        set.cmp.presentation().new_to_old(),
        header["family_source"].as_str(),
    )
    .unwrap_or_else(|e| die(&format!("verified group: {e}")));
    let qd = SymQd::new().unwrap_or_else(|e| die(&e));
    let cat = SetCatalogue::new(&set, alg, policy.composite.max_record_bytes);
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let lower = |k: &[i32], t: &[i32]| import_lower(&lines, k, t, &cmp);
    let opts = ComposeOptions {
        max_depth: policy.composite.max_depth,
        max_terms: policy.composite.max_terms,
        max_replacements: policy.composite.max_replacements,
    };
    let max_row_terms = policy.route0.max_terms;
    // compose every key (threads over chunks; results in key order: deterministic)
    type R = Result<KeyComposites<compose_sym::Rd>, String>;
    let results: Vec<(Vec<i32>, R)> = {
        let chunk = keys.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            let hs: Vec<_> =
                keys.chunks(chunk)
                    .map(|ks| {
                        let (qd, cat, group) = (&qd, &cat, &group);
                        let (cmp, lower) = (&cmp, &lower);
                        s.spawn(move || {
                            ks.iter()
                                .map(|k| {
                                    let r = rows_by_element(&Independent, group, k, max_row_terms)
                                        .map(|by| {
                                            compose_all(
                                                &Independent,
                                                qd,
                                                cat,
                                                &by,
                                                k,
                                                lower,
                                                cmp,
                                                &opts,
                                            )
                                        });
                                    (k.clone(), r)
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
            hs.into_iter()
                .flat_map(|h| h.join().expect("worker"))
                .collect()
        })
    };
    // wireflow (b), TRICKS 8 / 1x8: the TWO-ROW step (policy `tworow.enabled`, default off): lane tworow's two-row
    // combinations at the keys with no single-element choice, then the composite with one catalogue rule on the near
    // misses. Shipped with verification `kind` two_row / two_row_composite: the import admits them ONLY through a kernel
    // certificate (audit F8, import.md 21 (b)); re-checked below by the Symbolica second route (tworow_check).
    let fam = rustred_campaign_core::replay_base::family::Family::parse(
        header["family_source"].as_str().unwrap_or(""),
    )
    .ok();
    let zero_fn = |k: &[i32]| -> bool {
        fam.as_ref()
            .is_some_and(|f| f.zero_key(&k.iter().map(|&x| x as i64).collect::<Vec<i64>>()))
    };
    let mut two_rows: BTreeMap<Vec<i32>, TwoRowShip> = BTreeMap::new();
    let mut tworow_summary = Value::Null;
    if policy.tworow.enabled {
        let core_keys: Vec<Vec<i32>> = results
            .iter()
            .filter(|(_, r)| matches!(r, Ok(kc) if kc.best().is_none()))
            .map(|(k, _)| k.clone())
            .collect();
        let tpol = &policy.tworow.policy;
        let tr_results: Vec<rustred_campaign_core::tworow::KeyResult> = {
            let chunk = core_keys.len().div_ceil(threads).max(1);
            std::thread::scope(|s| {
                let hs: Vec<_> = core_keys
                    .chunks(chunk)
                    .map(|ks| {
                        let (group, cmp, zero_fn) = (&group, &cmp, &zero_fn);
                        s.spawn(move || {
                            rustred_campaign_core::tworow::two_row_relations(
                                &Independent,
                                ks,
                                group,
                                cmp,
                                tpol,
                                Some(zero_fn as &dyn Fn(&[i32]) -> bool),
                            )
                        })
                    })
                    .collect();
                hs.into_iter()
                    .flat_map(|h| h.join().expect("worker"))
                    .collect()
            })
        };
        let mut words: BTreeMap<String, u64> = BTreeMap::new();
        for tr in tr_results {
            let found = match &tr.outcome {
                rustred_campaign_core::tworow::KeyOutcome::Served { relations }
                    if !relations.is_empty() =>
                {
                    let c = &relations[0];
                    let rhs: Vec<(Vec<i32>, compose_sym::Rd)> = c
                        .rhs
                        .iter()
                        .map(|(q, a)| (q.clone(), qd.from_q(*a).unwrap_or_else(|e| die(&e))))
                        .collect();
                    Some(TwoRowShip {
                        composite: rustred_campaign_core::repairflow::composite::Composite {
                            element: c.rows.first().map(|r| r.spec.element).unwrap_or(0),
                            rhs,
                            image: c.rhs.clone(),
                            replacements: vec![],
                            key_coefficient: None,
                            dropped_zero: vec![],
                        },
                        comb: c.clone(),
                        kind: "two_row",
                    })
                }
                rustred_campaign_core::tworow::KeyOutcome::OverCap { .. } => None,
                _ => match rustred_campaign_core::tworow::composite::with_catalogue_rule(
                    &qd,
                    &cat,
                    &tr.near_misses,
                    &lower,
                    &cmp,
                    &opts,
                ) {
                    Ok(rustred_campaign_core::tworow::composite::CompositeStep::Accepted {
                        near,
                        composite,
                    }) => Some(TwoRowShip {
                        composite,
                        comb: near.comb.clone(),
                        kind: "two_row_composite",
                    }),
                    _ => None,
                },
            };
            let w = match &found {
                Some(f) => f.kind.to_string(),
                None => tr.outcome.word().to_string(),
            };
            *words.entry(w).or_default() += 1;
            if let Some(f) = found {
                two_rows.insert(tr.key.clone(), f);
            }
        }
        tworow_summary = json!({"keys": core_keys.len(), "outcomes": words});
    }
    let t_compose = t0.elapsed().as_secs_f64();
    // the sealed records
    let chart = set.frame.mask_string();
    let mut fk = String::new();
    let mut fc = String::new();
    let mut table =
        json!({"type": "header", "schema": "table_dump.v1", "family": header["family"].clone(),
        "producer": "rr_rf_symflow", "kind": "symmetry-composite"})
        .to_string();
    table.push('\n');
    let mut ver = String::new();
    let mut accepted: Vec<symflow::KeyService> = vec![];
    let mut all_choices: Vec<KeyChoices> = vec![];
    for (k, r) in &results {
        let line = match r {
            Err(e) => {
                let ch = KeyChoices::failed(k, &lines, e);
                fc.push_str(&serde_json::to_string(&ch).expect("json"));
                fc.push('\n');
                all_choices.push(ch.clone());
                json!({"key": k, "face": ch.face, "status": ch.status})
            }
            Ok(kc) => {
                let ch = KeyChoices::of_composites(k, kc, &lines);
                fc.push_str(&serde_json::to_string(&ch).expect("json"));
                fc.push('\n');
                all_choices.push(ch.clone());
                let outs: Vec<Value> = kc
                    .outcomes
                    .iter()
                    .map(|o| match o {
                        ElementOutcome::Accepted(c) => json!({"element": c.element, "accepted": true,
                            "terms": c.rhs.len(), "replacements": c.replacements.len(),
                            "key_coefficient": c.key_coefficient.is_some(), "dropped_zero": c.dropped_zero.len()}),
                        ElementOutcome::Refused { element, why } => json!({"element": element, "accepted": false,
                            "class": why.class(), "why": format!("{why:?}")}),
                    })
                    .collect();
                let mut best = kc.best().map(|b| json!({"element": b.element,
                    "terms": b.rhs.len(), "plain": b.is_plain(), "replacements": b.replacements.len()}));
                if let Some(b) = kc.best() {
                    let idx = accepted.len();
                    table.push_str(&shipped_record(&qd, k, b, &chart, idx).to_string());
                    table.push('\n');
                    ver.push_str(
                        &verification_record(&qd, k, b, &grep.digest, "rr_rf_symflow/symbolica")
                            .to_string(),
                    );
                    ver.push('\n');
                    accepted.push(symflow::KeyService::from_composite(k, b, &ch.face));
                } else if let Some(t) = two_rows.get(k) {
                    // wireflow (b): the key's two-row form (never plain; kernel route only at the import)
                    let c = &t.composite;
                    let idx = accepted.len();
                    let mut rec = shipped_record(&qd, k, c, &chart, idx);
                    rec["kind"] = json!(format!("symmetry-{}", t.kind));
                    table.push_str(&rec.to_string());
                    table.push('\n');
                    let mut vline =
                        verification_record(&qd, k, c, &grep.digest, "rr_rf_symflow/symbolica");
                    vline["plain"] = json!(false);
                    vline["kind"] = json!(t.kind);
                    vline["combination"] = t.comb.to_json();
                    ver.push_str(&vline.to_string());
                    ver.push('\n');
                    let mut ks = symflow::KeyService::from_composite(k, c, &ch.face);
                    ks.kind = t.kind.into();
                    accepted.push(ks);
                    best = Some(json!({"element": c.element, "terms": c.rhs.len(), "plain": false,
                        "replacements": c.replacements.len(), "kind": t.kind}));
                }
                json!({"key": k, "face": ch.face, "status": "ok", "best": best,
                    "alternatives": kc.accepted().count(), "outcomes": outs})
            }
        };
        fk.push_str(&line.to_string());
        fk.push('\n');
    }
    // independent check of every shipped relation: the image re-derived, the cited records, the composition repeated
    // mod p (engine primes x three d values) with the core's certificate backend (no Symbolica)
    let mf = rustred_campaign_core::repairflow::composite_verify::ModField::engine(
        &Independent,
        &[1_000_003, 7_777_771, 123_456_791],
    );
    let line_of = |t: usize, r: usize| cat.record_line(t, r);
    let mcat = rustred_campaign_core::repairflow::composite_verify::ModCatalogue {
        set: &set,
        line_of: &line_of,
        field: &mf,
    };
    let mut verified = 0usize;
    let mut not_verified: Vec<Value> = vec![];
    for (k, r) in &results {
        let Ok(kc) = r else { continue };
        let Some(b) = kc.best() else {
            if let Some(t) = two_rows.get(k) {
                match check_two_row(t, k, &group, &cmp, &lower, &qd, &mf, &mcat, max_row_terms, &zero_fn, opts.max_depth) {
                    Ok(()) => verified += 1,
                    Err(e) => {
                        if not_verified.len() < 50 {
                            not_verified.push(json!({"key": k, "why": e, "kind": t.kind}));
                        }
                    }
                }
            }
            continue;
        };
        let records: Vec<(Vec<i32>, String)> = b
            .replacements
            .iter()
            .map(|x| (x.term.clone(), x.rule.record_sha256.clone()))
            .collect();
        let rhs: Vec<(Vec<i32>, String)> =
            b.rhs.iter().map(|(q, v)| (q.clone(), qd.text(v))).collect();
        let depth = b
            .replacements
            .iter()
            .map(|x| x.depth + 1)
            .max()
            .unwrap_or(0)
            .max(opts.max_depth);
        match rustred_campaign_core::repairflow::composite_verify::verify_shipped(
            &Independent,
            &mf,
            &mcat,
            &group,
            k,
            b.element,
            &b.image,
            &records,
            &rhs,
            &lower,
            &cmp,
            max_row_terms,
            depth,
        ) {
            Ok(()) => verified += 1,
            Err(e) => {
                if not_verified.len() < 50 {
                    not_verified.push(json!({"key": k, "why": e}));
                }
            }
        }
    }
    let t_verify = t0.elapsed().as_secs_f64();
    let summary = symflow::summarise(
        &results
            .iter()
            .map(|(k, r)| (k.clone(), r.as_ref().map_err(|e| e.clone())))
            .collect::<Vec<_>>(),
        &lines,
    );
    let gate = symflow::gate(&accepted, &|a: &[i32], b: &[i32]| lower(a, b));
    let rep = json!({"schema": "rustred.campaign.repairflow-symflow.v1", "node": node,
        "policy_digest": policy.digest(), "group": {"elements": grep.elements, "digest": grep.digest},
        "check": check_info, "keys": keys.len(), "summary": summary, "gate": gate,
        "row_expansion_cap": max_row_terms, "composite_max_depth": opts.max_depth, "witness": witness,
        "tworow": tworow_summary,
        "independent_check": {"method": "image re-derived from the verified group; cited records = first match; composition repeated mod p (2 engine primes x 3 values of d) with the core's certificate backend; shipped coefficients compared",
            "verified": verified, "shipped": accepted.len(), "not_verified": not_verified}});
    let mut seal = Sealer {
        out: out.clone(),
        sums: vec![],
    };
    seal.write("choices.jsonl", fc.as_bytes());
    seal.write("keys.jsonl", fk.as_bytes());
    seal.write("composite.table.jsonl", table.as_bytes());
    seal.write("composite.verification.jsonl", ver.as_bytes());
    let regions = symflow::fit_face_regions(&all_choices, &policy.regions);
    seal.write(
        "regions.json",
        (serde_json::to_string_pretty(&regions).expect("json") + "\n").as_bytes(),
    );
    seal.write(
        "report.json",
        (serde_json::to_string_pretty(&rep).expect("json") + "\n").as_bytes(),
    );
    let sums: String = seal
        .sums
        .iter()
        .map(|(n, h, _)| format!("{h}  {n}\n"))
        .collect();
    std::fs::write(out.join("SHA256SUMS"), &sums).unwrap_or_else(|e| die(&e.to_string()));
    let exe = std::fs::read("/proc/self/exe")
        .map(|b| sha256_hex(&b))
        .unwrap_or_default();
    let mut inputs = vec![json!({"role": "import_spec", "path": spec_path, "sha256": spec_sha})];
    for (role, i) in [("keys", &keys_in), ("policy", &policy_in)] {
        if let Some((p, _, h)) = i {
            inputs.push(json!({"role": role, "path": p, "sha256": h}));
        }
    }
    let result = json!({"schema": RESULT_SCHEMA, "verdict": "DONE", "request_sha256": sha256_hex(&request_bytes),
        "tool": {"name": "rr_rf_symflow", "sha256": exe}, "inputs": inputs,
        "group": {"digest": grep.digest, "elements": grep.elements}, "policy_digest": policy.digest(),
        "spec_digest": rep["check"]["spec_digest"].clone(), "keys": keys.len(),
        "outputs": seal.sums.iter().map(|(n, h, l)| json!({"file": n, "sha256": h, "lines": l})).collect::<Vec<_>>(),
        "totals": rep["summary"]["totals"].clone(), "gate": rep["gate"]["verdict"].clone(),
        "shipped_relations": accepted.len(), "independently_verified": verified, "witness": witness,
        "round": round_tag});
    let _ = std::fs::write(
        out.join("timing.json"),
        json!({"load_s": t_load, "check_s": t_check - t_load, "compose_s": t_compose - t_check,
            "verify_s": t_verify - t_compose, "threads": threads})
        .to_string(),
    );
    // ROUND CHECK (request field `ship_check`, default true): the spec + the shipped composites as a supplied_relations
    // source WITH its verification (pinned) + the policy's extra evidence (zero-fact slot): import load + check (do the
    // import's own checks accept every shipped relation?), the keys still open (the next round's pool), and lane pieces'
    // ship gate over everything that spec ships (positive-numerator unknowns as their own class). Unsealed: it carries
    // timings; its counts are copied into result.json.
    let mut ship_summary = Value::Null;
    if request["ship_check"].as_bool().unwrap_or(true) && !accepted.is_empty() {
        drop(cat);
        drop(set);
        let tpath = out.join("composite.table.jsonl");
        let vpath = out.join("composite.verification.jsonl");
        let mut sv: Value =
            serde_json::from_slice(&spec_bytes).unwrap_or_else(|e| die(&e.to_string()));
        let tsha = sha256_hex(&std::fs::read(&tpath).unwrap_or_default());
        let vsha = sha256_hex(&std::fs::read(&vpath).unwrap_or_default());
        let sid = format!("rf:symmetry-composite:{round_tag}");
        if let Some(srcs) = sv["sources"].as_array_mut() {
            srcs.retain(|x| x["id"] != sid.as_str());
            let at = srcs
                .iter()
                .position(|x| x["id"] == "default-recipe")
                .unwrap_or(srcs.len());
            // lane symfix (interfaces/import.md 21): the import re-verifies every shipped relation at load
            let mut src = json!({"id": sid, "kind": "supplied_relations", "class": "symmetry-composite",
                "path": tpath, "sha256": tsha, "verification": {"path": vpath, "sha256": vsha}});
            if let Some(f) = &policy.route0.family {
                src["verify"] = json!({"family": {"path": f}});
            }
            srcs.insert(at, src);
        }
        for f in &policy.evidence.facts {
            let facts = sv["evidence"]["facts"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let mut facts = facts;
            facts.push(json!({"path": f.path, "sha256": f.sha256}));
            sv["evidence"]["facts"] = json!(facts);
        }
        let next_text = serde_json::to_string_pretty(&sv).expect("json") + "\n";
        let _ = std::fs::write(out.join("next.spec.json"), &next_text);
        let t1 = Instant::now();
        let spec2 =
            TableSetSpec::from_json(&next_text).unwrap_or_else(|e| die(&format!("next spec: {e}")));
        let set2 = rustred_campaign_core::import::load(&spec2, &base)
            .unwrap_or_else(|e| die(&format!("next spec load: {e}")));
        let ev2 = Evidence::from_spec(&set2, &base)
            .unwrap_or_else(|e| die(&format!("next evidence: {e}")));
        let rows2 = out.join("check2.rows");
        let res2 = rustred_campaign_core::import::check::check_streamed(
            &set2,
            &ev2,
            &CheckOptions {
                threads,
                tier: None,
                max_rounds: None,
            },
            &rows2,
        )
        .unwrap_or_else(|e| die(&format!("next check: {e}")));
        let shipped: std::collections::HashSet<&Vec<i32>> =
            accepted.iter().map(|a| &a.key).collect();
        let round_keys: std::collections::HashSet<&Vec<i32>> = keys.iter().collect();
        let (mut served_shipped, mut open_shipped, mut new_open) =
            (0u64, BTreeMap::<String, u64>::new(), vec![]);
        let mut open_status: BTreeMap<String, u64> = BTreeMap::new();
        for r in &res2.rows {
            let st = r.open_status.clone().unwrap_or_default();
            if shipped.contains(&r.key) {
                if r.status == RowStatus::Open {
                    *open_shipped.entry(st.clone()).or_default() += 1;
                } else {
                    served_shipped += 1;
                }
            }
            if r.status == RowStatus::Open {
                let cls = match (st.as_str(), r.open_rule) {
                    ("blocked", Some((t, rr))) => {
                        format!("blocked:{:?}", set2.rules[t][rr].witness)
                    }
                    _ => st.clone(),
                };
                *open_status.entry(cls).or_default() += 1;
                if st == "uncovered" && !round_keys.contains(&r.key) {
                    new_open.push(r.key.clone());
                }
            }
        }
        let mut nk = String::new();
        for k in &new_open {
            nk.push_str(&serde_json::to_string(k).expect("json"));
            nk.push('\n');
        }
        let _ = std::fs::write(out.join("next_open.keys.jsonl"), nk);
        let tmpg = out.join("gate.spill");
        let _ = std::fs::create_dir_all(&tmpg);
        let gate2 = symflow::ship_gate_split(
            &set2,
            &ev2,
            &res2,
            rustred_campaign_core::pieces::gate::GateConfig::disk(&tmpg),
        );
        let _ = std::fs::remove_dir(&tmpg);
        let _ = std::fs::remove_file(&rows2);
        ship_summary = json!({"closure_keys": res2.summary.keys_total, "closure_open": res2.summary.open_total,
            "open_by_status": open_status, "shipped": accepted.len(), "shipped_served_by_the_import": served_shipped,
            "shipped_left_open_by_status": open_shipped, "new_open_keys": new_open.len(),
            "ship_gate": match &gate2 { Ok(v) => v.clone(), Err(e) => json!({"error": e}) },
            "seconds": t1.elapsed().as_secs_f64()});
        let _ = std::fs::write(
            out.join("ship_check.json"),
            serde_json::to_string_pretty(&ship_summary).expect("json") + "\n",
        );
    }
    let mut result = result;
    if !ship_summary.is_null() {
        result["ship_check"] = json!({"shipped_served_by_the_import": ship_summary["shipped_served_by_the_import"],
            "shipped_left_open_by_status": ship_summary["shipped_left_open_by_status"],
            "new_open_keys": ship_summary["new_open_keys"],
            "gate_verdict": ship_summary["ship_gate"]["gate"]["verdict"],
            "gate_non_descending": ship_summary["ship_gate"]["gate"]["non_descending"],
            "positive_numerator_unknown_terms": ship_summary["ship_gate"]["positive_numerator_unknown"]["terms"]});
    }
    let part = out.join("result.json.part");
    std::fs::write(
        &part,
        serde_json::to_string_pretty(&result).expect("json") + "\n",
    )
    .unwrap_or_else(|e| die(&e.to_string()));
    std::fs::rename(&part, out.join("result.json")).unwrap_or_else(|e| die(&e.to_string()));
    println!(
        "{}",
        json!({"keys": keys.len(), "totals": rep["summary"]["totals"], "gate": rep["gate"]["verdict"]})
    );
}
