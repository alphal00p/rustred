//! Public sector-symmetry census on RustFlow's native four-loop wheel (P14).
//!
//! The family is RustFlow's exact native presentation: physical lines
//! `D0..D7` (rim `D0, D2, D4, D5`; spokes `D1, D3, D6, D7`) and auxiliary
//! ISPs `D8 = k0.k1`, `D9 = k0.k2`. A deformation `D_s -> D_s - eta` keeps
//! `eta` symbolic. Kira's independent sector partitions, in-sector symmetry
//! counts and representative choices for the same three families are the
//! oracle below. Every RustRed route and automorphism is additionally
//! re-checked here, independently of the census code:
//!
//! - plain integer line arithmetic (`q_s A = +-q_t`, equal complete constants,
//!   `|det A| = 1`);
//! - the exact integrand identity `f_I(A k') = sum_j c_j f_j(k')` of every
//!   transported probe integral, evaluated over Q(eta) at random rational Gram
//!   matrices `G' = (k'_a . k'_b)` with `G = A G' A^T`, built from this file's
//!   own denominator table. Unlike a round trip `T^-1(T(I))`, this detects a
//!   consistently wrong affine constant (for example `eta -> -eta`); the
//!   opposite map convention is confirmed to fail;
//! - an exact numerator round trip `T^-1(T(I)) = I`;
//! - recursive normal forms against a plain recursion over the public API.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use rustred::sector::Mask;
use rustred::sector::symmetry::integral_transport::{
    AffineConstants, ExpansionLimits, Prepared, compile_with_affine_constants,
};
use rustred::sector::symmetry::{
    CoefficientMatrix, DenominatorAction, Jacobian, Limits, MomentumMap, RelationEndpoints,
    RepresentativePolicy, SectorSymmetryCensus, SectorSymmetryCensusOptions, verify,
};
use rustred::sector::zero::Analyzer;

/// RustFlow `native_family` coefficients over
/// (k0k0, k0k1, k0k2, k0k3, k1k1, k1k2, k1k3, k2k2, k2k3, k3k3).
const SCALAR_PRODUCTS: [[i64; 10]; 10] = [
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
    [1, -2, 0, -2, 1, 0, 2, 0, 0, 1],
    [0, 0, 0, 0, 1, 2, 2, 1, 2, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, -2, -2, -2, 1, 2, 2, 1, 2, 1],
    [0, 0, 0, 0, 1, 2, 0, 1, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
];
const CONSTANTS: [i64; 10] = [-1, -1, -1, -1, -1, -1, -1, -1, 0, 0];
/// Integer line momenta of the physical lines in (k0, k1, k2, k3).
const LINES: [[i64; 4]; 8] = [
    [0, 0, 1, 0],
    [1, -1, 0, -1],
    [0, 1, 1, 1],
    [0, 0, 0, 1],
    [1, -1, -1, -1],
    [0, 1, 1, 0],
    [1, 0, 0, 0],
    [0, 1, 0, 0],
];
const PHYSICAL: usize = 8;

/// Kira undeformed: 11 classes over 134 sectors, (members, |G| - 1).
const KIRA_UNDEFORMED: &[(&[u16], usize)] = &[
    (
        &[
            15, 29, 30, 39, 43, 53, 54, 57, 58, 75, 77, 78, 89, 90, 99, 101, 102, 105, 106, 113,
            114, 135, 139, 142, 149, 150, 153, 154, 156, 166, 170, 180, 184, 195, 197, 198, 201,
            204, 209, 210, 216, 226, 228, 232, 240,
        ],
        23,
    ),
    (
        &[
            31, 47, 55, 59, 61, 62, 91, 93, 94, 109, 110, 115, 117, 118, 151, 155, 167, 171, 174,
            181, 185, 188, 211, 213, 214, 220, 227, 229, 233, 236, 241, 244,
        ],
        11,
    ),
    (&[63, 245], 71),
    (
        &[
            79, 103, 121, 122, 143, 157, 182, 186, 199, 203, 205, 206, 218, 234, 242, 248,
        ],
        23,
    ),
    (
        &[95, 119, 125, 126, 175, 183, 187, 189, 215, 237, 243, 252],
        7,
    ),
    (&[107, 158, 217, 230], 119),
    (
        &[111, 123, 159, 190, 219, 221, 222, 231, 235, 238, 246, 249],
        11,
    ),
    (&[127, 191, 247, 253], 7),
    (&[207, 250], 47),
    (&[223, 239, 251, 254], 7),
    (&[255], 7),
];

/// Kira rim (native slot 0): 27 classes over 134 sectors, (members, |G| - 1).
const KIRA_RIM: &[(&[u16], usize)] = &[
    (
        &[
            15, 29, 39, 43, 53, 57, 75, 77, 89, 99, 101, 105, 113, 135, 139, 149, 153, 195, 197,
            201, 209,
        ],
        5,
    ),
    (
        &[
            30, 54, 58, 78, 90, 102, 106, 114, 142, 150, 154, 156, 166, 170, 180, 184, 198, 204,
            210, 216, 226, 228, 232, 240,
        ],
        23,
    ),
    (
        &[
            31, 55, 59, 91, 115, 151, 155, 167, 171, 181, 185, 211, 227, 229, 233, 241,
        ],
        3,
    ),
    (&[47, 61, 93, 109, 117, 213], 5),
    (&[62, 94, 110, 118, 174, 188, 214, 220, 236, 244], 11),
    (&[63, 245], 11),
    (&[79, 103, 143, 157, 199, 205], 5),
    (&[95, 119, 175, 189, 215, 237], 1),
    (&[107, 217], 23),
    (&[111, 221], 3),
    (&[121, 203], 23),
    (&[122, 182, 186, 206, 218, 234, 242, 248], 23),
    (&[123, 219, 235, 249], 5),
    (&[125], 7),
    (&[126, 252], 7),
    (&[127, 253], 1),
    (&[158, 230], 119),
    (&[159, 231], 11),
    (&[183, 187, 243], 7),
    (&[190, 222, 238, 246], 11),
    (&[191, 247], 3),
    (&[207], 7),
    (&[223, 239], 1),
    (&[250], 47),
    (&[251], 7),
    (&[254], 7),
    (&[255], 1),
];

/// Kira spoke (native slot 1): 26 classes over 134 sectors, (members, |G| - 1).
const KIRA_SPOKE: &[(&[u16], usize)] = &[
    (
        &[
            15, 30, 39, 43, 54, 58, 75, 78, 90, 99, 102, 106, 114, 135, 139, 142, 150, 154, 166,
            170, 195, 198, 210, 226,
        ],
        5,
    ),
    (
        &[
            29, 53, 57, 77, 89, 101, 105, 113, 149, 153, 156, 180, 184, 197, 201, 204, 209, 216,
            228, 232, 240,
        ],
        23,
    ),
    (&[31, 55, 59, 91, 115, 151, 155, 211], 3),
    (&[47, 62, 94, 110, 118, 167, 171, 174, 214, 227], 5),
    (
        &[
            61, 93, 109, 117, 181, 185, 188, 213, 220, 229, 233, 236, 241, 244,
        ],
        11,
    ),
    (&[63], 11),
    (&[79, 103, 182, 186, 199, 203, 206, 218, 234, 242], 5),
    (&[95, 119, 183, 187, 215, 243], 1),
    (&[107, 158, 230], 23),
    (&[111, 190, 222, 231, 235, 238, 246], 3),
    (&[121, 157, 205, 248], 23),
    (&[122, 143], 23),
    (&[123, 159], 5),
    (&[125, 189, 237, 252], 7),
    (&[126, 175], 7),
    (&[127, 191], 1),
    (&[207, 250], 7),
    (&[217], 119),
    (&[219], 11),
    (&[221, 249], 11),
    (&[223, 251], 1),
    (&[239, 254], 3),
    (&[245], 71),
    (&[247], 7),
    (&[253], 7),
    (&[255], 1),
];

/// Kira's representative ("unique") sectors, from the same oracle runs.
const KIRA_UNDEFORMED_REPRESENTATIVES: &[u16] =
    &[107, 201, 203, 207, 233, 235, 237, 239, 245, 253, 255];
const KIRA_RIM_REPRESENTATIVES: &[u16] = &[
    107, 109, 111, 125, 187, 191, 201, 203, 205, 207, 230, 231, 232, 233, 234, 235, 236, 237, 238,
    239, 245, 250, 251, 252, 253, 254, 255,
];
const KIRA_SPOKE_REPRESENTATIVES: &[u16] = &[
    63, 75, 91, 107, 123, 127, 143, 171, 175, 187, 201, 203, 205, 207, 217, 219, 233, 235, 237,
    239, 245, 247, 249, 251, 253, 255,
];

fn p14(deformed: Option<usize>) -> Arc<IntegralFamily> {
    let parameters: &[&str] = if deformed.is_some() {
        &["d", "eta"]
    } else {
        &["d"]
    };
    let context = CoefficientContext::try_new(parameters.iter().copied()).unwrap();
    let denominators = SCALAR_PRODUCTS
        .iter()
        .zip(CONSTANTS)
        .enumerate()
        .map(|(slot, (row, constant))| {
            let mut constant = context.integer(constant);
            if deformed == Some(slot) {
                constant = context
                    .try_sub(
                        &constant,
                        &context.parameter("eta").unwrap(),
                        ExactAlgebraLimits::default(),
                    )
                    .unwrap();
            }
            AffineDenominator::new(constant, row.map(|value| context.integer(value)).to_vec())
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "p14_native",
            (0..4).map(|loop_| format!("k{loop_}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); 10],
        )
        .unwrap(),
    )
}

fn top() -> Mask {
    Mask::try_new((0..10).map(|slot| slot < PHYSICAL)).unwrap()
}

fn mask_of(id: u16) -> Mask {
    Mask::try_new((0..10).map(|slot| id & (1 << slot) != 0)).unwrap()
}

fn sector_id(mask: &Mask) -> u16 {
    mask.active_bits()
        .iter()
        .enumerate()
        .filter(|(_, active)| **active)
        .map(|(slot, _)| 1 << slot)
        .sum()
}

fn small_integer(context: &CoefficientContext, value: &Coefficient) -> i64 {
    (-8..=8)
        .find(|&candidate| &context.integer(candidate) == value)
        .expect("routes are small integer matrices")
}

fn integer_matrix(context: &CoefficientContext, matrix: &CoefficientMatrix) -> [[i64; 4]; 4] {
    let mut result = [[0; 4]; 4];
    for (row, values) in result.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            *value = small_integer(context, matrix.get(row, column).unwrap());
        }
    }
    result
}

fn minor(matrix: &[[i64; 4]; 4], skip_row: usize, skip_column: usize) -> i64 {
    let rows: Vec<_> = (0..4).filter(|&row| row != skip_row).collect();
    let columns: Vec<_> = (0..4).filter(|&column| column != skip_column).collect();
    let m = |r: usize, c: usize| matrix[rows[r]][columns[c]];
    m(0, 0) * (m(1, 1) * m(2, 2) - m(1, 2) * m(2, 1))
        - m(0, 1) * (m(1, 0) * m(2, 2) - m(1, 2) * m(2, 0))
        + m(0, 2) * (m(1, 0) * m(2, 1) - m(1, 1) * m(2, 0))
}

fn determinant(matrix: &[[i64; 4]; 4]) -> i64 {
    (0..4)
        .map(|column| {
            let sign = if column % 2 == 0 { 1 } else { -1 };
            sign * matrix[0][column] * minor(matrix, 0, column)
        })
        .sum()
}

/// Exact integer inverse of a unimodular matrix via the adjugate.
fn unimodular_inverse(matrix: &[[i64; 4]; 4]) -> [[i64; 4]; 4] {
    let det = determinant(matrix);
    assert!(det == 1 || det == -1);
    let mut inverse = [[0; 4]; 4];
    for (row, values) in inverse.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            let sign = if (row + column) % 2 == 0 { 1 } else { -1 };
            *value = sign * minor(matrix, column, row) * det;
        }
    }
    inverse
}

fn momentum(context: &CoefficientContext, matrix: &[[i64; 4]; 4]) -> MomentumMap {
    MomentumMap::new(
        CoefficientMatrix::try_new(
            4,
            4,
            matrix.iter().flatten().map(|&value| context.integer(value)),
        )
        .unwrap(),
        CoefficientMatrix::try_new(4, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

/// Independent integer check: `q_s A = +-q_t` with equal complete constants.
fn check_route_geometry(family: &IntegralFamily, source: &Mask, target: &Mask, map: &MomentumMap) {
    let context = family.coefficient_context();
    let a = integer_matrix(context, map.loop_linear());
    assert!(matches!(determinant(&a), 1 | -1));
    let mut images = BTreeSet::new();
    for slot in (0..PHYSICAL).filter(|&slot| source.active_bits()[slot]) {
        let image: [i64; 4] =
            std::array::from_fn(|column| (0..4).map(|k| LINES[slot][k] * a[k][column]).sum());
        let negated = image.map(|value| -value);
        let target_slot = (0..PHYSICAL)
            .find(|&candidate| {
                target.active_bits()[candidate]
                    && (LINES[candidate] == image || LINES[candidate] == negated)
            })
            .expect("active line maps onto an active representative line");
        assert_eq!(
            family.denominators()[slot].constant(),
            family.denominators()[target_slot].constant()
        );
        assert!(images.insert(target_slot));
    }
    assert_eq!(images.len(), target.active_count());
}

/// Symmetric Gram matrix `G[a][b] = k_a . k_b` over the family context.
type Gram = [[Coefficient; 4]; 4];

fn rational(context: &CoefficientContext, numerator: i64, denominator: i64) -> Coefficient {
    context
        .try_div(
            &context.integer(numerator),
            &context.integer(denominator),
            ExactAlgebraLimits::unbounded_resources(),
        )
        .unwrap()
}

/// Deterministic random symmetric rational Gram matrix (xorshift64).
fn random_gram(context: &CoefficientContext, state: &mut u64) -> Gram {
    let mut next = || {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    };
    let mut upper = BTreeMap::new();
    for a in 0..4 {
        for b in a..4 {
            let numerator = (next() % 195) as i64 - 97;
            let denominator = (next() % 31) as i64 + 1;
            upper.insert((a, b), rational(context, numerator, denominator));
        }
    }
    std::array::from_fn(|a| std::array::from_fn(|b| upper[&(a.min(b), a.max(b))].clone()))
}

/// Gram matrix of `k = A k'`: `G = A G' A^T`.
fn transformed(context: &CoefficientContext, a: &[[i64; 4]; 4], gram: &Gram) -> Gram {
    let limits = ExactAlgebraLimits::unbounded_resources();
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            let mut sum = context.zero();
            for (c, a_row) in a[row].iter().enumerate() {
                for (e, a_column) in a[column].iter().enumerate() {
                    let weight = a_row * a_column;
                    if weight != 0 {
                        let term = context
                            .try_mul(&context.integer(weight), &gram[c][e], limits)
                            .unwrap();
                        sum = context.try_add(&sum, &term, limits).unwrap();
                    }
                }
            }
            sum
        })
    })
}

/// Denominator values `D_m(G)` from this file's own table, with the
/// deformation `D_s -> D_s - eta` applied independently of the family.
fn denominator_values(
    context: &CoefficientContext,
    deformed: Option<usize>,
    gram: &Gram,
) -> Vec<Coefficient> {
    let limits = ExactAlgebraLimits::unbounded_resources();
    let pairs: Vec<(usize, usize)> = (0..4).flat_map(|a| (a..4).map(move |b| (a, b))).collect();
    SCALAR_PRODUCTS
        .iter()
        .zip(CONSTANTS)
        .enumerate()
        .map(|(slot, (row, constant))| {
            let mut value = context.integer(constant);
            if deformed == Some(slot) {
                value = context
                    .try_sub(&value, &context.parameter("eta").unwrap(), limits)
                    .unwrap();
            }
            for (&weight, &(a, b)) in row.iter().zip(&pairs) {
                if weight != 0 {
                    let term = context
                        .try_mul(&context.integer(weight), &gram[a][b], limits)
                        .unwrap();
                    value = context.try_add(&value, &term, limits).unwrap();
                }
            }
            value
        })
        .collect()
}

/// `f_n = prod_m D_m^(-n_m)` at precomputed denominator values.
fn integrand(
    context: &CoefficientContext,
    values: &[Coefficient],
    key: &IntegralKey,
) -> Coefficient {
    let limits = ExactAlgebraLimits::unbounded_resources();
    let mut numerator = context.one();
    let mut denominator = context.one();
    for (value, &power) in values.iter().zip(key.powers()) {
        let target = if power > 0 {
            &mut denominator
        } else {
            &mut numerator
        };
        for _ in 0..power.unsigned_abs() {
            *target = context.try_mul(target, value, limits).unwrap();
        }
    }
    context.try_div(&numerator, &denominator, limits).unwrap()
}

/// Random evaluation points shared by every check of one variant.
struct Points {
    grams: Vec<Gram>,
    values: Vec<Vec<Coefficient>>,
}

impl Points {
    fn new(context: &CoefficientContext, deformed: Option<usize>, seed: u64) -> Self {
        let mut state = seed;
        let grams: Vec<Gram> = (0..2).map(|_| random_gram(context, &mut state)).collect();
        let values = grams
            .iter()
            .map(|gram| denominator_values(context, deformed, gram))
            .collect();
        Self { grams, values }
    }

    /// Denominator values of the source family at `k = A k'`.
    fn pulled_back(
        &self,
        context: &CoefficientContext,
        deformed: Option<usize>,
        a: &[[i64; 4]; 4],
    ) -> Vec<Vec<Coefficient>> {
        self.grams
            .iter()
            .map(|gram| denominator_values(context, deformed, &transformed(context, a, gram)))
            .collect()
    }

    /// `f_I(A k') == sum_j c_j f_j(k')` at every point.
    fn identity_holds(
        &self,
        context: &CoefficientContext,
        source_values: &[Vec<Coefficient>],
        source: &IntegralKey,
        terms: &[(IntegralKey, Coefficient)],
    ) -> bool {
        let limits = ExactAlgebraLimits::unbounded_resources();
        source_values
            .iter()
            .zip(&self.values)
            .all(|(source_values, target_values)| {
                let mut rhs = context.zero();
                for (key, coefficient) in terms {
                    let term = context
                        .try_mul(coefficient, &integrand(context, target_values, key), limits)
                        .unwrap();
                    rhs = context.try_add(&rhs, &term, limits).unwrap();
                }
                integrand(context, source_values, source) == rhs
            })
    }
}

fn pairs(
    transport: &rustred::sector::symmetry::integral_transport::TransportedIntegral,
) -> Vec<(IntegralKey, Coefficient)> {
    transport
        .terms()
        .iter()
        .map(|term| (term.key().clone(), term.coefficient().clone()))
        .collect()
}

/// `I - T(I)` merged exactly, as a relation row must read.
fn expected_row(
    context: &CoefficientContext,
    source: &IntegralKey,
    image: &[(IntegralKey, Coefficient)],
) -> Vec<(IntegralKey, Coefficient)> {
    let limits = ExactAlgebraLimits::unbounded_resources();
    let mut terms = BTreeMap::from([(source.clone(), context.one())]);
    for (key, value) in image {
        accumulate(
            context,
            &mut terms,
            key,
            &context.try_neg(value, limits).unwrap(),
        );
    }
    terms.into_iter().collect()
}

/// Plain recursion over the public API, without coalescing.
fn naive_normal_form(
    census: &SectorSymmetryCensus,
    source: &IntegralKey,
) -> BTreeMap<IntegralKey, Coefficient> {
    let context = census.family().coefficient_context();
    let limits = ExactAlgebraLimits::unbounded_resources();
    let sector = Mask::try_from_indices(source.powers()).unwrap();
    if census.is_zero_sector(&sector) {
        return BTreeMap::new();
    }
    if census.member(&sector).unwrap().is_representative() {
        return BTreeMap::from([(source.clone(), context.one())]);
    }
    let mut result = BTreeMap::new();
    for term in census.transport_to_representative(source).unwrap().terms() {
        for (key, value) in naive_normal_form(census, term.key()) {
            let value = context.try_mul(term.coefficient(), &value, limits).unwrap();
            accumulate(context, &mut result, &key, &value);
        }
    }
    result
}

fn accumulate(
    context: &CoefficientContext,
    terms: &mut BTreeMap<IntegralKey, Coefficient>,
    key: &IntegralKey,
    value: &Coefficient,
) {
    let limits = ExactAlgebraLimits::unbounded_resources();
    let sum = match terms.get(key) {
        Some(old) => context.try_add(old, value, limits).unwrap(),
        None => value.clone(),
    };
    if sum.is_zero() {
        terms.remove(key);
    } else {
        terms.insert(key.clone(), sum);
    }
}

/// `T^-1(T(I)) = I` exactly, as a unique Laurent-monomial decomposition.
fn round_trip(
    family: &Arc<IntegralFamily>,
    forward: &Prepared,
    backward: &Prepared,
    source: &IntegralKey,
) -> bool {
    let context = family.coefficient_context();
    let limits = ExactAlgebraLimits::unbounded_resources();
    let mut parametric = false;
    let mut result = BTreeMap::new();
    for term in forward
        .transport(source, ExpansionLimits::default())
        .unwrap()
        .terms()
    {
        parametric |= !term.coefficient().is_constant();
        for back in backward
            .transport(term.key(), ExpansionLimits::default())
            .unwrap()
            .terms()
        {
            let value = context
                .try_mul(term.coefficient(), back.coefficient(), limits)
                .unwrap();
            accumulate(context, &mut result, back.key(), &value);
        }
    }
    assert_eq!(result, BTreeMap::from([(source.clone(), context.one())]));
    parametric
}

fn probe_keys(sector: &Mask) -> Vec<IntegralKey> {
    let active: Vec<_> = (0..PHYSICAL)
        .filter(|&slot| sector.active_bits()[slot])
        .collect();
    let inactive = (0..PHYSICAL).find(|&slot| !sector.active_bits()[slot]);
    let base: Vec<i64> = (0..10)
        .map(|slot| i64::from(sector.active_bits()[slot]))
        .collect();
    let mut keys = vec![base.clone()];
    let mut isp = base.clone();
    isp[8] = -1;
    isp[9] = -1;
    keys.push(isp);
    let mut dotted = base.clone();
    dotted[active[0]] = 2;
    dotted[9] = -2;
    keys.push(dotted);
    if let Some(slot) = inactive {
        let mut line = base.clone();
        line[slot] = -1;
        line[8] = -1;
        keys.push(line);
    }
    // Every inactive physical line once, plus a dot on the last line.
    let mut lines = base.clone();
    for slot in (0..PHYSICAL).filter(|&slot| !sector.active_bits()[slot]) {
        lines[slot] = -1;
    }
    lines[active[active.len() - 1]] += 1;
    keys.push(lines);
    // A deformed-slot numerator whenever slot 0 or 1 is inactive.
    for slot in [0, 1] {
        if !sector.active_bits()[slot] {
            let mut deformed = base.clone();
            deformed[slot] = -2;
            deformed[9] = -1;
            keys.push(deformed);
        }
    }
    keys.into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|powers| IntegralKey::try_new(powers).unwrap())
        .collect()
}

struct Outcome {
    classes: usize,
    nonzero: usize,
    nontrivial_automorphisms: Option<usize>,
    digest: String,
}

fn inverse_transport(
    family: &Arc<IntegralFamily>,
    a: &[[i64; 4]; 4],
    source: &Mask,
    target: &Mask,
) -> Prepared {
    let context = family.coefficient_context();
    compile_with_affine_constants(
        family,
        family.clone(),
        Arc::new(
            verify(
                family,
                family,
                momentum(context, &unimodular_inverse(a)),
                Limits::default(),
            )
            .unwrap(),
        ),
        source.clone(),
        target.clone(),
        ExpansionLimits::default(),
        AffineConstants::Parametric,
    )
    .unwrap()
}

fn check_variant(
    name: &str,
    deformed: Option<usize>,
    kira: &[(&[u16], usize)],
    kira_representatives: &[u16],
) -> Outcome {
    let family = p14(deformed);
    let context = family.coefficient_context();
    let analyzer = Analyzer::try_unrestricted(&family).unwrap();
    let start = Instant::now();
    let census = SectorSymmetryCensus::discover_subsectors(
        family.clone(),
        &top(),
        &analyzer,
        Default::default(),
        &Default::default(),
    )
    .unwrap();
    let elapsed = start.elapsed();
    let statistics = census.statistics().clone();
    assert!(census.skips().is_empty(), "{name}: {:?}", census.skips());

    // Partition and in-sector automorphism groups equal Kira's.
    let partition = |census: &SectorSymmetryCensus| -> BTreeSet<(BTreeSet<u16>, Option<usize>)> {
        census
            .classes()
            .iter()
            .map(|class| {
                (
                    class.members().iter().map(sector_id).collect(),
                    class.line_group_order().map(|order| order - 1),
                )
            })
            .collect()
    };
    let ours = partition(&census);
    let oracle: BTreeSet<(BTreeSet<u16>, Option<usize>)> = kira
        .iter()
        .map(|(members, symmetries)| (members.iter().copied().collect(), Some(*symmetries)))
        .collect();
    assert_eq!(
        ours, oracle,
        "{name}: sector partition or group orders differ"
    );

    // The zero set is retained: every enumerated subsector is either declared
    // or zero, and so is the empty sector.
    assert_eq!(
        census.zero_sectors().len(),
        statistics.zero_sectors + 1,
        "{name}: empty sector"
    );
    for id in 0..256u16 {
        let mask = mask_of(id);
        assert_ne!(
            census.member(&mask).is_some(),
            census.is_zero_sector(&mask),
            "{name}: sector {id}"
        );
    }

    let points = Points::new(context, deformed, 0x9e37_79b9_7f4a_7c15 ^ name.len() as u64);
    let mut parametric_routes = 0;
    let mut checked_keys = 0;
    let mut identity_checks = 0;
    let mut wrong_convention_failures = 0;
    let mut determinant_signs = BTreeMap::<i64, usize>::new();
    let mut endpoints = [0usize; 3]; // representative, other nonzero, zero
    let mut tally = |census: &SectorSymmetryCensus, terms: &[(IntegralKey, Coefficient)]| {
        for (key, _) in terms {
            let sector = Mask::try_from_indices(key.powers()).unwrap();
            let slot = if census.is_zero_sector(&sector) {
                2
            } else if census.member(&sector).unwrap().is_representative() {
                0
            } else {
                1
            };
            endpoints[slot] += 1;
        }
    };

    // Every route is re-checked independently.
    for member in census.members() {
        let map = member.verified_map();
        assert!(matches!(map.jacobian(), Jacobian::Unit { .. }));
        check_route_geometry(
            &family,
            member.sector(),
            member.representative(),
            map.momentum(),
        );
        let fresh = verify(&family, &family, map.momentum().clone(), Limits::default()).unwrap();
        for slot in (0..PHYSICAL).filter(|&slot| member.sector().active_bits()[slot]) {
            assert!(matches!(
                &fresh.row_actions()[slot],
                DenominatorAction::Monomial { scale, .. } if scale == &context.one()
            ));
        }
        if member.is_representative() {
            continue;
        }
        let a = integer_matrix(context, map.momentum().loop_linear());
        *determinant_signs.entry(determinant(&a)).or_default() += 1;
        let pulled = points.pulled_back(context, deformed, &a);
        let wrong = points.pulled_back(context, deformed, &unimodular_inverse(&a));
        let backward = inverse_transport(&family, &a, member.representative(), member.sector());
        for key in probe_keys(member.sector()) {
            let transported = pairs(&census.transport_to_representative(&key).unwrap());
            assert!(
                points.identity_holds(context, &pulled, &key, &transported),
                "{name}: route integrand identity fails for {key:?}"
            );
            identity_checks += 1;
            wrong_convention_failures +=
                usize::from(!points.identity_holds(context, &wrong, &key, &transported));
            tally(&census, &transported);
            let relation = census.route_relation(&key).unwrap().unwrap();
            assert_eq!(relation.source(), &key);
            assert_eq!(relation.terms(), expected_row(context, &key, &transported));
            parametric_routes += usize::from(round_trip(&family, member.route(), &backward, &key));
            checked_keys += 1;
        }
    }
    for class in census.classes() {
        let representative = class.representative();
        for (generator, images) in class.automorphisms().iter().zip(class.line_permutations()) {
            let map = generator.verified_map().momentum();
            check_route_geometry(&family, representative, representative, map);
            assert_eq!(images.len(), representative.active_count());
            let a = integer_matrix(context, map.loop_linear());
            let pulled = points.pulled_back(context, deformed, &a);
            let backward = inverse_transport(&family, &a, representative, representative);
            for key in probe_keys(representative) {
                let transported = pairs(
                    &generator
                        .transport(&key, ExpansionLimits::default())
                        .unwrap(),
                );
                assert!(
                    points.identity_holds(context, &pulled, &key, &transported),
                    "{name}: automorphism integrand identity fails for {key:?}"
                );
                identity_checks += 1;
                round_trip(&family, generator, &backward, &key);
                checked_keys += 1;
            }
        }
        for key in probe_keys(representative) {
            // `T_g(I)` may contain `I` itself; the row then relates the rest.
            for relation in census.automorphism_relations(&key).unwrap() {
                assert_eq!(relation.source(), &key);
                assert!(!relation.terms().is_empty());
                let generator = &class.automorphisms()[relation.automorphism().unwrap()];
                let image = pairs(
                    &generator
                        .transport(&key, ExpansionLimits::default())
                        .unwrap(),
                );
                assert_eq!(relation.terms(), expected_row(context, &key, &image));
            }
        }
    }
    assert!(identity_checks > 0);
    assert!(
        wrong_convention_failures > 0,
        "{name}: the integrand check does not discriminate the map convention"
    );
    if deformed.is_some() {
        assert!(parametric_routes > 0, "{name}: eta never entered a route");
        // Both orientations of the unimodular route lattice occur.
        assert!(
            determinant_signs.len() == 2,
            "{name}: {determinant_signs:?}"
        );
    } else {
        assert_eq!(parametric_routes, 0);
    }

    // Recursive normal forms land on representative sectors and agree with a
    // plain recursion; route rows on representatives are I - normal form.
    let mut normalized = 0;
    let mut deepest = 0;
    let mut keys = Vec::new();
    for member in census.members() {
        keys.extend(probe_keys(member.sector()));
    }
    let batch = census.normalize_many(&keys).unwrap();
    for (key, normal) in keys.iter().zip(&batch) {
        let reference: Vec<_> = naive_normal_form(&census, key).into_iter().collect();
        assert_eq!(normal.terms(), reference.as_slice(), "{name}: {key:?}");
        for (endpoint, _) in normal.terms() {
            let sector = Mask::try_from_indices(endpoint.powers()).unwrap();
            assert!(census.member(&sector).unwrap().is_representative());
        }
        if let Some(row) = census
            .route_relation_with(key, RelationEndpoints::Representatives)
            .unwrap()
        {
            assert_eq!(row.terms(), expected_row(context, key, normal.terms()));
        }
        deepest = deepest.max(normal.transports());
        normalized += 1;
    }
    assert!(deepest >= 2, "{name}: no recursive route");

    // Kira's own representatives, as a caller preference, are reproduced
    // class by class without changing the partition.
    let preferred = SectorSymmetryCensus::discover_subsectors(
        family.clone(),
        &top(),
        &analyzer,
        SectorSymmetryCensusOptions {
            representative: RepresentativePolicy::Preferred(
                kira_representatives.iter().map(|&id| mask_of(id)).collect(),
            ),
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    assert_eq!(partition(&preferred), ours);
    let representatives: BTreeSet<u16> = preferred
        .classes()
        .iter()
        .map(|class| sector_id(class.representative()))
        .collect();
    assert_eq!(
        representatives,
        kira_representatives.iter().copied().collect(),
        "{name}: Kira representatives"
    );

    // Determinism: a fresh census reproduces every choice exactly.
    let digest = digest(&census);
    let again = SectorSymmetryCensus::discover_subsectors(
        p14(deformed),
        &top(),
        &Analyzer::try_unrestricted(&family).unwrap(),
        Default::default(),
        &Default::default(),
    )
    .unwrap();
    assert_eq!(digest, self::digest(&again));

    eprintln!(
        "{name}: {} nonzero / {} zero sectors -> {} classes ({} routed), {} generators, \
         sum(|G|-1) = {:?}, {} keys round-tripped ({} with eta), {} integrand identities \
         ({} fail under the opposite convention), route det signs {:?}, transported endpoints \
         [representative, other nonzero, zero] = {:?}, {} normal forms (max {} routes), \
         census {:.3} s, digest {}",
        statistics.declared_sectors,
        statistics.zero_sectors,
        statistics.classes,
        statistics.routed_sectors,
        statistics.automorphism_generators,
        statistics.nontrivial_line_automorphisms,
        checked_keys,
        parametric_routes,
        identity_checks,
        wrong_convention_failures,
        determinant_signs,
        endpoints,
        normalized,
        deepest,
        elapsed.as_secs_f64(),
        &digest[..16],
    );
    Outcome {
        classes: statistics.classes,
        nonzero: statistics.declared_sectors,
        nontrivial_automorphisms: statistics.nontrivial_line_automorphisms,
        digest,
    }
}

fn digest(census: &SectorSymmetryCensus) -> String {
    let mut text = String::new();
    for member in census.members() {
        text.push_str(&format!(
            "{}>{}:{:?};",
            member.sector(),
            member.representative(),
            member.verified_map().momentum().loop_linear().entries()
        ));
    }
    for class in census.classes() {
        for generator in class.automorphisms() {
            text.push_str(&format!(
                "{}@{:?};",
                class.representative(),
                generator.verified_map().momentum().loop_linear().entries()
            ));
        }
    }
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

#[test]
fn undeformed_p14_matches_kira_eleven_classes() {
    let outcome = check_variant(
        "undeformed",
        None,
        KIRA_UNDEFORMED,
        KIRA_UNDEFORMED_REPRESENTATIVES,
    );
    assert_eq!((outcome.nonzero, outcome.classes), (134, 11));
    assert_eq!(outcome.nontrivial_automorphisms, Some(333));
    assert_eq!(outcome.digest.len(), 64);
}

#[test]
fn rim_deformed_p14_matches_kira_twenty_seven_classes() {
    let outcome = check_variant(
        "rim (slot 0, symbolic eta)",
        Some(0),
        KIRA_RIM,
        KIRA_RIM_REPRESENTATIVES,
    );
    assert_eq!((outcome.nonzero, outcome.classes), (134, 27));
    assert_eq!(outcome.nontrivial_automorphisms, Some(377));
}

#[test]
fn spoke_deformed_p14_matches_kira_twenty_six_classes() {
    let outcome = check_variant(
        "spoke (slot 1, symbolic eta)",
        Some(1),
        KIRA_SPOKE,
        KIRA_SPOKE_REPRESENTATIVES,
    );
    assert_eq!((outcome.nonzero, outcome.classes), (134, 26));
    assert_eq!(outcome.nontrivial_automorphisms, Some(394));
}
