use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::sector::Mask;
use crate::sector::symmetry::integral_transport::{ExpansionLimits, compile};
use crate::sector::symmetry::{CoefficientMatrix, Limits, MomentumMap, verify};

use super::*;

/// Two-loop vacuum family `k1^2-1, k2^2-1, (k1-k2)^2-1` over coordinates
/// `k1.k1, k1.k2, k2.k2`, with the dimension `d` as the only parameter.
fn family(context: &CoefficientContext) -> Arc<IntegralFamily> {
    let c = |s: &str| context.coefficient_fixture(s);
    Arc::new(
        IntegralFamily::new(
            "sunrise",
            vec!["k1".into(), "k2".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(c("-1"), vec![c("1"), c("0"), c("0")]),
                AffineDenominator::new(c("-1"), vec![c("0"), c("0"), c("1")]),
                AffineDenominator::new(c("-1"), vec![c("1"), c("-2"), c("1")]),
            ],
            vec![],
            vec![c("0"), c("0"), c("0")],
        )
        .unwrap(),
    )
}

fn momentum(context: &CoefficientContext, entries: [&str; 4]) -> MomentumMap {
    MomentumMap::new(
        CoefficientMatrix::try_new(2, 2, entries.map(|s| context.coefficient_fixture(s))).unwrap(),
        CoefficientMatrix::try_new(2, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn roundtrip(family: &IntegralFamily, map: &VerifiedMap) -> (Vec<u8>, VerifiedMap) {
    let mut table = SavedValueTableBuilder::new();
    let saved = SavedVerifiedMap::capture(map, &mut table).unwrap();
    let record = (table.finish(), vec![saved]);
    let bytes = bincode::encode_to_vec(&record, bincode::config::standard()).unwrap();
    let ((table, saved), used): ((SavedValueTable, Vec<SavedVerifiedMap>), usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(used, bytes.len());
    let values = table.materialize(family.coefficient_context()).unwrap();
    (bytes, saved[0].restore(family, family, &values).unwrap())
}

#[test]
fn restore_rebuilds_the_identical_verified_map_and_transport() {
    let context = CoefficientContext::new(["d"]);
    let family = family(&context);
    // Swap k1 <-> k2 (monomial rows) and a shear k2 -> k2 - k1 (affine rows).
    for entries in [["0", "1", "1", "0"], ["1", "0", "-1", "1"], ["-1", "0", "0", "-1"]] {
        let map = verify(
            &family,
            &family,
            momentum(&context, entries),
            Limits::default(),
        )
        .unwrap();
        let (bytes, restored) = roundtrip(&family, &map);
        assert_eq!(restored, map);
        // Deterministic bytes for the same map.
        assert_eq!(roundtrip(&family, &map).0, bytes);
        let root = Mask::try_new([true, true, true]).unwrap();
        let fresh = compile(
            &family,
            family.clone(),
            Arc::new(map),
            root.clone(),
            root.clone(),
            ExpansionLimits::default(),
        );
        let again = compile(
            &family,
            family.clone(),
            Arc::new(restored),
            root.clone(),
            root,
            ExpansionLimits::default(),
        );
        match (fresh, again) {
            (Ok(fresh), Ok(again)) => {
                assert_eq!(fresh.verified_map(), again.verified_map());
                assert_eq!(fresh.active_target_axes(), again.active_target_axes());
            }
            (Err(fresh), Err(again)) => assert_eq!(fresh, again),
            (fresh, again) => panic!("compile differs: {fresh:?} vs {again:?}"),
        }
    }
}

#[test]
fn capture_refuses_parametric_values_and_restore_checks_values_and_shapes() {
    let context = CoefficientContext::new(["d"]);
    let family = family(&context);
    let mut table = SavedValueTableBuilder::new();
    assert!(
        table
            .coefficient(&context.parameter("d").unwrap())
            .unwrap_err()
            .to_string()
            .contains("constant coefficients only")
    );
    for (numerator, denominator) in [("2", "4"), ("1", "0"), ("1", "-3"), ("0", "2"), ("x", "1")]
    {
        let table = SavedValueTable {
            values: vec![SavedRational {
                numerator: numerator.into(),
                denominator: denominator.into(),
            }],
        };
        assert!(table.materialize(&context).is_err(), "{numerator}/{denominator}");
    }
    let map = verify(
        &family,
        &family,
        momentum(&context, ["0", "1", "1", "0"]),
        Limits::default(),
    )
    .unwrap();
    let mut table = SavedValueTableBuilder::new();
    let saved = SavedVerifiedMap::capture(&map, &mut table).unwrap();
    let values = table.finish().materialize(&context).unwrap();
    let mut wrong_shape = saved.clone();
    wrong_shape.denominator_linear.entries.pop();
    assert!(wrong_shape.restore(&family, &family, &values).is_err());
    let mut wrong_index = saved.clone();
    wrong_index.loop_determinant = u32::MAX;
    assert!(wrong_index.restore(&family, &family, &values).is_err());
    let mut wrong_target = saved;
    wrong_target.row_actions[0] = SavedRowAction::Monomial {
        target: 3,
        scale: 0,
    };
    assert!(wrong_target.restore(&family, &family, &values).is_err());
}
