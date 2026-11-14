//! Relation sets (R48 recipe rules) as dispatch entries: a set matches a key iff the key is in its domain AND in its
//! certified set (otherwise the dispatch falls through, coordinator ruling 2026-11-14); the closure follows the
//! recorded right-hand-side keys (descent checked, no facts needed in Tier C), counts (recipe, key) uses, reports
//! recipe candidates of uncovered keys; priority among tables and sets is explicit. Default suite.

use std::sync::Arc;

use rustred_campaign_core::closure::{
    ClosureConfig, DispatchSlot, ExclusionReason, HandDowns, KeyCodec, KeyExclusion, KeySet,
    NoFacts, OpenStatus, RelationSet, RelationStore, RelationStoreBuilder, RuleBodies,
    TerminalSets, Tier, VERDICT_CERTIFIED, VERDICT_REGULAR, Verdict, closure, closure_incremental,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::symmetry::{CreditPolicy, GroupElement};

const CHART: [bool; 4] = [true, true, false, false];

fn dom(fixed: [Option<i32>; 4]) -> CaseDomain {
    let mut d = CaseDomain::free(4, Some(CHART.to_vec()));
    d.fixed = fixed.to_vec();
    d
}
fn cmp() -> NodeComparator {
    NodeComparator::legacy(4, None).unwrap()
}
fn ks(keys: &[[i32; 4]]) -> KeySet {
    KeySet::from_keys(
        KeyCodec::new(4, 6).unwrap(),
        keys.iter().map(|k| k.as_slice()),
    )
    .unwrap()
}
/// One table "A": a free rule lowering line 1 (n1-1); the recipe will lower line 0 instead.
fn node() -> (Dispatcher, RuleBodies) {
    let mut t = Table::new("A", 4);
    t.push(0, dom([None; 4])).unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    b.set(&d, 0, 0, &["I(n0,n1-1,n2,n3)"]).unwrap();
    (d, b)
}
/// Recipe on the face line 0 >= 2: certified at the given keys with rhs = (k0-1, k1) and the subsector (0, k1).
fn recipe(certified: &[[i32; 4]], slot: DispatchSlot, verdict: u8) -> Arc<RelationSet> {
    let mut b = RelationStoreBuilder::new(KeyCodec::new(4, 6).unwrap());
    for k in certified {
        b.push(
            k,
            &[vec![k[0] - 1, k[1], 0, 0], vec![0, k[1], 0, 0]],
            [k[0] as u8; 32],
            verdict,
        )
        .unwrap();
    }
    let mut domain = CaseDomain::free(4, Some(CHART.to_vec()));
    domain.bounds = vec![rustred_campaign_core::poly::Poly::parse_d_free("n0-2", 4).unwrap()];
    Arc::new(RelationSet {
        name: "rec".into(),
        recipe_digest: "sha256:recipe".into(),
        domain,
        relations: b.finish().unwrap(),
        slot,
    })
}
fn cfg(tier: Tier, sets: Vec<Arc<RelationSet>>) -> ClosureConfig {
    ClosureConfig {
        tier,
        threads: 1,
        hand_downs: HandDowns::Keys,
        table_digests: [("A".to_string(), "sha256:A".to_string())].into(),
        relation_sets: sets,
        ..Default::default()
    }
}

#[test]
fn recipe_dispatch_fall_through_priority_and_closure() {
    let (d, b) = node();
    let st = ks(&[[3, 1, 0, 0]]);
    // before table A, certified at (3,1) and (2,1): both served by the recipe: (3,1) -> (2,1) -> (1,1); (1,1) by A
    let r = recipe(
        &[[3, 1, 0, 0], [2, 1, 0, 0]],
        DispatchSlot::BeforeTable("A".into()),
        VERDICT_CERTIFIED,
    );
    let c = closure(
        &d,
        &b,
        &cmp(),
        &st,
        &cfg(Tier::K, vec![r.clone()]),
        &NoFacts,
    )
    .unwrap();
    let s = c.summary();
    assert_eq!(
        (
            s.keys_total,
            s.recipe_served,
            s.descent_violations,
            s.open_total
        ),
        (3, 2, 0, 0)
    );
    let u = &c.report(&Default::default()).used_relations[0];
    assert_eq!((u.set.as_str(), u.keys, u.keys_in_start), ("rec", 2, 1));
    assert!(c.is_recipe_served(0));
    // Tier C: the recipe keys need no facts; the table key (1,1) needs its regularity -> undecided without facts
    let c = closure(
        &d,
        &b,
        &cmp(),
        &st,
        &cfg(Tier::C, vec![r.clone()]),
        &NoFacts,
    )
    .unwrap();
    assert_eq!(c.summary().undecided.regularity_unknown, 1);
    assert_eq!(c.fact_requests().len(), 1);
    // after table A: A matches first everywhere (free rule): the recipe is never used
    let r_last = recipe(
        &[[3, 1, 0, 0], [2, 1, 0, 0]],
        DispatchSlot::Last,
        VERDICT_CERTIFIED,
    );
    let c = closure(&d, &b, &cmp(), &st, &cfg(Tier::K, vec![r_last]), &NoFacts).unwrap();
    assert_eq!(c.summary().recipe_served, 0);
    // certified only at (3,1): (2,1) is in the domain but not certified -> falls through to A
    let r1 = recipe(
        &[[3, 1, 0, 0]],
        DispatchSlot::BeforeTable("A".into()),
        VERDICT_CERTIFIED,
    );
    let c = closure(
        &d,
        &b,
        &cmp(),
        &st,
        &cfg(Tier::K, vec![r1.clone()]),
        &NoFacts,
    )
    .unwrap();
    let i = c.index_of(&[2, 1, 0, 0]).unwrap();
    assert_eq!(c.winner(i).unwrap().table, "A");
    assert_eq!(c.summary().recipe_served, 1);
    // a record whose verdict bits are incomplete: open (recipe_unverified)
    let rv = recipe(
        &[[3, 1, 0, 0]],
        DispatchSlot::BeforeTable("A".into()),
        VERDICT_REGULAR,
    );
    let c = closure(&d, &b, &cmp(), &st, &cfg(Tier::K, vec![rv]), &NoFacts).unwrap();
    assert_eq!(c.open_keys()[0].status, OpenStatus::RecipeUnverified);
    // the identity records the relation sets
    assert_ne!(
        closure(&d, &b, &cmp(), &st, &cfg(Tier::K, vec![]), &NoFacts)
            .unwrap()
            .summary()
            .config_digest,
        closure(&d, &b, &cmp(), &st, &cfg(Tier::K, vec![r1]), &NoFacts)
            .unwrap()
            .summary()
            .config_digest
    );
}

#[test]
fn recipe_candidates_terminals_exclusions_credits() {
    // no table rule at all: uncovered keys in the recipe domain list the recipe as a candidate
    let mut t = Table::new("A", 4);
    t.push(0, dom([Some(9), Some(9), Some(0), Some(0)]))
        .unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    b.set(&d, 0, 0, &["I(1,1,0,0)"]).unwrap();
    let r = recipe(&[[3, 1, 0, 0]], DispatchSlot::Last, VERDICT_CERTIFIED);
    let c = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[3, 1, 0, 0]]),
        &cfg(Tier::K, vec![r.clone()]),
        &NoFacts,
    )
    .unwrap();
    let o = c.open_keys();
    assert_eq!(o.len(), 1);
    assert_eq!(
        (o[0].key.clone(), o[0].recipe_candidates.clone()),
        (vec![2, 1, 0, 0], vec!["rec".to_string()])
    );
    // a declared terminal served by the recipe: terminal_in_rule_domain unless the set is excluded there
    let term = TerminalSets {
        preferred: Some(ks(&[[3, 1, 0, 0]])),
        other: None,
    };
    let c1 = ClosureConfig {
        terminals: Some(term.clone()),
        ..cfg(Tier::K, vec![r.clone()])
    };
    assert_eq!(
        closure(&d, &b, &cmp(), &ks(&[[3, 1, 0, 0]]), &c1, &NoFacts)
            .unwrap()
            .summary()
            .verdict,
        Verdict::TerminalInRuleDomain
    );
    let c2 = ClosureConfig {
        exclusions: vec![KeyExclusion {
            key: vec![3, 1, 0, 0],
            table: "rec".into(),
            record: 0,
            label: 0,
            reason: ExclusionReason::KeepsTerminal,
        }],
        ..c1
    };
    assert_eq!(
        closure(&d, &b, &cmp(), &ks(&[[3, 1, 0, 0]]), &c2, &NoFacts)
            .unwrap()
            .summary()
            .verdict,
        Verdict::SupersetClean
    );
    // a credit whose image is served by a CERTIFIED recipe entry (R44, re-audit F8, lane closurefix): the recorded right
    // side at the image, (2,1,0,0) and the subsector (0,1,0,0), lies below the ORIGINAL key (1,3,0,0): credited; the
    // recorded top key (2,1,0,0) is then reached (uncovered: open)
    let c3 = ClosureConfig {
        credits: Some(CreditPolicy::new(vec![GroupElement {
            lines: vec![1, 0],
            numerators: None,
        }])),
        ..cfg(Tier::K, vec![r.clone()])
    };
    let c = closure(&d, &b, &cmp(), &ks(&[[1, 3, 0, 0]]), &c3, &NoFacts).unwrap();
    assert_eq!((c.summary().credited, c.summary().open_total), (1, 1));
    let cr = &c.credits()[0];
    assert_eq!(
        (cr.image.clone(), cr.rule.table.clone(), cr.rule.label),
        (vec![3, 1, 0, 0], "rec".to_string(), -1)
    );
    assert_eq!(c.used_relations()[0].credit_images, 1);
    assert_eq!(c.open_keys()[0].key, vec![2, 1, 0, 0]);
    // the same in Tier C: exact, no fact needed for the recipe image
    let c3c = ClosureConfig {
        tier: Tier::C,
        ..c3.clone()
    };
    let c = closure(&d, &b, &cmp(), &ks(&[[1, 3, 0, 0]]), &c3c, &NoFacts).unwrap();
    assert_eq!((c.summary().credited, c.summary().fact_requests), (1, 0));
    // an unverified recipe entry at the image refuses the element (open, nothing followed)
    let ru = recipe(&[[3, 1, 0, 0]], DispatchSlot::Last, VERDICT_REGULAR);
    let c = closure(
        &d,
        &b,
        &cmp(),
        &ks(&[[1, 3, 0, 0]]),
        &ClosureConfig {
            relation_sets: vec![ru],
            ..c3.clone()
        },
        &NoFacts,
    )
    .unwrap();
    assert_eq!((c.summary().credited, c.summary().keys_total), (0, 1));
    // a recorded key NOT below the original key refuses the element in both tiers (recorded terms are non-zero)
    let mut hb = RelationStoreBuilder::new(KeyCodec::new(4, 6).unwrap());
    hb.push(&[3, 1, 0, 0], &[vec![1, 4, 0, 0]], [7; 32], VERDICT_CERTIFIED)
        .unwrap();
    let rhigh = Arc::new(RelationSet {
        relations: hb.finish().unwrap(),
        ..(*r).clone()
    });
    for tier in [Tier::K, Tier::C] {
        let c = closure(
            &d,
            &b,
            &cmp(),
            &ks(&[[1, 3, 0, 0]]),
            &ClosureConfig {
                tier,
                relation_sets: vec![rhigh.clone()],
                ..c3.clone()
            },
            &NoFacts,
        )
        .unwrap();
        assert_eq!(
            (c.summary().credited, c.summary().keys_total, c.summary().open_followed),
            (0, 1, 0),
            "{tier:?}"
        );
    }
}

#[test]
fn recipe_sets_with_incremental_and_file_roundtrip() {
    let (d, b) = node();
    let st = ks(&[[3, 2, 0, 0], [3, 1, 0, 0]]);
    let r = recipe(
        &[[3, 1, 0, 0], [2, 1, 0, 0], [3, 2, 0, 0]],
        DispatchSlot::BeforeTable("A".into()),
        VERDICT_CERTIFIED,
    );
    let prev = closure(&d, &b, &cmp(), &st, &cfg(Tier::K, vec![]), &NoFacts).unwrap();
    let c = cfg(Tier::K, vec![r.clone()]);
    let inc =
        closure_incremental(&prev.dispatch_cache(), &d, &b, &cmp(), &st, &c, &NoFacts).unwrap();
    let scratch = closure(&d, &b, &cmp(), &st, &c, &NoFacts).unwrap();
    assert_eq!(
        serde_json::to_value(inc.report(&Default::default())).unwrap(),
        serde_json::to_value(scratch.report(&Default::default())).unwrap()
    );
    let mut buf = vec![];
    let dg = r
        .relations
        .write(&r.name, &r.recipe_digest, &mut buf)
        .unwrap();
    let (_, _, back) = RelationStore::read(&buf[..]).unwrap();
    assert_eq!(back.digest(), dg);
    assert_eq!(back, r.relations);
}
