//! R46 (symmetry rows in the seed system, orbits first, terminal merge) and addendum 3 (capability data, tight layer
//! caps): a synthetic toy with a 2-element symmetry group.

mod ladder_common;

use std::collections::{BTreeMap, BTreeSet};

use ladder_common::{ReplayProver, root_item};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::economy::{EconomyWeights, NodeMeasure};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::repair::{
    CaseRequest, CaseSolver, FactSource, NoTables, RepairBudget, RepairFailure, RepairInputs,
    repair_key,
};
use rustred_campaign_core::worklist::items::{
    CaseSpec, EvidenceLevel, Face, Key, NodeParams, ProverCapabilities,
};
use rustred_campaign_core::worklist::ladder::{
    CandidateRule, PointFacts, SolveAnswer, SolveOutcome, Tri,
};
use rustred_campaign_core::worklist::orbits::{
    GroupElement, ImageCredit, SymmetryGroup, merge_terminals_by_symmetry, orbit_reduce,
    serve_by_image,
};
use rustred_campaign_core::worklist::seed::{
    AttemptBudget, SeedSetting, engine_seed_args, seed_space,
};

/// Arity 6, lines 0..3, numerators 3..6; the group swaps lines 0 and 1 (numerators fixed).
fn group(verified: bool) -> SymmetryGroup {
    SymmetryGroup {
        arity: 6,
        elements: vec![
            GroupElement {
                slot_image: (0..6).map(Some).collect(),
            },
            GroupElement {
                slot_image: vec![Some(1), Some(0), Some(2), Some(3), Some(4), Some(5)],
            },
        ],
        verified,
        source: "toy".into(),
    }
}

fn params(sym: bool, caps: ProverCapabilities) -> NodeParams {
    let mut p = NodeParams::new("TOY", 6, 3);
    if sym {
        p.symmetry = Some(group(true));
    }
    p.capabilities = caps;
    p
}

fn k(a: i32, b: i32) -> Key {
    vec![a, b, 1, 0, 0, 0]
}

fn dom(p: &NodeParams, case: &CaseSpec) -> CaseDomain {
    let mut d = CaseDomain::free(p.arity, Some(p.lines.clone()));
    let free = case.free();
    for i in 0..p.arity {
        if !free.contains(&i) {
            d.fixed[i] =
                Some(
                    case.fixed
                        .get(&i)
                        .copied()
                        .unwrap_or(if p.lines[i] { 1 } else { 0 }),
                );
        }
    }
    d
}

fn rule(d: CaseDomain, tag: &str, sym: bool) -> CandidateRule {
    CandidateRule {
        domain: d,
        digest: tag.into(),
        rhs_terms: 5,
        monomials: None,
        max_degree: None,
        bytes: None,
        rows: None,
        tie_break_only: None,
        symmetry_rows: sym, witness: rustred_campaign_core::cover::WitnessStatus::Complete,
    }
}

struct Down;
impl FactSource for Down {
    fn facts(&mut self, _r: &CandidateRule, key: &[i32]) -> PointFacts {
        let mut r = key.to_vec();
        if let Some(i) = (0..3).find(|&i| r[i] >= 2) {
            r[i] -= 1;
        }
        PointFacts {
            regular: Tri::Pass,
            rhs_top: Some(vec![r]),
            exceptions_complete: Tri::Pass,
            note: None,
        }
    }
}

/// Answers ONLY settings with symmetry rows (a rule found with symmetry rows); everything else: no relation.
struct SymOnly {
    p: NodeParams,
    calls: Vec<String>,
}
impl CaseSolver for SymOnly {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        self.calls.push(req.setting.label());
        let o = if req.setting.symmetry_rows {
            SolveOutcome::Rules(vec![rule(dom(&self.p, &req.case), "symrule", true)])
        } else {
            SolveOutcome::NoRelation
        };
        SolveAnswer {
            outcome: o,
            cpu_ms: 1,
            rows: None,
        }
    }
}

#[test]
fn symmetry_rows_first_only_when_certifiable_addendum3() {
    let face = CaseSpec::face_case(Face::new(vec![0], vec![]));
    let point = CaseSpec::face_case(Face::new(vec![], vec![]));
    // no group: no symmetry dimension at all
    let p0 = params(false, ProverCapabilities::default());
    assert!(
        seed_space(&face, &p0)
            .iter()
            .flatten()
            .all(|s| !s.symmetry_rows)
    );
    // group, but no certificate route for symmetry-row rules (today): plain space unchanged for faces; points get
    // the cheap information wave first
    let p1 = params(true, ProverCapabilities::default());
    assert_eq!(seed_space(&face, &p1), seed_space(&face, &p0));
    let w = seed_space(&point, &p1);
    assert_eq!(w[0], vec![SeedSetting::symmetry_target()]);
    // capability present: symmetry rows at the target alone first, then each wave with symmetry rows first
    let caps = ProverCapabilities {
        plain: Some(EvidenceLevel::A4),
        symmetry_rows: Some(EvidenceLevel::A2),
    };
    let p2 = params(true, caps);
    let w = seed_space(&face, &p2);
    assert_eq!(w[0], vec![SeedSetting::symmetry_target()]);
    assert!(w[1][0].symmetry_rows && !w[1].last().unwrap().symmetry_rows);
    assert_eq!(
        engine_seed_args(&SeedSetting::symmetry_target()),
        vec!["--max-depth", "0", "--symmetry-rows"]
    );
    // acceptance: a rule found with symmetry rows is accepted only when the required level is reachable for it
    let cmp = NodeComparator::legacy(6, None).unwrap();
    let run = |p: &NodeParams| {
        let inp = RepairInputs {
            params: p,
            descent: &cmp,
            key: k(3, 1),
            extra_targets: vec![],
            tables: &NoTables,
        };
        let mut s = SymOnly {
            p: p.clone(),
            calls: vec![],
        };
        let r = repair_key(
            &inp,
            &RepairBudget::default(),
            &mut s,
            &mut Down,
            &mut ReplayProver,
            None,
        );
        (r, s.calls)
    };
    let (r, calls) = run(&p2);
    let out = r.expect("accepted with a certificate route");
    assert_eq!(out.rules[0].digest, "symrule");
    assert_eq!(calls[0], "target/unfl/d0/sym", "the cheapest attempt first");
    let (r, _) = run(&p1);
    assert!(
        r.is_err(),
        "found with symmetry rows, awaiting a certificate route: never accepted: {r:?}"
    );
    // the point ladder then has a VERDICT on every setting (the info wave included): a terminal proposal
    let p1b = params(true, ProverCapabilities::default());
    let inp = RepairInputs {
        params: &p1b,
        descent: &cmp,
        key: vec![1, 1, 1, 0, 0, 0],
        extra_targets: vec![],
        tables: &NoTables,
    };
    let mut s = SymOnly {
        p: p1b.clone(),
        calls: vec![],
    };
    let r = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut s,
        &mut Down,
        &mut ReplayProver,
        None,
    );
    assert!(matches!(r, Err(RepairFailure::Terminal { .. })), "{r:?}");
    assert_eq!(s.calls[0], "target/unfl/d0/sym");
}

#[test]
fn tight_layer_caps_map_to_the_engine_flag() {
    use rustred_campaign_core::worklist::policy::{
        CentreSpec, ClassMatch, PolicyClass, SeedPolicy, SettingSpec, Wave,
    };
    let mut p = params(false, ProverCapabilities::default());
    p.policy = SeedPolicy {
        schema: rustred_campaign_core::worklist::policy::SEED_POLICY_SCHEMA.into(),
        name: "tight".into(),
        source: "test".into(),
        classes: vec![PolicyClass {
            matcher: ClassMatch {
                node_class: "*".into(),
                face_type: "*".into(),
            },
            waves: vec![Wave {
                depth: 3,
                settings: vec![SettingSpec {
                    centre: CentreSpec::Target,
                    floor: None,
                    tight: Some(true),
                    layer_caps: None,
                }],
            }],
            tight: false,
            budgets: None,
            replace_builtin: true,
        }],
        evidence: vec![],
    };
    let face = CaseSpec::face_case(Face::new(vec![0], vec![]));
    let w: Vec<SeedSetting> = seed_space(&face, &p).into_iter().flatten().collect();
    assert_eq!(w.len(), 4, "default relaxation: 4 caps");
    let a = engine_seed_args(&w[0]);
    assert_eq!(a, vec!["--max-depth", "3", "--seed-layer-cap", "0,0,1"]);
    assert!(engine_seed_args(&w[3]).contains(&"0,1,1".to_string()));
}

struct CreditExcept(Key);
impl ImageCredit for CreditExcept {
    fn credit(&mut self, key: &[i32], _image: &[i32], _g: usize, _rule: &str) -> Tri {
        if key == self.0.as_slice() {
            Tri::Fail
        } else {
            Tri::Pass
        }
    }
}

#[test]
fn orbit_reduction_image_service_and_own_search_fallback() {
    let p = params(true, ProverCapabilities::default());
    let g = group(true);
    let cmp = NodeComparator::legacy(6, None).unwrap();
    let a = root_item(&p, Face::new(vec![0], vec![]), vec![k(2, 1), k(3, 1)]);
    let b = root_item(
        &p,
        Face::new(vec![1], vec![]),
        vec![k(1, 2), k(1, 3), k(1, 4)],
    );
    let c = root_item(&p, Face::new(vec![0, 1], vec![]), vec![k(2, 2)]);
    let orbits = orbit_reduce(vec![a.clone(), b.clone(), c.clone()], &g, &p.lines, &cmp);
    assert_eq!(orbits.len(), 2, "{{0}} and {{1}} form one orbit");
    let o = orbits.iter().find(|o| !o.members.is_empty()).unwrap();
    let (rep, member) = (&o.representative, &o.members[0]);
    assert!(rep.case == a.case || rep.case == b.case);
    assert_eq!(
        g.image_case(member.element, &member.item.case, &p.lines)
            .as_ref(),
        Some(&rep.case)
    );
    // the representative's own rule is credited at its keys (and at the image of (1,4) only if listed)
    let rep_keys: BTreeSet<Key> = [k(2, 1), k(3, 1), k(1, 2), k(1, 3)].into_iter().collect();
    let credited: BTreeMap<String, BTreeSet<Key>> =
        [("repRule".to_string(), rep_keys)].into_iter().collect();
    let failing = member.item.gap_points[0].clone();
    let out = serve_by_image(
        member,
        &g,
        &p.lines,
        &credited,
        &mut CreditExcept(failing.clone()),
    );
    let own = out
        .own_search
        .expect("the failing credit and the uncredited image go to the own search");
    assert!(own.gap_points.contains(&failing));
    for s in &out.served {
        assert_eq!(
            g.image_key(s.element, &s.key, &p.lines).as_ref(),
            Some(&s.image)
        );
        assert!(credited["repRule"].contains(&s.image));
    }
    assert_eq!(
        out.served.len() + own.gap_points.len(),
        member.item.gap_points.len()
    );
}

#[test]
fn terminal_merge_needs_a_verified_group() {
    let cmp = NodeComparator::legacy(6, None).unwrap();
    let lines = vec![true, true, true, false, false, false];
    let terms = vec![k(2, 1), k(1, 2), k(1, 1)];
    let (keep, rel) = merge_terminals_by_symmetry(&terms, &group(true), &lines, &cmp);
    assert_eq!(keep.len(), 2);
    assert_eq!(rel.len(), 1);
    let (keep, rel) = merge_terminals_by_symmetry(&terms, &group(false), &lines, &cmp);
    assert_eq!(
        (keep.len(), rel.len()),
        (3, 0),
        "unverified group: no merge"
    );
}

#[test]
fn symmetry_json_fixture_parses_if_present() {
    let path = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/fixtures/campaign-20261114/small/m2-slice/L3a/SYMMETRY.json";
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("SKIP: {path} absent");
        return;
    };
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let slots: Vec<String> = v["slots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    let line_names: BTreeSet<String> = v["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    let lines: Vec<bool> = slots.iter().map(|s| line_names.contains(s)).collect();
    let g = SymmetryGroup::from_symmetry_json(&text, &lines).unwrap();
    assert_eq!(g.elements.len() as u64, v["order_full"].as_u64().unwrap());
    assert!(g.nontrivial());
    // every element acts on the all-floor corner as the identity
    let corner: Key = lines.iter().map(|&l| if l { 1 } else { 0 }).collect();
    for e in 0..g.elements.len() {
        assert_eq!(g.image_key(e, &corner, &lines), Some(corner.clone()));
    }
}

#[test]
fn economy_weights_default_trade() {
    let w = EconomyWeights::default();
    let one = NodeMeasure {
        cases: 1,
        rhs_terms: 4000,
        ..Default::default()
    };
    let three = NodeMeasure {
        cases: 3,
        rhs_terms: 300,
        ..Default::default()
    };
    // one case of 4,000 terms beats three cases of 100 terms each (a case is worth 2,000 terms)
    assert!(one.weighted_score(&w) < three.weighted_score(&w));
}
