//! Native regression for the actual stored P102/P106/P162/P86 momentum bases.
//!
//! Public source and all raw DOT/model hashes are independently authenticated by
//! tools/research/numerator_symmetry/verify_p102.py. P-names label fixture data,
//! never solver dispatch. Every assertion below uses native family fingerprints,
//! signed keys, exact verified maps, and the existing numerator transporter.

use super::*;
use crate::algebra::matrix::{SymbolicaCoefficientMatrixLimits, rank_of_coefficient_matrix};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::AffineDenominator;
use crate::family::isp::IspCompletion;
use crate::sector::symmetry::CoefficientMatrix;

// Native stored P102 basis: a=l0,b=l1,h=l2,c=l3,q=l4.
const P102: [[i64; 5]; 7] = [
    [-1, -1, 0, 1, 1], // p=q-a-b+c, graph edge 0
    [1, 0, 0, 0, 0],   // a, graph edge 1, lmb_id=0 (present!)
    [0, 1, 0, 0, 0],   // b
    [0, 0, 1, 0, 1],   // h+q
    [0, 0, 1, 0, 0],   // h
    [0, 0, 0, 1, 0],   // c
    [0, 0, 0, 0, 1],   // q
];
const NUMERATOR: [i64; 5] = [1, 1, 0, -1, 0];
const IDENTITY: [[i64; 5]; 5] = [
    [1, 0, 0, 0, 0],
    [0, 1, 0, 0, 0],
    [0, 0, 1, 0, 0],
    [0, 0, 0, 1, 0],
    [0, 0, 0, 0, 1],
];

fn family(name: &str, context: &CoefficientContext, momenta: &[[i64; 5]]) -> Arc<IntegralFamily> {
    // This is a literal quadratic input constructor, not a CAS primitive.
    // Coordinate order is the family's documented loop-loop upper triangle.
    let denominators = momenta
        .iter()
        .map(|momentum| {
            let mut row = Vec::new();
            for left in 0..5 {
                for right in left..5 {
                    row.push(context.integer(
                        momentum[left] * momentum[right] * if left == right { 1 } else { 2 },
                    ));
                }
            }
            AffineDenominator::new(context.integer(-1), row)
        })
        .collect();
    Arc::new(
        IspCompletion::try_new(
            name,
            (0..5).map(|i| format!("l{i}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); momenta.len()],
        )
        .unwrap()
        .into_family(),
    )
}

fn key(physical: usize, pinch: Option<usize>, numerator: bool) -> IntegralKey {
    let mut powers = vec![0; 15];
    powers[..physical].fill(1);
    if let Some(axis) = pinch {
        powers[axis] = 0;
    }
    if numerator {
        powers[7] = -1;
    }
    IntegralKey::try_new(powers).unwrap()
}

fn momentum(context: &CoefficientContext, rows: [[i64; 5]; 5]) -> MomentumMap {
    MomentumMap::new(
        CoefficientMatrix::try_new(
            5,
            5,
            rows.into_iter()
                .flatten()
                .map(|value| context.integer(value)),
        )
        .unwrap(),
        CoefficientMatrix::try_new(5, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn transport_scalar(
    source: &IntegralFamily,
    target: Arc<IntegralFamily>,
    source_key: &IntegralKey,
    target_key: &IntegralKey,
    rows: [[i64; 5]; 5],
) {
    let limits = TerminalNumeratorSymmetryLimits::default();
    let verified = Arc::new(
        symmetry::verify(
            source,
            &target,
            momentum(source.coefficient_context(), rows),
            limits.verification,
        )
        .unwrap(),
    );
    assert!(matches!(verified.jacobian(), Jacobian::Unit { .. }));
    assert_eq!(verified.source_family_fingerprint(), source.fingerprint());
    assert_eq!(verified.target_family_fingerprint(), target.fingerprint());
    let prepared = integral_transport::compile(
        source,
        target,
        verified,
        Mask::try_from_indices(source_key.powers()).unwrap(),
        Mask::try_from_indices(target_key.powers()).unwrap(),
        limits.expansion,
    )
    .unwrap();
    let transported = prepared.transport(source_key, limits.expansion).unwrap();
    assert_eq!(transported.terms().len(), 1);
    assert_eq!(transported.terms()[0].key(), target_key);
    assert!(transported.terms()[0].coefficient().is_one());
}

fn rank(context: &CoefficientContext, rows: &[Vec<Coefficient>]) -> usize {
    rank_of_coefficient_matrix(context, rows, SymbolicaCoefficientMatrixLimits::default())
        .unwrap()
        .0
}

#[test]
fn p102_full_numerator_relation_is_in_automatically_discovered_exact_row_span() {
    let context = CoefficientContext::new(["d"]);
    let mut input = P102.to_vec();
    input.push(NUMERATOR); // N=(a+b-c)^2-1 remains denominator coordinate 7.
    let family = family("P102_native_stored_lmb", &context, &input);
    assert_eq!(family.denominator_count(), 15);
    let source = key(7, None, true);
    let plan = TerminalNumeratorSymmetryEquations::prepare(
        Arc::clone(&family),
        &BTreeSet::from([source.clone()]),
        Default::default(),
    )
    .unwrap();
    assert!(plan.statistics().verified_generators >= 3);
    for witness in plan.witnesses(&source) {
        witness
            .replay(Arc::clone(&family), Default::default())
            .unwrap();
    }
    let equations = plan.equations(&source);
    assert!(!equations.is_empty());

    // 4N - 2 pinch(q) - 2 scalar - sum_{p,a,b,c} pinch(r) = 0.
    // The -2 scalar term is the indispensable mass constant at m^2=1.
    let mut expected = BTreeMap::from([
        (source, context.integer(4)),
        (key(7, Some(6), false), context.integer(-2)),
        (key(7, None, false), context.integer(-2)),
    ]);
    for axis in [0, 1, 2, 5] {
        expected.insert(key(7, Some(axis), false), context.integer(-1));
    }
    let columns = equations
        .iter()
        .flat_map(|row| row.terms.keys().cloned())
        .chain(expected.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut matrix = equations
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|column| {
                    row.terms
                        .get(column)
                        .cloned()
                        .unwrap_or_else(|| context.zero())
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let original_rank = rank(&context, &matrix);
    matrix.push(
        columns
            .iter()
            .map(|column| {
                expected
                    .get(column)
                    .cloned()
                    .unwrap_or_else(|| context.zero())
            })
            .collect(),
    );
    assert_eq!(rank(&context, &matrix), original_rank);

    // Independently authenticate the equivalence of the four r-line pinches.
    // This is an exact change of variables, not a denominator-name alias.
    for (axis, rows) in [
        (
            1,
            [P102[0], IDENTITY[1], IDENTITY[2], IDENTITY[3], IDENTITY[4]],
        ),
        (
            2,
            [IDENTITY[0], P102[0], IDENTITY[2], IDENTITY[3], IDENTITY[4]],
        ),
        (
            5,
            [
                IDENTITY[0],
                IDENTITY[1],
                IDENTITY[2],
                [1, 1, 0, -1, -1],
                IDENTITY[4],
            ],
        ),
    ] {
        transport_scalar(
            &family,
            Arc::clone(&family),
            &key(7, Some(axis), false),
            &key(7, Some(0), false),
            rows,
        );
    }
}

#[test]
fn p102_pinches_route_exactly_to_actual_p162_p86_and_p106_stored_bases() {
    let context = CoefficientContext::new(["d"]);
    let mut input = P102.to_vec();
    input.push(NUMERATOR);
    let source = family("P102_native_stored_lmb", &context, &input);
    let p106 = family("P106_native_stored_lmb", &context, &P102);
    transport_scalar(
        &source,
        p106,
        &key(7, None, false),
        &key(7, None, false),
        IDENTITY,
    );

    // P162: tadpole(t0)*tadpole(t2)*tadpole(t4)*sunset(t1,t3).
    let p162 = family(
        "P162_native_stored_lmb",
        &context,
        &[
            [1, 0, 0, 0, 0],
            [0, 1, 0, -1, 0],
            [0, 1, 0, 0, 0],
            [0, 0, 1, 0, 0],
            [0, 0, 0, 1, 0],
            [0, 0, 0, 0, 1],
        ],
    );
    transport_scalar(
        &source,
        p162,
        &key(7, Some(0), false),
        &key(6, None, false),
        [
            [1, 0, 0, 0, 0],
            [0, 0, 1, 0, 0],
            [0, 1, 0, 0, 0],
            [0, 0, 0, 0, 1],
            [0, 0, 0, -1, 0],
        ],
    );

    // P86: six physical lines but five independent integrations (banana).
    let p86 = family(
        "P86_native_stored_lmb",
        &context,
        &[
            [-1, -1, -1, 1, -1],
            [1, 0, 0, 0, 0],
            [0, 1, 0, 0, 0],
            [0, 0, 1, 0, 0],
            [0, 0, 0, 1, 0],
            [0, 0, 0, 0, 1],
        ],
    );
    transport_scalar(
        &source,
        p86,
        &key(7, Some(6), false),
        &key(6, None, false),
        [
            [1, 0, 0, 0, 0],
            [0, 1, 0, 0, 0],
            [0, 0, 1, 0, 0],
            [0, 0, 0, 1, 0],
            [0, 0, -1, 0, -1],
        ],
    );
}
