use super::super::super::queue::{Domain, Phase};
use super::*;
use rustred::solver::DomainPowerBounds;

fn fixture<const N: usize>(wire_arity: usize) -> (EpochSections, RecordRow, Vec<CompactDomain<N>>) {
    let lower: Vec<u16> = (0..wire_arity).map(|axis| u16::from(axis == 0)).collect();
    let upper = lower.clone();
    let mut scope = 1u32.to_le_bytes().to_vec();
    for bound in [0i64, 1] {
        scope.push(1);
        scope.extend(bound.to_le_bytes());
    }
    for values in [&lower, &upper] {
        for value in values {
            scope.extend(value.to_le_bytes());
        }
    }
    let sections = EpochSections {
        ledger: vec![1, 3],
        runs: vec![],
        anchors: vec![(1, 1, 2, vec![(0, 0, 1)], scope)],
        manifest: json!({"arity":wire_arity}),
    };
    let row = serde_json::from_value(json!({
        "id":1,"record_kind":"g2_residual_inspection","phase":"Apply",
        "owner":format!("1{}", "0".repeat(wire_arity - 1)),
        "lower":lower,"upper":upper,"rank":null,
        "power_bounds":{"max_positive_power":null,"min_power_difference":null,"max_power_difference":null},
        "g2":{"kind":"g2_native","dispatch_version":2,
            "anchors":[{"id":0,"stamp":1,"lent":"domain"}],
            "residual":[{"d_lo":0,"d_hi":1,"lower":lower,"upper":upper}],
            "authority":"exact_union_cover_lattice"},
        "epoch":{"merge_epoch":3,"v0":2}
    })).unwrap();
    let domain = CompactDomain::restore(&Domain {
        phase: Phase::Apply,
        owner: std::array::from_fn(|axis| axis == 0),
        lower: lower.into_iter().map(u64::from).collect(),
        upper: upper
            .into_iter()
            .map(|value| Some(u64::from(value)))
            .collect(),
        rank: None,
        powers: DomainPowerBounds::default(),
    })
    .unwrap();
    (sections, row, vec![domain.clone(), domain])
}

#[test]
fn g2_wire_scope_is_checked_without_changing_record_identity() {
    let (sections, mut row, domains) = fixture::<1>(1);
    let original = row.g2.clone();
    View::new(&sections)
        .unwrap()
        .normalize(&mut row, &domains)
        .unwrap();
    assert_eq!(row.g2, original);
    assert_eq!(row.record_kind, "g2_residual_anchor_inspection");
    let bounds = row
        .g2_residual_anchors
        .unwrap()
        .residual_power_bounds
        .unwrap();
    assert_eq!(bounds.min_power_difference, Some(0));
    assert_eq!(bounds.max_power_difference, Some(1));
}

#[test]
#[cfg(feature = "capacity-dispatch")]
fn g2_physical_wire_scope_expands_to_capacity_without_padding_saved_json() {
    let (sections, mut row, domains) = fixture::<16>(15);
    let original = row.g2.clone();
    View::new(&sections)
        .unwrap()
        .normalize(&mut row, &domains)
        .unwrap();
    assert_eq!(row.g2, original);
    assert_eq!(
        row.g2.as_ref().unwrap()["residual"][0]["lower"]
            .as_array()
            .unwrap()
            .len(),
        15
    );
    assert_eq!(domains[1].raw_bounds().0[15], 0);
    assert_eq!(domains[1].raw_bounds().1[15], 0);
    assert!(!domains[1].owner()[15]);

    // A capacity-width record cannot replace the original wire-width claim.
    let (_, mut wrong, _) = fixture::<16>(16);
    assert!(
        View::new(&sections)
            .unwrap()
            .normalize(&mut wrong, &domains)
            .is_err()
    );
}

#[test]
#[cfg(feature = "capacity-dispatch")]
fn g2_capacity_wire_scope_narrows_only_zero_finite_padding() {
    let (sections, mut row, domains) = fixture::<15>(16);
    View::new(&sections)
        .unwrap()
        .normalize(&mut row, &domains)
        .unwrap();
    for (upper, bad) in [(false, 1u16), (true, 1), (true, u16::MAX)] {
        let (mut sections, mut row, domains) = fixture::<15>(16);
        let offset = 22 + usize::from(upper) * 2 * 16 + 2 * 15;
        sections.anchors[0].4[offset..offset + 2].copy_from_slice(&bad.to_le_bytes());
        row.g2.as_mut().unwrap()["residual"][0][if upper { "upper" } else { "lower" }][15] =
            json!(bad);
        let error = View::new(&sections)
            .unwrap()
            .normalize(&mut row, &domains)
            .unwrap_err();
        assert!(error.contains("coordinate scope"), "{error}");
    }
}

#[test]
fn g2_wire_arity_and_scope_length_are_authenticated_inputs() {
    let (mut sections, _, _) = fixture::<1>(1);
    for arity in [Value::Null, json!(0), json!(33)] {
        sections.manifest["arity"] = arity;
        assert!(View::new(&sections).is_err());
    }
    let (mut sections, mut row, domains) = fixture::<1>(1);
    sections.anchors[0].4.push(0);
    assert!(
        View::new(&sections)
            .unwrap()
            .normalize(&mut row, &domains)
            .is_err()
    );
}
