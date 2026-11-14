//! R48 recipe rules with a scripted recipe solver: per-key acceptance over every credited key, certified set,
//! fall-through outside it, explicit floor slots, form choice, R35 order (full attempt ended first).

mod ladder_common;

use std::collections::BTreeMap;

use ladder_common::root_item;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::repair::{CaseRequest, CaseSolver};
use rustred_campaign_core::repair::{TreeLimits, drive_tree};
use rustred_campaign_core::worklist::items::{
    CaseSpec, EvidenceLevel, Face, FullAttemptEnded, FullAttemptEvidence, Key, NodeParams,
};
use rustred_campaign_core::worklist::ladder::{
    Disposition, ItemState, SolveAnswer, SolveOutcome, Tri,
};
use rustred_campaign_core::worklist::recipe::{
    FormThresholds, KeyRelation, Recipe, RecipeFailure, RecipeRefusal, RuleForm, accept_recipe,
    choose_form, keys_to_materialise, recipe_disposition,
};
use rustred_campaign_core::worklist::seed::{AttemptBudget, SeedSetting};

fn p() -> NodeParams {
    NodeParams::new("R", 6, 3)
}
fn k(a: i32, b: i32) -> Key {
    vec![a, b, 1, 0, 0, 0]
}
fn ended(p: &NodeParams) -> FullAttemptEnded {
    FullAttemptEnded::from_evidence(
        p,
        FullAttemptEvidence {
            node: p.node.clone(),
            cpu_ms: 0,
            fixer_rounds: p.full_attempt.fixer_rounds,
            open: vec!["face".into()],
        },
    )
    .unwrap()
}

/// Scripted recipe verb: descending relation, except (4,2) not descending, (2,4) a pole, (3,3) proof failed,
/// (4,4) no relation; (3,2) is slow.
struct Scripted;
impl CaseSolver for Scripted {
    fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        SolveAnswer {
            outcome: SolveOutcome::NoRelation,
            cpu_ms: 1,
            rows: None,
        }
    }
    fn solve_recipe_at_key(
        &mut self,
        _r: &Recipe,
        key: &[i32],
    ) -> Result<KeyRelation, RecipeFailure> {
        let (a, b) = (key[0], key[1]);
        if (a, b) == (4, 4) {
            return Err(RecipeFailure::NoRelation);
        }
        let mut down = key.to_vec();
        down[0] -= 1;
        let mut up = key.to_vec();
        up[0] += 2;
        Ok(KeyRelation {
            key: key.to_vec(),
            rhs_keys: vec![if (a, b) == (4, 2) { up } else { down }],
            relation_digest: format!("rel{a}{b}"),
            regular: if (a, b) == (2, 4) {
                Tri::Fail
            } else {
                Tri::Pass
            },
            proof_ok: (a, b) != (3, 3),
            determinism_id: Some("det-v1".into()),
            cost_ms: if (a, b) == (3, 2) { 120_000 } else { 10 },
            recipe_digest: Some("blake3:engine".into()),
            binary_digest: Some("blake3:bin".into()),
        })
    }
}

/// Facts are never asked here (the scripted case solver finds no relation).
struct NoFacts;
impl rustred_campaign_core::repair::FactSource for NoFacts {
    fn facts(
        &mut self,
        _r: &rustred_campaign_core::worklist::ladder::CandidateRule,
        _k: &[i32],
    ) -> rustred_campaign_core::worklist::ladder::PointFacts {
        rustred_campaign_core::worklist::ladder::PointFacts {
            regular: Tri::Unknown,
            rhs_top: None,
            exceptions_complete: Tri::Unknown,
            note: None,
        }
    }
}

#[test]
fn recipe_acceptance_per_key_and_fall_through() {
    let mut p = p();
    p.min_evidence = EvidenceLevel::A3;
    let cmp = NodeComparator::legacy(6, None).unwrap();
    let case = CaseSpec::face_case(Face::new(vec![0, 1], vec![]));
    let setting = SeedSetting {
        offsets: vec![BTreeMap::from([(0usize, -1i32)])],
        floor: true,
        ..SeedSetting::target(2)
    };
    let recipe = Recipe::from_setting("toy/face01/x0", &setting, &case, &p, "legacy");
    // explicit floor slots: the face's dotted lines, named
    assert_eq!(recipe.seeds.min_offset, BTreeMap::from([(0, -1), (1, -1)]));
    assert_eq!(recipe.seeds.centres, vec![vec![-1, 0, 0, 0, 0, 0]]);
    assert_eq!(recipe.digest(), recipe.clone().digest());
    // a child with line 0 fixed at 1 is NOT floored on slot 0 (R48 addendum 2)
    let child = case.constrained(&[(0, 1)], &[]).unwrap();
    let r2 = Recipe::from_setting("toy/child", &setting, &child, &p, "legacy");
    assert_eq!(r2.seeds.min_offset, BTreeMap::from([(1, -1)]));
    let keys = vec![
        k(2, 2),
        k(3, 2),
        k(4, 2),
        k(2, 4),
        k(3, 3),
        k(4, 4),
        vec![2, 2, 2, 0, 0, 0],
    ];
    let rule = accept_recipe(&recipe, &keys, &mut Scripted, &cmp, &p, &ended(&p));
    assert_eq!(rule.certified(), vec![k(2, 2), k(3, 2)]);
    assert_eq!(rule.certified_count(), 2);
    assert_eq!(rule.determinism_id(), Some("det-v1"));
    assert_eq!(rule.engine_recipe_digest(), Some("blake3:engine"));
    let class = |key: Key| {
        rule.failed()
            .iter()
            .find(|(x, _)| *x == key)
            .map(|(_, f)| f.clone())
    };
    assert!(matches!(
        class(k(4, 2)),
        Some(RecipeFailure::NotDescending { .. })
    ));
    assert_eq!(class(k(2, 4)), Some(RecipeFailure::Pole));
    assert_eq!(class(k(3, 3)), Some(RecipeFailure::ProofFailed));
    assert_eq!(class(k(4, 4)), Some(RecipeFailure::NoRelation));
    assert_eq!(
        class(vec![2, 2, 2, 0, 0, 0]),
        Some(RecipeFailure::NotInDomain)
    );
    // matches only in domain AND certified: other domain keys fall through (never blocked)
    assert!(rule.matches(&k(2, 2)));
    assert!(!rule.matches(&k(4, 2)) && !rule.matches(&k(5, 5)));
    // materialise the slow key as a point rule
    let t = FormThresholds::default();
    assert!(keys_to_materialise(&rule, &t).contains(&k(3, 2)));
    // item outcome class: only after the item's own cheap symbolic attempt ran (R48 addendum 3 (a))
    let item = root_item(&p, Face::new(vec![0, 1], vec![]), keys.clone());
    let fresh = ItemState::new(item.clone());
    assert_eq!(
        recipe_disposition(&fresh, &recipe, &mut Scripted, &cmp, &p, &ended(&p)).unwrap_err(),
        RecipeRefusal::SymbolicNotTried
    );
    let rep = drive_tree(
        vec![item.clone()],
        &p,
        &cmp,
        &mut Scripted,
        &mut NoFacts,
        &mut ladder_common::ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    let state = rep.items[0].state.clone();
    match recipe_disposition(&state, &recipe, &mut Scripted, &cmp, &p, &ended(&p)) {
        Ok(Disposition::RecipeRule {
            rule,
            uncertified,
            rest,
        }) => {
            assert_eq!(rule.certified_count(), 2);
            assert_eq!(uncertified.len(), state.item.gap_points.len() - 2);
            // every uncertified key goes on with ONE disposition: the rest item (re-audit A m5)
            assert_eq!(rest.expect("rest item").gap_points, uncertified);
        }
        other => panic!("{other:?}"),
    }
    // a node requiring more than per-key evidence cannot accept recipes
    p.min_evidence = EvidenceLevel::A4;
    let rule = accept_recipe(&recipe, &keys, &mut Scripted, &cmp, &p, &ended(&p));
    assert!(rule.certified().is_empty());
}

#[test]
fn default_solver_has_no_recipe_verb_and_form_choice() {
    struct Plain;
    impl CaseSolver for Plain {
        fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            SolveAnswer {
                outcome: SolveOutcome::NoRelation,
                cpu_ms: 1,
                rows: None,
            }
        }
    }
    let p = p();
    let cmp = NodeComparator::legacy(6, None).unwrap();
    let case = CaseSpec::face_case(Face::new(vec![0], vec![]));
    let recipe = Recipe::from_setting("x", &SeedSetting::target(2), &case, &p, "legacy");
    let rule = accept_recipe(
        &recipe,
        &[vec![2, 1, 1, 0, 0, 0]],
        &mut Plain,
        &cmp,
        &p,
        &ended(&p),
    );
    assert!(rule.certified().is_empty());
    assert!(matches!(
        rule.failed()[0].1,
        RecipeFailure::Unsupported { .. }
    ));
    let t = FormThresholds::default();
    assert_eq!(
        choose_form(Some((100, None)), true, &t),
        Some(RuleForm::Symbolic)
    );
    assert_eq!(
        choose_form(Some((1_000_000, None)), true, &t),
        Some(RuleForm::Recipe)
    );
    assert_eq!(
        choose_form(None, true, &t),
        Some(RuleForm::Recipe),
        "heavy-refused lift: the recipe closes"
    );
    assert_eq!(choose_form(None, false, &t), None);
}
