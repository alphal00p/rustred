//! Lane pieces: the gate and the piece plan on REAL repair-flow output (ignored; data paths come from the
//! environment, nothing node-specific in the code under test).
//!
//! PIECES_GATE_SPECS=a.json:b.json PIECES_OUT=dir  cargo test --release --test pieces_real -- --ignored real_gate
//! PIECES_PLAN=config.json                         cargo test --release --test pieces_real -- --ignored real_piece_plan
//! PIECES_KC=choices.jsonl PIECES_KC_OPEN=open.jsonl PIECES_KC_SPEC=spec.json [PIECES_KC_TABLE=shipped.table.jsonl]
//!   PIECES_OUT=dir                                cargo test --release --test pieces_real -- --ignored real_keychoices
//!
//! Plan config: {"name", "unserved": {"spec": path} | {"keys": path.jsonl}, "candidates": [spec paths],
//! "route0b": [route-0b json paths], "out": path}.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rustred_campaign_core::import::check::{CheckOptions, RowStatus};
use rustred_campaign_core::import::{Evidence, ImportedSet, TableSetSpec};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::pieces::import_edges::{gate_check, shipped_relations};
use rustred_campaign_core::pieces::plan::RefusalReason;
use rustred_campaign_core::pieces::{
    Candidate, GateConfig, NonZero, PlanInput, ServiceKind, Term, UnservedKey, plan,
};
use rustred_campaign_core::worklist::items::Face;
use serde_json::{Value, json};

fn status_kb(field: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn load(spec: &str) -> (ImportedSet, Evidence) {
    let ts = TableSetSpec::from_json(&std::fs::read_to_string(spec).unwrap()).unwrap();
    let base = Path::new("/");
    let set = rustred_campaign_core::import::load(&ts, base).unwrap();
    let ev = Evidence::from_spec(&set, base).unwrap();
    (set, ev)
}

fn check(set: &ImportedSet, ev: &Evidence) -> rustred_campaign_core::import::CheckResult {
    rustred_campaign_core::import::check::check(
        set,
        ev,
        &CheckOptions {
            threads: 4,
            tier: None,
            max_rounds: None,
        },
    )
    .unwrap()
}

fn stem(p: &str) -> String {
    Path::new(p).file_stem().unwrap().to_string_lossy().into_owned()
}

#[test]
#[ignore = "real data: PIECES_GATE_SPECS, PIECES_OUT"]
fn real_gate() {
    let out = PathBuf::from(std::env::var("PIECES_OUT").expect("PIECES_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    for spec in std::env::var("PIECES_GATE_SPECS").expect("PIECES_GATE_SPECS").split(':') {
        let t0 = Instant::now();
        let (set, ev) = load(spec);
        let res = check(&set, &ev);
        let t_check = t0.elapsed().as_secs_f64();
        let mut cfg = if std::env::var("PIECES_GATE_DISK").is_ok() {
            GateConfig::disk(out.join("spill"))
        } else {
            GateConfig::memory()
        };
        cfg.max_listed = 200;
        let (rep, stats) = gate_check(&set, &ev, &res, cfg).unwrap();
        let summary = json!({"spec": spec, "closure_keys": res.summary.keys_total, "open": res.summary.open_total,
            "load_check_seconds": t_check, "verdict": rep.verdict, "relations": rep.relations, "edges": rep.edges,
            "zero_terms_skipped": rep.zero_terms_skipped, "keys": rep.keys, "non_descending": rep.non_descending,
            "non_descending_equal": rep.non_descending_equal, "graph": {"kahn_processed": rep.graph.kahn_processed,
            "residual": rep.graph.residual, "sccs": rep.graph.sccs, "witness_len": rep.graph.witness_cycle.len()},
            "by_provenance": rep.by_provenance, "stats": stats, "seconds_intake": rep.seconds_intake,
            "seconds_graph": rep.seconds_graph, "edges_per_second": rep.edges_per_second,
            "gate_memory_bytes": rep.memory_bytes, "process_peak_kb": status_kb("VmHWM:")});
        println!("{summary}");
        let full = json!({"summary": summary, "report": rep});
        std::fs::write(
            out.join(format!("{}.gate.json", stem(spec))),
            serde_json::to_string_pretty(&full).unwrap(),
        )
        .unwrap();
    }
}

/// Candidates of the unserved keys from one checked spec: every served row's relation, grouped per (key, server).
fn candidates_from(
    spec: &str,
    unserved: &BTreeSet<Vec<i32>>,
    out: &mut BTreeMap<Vec<i32>, Vec<Candidate>>,
) -> (NodeComparator, Vec<bool>, Value) {
    let t0 = Instant::now();
    let (set, ev) = load(spec);
    let res = check(&set, &ev);
    let mut grouped: BTreeMap<(Vec<i32>, String), Vec<Term>> = BTreeMap::new();
    {
        let mut sink = |key: &[i32], prov: &str, terms: &[(Vec<i32>, NonZero)]| -> Result<(), String> {
            if unserved.contains(key) {
                // provenance = class|server|term-class: the candidate is the whole relation (all term classes)
                let server = prov.rsplit_once('|').map(|x| x.0).unwrap_or(prov).to_string();
                let e = grouped.entry((key.to_vec(), server)).or_default();
                e.extend(terms.iter().map(|(k, n)| Term::new(k.clone(), *n)));
            }
            Ok(())
        };
        shipped_relations(&set, &ev, &ev, &res, &mut sink).unwrap();
    }
    let n = grouped.len();
    for ((key, server), terms) in grouped {
        let kind = match server.rsplit('|').next().unwrap_or("") {
            "rule" => ServiceKind::TableRule,
            "recipe" => ServiceKind::CoreRecipe,
            _ => ServiceKind::Composite, // credit (image rule composed), supplied relation, refused relation
        };
        out.entry(key).or_default().push(Candidate {
            kind,
            terms,
            provenance: format!("{}:{server}", stem(spec)),
            regular_at_4: None,
        });
    }
    let info = json!({"spec": spec, "closure_keys": res.summary.keys_total, "open": res.summary.open_total,
        "candidates_for_unserved": n, "seconds": t0.elapsed().as_secs_f64(), "process_peak_kb": status_kb("VmHWM:")});
    println!("{info}");
    (set.cmp.clone(), set.frame.mask().to_vec(), info)
}

#[test]
#[ignore = "real data: PIECES_PLAN"]
fn real_piece_plan() {
    let cfg: Value =
        serde_json::from_str(&std::fs::read_to_string(std::env::var("PIECES_PLAN").expect("PIECES_PLAN")).unwrap())
            .unwrap();
    let mut infos = vec![];
    // the unserved set
    let (mut unserved, mut cmp_lines): (BTreeSet<Vec<i32>>, Option<(NodeComparator, Vec<bool>)>) =
        (BTreeSet::new(), None);
    if let Some(spec) = cfg["unserved"]["spec"].as_str() {
        let (set, ev) = load(spec);
        let res = check(&set, &ev);
        unserved = res
            .rows
            .iter()
            .filter(|r| r.status == RowStatus::Open)
            .map(|r| r.key.clone())
            .collect();
        infos.push(json!({"unserved_from": spec, "closure_keys": res.summary.keys_total, "open": unserved.len()}));
        cmp_lines = Some((set.cmp.clone(), set.frame.mask().to_vec()));
    } else if let Some(p) = cfg["unserved"]["keys"].as_str() {
        for l in std::fs::read_to_string(p).unwrap().lines() {
            if !l.trim().is_empty() {
                unserved.insert(serde_json::from_str(l).unwrap());
            }
        }
        infos.push(json!({"unserved_from": p, "open": unserved.len()}));
    }
    let mut cands: BTreeMap<Vec<i32>, Vec<Candidate>> = BTreeMap::new();
    for s in cfg["candidates"].as_array().cloned().unwrap_or_default() {
        let (c, l, info) = candidates_from(s.as_str().unwrap(), &unserved, &mut cands);
        infos.push(info);
        cmp_lines.get_or_insert((c, l));
    }
    let mut r0b = 0;
    for p in cfg["route0b"].as_array().cloned().unwrap_or_default() {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(p.as_str().unwrap()).unwrap()).unwrap();
        for e in v["first"].as_array().cloned().unwrap_or_default() {
            let key: Vec<i32> = serde_json::from_value(e["key"].clone()).unwrap();
            let rhs: Vec<Vec<i32>> = serde_json::from_value(e["rhs_keys"].clone()).unwrap();
            cands.entry(key).or_default().push(Candidate {
                kind: ServiceKind::Composite,
                terms: rhs.into_iter().map(|k| Term::new(k, NonZero::Yes)).collect(),
                provenance: format!("route0b:element{}", e["best"][0]),
                regular_at_4: None,
            });
            r0b += 1;
        }
    }
    let (cmp, lines) = cmp_lines.expect("a spec to take the order from");
    // a symmetry service (composite) with exactly one edge term onto an unserved key is a hand-down
    let mut keys = vec![];
    for k in &unserved {
        let mut cs = cands.remove(k).unwrap_or_default();
        let extra: Vec<Candidate> = cs
            .iter()
            .filter(|c| c.kind == ServiceKind::Composite)
            .filter(|c| {
                let e: Vec<&Term> = c.terms.iter().filter(|t| t.nonzero != NonZero::Zero).collect();
                e.len() == 1 && unserved.contains(&e[0].key)
            })
            .map(|c| Candidate {
                kind: ServiceKind::HandDown,
                ..c.clone()
            })
            .collect();
        cs.extend(extra);
        keys.push(UnservedKey {
            key: k.clone(),
            face: Face::of_key(k, &lines).map(|f| f.label()).unwrap_or_else(|| "?".into()),
            candidates: cs,
        });
    }
    let t0 = Instant::now();
    let p = plan(&cmp, &PlanInput::new(lines.len(), keys)).unwrap();
    let secs = t0.elapsed().as_secs_f64();
    let mut refused: BTreeMap<String, u64> = BTreeMap::new();
    for r in &p.refused {
        let w = match r.reason {
            RefusalReason::NonDescending => "non_descending",
            RefusalReason::Equal => "equal",
            RefusalReason::HandDownNotOneTerm => "hand_down_not_one_term",
            RefusalReason::KindNotRanked => "kind_not_ranked",
        };
        *refused.entry(format!("{w}|{}", r.provenance)).or_default() += 1;
    }
    let faces: BTreeSet<&str> = p.chosen.iter().map(|c| c.face.as_str()).collect();
    let summary = json!({"name": cfg["name"], "inputs": infos, "route0b_candidates": r0b, "unserved": p.keys,
        "faces": faces.len(), "chosen_by_kind": p.counts_by_kind, "core_open": p.core_open.len(),
        "hand_down_targets": p.hand_down_targets, "edges": p.edges, "pieces": p.pieces.len(),
        "piece_groups": p.piece_order.len(),
        "cyclic_piece_groups": p.piece_order.iter().filter(|g| g.len() > 1).count(),
        "face_level_cyclic": p.face_report.face_level_cyclic, "face_cycles": p.face_report.face_cycles,
        "piece_level_acyclic": p.face_report.piece_level_acyclic, "key_level_acyclic": p.face_report.key_level_acyclic,
        "candidate_cyclic_sccs": p.candidate_cyclic_sccs, "refused": refused, "plan_seconds": secs});
    println!("{summary}");
    if let Some(o) = cfg["out"].as_str() {
        std::fs::write(o, serde_json::to_string_pretty(&json!({"summary": summary, "plan": p})).unwrap()).unwrap();
    }
}

/// `I(1,2,-1)` -> [1, 2, -1].
fn parse_i(t: &str) -> Vec<i32> {
    let t = t.trim();
    let inner = t.strip_prefix("I(").and_then(|x| x.strip_suffix(')')).unwrap_or_else(|| panic!("key text {t}"));
    inner.split(',').map(|x| x.trim().parse().unwrap()).collect()
}

/// The plan from a repair-flow KeyChoices file, and the gate over the point table its composite step shipped (alone,
/// and inside the spec it extends when PIECES_KC_TABLE is given).
#[test]
#[ignore = "real data: PIECES_KC, PIECES_KC_OPEN, PIECES_KC_SPEC, PIECES_OUT"]
fn real_keychoices() {
    use rustred_campaign_core::pieces::keychoices::{
        KeyChoicesOptions, plan_input_from_key_choices, read_key_choices,
    };
    use rustred_campaign_core::pieces::{GateVerdict, ShipGate};
    let out = PathBuf::from(std::env::var("PIECES_OUT").expect("PIECES_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let spec = std::env::var("PIECES_KC_SPEC").expect("PIECES_KC_SPEC");
    let t0 = Instant::now();
    let (set, ev) = load(&spec);
    let t_load = t0.elapsed().as_secs_f64();
    let lines = set.frame.mask().to_vec();
    let recs = read_key_choices(Path::new(&std::env::var("PIECES_KC").expect("PIECES_KC"))).unwrap();
    let mut open: BTreeSet<Vec<i32>> = BTreeSet::new();
    for l in std::fs::read_to_string(std::env::var("PIECES_KC_OPEN").expect("PIECES_KC_OPEN")).unwrap().lines() {
        if !l.trim().is_empty() {
            open.insert(serde_json::from_str(l).unwrap());
        }
    }
    let face_of = |k: &[i32]| Face::of_key(k, &lines).map(|f| f.label()).unwrap_or_else(|| "?".into());
    let inp = plan_input_from_key_choices(lines.len(), &recs, &open, &face_of, &KeyChoicesOptions::default()).unwrap();
    let t1 = Instant::now();
    let p = plan(&set.cmp, &inp).unwrap();
    let t_plan = t1.elapsed().as_secs_f64();
    let mut refused: BTreeMap<String, u64> = BTreeMap::new();
    for r in &p.refused {
        *refused.entry(format!("{:?}", r.reason)).or_default() += 1;
    }
    let core_faces: BTreeMap<String, u64> = p.chosen.iter().filter(|c| c.kind.is_none()).fold(BTreeMap::new(), |mut m, c| {
        *m.entry(c.face.clone()).or_default() += 1;
        m
    });
    let summary = json!({"keychoices": recs.len(), "open": open.len(), "unserved": p.keys,
        "candidates": inp.keys.iter().map(|k| k.candidates.len()).sum::<usize>(),
        "faces": p.chosen.iter().map(|c| c.face.as_str()).collect::<BTreeSet<_>>().len(),
        "chosen_by_kind": p.counts_by_kind, "core_open": p.core_open.len(), "core_open_by_face": core_faces,
        "hand_down_targets": p.hand_down_targets, "edges": p.edges, "pieces": p.pieces.len(),
        "piece_groups": p.piece_order.len(), "cyclic_piece_groups": p.piece_order.iter().filter(|g| g.len() > 1).count(),
        "face_graph": p.face_report.face_graph, "face_cycles": p.face_report.face_cycles,
        "face_level_cyclic": p.face_report.face_level_cyclic, "piece_level_acyclic": p.face_report.piece_level_acyclic,
        "key_level_acyclic": p.face_report.key_level_acyclic, "candidate_cyclic_sccs": p.candidate_cyclic_sccs,
        "refused": refused, "plan_seconds": t_plan, "load_seconds": t_load});
    println!("{summary}");
    std::fs::write(out.join("keychoices.plan.json"), serde_json::to_string_pretty(&json!({"summary": summary, "plan": p})).unwrap()).unwrap();
    let Ok(table) = std::env::var("PIECES_KC_TABLE") else { return };
    // (a) the shipped point table alone
    let mut g = ShipGate::new(&set.cmp, lines.len(), GateConfig::memory()).unwrap();
    for l in std::fs::read_to_string(&table).unwrap().lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        if v["type"] != "rule" {
            continue;
        }
        let key = parse_i(v["target"].as_str().unwrap());
        let terms: Vec<(Vec<i32>, NonZero)> = v["rhs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                let c = t[1].as_str().unwrap().trim();
                (parse_i(t[0].as_str().unwrap()), if c == "0" { NonZero::Zero } else { NonZero::Yes })
            })
            .collect();
        let prov = format!("{}", v["kind"].as_str().unwrap_or("relation"));
        g.push_relation(&key, &prov, terms.iter().map(|(k, n)| (k.as_slice(), *n))).unwrap();
    }
    let rep = g.finish().unwrap();
    let alone = json!({"table": table, "verdict": rep.verdict, "relations": rep.relations, "edges": rep.edges,
        "keys": rep.keys, "non_descending": rep.non_descending, "zero_terms_skipped": rep.zero_terms_skipped,
        "residual": rep.graph.residual, "seconds": rep.seconds_intake + rep.seconds_graph});
    println!("{alone}");
    assert!(matches!(rep.verdict, GateVerdict::Pass | GateVerdict::Fail { .. }), "{:?}", rep.verdict);
    // (b) inside the spec it extends: the spec + a supplied_relations source with the table, checked and gated
    let mut sv: Value = serde_json::from_str(&std::fs::read_to_string(&spec).unwrap()).unwrap();
    sv["sources"].as_array_mut().unwrap().push(json!({"id": "pieces:composite", "kind": "supplied_relations",
        "class": "symmetry-composite", "path": table}));
    let sp = out.join("spec.with-composite.json");
    std::fs::write(&sp, serde_json::to_string_pretty(&sv).unwrap()).unwrap();
    drop((set, ev));
    let r = std::panic::catch_unwind(|| {
        let (set, ev) = load(sp.to_str().unwrap());
        let res = check(&set, &ev);
        let (rep, stats) = gate_check(&set, &ev, &res, GateConfig::memory()).unwrap();
        json!({"closure_keys": res.summary.keys_total, "open": res.summary.open_total, "verdict": rep.verdict,
            "relations": rep.relations, "edges": rep.edges, "keys": rep.keys, "non_descending": rep.non_descending,
            "non_descending_listed": rep.non_descending_listed.iter().take(5).collect::<Vec<_>>(),
            "by_provenance": rep.by_provenance, "residual": rep.graph.residual, "sccs": rep.graph.sccs,
            "stats": stats, "seconds_graph": rep.seconds_graph, "edges_per_second": rep.edges_per_second,
            "process_peak_kb": status_kb("VmHWM:")})
    });
    let in_spec = match r {
        Ok(v) => v,
        Err(e) => json!({"error": format!("{:?}", e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())))}),
    };
    println!("{in_spec}");
    std::fs::write(out.join("keychoices.gate.json"), serde_json::to_string_pretty(&json!({"alone": alone, "in_spec": in_spec})).unwrap()).unwrap();
}
