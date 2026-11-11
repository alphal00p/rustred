//! Focused tests on genuine fixed6c49 Insertion/Carpet edge fixtures.
use rustred::algebra::CoefficientContext;
use rustred::propagator::PresentationId;
use rustred::propagator::census::SignedAffineLoopMap;
use rustred::propagator::lane::{BubbleRoles, CarpetRoles, LaneRoles};
use rustred::propagator::presentation::{
    EdgeKind, EdgeMap, Presentation, PresentationKind, Slot, SlotIsp, SlotLine, verify_edge_map,
};
use rustred::propagator::route_choice::outer;
use rustred::propagator::route_choice::{Frac, compose, identity_map, inverse};
use serde_json::Value;

fn fixtures() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/outer_map_choice.json")).unwrap()
}
fn ints(x: &Value) -> Vec<i64> {
    x.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}
fn presentation(x: &Value) -> Presentation {
    let slots = x["slots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            if let Some(line) = s.get("line") {
                Slot::Line(SlotLine {
                    momentum: ints(&line["momentum"]),
                    eps_units: line["eps_units"].as_u64().unwrap() as u8,
                })
            } else {
                Slot::Isp(SlotIsp {
                    left: ints(&s["isp"]["left"]),
                    right: ints(&s["isp"]["right"]),
                })
            }
        })
        .collect();
    Presentation::from_slots(
        PresentationId(x["index"].as_u64().unwrap() as u32),
        PresentationKind::EpsNode,
        &format!("outer_fixture_{}", x["index"]),
        x["loops"].as_u64().unwrap() as u8,
        slots,
        &CoefficientContext::try_new(["d"]).unwrap(),
        None,
    )
    .unwrap()
}
fn map(x: &Value) -> SignedAffineLoopMap {
    SignedAffineLoopMap {
        loops_in: x["loops_in"].as_u64().unwrap() as u8,
        loops_out: x["loops_out"].as_u64().unwrap() as u8,
        loop_linear: ints(&x["loop_linear"]).into(),
        loop_external: ints(&x["loop_external"]).into(),
        external_sign: x["external_sign"].as_i64().unwrap() as i8,
    }
}
fn optional_slots(x: &Value) -> Vec<Option<u16>> {
    x.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().map(|n| n as u16))
        .collect()
}
fn edge(x: &Value) -> EdgeMap {
    EdgeMap {
        parent: PresentationId(x["parent"].as_u64().unwrap() as u32),
        child: PresentationId(x["child"].as_u64().unwrap() as u32),
        kind: if x["kind"] == "insertion" {
            EdgeKind::Insertion
        } else {
            EdgeKind::Carpet
        },
        integrated_loop: x["integrated_loop"].as_u64().unwrap() as u8,
        outer_map: map(&x["outer_map"]),
        line_slots: optional_slots(&x["line_slots"]),
        merged_slot: x["merged_slot"].as_u64().map(|n| n as u16),
    }
}
fn roles(x: &Value) -> LaneRoles {
    if let Some(x) = x.get("insertion") {
        LaneRoles::Insertion(BubbleRoles {
            lines: [
                x["lines"][0].as_u64().unwrap() as u16,
                x["lines"][1].as_u64().unwrap() as u16,
            ],
            loop_index: x["loop_index"].as_u64().unwrap() as u8,
        })
    } else {
        let x = &x["carpet"];
        LaneRoles::Carpet(CarpetRoles {
            r: x["r"].as_u64().unwrap() as u16,
            third: x["third"].as_u64().map(|n| n as u16),
            outer_loop: x["outer_loop"].as_u64().unwrap() as u8,
        })
    }
}
fn matrix_map(x: &[i64], loops: u8) -> SignedAffineLoopMap {
    let n = usize::from(loops);
    let w = n + 1;
    SignedAffineLoopMap {
        loops_in: loops,
        loops_out: loops,
        loop_linear: (0..n)
            .flat_map(|i| (0..n).map(move |j| x[i * w + j]))
            .collect(),
        loop_external: (0..n).map(|i| x[i * w + n]).collect(),
        external_sign: x[n * w + n] as i8,
    }
}

#[test]
fn outer_physical_permutations_and_inverse_are_exact() {
    for f in fixtures() {
        let p = presentation(&f["parent_forms"]);
        let c = presentation(&f["child_forms"]);
        let e = edge(&f["edge_record"]);
        for action in f["native_group_actions_and_derived_edge_fields"]
            .as_array()
            .unwrap()
        {
            let flat = ints(&action["matrix_row_major"]);
            let m = matrix_map(&flat, c.loops);
            let (composed, _) = outer::composed_edge(&p, &c, &e, &m).unwrap();
            assert_eq!(
                composed.line_slots,
                optional_slots(&action["derived_line_slots_after_composition"])
            );
            assert_eq!(
                composed.merged_slot,
                action["derived_merged_slot_after_composition"]
                    .as_u64()
                    .map(|n| n as u16)
            );
            let w = usize::from(c.loops) + 1;
            let matrix: Vec<_> = flat
                .chunks(w)
                .map(|r| r.iter().map(|n| Frac::int(i128::from(*n))).collect())
                .collect();
            let inv = inverse(&matrix).unwrap().unwrap();
            let inv: Vec<_> = inv
                .into_iter()
                .flatten()
                .map(|f| {
                    assert_eq!(f.den, 1);
                    i64::try_from(f.num).unwrap()
                })
                .collect();
            let inv = matrix_map(&inv, c.loops);
            assert_eq!(compose(&m, &inv), identity_map(c.loops));
            let (restored, _) = outer::composed_edge(&p, &c, &composed, &inv).unwrap();
            assert_eq!(restored, e);
        }
    }
}

#[test]
fn outer_factor_counts_agree_with_native_scalar_products() {
    for f in fixtures() {
        let p = presentation(&f["parent_forms"]);
        let c = presentation(&f["child_forms"]);
        let e = edge(&f["edge_record"]);
        let roles = roles(&f["roles"]);
        for action in f["native_group_actions_and_derived_edge_fields"]
            .as_array()
            .unwrap()
        {
            let m = matrix_map(&ints(&action["matrix_row_major"]), c.loops);
            let (e, _) = outer::composed_edge(&p, &c, &e, &m).unwrap();
            let (_, got) = outer::static_score(&p, &c, &e, roles).unwrap();
            let want: Vec<_> = outer::factor_rows(&p, &c, &e, roles)
                .unwrap()
                .into_iter()
                .map(|(l, r)| {
                    let f = c.express_scalar_product(&l, &r).unwrap();
                    f.terms.len() + usize::from(!f.constant.is_zero())
                })
                .collect();
            assert_eq!(got, want);
        }
    }
}

#[test]
fn outer_marked_physical_symmetry_cannot_swap_unequal_eps() {
    let mut f = fixtures()
        .into_iter()
        .find(|x| x["edge_map_id"] == 61)
        .unwrap();
    f["parent_forms"]["slots"][5]["line"]["eps_units"] = 1.into();
    f["child_forms"]["slots"][4]["line"]["eps_units"] = 1.into();
    let p = presentation(&f["parent_forms"]);
    let c = presentation(&f["child_forms"]);
    let e = edge(&f["edge_record"]);
    verify_edge_map(&p, &c, &e).unwrap();
    let action = &f["native_group_actions_and_derived_edge_fields"][0];
    let m = matrix_map(&ints(&action["matrix_row_major"]), c.loops);
    assert!(outer::composed_edge(&p, &c, &e, &m).is_err());
}

#[test]
fn outer_choices_are_strict_and_identity_control_stays_fixed() {
    for f in fixtures() {
        let p = presentation(&f["parent_forms"]);
        let c = presentation(&f["child_forms"]);
        let e = edge(&f["edge_record"]);
        let roles = roles(&f["roles"]);
        let chosen = outer::choose_static(&p, &c, &e, roles).unwrap();
        assert!(chosen.score_after <= chosen.score_before);
        if chosen.edge != e {
            assert!(chosen.score_after < chosen.score_before);
        } else {
            assert_eq!(chosen.score_after, chosen.score_before);
        }
        if f["edge_map_id"] == 129 {
            assert_eq!(chosen.edge, e);
        }
        let (same, _) = outer::composed_edge(&p, &c, &e, &identity_map(c.loops)).unwrap();
        assert_eq!(same, e);
    }
}
