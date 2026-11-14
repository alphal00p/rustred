//! Agreement of the int3 glue conversions (src/glue.rs) on the frozen acceptance fixtures (TOY2kite):
//! (1) acceptance's recipe served-set computation (composed first match + `RecipeEntry::matches`) and the closure's
//!     dispatch over the converted relation set give the same answer at every key the closure reaches (coordinator
//!     ruling: a recipe entry matches only in its domain AND certified, else the dispatch falls through);
//! (2) the acceptance witness status mapped to the core's equals the core loader's status where both rules apply, and
//!     the documented divergence (pre-guard record) and the "..." loci case (both fail closed) are pinned.
//! Skipped (pass) when the fixture tree is absent, like the other acceptance fixture tests.

mod accept_common;

use accept_common::*;
use rustred_campaign_core::accept::descent::Instantiator;
use rustred_campaign_core::accept::recipe::{
    Determinism, Recipe, RecipeEntry, RecipeInput, RecipeVerdict, SolveAnswers, accept_recipe,
};
use rustred_campaign_core::accept::rule::{RuleText, sha256_tag};
use rustred_campaign_core::accept::{NodeTarget, RequiredEvidence, Stop, composed_stop_domain};
use rustred_campaign_core::closure::{
    ClosureConfig, DispatchSlot, HandDowns, KeyCodec, KeySet, NoFacts, RuleBodies, TerminalSets,
    Tier, closure,
};
use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table, WitnessStatus};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::glue::{
    GlueError, core_witness_status, relation_set_from_accepted_recipe,
};
use rustred_campaign_core::key::Key;
use rustred_campaign_core::record::{ReadOptions, RecordReader, RhsMode, TableFile, TableLine};
use serde_json::{Value, json};

const BIN: &str = "sha256:engine-binary-under-test";

fn answer(digest: &str, k: &Key, rhs: &[Key], f: impl Fn(&mut Value)) -> String {
    // a well-formed 64-hex relation digest per key (the closure stores 32 bytes)
    let rel = sha256_tag(format!("rel-{k:?}").as_bytes());
    let mut v = json!({"schema": "rustred.campaign.recipe-solve.v1", "recipe_digest": digest, "key": k, "run": 1,
        "status": "ok", "failure": null, "rhs_keys": rhs, "relation_digest": rel,
        "regular": "regular", "exact_check": "pass", "cost_ms": 7, "binary_sha256": BIN,
        "determinism_contract": "test-contract-v1"});
    f(&mut v);
    format!("{v}\n")
}

#[test]
fn accepted_recipe_served_set_equals_closure_dispatch() {
    if !available("TOY2kite") {
        return;
    }
    let node = load_node("TOY2kite");
    let ko_path = ko_table(&node, "K2-single");
    let (tf, t) = load_table(&ko_path, "ko");
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
    let mut keys = gaps.clone();
    keys.push(
        node.box_keys
            .iter()
            .find(|k| ko.first_match(k).unwrap().is_covered())
            .unwrap()
            .clone(),
    );
    let inst = Instantiator::new(&w).unwrap();
    let rhs_of =
        |k: &Key| -> Vec<Key> { inst.terms(k).unwrap().into_iter().map(|(t, _)| t).collect() };
    // key 0 solve failed, key 2 exact check failed: in the domain, NOT certified (must fall through)
    let mut text = String::new();
    for (i, k) in gaps.iter().enumerate() {
        text.push_str(&answer(recipe.digest(), k, &rhs_of(k), |v| match i {
            0 => {
                v["status"] = json!("failed");
                v["relation_digest"] = Value::Null;
            }
            2 => v["exact_check"] = json!("fail"),
            _ => {}
        }));
    }
    let mut ans = SolveAnswers::new(recipe.digest());
    ans.load(&text).unwrap();
    let nv = || {
        view(
            &node,
            NodeTarget::Boxed {
                box_id: "fixture-box".into(),
            },
            RequiredEvidence::PerKey,
        )
    };
    let v = accept_recipe(
        &RecipeInput {
            node: nv(),
            before: Some(&ko),
            after: None,
            recipe: &recipe,
            keys: &keys,
            determinism: Determinism::None,
            expected_binary: Some(BIN.into()),
        },
        &ans,
    )
    .unwrap();
    let RecipeVerdict::Certified { certified, .. } = &v else {
        panic!("{v:?}")
    };
    assert!(certified.len() >= 2 && certified.len() < gaps.len());
    let codec = KeyCodec::new(node.frame.arity(), 8).unwrap();
    // acceptance composes (ko, recipe): nothing after -> the set goes last
    let set =
        relation_set_from_accepted_recipe("rec", &recipe, &v, &ans, codec, DispatchSlot::Last)
            .unwrap();
    assert_eq!(set.relations.len(), certified.len());
    // strictness: other answers than the certified ones are refused
    let mut other = SolveAnswers::new(recipe.digest());
    other
        .load(&text.replace("\"cost_ms\":7", "\"cost_ms\":8"))
        .unwrap();
    assert!(matches!(
        relation_set_from_accepted_recipe("rec", &recipe, &v, &other, codec, DispatchSlot::Last),
        Err(GlueError::RelationSetMismatch { .. })
    ));
    // the closure over the KO table + the relation set, from the recipe keys
    let mut bodies = RuleBodies::new(&ko);
    bodies.load_table_records(&ko, 0, &tf).unwrap();
    let start = KeySet::from_keys(codec, keys.iter().map(|k| k.as_slice())).unwrap();
    let tset = KeySet::from_keys(codec, node.terminals.iter().map(|k| k.as_slice())).unwrap();
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 2,
        hand_downs: HandDowns::Off,
        longest_chain: false,
        zero_sectors: Some(node.zero_dropped.clone()),
        zero_sector_source: Some(node.zero_source.clone()),
        terminals: Some(TerminalSets {
            preferred: None,
            other: Some(tset),
        }),
        relation_sets: vec![std::sync::Arc::new(set)],
        ..Default::default()
    };
    let c = closure(&ko, &bodies, &node.cmp, &start, &cfg, &NoFacts).unwrap();
    let cert: std::collections::BTreeSet<Key> = certified.iter().cloned().collect();
    let entry = RecipeEntry {
        domain: recipe.domain(),
        certified: &cert,
    };
    let (mut n_recipe, mut n_fell_through, mut n) = (0, 0, 0);
    for k in c.keys().iter() {
        let i = c.index_of(&k).unwrap();
        let stop = composed_stop_domain(&nv(), Some(&ko), Some(recipe.domain()), None, &k).unwrap();
        let accept_says = stop == Stop::Served && entry.matches(&k);
        assert_eq!(c.is_recipe_served(i), accept_says, "{k:?}: stop {stop:?}");
        if !accept_says {
            // the table answer agrees as well: covered by ko in both, or by nobody
            let ko_wins = matches!(stop, Stop::Before(Dispatch::Covered(_)));
            assert_eq!(
                c.winner(i).is_some_and(|r| r.table == "ko"),
                ko_wins,
                "{k:?}: stop {stop:?}"
            );
            if stop == Stop::Served {
                n_fell_through += 1;
            }
        }
        n_recipe += accept_says as usize;
        n += 1;
    }
    println!(
        "glue recipe agreement: {n} closure keys, {n_recipe} recipe-served, {n_fell_through} in the domain and not certified (fell through), certified {}",
        certified.len()
    );
    assert_eq!(n_recipe, certified.len());
    assert!(n_fell_through >= 2);
}

fn edit(line: &[u8], f: impl Fn(&mut Value)) -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(line).unwrap();
    f(&mut v);
    serde_json::to_vec(&v).unwrap()
}

/// The core loader's witness status of one record line (header of the KO table); `None` = the loader refuses it.
fn core_status(header: &TableFile, line: &[u8]) -> Option<WitnessStatus> {
    let rec = match RecordReader::new(line, ReadOptions { rhs: RhsMode::Skip }).next() {
        Some(Ok(TableLine::Rule(r))) => *r,
        other => panic!("{other:?}"),
    };
    let tf = TableFile {
        header: header.header.clone(),
        rules: vec![rec],
        ..Default::default()
    };
    let t = Table::from_table_file("w", "w", &tf, DomainOptions::default()).ok()?;
    Some(t.meta[0].witness)
}

#[test]
fn witness_status_mapping_agrees_with_the_core_loader() {
    if !available("TOY2kite") {
        return;
    }
    let node = load_node("TOY2kite");
    let (tf, _) = load_table(&ko_table(&node, "K2-single"), "ko");
    let (_, line) = witness_lines("TOY2kite", "K2-single")[0].clone();
    let line = edit(&line, |v| {
        v["guard_version"] = json!(2);
        if v.get("witness_loci").is_none_or(Value::is_null) {
            v["witness_loci"] = json!([]);
        }
    });
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("present", line.clone()),
        (
            "absent",
            edit(&line, |v| {
                v.as_object_mut().unwrap().remove("witness_loci");
            }),
        ),
        (
            "elided",
            edit(&line, |v| v["witness_loci"] = json!(["elided"])),
        ),
    ];
    for (name, l) in &cases {
        let rt = RuleText::parse(l, &node.frame).unwrap();
        assert_eq!(
            Some(core_witness_status(&rt)),
            core_status(&tf, l),
            "{name}"
        );
    }
    // DIVERGENCE (listed in the int3 notes): acceptance is stricter than the core loader on a pre-guard record
    let pre_guard = edit(&line, |v| {
        v.as_object_mut().unwrap().remove("guard_version");
    });
    let rt = RuleText::parse(&pre_guard, &node.frame).unwrap();
    assert_eq!(
        (core_witness_status(&rt), core_status(&tf, &pre_guard)),
        (WitnessStatus::NotCarried, Some(WitnessStatus::Complete)),
        "pre-guard record"
    );
    // "..." in the loci: acceptance says Elided, the core loader refuses the record (a pole parse error): both fail
    // closed
    let dots = edit(&line, |v| v["witness_loci"] = json!(["n1-1", "..."]));
    let rt = RuleText::parse(&dots, &node.frame).unwrap();
    assert_eq!(
        (core_witness_status(&rt), core_status(&tf, &dots)),
        (WitnessStatus::Elided, None),
        "\"...\" in the loci"
    );
}
