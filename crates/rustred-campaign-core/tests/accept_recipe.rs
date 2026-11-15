//! RECIPE RULES (R48): certification from per-key solve answers and assembly by form. The engine verb that solves a
//! recipe at keys does not exist yet: the answers here are SYNTHETIC (plumbing only), built from the M2 witness'
//! right-hand side instantiated at each key (closure instantiation code); every check of the chain is exercised.

mod accept_common;

use accept_common::*;
use rustred_campaign_core::accept::descent::Instantiator;
use rustred_campaign_core::accept::recipe::{
    Determinism, OpenReason, Recipe, RecipeInput, RecipeVerdict, SolveAnswers, accept_recipe,
};
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::accept::{NodeTarget, Position, PriorityClass, RequiredEvidence};
use rustred_campaign_core::assemble::recipes::{RecipeToShip, write};
use rustred_campaign_core::cover::Dispatcher;
use rustred_campaign_core::key::Key;
use serde_json::{Value, json};

const BIN: &str = "sha256:engine-binary-under-test";

fn answer(digest: &str, k: &Key, run: u32, rhs: &[Key], f: impl Fn(&mut Value)) -> String {
    let mut v = json!({"schema": "rustred.campaign.recipe-solve.v1", "recipe_digest": digest, "key": k, "run": run,
        "status": "ok", "failure": null, "rhs_keys": rhs, "relation_digest": format!("sha256:rel-{k:?}"),
        "regular": "regular", "exact_check": "pass", "cost_ms": 7, "binary_sha256": BIN,
        "determinism_contract": "test-contract-v1"});
    f(&mut v);
    format!("{v}\n")
}

#[test]
fn recipe_certification_and_assembly() {
    if !available("TOY2kite") {
        return;
    }
    let node = load_node("TOY2kite");
    let ko_path = ko_table(&node, "K2-single");
    let (_, t) = load_table(&ko_path, "ko");
    let ko = Dispatcher::new(vec![t], vec![0]).unwrap();
    let (_, line) = witness_lines("TOY2kite", "K2-single")[0].clone();
    let w = RuleText::parse(&line, &node.frame).unwrap();
    let recipe = Recipe::from_file(
        &recipe_file(&w.value, &node.frame, 2),
        "sha256:order",
        "test-contract-v1",
        &node.frame,
    )
    .unwrap();
    let gaps = gap_keys(&node, "K2-single");
    // keys: the 20 gap keys (served) + one key the KO table already serves (not dispatched to the recipe)
    let mut keys = gaps.clone();
    let covered = node
        .box_keys
        .iter()
        .find(|k| ko.first_match(k).unwrap().is_covered())
        .unwrap()
        .clone();
    keys.push(covered.clone());
    let inst = Instantiator::new(&w).unwrap();
    let rhs_of =
        |k: &Key| -> Vec<Key> { inst.terms(k).unwrap().into_iter().map(|(t, _)| t).collect() };
    let mk_input = |det| RecipeInput {
        node: view(
            &node,
            NodeTarget::Boxed {
                box_id: "fixture-box".into(),
            },
            RequiredEvidence::PerKey,
        ),
        before: Some(&ko),
        after: None,
        recipe: &recipe,
        keys: &keys,
        determinism: det,
        expected_binary: Some(BIN.into()),
    };
    // 1. no answers: a request for the 20 dispatched keys and the determinism sample
    let mut ans = SolveAnswers::new(recipe.digest());
    let v = accept_recipe(&mk_input(Determinism::Count(3)), &ans).unwrap();
    let RecipeVerdict::NeedsExternal { requests } = &v else {
        panic!("{v:?}")
    };
    assert_eq!(requests[0].keys_file.lines().count(), 20);
    // 2. answers: key 0 solve failed, key 1 rhs not lower, key 2 exact check failed, others fine; run 2 for a sample
    let mut text = String::new();
    for (i, k) in gaps.iter().enumerate() {
        let rhs = if i == 1 {
            vec![vec![k[0] + 1, k[1], k[2], 1, 1]]
        } else {
            rhs_of(k)
        };
        let edit = |v: &mut Value| match i {
            0 => {
                v["status"] = json!("failed");
                v["failure"] = json!({"kind": "no_relation", "text": "synthetic"});
                v["relation_digest"] = Value::Null;
            }
            2 => v["exact_check"] = json!("fail"),
            _ => {}
        };
        text.push_str(&answer(recipe.digest(), k, 1, &rhs, edit));
        text.push_str(&answer(recipe.digest(), k, 2, &rhs, edit));
    }
    ans.load(&text).unwrap();
    let v = accept_recipe(&mk_input(Determinism::Count(3)), &ans).unwrap();
    let RecipeVerdict::Certified {
        certified,
        open,
        cost_ms,
        determinism_checked,
        ..
    } = &v
    else {
        panic!("{v:?}")
    };
    assert_eq!(certified.len(), 17);
    assert_eq!(*cost_ms, 17 * 7);
    assert!(*determinism_checked >= 2);
    assert!(
        open.iter()
            .any(|(k, r)| *k == covered && matches!(r, OpenReason::NotDispatched(_)))
    );
    assert!(
        open.iter()
            .any(|(_, r)| matches!(r, OpenReason::SolveFailed(_)))
    );
    assert!(
        open.iter()
            .any(|(_, r)| matches!(r, OpenReason::NotDescending { .. }))
    );
    assert!(
        open.iter()
            .any(|(_, r)| matches!(r, OpenReason::ExactCheck(_)))
    );
    // 3. assembly by form
    let dir = scratch("recipes");
    let (p, eco) = write(
        "TOY2kite",
        &dir,
        Some(&ko_path),
        &[RecipeToShip {
            recipe: &recipe,
            position: Position {
                class: PriorityClass::FaceOrLocus,
                input_priority: 0,
                round: 1,
                item: "K2".into(),
                index: 0,
            },
            verdict: &v,
        }],
    )
    .unwrap();
    println!("recipes: {} economy {eco:?}", p.display());
    assert_eq!(eco.recipes, 1);
    assert_eq!(eco.recipe_certified_keys, 17);
    assert!(!eco.engine_loadable);
    // 4. determinism mismatch -> Rejected; stale binary -> Undecided
    let mut bad = SolveAnswers::new(recipe.digest());
    let k = &gaps[5];
    let mut t2 = String::new();
    for kk in &gaps {
        t2.push_str(&answer(recipe.digest(), kk, 1, &rhs_of(kk), |_| {}));
        t2.push_str(&answer(recipe.digest(), kk, 2, &rhs_of(kk), |v| {
            v["relation_digest"] = json!("sha256:other")
        }));
    }
    bad.load(&t2).unwrap();
    let v = accept_recipe(&mk_input(Determinism::PerMille(1000)), &bad).unwrap();
    assert!(matches!(v, RecipeVerdict::Rejected { .. }), "{v:?}");
    let mut stale = SolveAnswers::new(recipe.digest());
    let mut t3 = String::new();
    for kk in &gaps {
        t3.push_str(&answer(recipe.digest(), kk, 1, &rhs_of(kk), |v| {
            if kk == k {
                v["binary_sha256"] = json!("sha256:another-binary")
            }
        }));
    }
    stale.load(&t3).unwrap();
    let v = accept_recipe(&mk_input(Determinism::None), &stale).unwrap();
    assert!(
        matches!(&v, RecipeVerdict::Undecided { what } if what.contains("stale")),
        "{v:?}"
    );
    // another determinism contract id: stale as well
    let mut other = SolveAnswers::new(recipe.digest());
    let mut t4 = String::new();
    for kk in &gaps {
        t4.push_str(&answer(recipe.digest(), kk, 1, &rhs_of(kk), |v| {
            v["determinism_contract"] = json!("v2")
        }));
    }
    other.load(&t4).unwrap();
    let v = accept_recipe(&mk_input(Determinism::None), &other).unwrap();
    assert!(
        matches!(&v, RecipeVerdict::Undecided { what } if what.contains("stale")),
        "{v:?}"
    );
    // dispatch semantics: a certified recipe matches only certified keys of its domain
    let RecipeVerdict::Certified { .. } =
        accept_recipe(&mk_input(Determinism::Count(3)), &ans).unwrap()
    else {
        panic!()
    };
}

/// The ENGINE verb's results.jsonl shape (coord-recipe-solve.md v0 s.7; SYNTHETIC records, no verb exists yet).
#[test]
fn engine_results_shape_is_read() {
    if !available("TOY2kite") {
        return;
    }
    let node = load_node("TOY2kite");
    let ko_path = ko_table(&node, "K2-single");
    let (_, t) = load_table(&ko_path, "ko");
    let ko = Dispatcher::new(vec![t], vec![0]).unwrap();
    let (_, line) = witness_lines("TOY2kite", "K2-single")[0].clone();
    let w = RuleText::parse(&line, &node.frame).unwrap();
    let det = "rustred.recipe-solve.v1/elim=rustred.recipe-elim.v1/prime=2305843009213693951/seed=0/backend=sparse-target-only-factorized";
    let recipe = Recipe::from_file(
        &recipe_file(&w.value, &node.frame, 2),
        "sha256:order",
        det,
        &node.frame,
    )
    .unwrap();
    let gaps = gap_keys(&node, "K2-single");
    let inst = Instantiator::new(&w).unwrap();
    let mut text = String::new();
    for (i, k) in gaps.iter().enumerate() {
        let rhs: Vec<Value> = inst
            .terms(k)
            .unwrap()
            .into_iter()
            .map(|(t, _)| {
                json!([
                    format!(
                        "I({})",
                        t.iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    "1"
                ])
            })
            .collect();
        let v = if i == 0 {
            json!({"type": "relation", "ordinal": i, "key": k, "verdict": "NO_RELATION_IN_SEEDS", "determinism_id": det,
                "recipe_digest": "abc", "rhs": null, "relation_digest": null,
                "failure": {"schema": "rustred.recipe-failure.v1", "class": "no_relation_in_seeds", "message": "synthetic"}})
        } else {
            json!({"type": "relation", "ordinal": i, "key": k, "verdict": "SOLVED", "determinism_id": det, "recipe_digest": "abc",
                "target": "I(..)", "rhs": rhs, "relation_digest": format!("{i:064x}"), "proof": {"kind": "exact-weights", "weights": 3, "ok": true},
                "seconds": 0.5})
        };
        text.push_str(&format!("{v}\n"));
    }
    text.push_str("{\"type\":\"end\",\"records\":20}\n");
    let mut ans = SolveAnswers::new(recipe.digest());
    assert_eq!(
        ans.load_engine_results(&text, 1, "abc", "blake3:binary")
            .unwrap(),
        20
    );
    assert!(
        ans.load_engine_results(&text, 1, "other", "blake3:binary")
            .is_err()
    );
    let input = RecipeInput {
        node: view(
            &node,
            NodeTarget::Boxed {
                box_id: "fixture-box".into(),
            },
            RequiredEvidence::PerKey,
        ),
        before: Some(&ko),
        after: None,
        recipe: &recipe,
        keys: &gaps,
        determinism: Determinism::None,
        expected_binary: Some("blake3:binary".into()),
    };
    let v = accept_recipe(&input, &ans).unwrap();
    let RecipeVerdict::Certified {
        certified,
        open,
        cost_ms,
        ..
    } = &v
    else {
        panic!("{v:?}")
    };
    assert_eq!(certified.len(), 19);
    assert_eq!(*cost_ms, 19 * 500);
    assert!(
        matches!(&open[0].1, OpenReason::SolveFailed(m) if m.starts_with("NO_RELATION_IN_SEEDS"))
    );
}
