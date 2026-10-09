//! Measure-preserving routing oracle for the supplied three-loop contour tests.
//!
//! Vertices are `[0, P, Q, R]`; an edge is a squared momentum difference.
//! Choosing a new origin, permuting the other vertices, and optionally reversing
//! every momentum has unit absolute Jacobian. Contour shifts are transformed
//! along with momenta. Energy numerators are expanded, never relabelled as if
//! they were scalars. The last three key entries are negative energy degrees.

use std::collections::{BTreeMap, BTreeSet};

pub(super) type ReferenceKey = [i16; 9];
pub(super) type RoutedTerms = Vec<(ReferenceKey, i64)>;

const EDGES: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
const MAX_ENERGY_DEGREE: usize = 24;
const MAX_REFLECTION_TERMS: usize = 16_384;

#[derive(Clone, Copy)]
struct Routing {
    old_vertices: [usize; 4],
    sign: i8,
}

/// The supplied binary contour classes: `BB+`, `++B`, or the vacuum class.
pub(super) fn canonical_contours(contours: [i8; 3]) -> Result<[i8; 3], String> {
    let shifts = [0, contours[0], contours[1], contours[2]];
    let lower = *shifts.iter().min().unwrap();
    let upper = *shifts.iter().max().unwrap();
    if lower == upper {
        return Ok([0; 3]);
    }
    if i16::from(upper) - i16::from(lower) != 1 {
        return Err(
            "reference routing supports only two adjacent chemical-potential levels".into(),
        );
    }
    match shifts.iter().filter(|&&value| value == lower).count() {
        1 | 3 => Ok([0, 0, 1]),
        2 => Ok([1, 1, 0]),
        _ => unreachable!("two nonempty contour levels partition four vertices"),
    }
}

/// Route one term to its canonical contour class and denominator permutation.
///
/// For energy tensors this deterministic representative is not a projection
/// onto all symmetry relations. Include [`symmetry_images`] as equations when
/// comparing linear combinations of separately canonicalized tensor terms.
pub(super) fn canonicalize(contours: [i8; 3], key: ReferenceKey) -> Result<RoutedTerms, String> {
    validate_key(&key)?;
    let target = canonical_contours(contours)?;
    let mut best: Option<([i16; 6], Routing)> = None;
    for routing in routings() {
        if routed_contours(contours, routing) != target {
            continue;
        }
        let powers = routed_denominators(&key, routing);
        if best.as_ref().is_none_or(|(previous, _)| powers < *previous) {
            best = Some((powers, routing));
        }
    }
    let (_, routing) = best
        .ok_or_else(|| "no measure-preserving route to the canonical contour class".to_string())?;
    transform(key, routing)
}

/// All exact images under routings preserving the supplied contour triple.
/// Subtract the original term from each image to obtain symmetry source rows.
pub(super) fn symmetry_images(
    contours: [i8; 3],
    key: ReferenceKey,
) -> Result<Vec<RoutedTerms>, String> {
    canonical_contours(contours)?;
    validate_key(&key)?;
    let mut images = BTreeSet::new();
    for routing in routings() {
        if routed_contours(contours, routing) == contours {
            images.insert(transform(key, routing)?);
        }
    }
    Ok(images.into_iter().collect())
}

/// Reflect a two-line subintegration through its two attachment vertices.
///
/// For four distinct vertices `(x,u,v,w)`, the map `x -> u+v-x` swaps the
/// `xu` and `xv` edges. It is admitted only when the remaining `xw` edge is
/// polynomial: its image is expanded in the original six-denominator basis.
/// When `x=0`, subtract the reflected origin from every new vertex. Contours
/// and energy numerators undergo that same invertible linear transformation.
pub(super) fn bubble_reflections(
    contours: [i8; 3],
    key: ReferenceKey,
) -> Result<Vec<([i8; 3], RoutedTerms)>, String> {
    canonical_contours(contours)?;
    validate_key(&key)?;
    let mut result = BTreeSet::new();
    for x in 0..4 {
        for w in 0..4 {
            if x == w {
                continue;
            }
            let missing = edge_index(x, w);
            if key[missing] > 0 {
                continue;
            }
            let attachments: Vec<_> = (0..4).filter(|&v| v != x && v != w).collect();
            let (u, v) = (attachments[0], attachments[1]);
            let matrix = reflection_matrix(x, u, v);
            let reflected: [i64; 3] = std::array::from_fn(|row| {
                matrix[row]
                    .iter()
                    .zip(contours)
                    .map(|(coefficient, shift)| coefficient * i64::from(shift))
                    .sum::<i64>()
            });
            let mut new_contours = [0_i8; 3];
            for axis in 0..3 {
                new_contours[axis] = i8::try_from(reflected[axis])
                    .map_err(|_| "reflected contour shift overflow".to_string())?;
            }
            if canonical_contours(new_contours).is_err() {
                continue;
            }

            let mut quadratic = [0_i64; 6];
            quadratic[edge_index(u, w)] += 1;
            quadratic[edge_index(v, w)] += 1;
            quadratic[edge_index(u, v)] -= 1;
            quadratic[edge_index(x, u)] += 1;
            quadratic[edge_index(x, v)] += 1;
            quadratic[missing] -= 1;
            let mut denominators = BTreeMap::from([([0_i16; 6], 1_i64)]);
            for _ in 0..key[missing].unsigned_abs() {
                denominators = multiply_linear(denominators, &quadratic)?;
            }
            let mut energies = BTreeMap::from([([0_i16; 3], 1_i64)]);
            for axis in 0..3 {
                for _ in 0..key[6 + axis].unsigned_abs() {
                    energies = multiply_linear(energies, &matrix[axis])?;
                }
            }
            if denominators
                .len()
                .checked_mul(energies.len())
                .is_none_or(|count| count > MAX_REFLECTION_TERMS)
            {
                return Err("reference reflection term budget exhausted".into());
            }
            let mut base = key;
            base.swap(edge_index(x, u), edge_index(x, v));
            base[missing] = 0;
            let mut terms = BTreeMap::new();
            for (powers, denominator_coefficient) in &denominators {
                for (energy, energy_coefficient) in &energies {
                    let mut output = base;
                    for axis in 0..6 {
                        output[axis] = output[axis]
                            .checked_sub(powers[axis])
                            .ok_or_else(|| "reference reflection index overflow".to_string())?;
                    }
                    for axis in 0..3 {
                        output[6 + axis] = -energy[axis];
                    }
                    let coefficient = denominator_coefficient
                        .checked_mul(*energy_coefficient)
                        .ok_or_else(|| "reference reflection coefficient overflow".to_string())?;
                    let entry = terms.entry(output).or_insert(0_i64);
                    *entry = entry
                        .checked_add(coefficient)
                        .ok_or_else(|| "reference reflection coefficient overflow".to_string())?;
                }
            }
            terms.retain(|_, coefficient| *coefficient != 0);
            result.insert((new_contours, terms.into_iter().collect()));
        }
    }
    Ok(result.into_iter().collect())
}

fn edge_index(left: usize, right: usize) -> usize {
    let edge = (left.min(right), left.max(right));
    EDGES
        .iter()
        .position(|&candidate| candidate == edge)
        .expect("distinct four-vertex endpoints")
}

fn reflection_matrix(x: usize, u: usize, v: usize) -> [[i64; 3]; 3] {
    let mut matrix =
        std::array::from_fn(|row| std::array::from_fn(|column| i64::from(row == column)));
    if x == 0 {
        for row in &mut matrix {
            row[u - 1] -= 1;
            row[v - 1] -= 1;
        }
    } else {
        matrix[x - 1] = [0; 3];
        matrix[x - 1][x - 1] = -1;
        for vertex in [u, v] {
            if vertex != 0 {
                matrix[x - 1][vertex - 1] += 1;
            }
        }
    }
    matrix
}

fn multiply_linear<const N: usize>(
    polynomial: BTreeMap<[i16; N], i64>,
    linear: &[i64; N],
) -> Result<BTreeMap<[i16; N], i64>, String> {
    let mut result = BTreeMap::new();
    for (degree, coefficient) in polynomial {
        for (axis, &factor) in linear.iter().enumerate() {
            if factor == 0 {
                continue;
            }
            let mut term = degree;
            term[axis] = term[axis]
                .checked_add(1)
                .ok_or_else(|| "reference reflection degree overflow".to_string())?;
            let value = coefficient
                .checked_mul(factor)
                .ok_or_else(|| "reference reflection coefficient overflow".to_string())?;
            let entry = result.entry(term).or_insert(0_i64);
            *entry = entry
                .checked_add(value)
                .ok_or_else(|| "reference reflection coefficient overflow".to_string())?;
        }
        if result.len() > MAX_REFLECTION_TERMS {
            return Err("reference reflection term budget exhausted".into());
        }
    }
    result.retain(|_, coefficient| *coefficient != 0);
    Ok(result)
}

/// Prove a genuine scaleless integration variable or vacuum block.
///
/// A block can be translated to vacuum variables only when every vertex in it
/// has the same contour as its unique attachment vertex. Polynomial numerators
/// do not introduce a scale into those vacuum tensor integrations. A block
/// with no attachment has a denominator-free common translation variable.
/// No other missing-propagator pattern is declared zero.
pub(super) fn is_scaleless(contours: [i8; 3], key: ReferenceKey) -> Result<bool, String> {
    canonical_contours(contours)?;
    validate_key(&key)?;
    let shifts = [0, contours[0], contours[1], contours[2]];
    // Only integrated vertices may belong to the candidate vacuum block.
    for mask in 1_u8..8 {
        let inside = |vertex: usize| vertex != 0 && mask & (1 << (vertex - 1)) != 0;
        let mut attachments = BTreeSet::new();
        for (edge, &(left, right)) in EDGES.iter().enumerate() {
            if key[edge] <= 0 || inside(left) == inside(right) {
                continue;
            }
            attachments.insert(if inside(left) { right } else { left });
        }
        if attachments.is_empty() {
            return Ok(true);
        }
        if attachments.len() == 1 {
            let attachment = *attachments.first().unwrap();
            if (1..4)
                .filter(|&vertex| inside(vertex))
                .all(|vertex| shifts[vertex] == shifts[attachment])
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn validate_key(key: &ReferenceKey) -> Result<(), String> {
    if key[6..].iter().any(|&power| power > 0) {
        return Err("reference routing requires polynomial energy numerators".into());
    }
    let degree: usize = key[6..]
        .iter()
        .map(|&power| usize::from(power.unsigned_abs()))
        .sum();
    if degree > MAX_ENERGY_DEGREE {
        return Err("reference routing energy-expansion degree budget exhausted".into());
    }
    Ok(())
}

fn routings() -> Vec<Routing> {
    let mut result = Vec::with_capacity(48);
    for origin in 0..4 {
        for first in 0..4 {
            if first == origin {
                continue;
            }
            for second in 0..4 {
                if second == origin || second == first {
                    continue;
                }
                let third = (0..4)
                    .find(|&vertex| vertex != origin && vertex != first && vertex != second)
                    .unwrap();
                // Prefer the identity when a denominator tuple is already
                // canonical, including the vacuum contour class.
                for sign in [1, -1] {
                    result.push(Routing {
                        old_vertices: [origin, first, second, third],
                        sign,
                    });
                }
            }
        }
    }
    result
}

fn routed_contours(contours: [i8; 3], routing: Routing) -> [i8; 3] {
    let shifts = [0, contours[0], contours[1], contours[2]];
    std::array::from_fn(|axis| {
        routing.sign * (shifts[routing.old_vertices[axis + 1]] - shifts[routing.old_vertices[0]])
    })
}

fn inverse(routing: Routing) -> [usize; 4] {
    let mut result = [0; 4];
    for (new, old) in routing.old_vertices.into_iter().enumerate() {
        result[old] = new;
    }
    result
}

fn routed_denominators(key: &ReferenceKey, routing: Routing) -> [i16; 6] {
    let inverse = inverse(routing);
    let mut result = [0; 6];
    for (edge, &(left, right)) in EDGES.iter().enumerate() {
        let mut transformed = [inverse[left], inverse[right]];
        transformed.sort();
        let target = EDGES
            .iter()
            .position(|&(a, b)| [a, b] == transformed)
            .unwrap();
        result[target] = key[edge];
    }
    result
}

fn transform(key: ReferenceKey, routing: Routing) -> Result<RoutedTerms, String> {
    let inverse = inverse(routing);
    let mut polynomial = BTreeMap::from([([0_i16; 3], 1_i64)]);
    for old_axis in 0..3 {
        let mut linear = [0_i64; 3];
        // old E_v = sign * (new E_position(v) - new E_position(old origin 0)).
        if inverse[old_axis + 1] != 0 {
            linear[inverse[old_axis + 1] - 1] += i64::from(routing.sign);
        }
        if inverse[0] != 0 {
            linear[inverse[0] - 1] -= i64::from(routing.sign);
        }
        for _ in 0..key[6 + old_axis].unsigned_abs() {
            polynomial = multiply_linear(polynomial, &linear)?;
        }
    }
    let denominators = routed_denominators(&key, routing);
    Ok(polynomial
        .into_iter()
        .map(|(degree, coefficient)| {
            let mut output = [0; 9];
            output[..6].copy_from_slice(&denominators);
            for axis in 0..3 {
                output[6 + axis] = -degree[axis];
            }
            (output, coefficient)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TadpoleSunsetSignature {
        tadpole_contour: i8,
        tadpole_power: i16,
        sunset_contours: [i8; 2],
        sunset_powers: [i16; 3],
    }

    /// An articulation decomposition, established from positive edges rather
    /// than a supplied integral equality. A leaf-edge momentum and the two
    /// momenta in the remaining triangle form an invertible unit-Jacobian
    /// coordinate set. Their denominator factors and contour shifts separate.
    fn tadpole_sunset_signature(
        contours: [i8; 3],
        point: ReferenceKey,
    ) -> Option<TadpoleSunsetSignature> {
        if point[6..] != [0; 3] || point[..6].iter().any(|&power| power < 0) {
            return None;
        }
        let shifts = [0, contours[0], contours[1], contours[2]];
        for leaf in 0..4 {
            let incident: Vec<_> = EDGES
                .iter()
                .enumerate()
                .filter(|(edge, (a, b))| point[*edge] > 0 && (*a == leaf || *b == leaf))
                .collect();
            if incident.len() != 1 {
                continue;
            }
            let (edge, &(a, b)) = incident[0];
            let attachment = if a == leaf { b } else { a };
            let triangle: Vec<_> = (0..4).filter(|&vertex| vertex != leaf).collect();
            let edge_power = |a: usize, b: usize| {
                let pair = (a.min(b), a.max(b));
                point[EDGES.iter().position(|&edge| edge == pair).unwrap()]
            };
            if (0..3).any(|i| ((i + 1)..3).any(|j| edge_power(triangle[i], triangle[j]) <= 0)) {
                continue;
            }
            let mut canonical = None;
            for &origin in &triangle {
                let others: Vec<_> = triangle
                    .iter()
                    .copied()
                    .filter(|&vertex| vertex != origin)
                    .collect();
                for order in [[others[0], others[1]], [others[1], others[0]]] {
                    for sign in [1_i8, -1] {
                        let contour = order.map(|vertex| sign * (shifts[vertex] - shifts[origin]));
                        if contour != [1, 1] {
                            continue;
                        }
                        let powers = [
                            edge_power(origin, order[0]),
                            edge_power(origin, order[1]),
                            edge_power(order[0], order[1]),
                        ];
                        if canonical.is_none_or(|previous| powers < previous) {
                            canonical = Some(powers);
                        }
                    }
                }
            }
            let sunset_powers = canonical?;
            return Some(TadpoleSunsetSignature {
                tadpole_contour: (shifts[leaf] - shifts[attachment]).abs(),
                tadpole_power: point[edge],
                sunset_contours: [1, 1],
                sunset_powers,
            });
        }
        None
    }

    fn key(powers: [i16; 6]) -> ReferenceKey {
        let mut result = [0; 9];
        result[..6].copy_from_slice(&powers);
        result
    }

    #[test]
    fn supplied_zero_sectors_have_explicit_vacuum_blocks() {
        for powers in [
            [1, 0, 0, 1, 1, 1],
            [0, 0, 1, 1, 1, 1],
            [1, 0, 1, 1, 1, 0],
            [1, 0, 1, 0, 1, 1],
        ] {
            assert!(is_scaleless([1, 1, 1], key(powers)).unwrap());
        }
        // A finite-density tadpole and B/F bubbles cannot be discarded by
        // treating shifted contours as vacuum propagators.
        assert!(!is_scaleless([1, 1, 1], key([1, 1, 1, 0, 0, 0])).unwrap());
        assert!(!is_scaleless([0, 0, 1], key([1, 1, 0, 0, 1, 1])).unwrap());
        assert!(!is_scaleless([1, 1, 0], key([1, 1, 0, 0, 1, 1])).unwrap());
    }

    #[test]
    fn origin_change_expands_shifted_energy_numerators_exactly() {
        // New P=Pold-Qold, Q=Pold-Rold, R=Pold. Hence old Q0=R0-P0.
        let routing = Routing {
            old_vertices: [1, 2, 3, 0],
            sign: -1,
        };
        assert_eq!(routed_contours([1, 1, 1], routing), [0, 0, 1]);
        let mut point = key([1; 6]);
        point[7] = -2;
        let image = transform(point, routing).unwrap();
        let energies: BTreeMap<_, _> = image
            .into_iter()
            .map(|(point, coefficient)| ([point[6], point[7], point[8]], coefficient))
            .collect();
        assert_eq!(
            energies,
            BTreeMap::from([([-2, 0, 0], 1), ([-1, 0, -1], -2), ([0, 0, -2], 1)])
        );
        assert_eq!(canonical_contours([1, 1, 1]).unwrap(), [0, 0, 1]);
        assert_eq!(canonical_contours([1, 1, 0]).unwrap(), [1, 1, 0]);
    }

    #[test]
    fn scalar_canonicalization_agrees_across_measure_preserving_images() {
        let point = key([1, 2, 1, 0, -1, 1]);
        let reference = canonicalize([1, 1, 1], point).unwrap();
        for routing in routings() {
            let image = transform(point, routing).unwrap();
            assert_eq!(image.len(), 1);
            assert_eq!(image[0].1, 1);
            assert_eq!(
                canonicalize(routed_contours([1, 1, 1], routing), image[0].0).unwrap(),
                reference
            );
        }
        assert!(!symmetry_images([1, 1, 0], point).unwrap().is_empty());
    }

    #[test]
    fn supplied_factorization_graphs_separate_the_same_tadpole_and_sunset() {
        let expected = TadpoleSunsetSignature {
            tadpole_contour: 1,
            tadpole_power: 1,
            sunset_contours: [1, 1],
            sunset_powers: [1, 1, 1],
        };
        for (contours, powers) in [
            ([1, 1, 1], [1, 1, 1, 1, 0, 0]),
            ([1, 1, 1], [1, 1, 1, 0, 1, 0]),
            ([1, 1, 0], [1, 1, 1, 0, 1, 0]),
            ([1, 1, 0], [1, 1, 0, 1, 0, 1]),
        ] {
            assert_eq!(
                tadpole_sunset_signature(contours, key(powers)),
                Some(TadpoleSunsetSignature { ..expected })
            );
        }
        // A connected two-bubble graph has no such factorization certificate.
        assert!(tadpole_sunset_signature([0, 0, 1], key([1, 1, 0, 0, 1, 1])).is_none());
    }

    #[test]
    fn general_bubble_reflections_are_unimodular_involutions_with_the_stated_quadratic_image() {
        let square = |v: [i64; 3]| {
            [
                v[0] * v[0] + v[0] * v[1] + v[0] * v[2],
                v[1] * v[1] + v[0] * v[1] + v[1] * v[2],
                v[2] * v[2] + v[0] * v[2] + v[1] * v[2],
                -v[0] * v[1],
                -v[0] * v[2],
                -v[1] * v[2],
            ]
        };
        for x in 0..4 {
            for w in 0..4 {
                if x == w {
                    continue;
                }
                let attachments: Vec<_> = (0..4)
                    .filter(|&vertex| vertex != x && vertex != w)
                    .collect();
                let (u, v) = (attachments[0], attachments[1]);
                let matrix = reflection_matrix(x, u, v);
                let determinant = matrix[0][0]
                    * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
                    - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
                    + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);
                assert_eq!(determinant.abs(), 1);
                for row in 0..3 {
                    for column in 0..3 {
                        assert_eq!(
                            (0..3)
                                .map(|k| matrix[row][k] * matrix[k][column])
                                .sum::<i64>(),
                            i64::from(row == column)
                        );
                    }
                }
                for (edge, &(left, right)) in EDGES.iter().enumerate() {
                    let vertex = |v: usize| if v == 0 { [0; 3] } else { matrix[v - 1] };
                    let (left, right) = (vertex(left), vertex(right));
                    let actual = square(std::array::from_fn(|axis| left[axis] - right[axis]));
                    let mut expected = [0_i64; 6];
                    if edge == edge_index(x, w) {
                        for (a, b, sign) in [
                            (u, w, 1),
                            (v, w, 1),
                            (u, v, -1),
                            (x, u, 1),
                            (x, v, 1),
                            (x, w, -1),
                        ] {
                            expected[edge_index(a, b)] += sign;
                        }
                    } else {
                        let target = if edge == edge_index(x, u) {
                            edge_index(x, v)
                        } else if edge == edge_index(x, v) {
                            edge_index(x, u)
                        } else {
                            edge
                        };
                        expected[target] = 1;
                    }
                    assert_eq!(actual, expected, "x={x}, w={w}, edge={edge}");
                }
            }
        }
    }

    #[test]
    fn bubble_reflection_expansion_handles_squared_numerators_and_shifted_origin_energies() {
        let momenta = [[2_i128, 1, -1], [3, -2, 4], [-1, 5, 2]];
        let denominator_values = |momenta: &[[i128; 3]; 3]| {
            EDGES.map(|(left, right)| {
                let vertex = |v: usize| if v == 0 { [0; 3] } else { momenta[v - 1] };
                let (left, right) = (vertex(left), vertex(right));
                (0..3)
                    .map(|axis| (left[axis] - right[axis]).pow(2))
                    .sum::<i128>()
            })
        };
        let values = denominator_values(&momenta);
        for contours in [[1_i8, 1, 1], [1, 1, 0], [0, 0, 1]] {
            for (missing, &(left, right)) in EDGES.iter().enumerate() {
                let mut point = key([1; 6]);
                point[missing] = -2;
                point[6..].copy_from_slice(&[-1, -2, -1]);
                let mut base = [1_i16; 6];
                base[missing] = 0;
                let mut expected = BTreeSet::new();
                for (x, w) in [(left, right), (right, left)] {
                    let attachments: Vec<_> = (0..4)
                        .filter(|&vertex| vertex != x && vertex != w)
                        .collect();
                    let matrix = reflection_matrix(x, attachments[0], attachments[1]);
                    let new_contours = std::array::from_fn(|row| {
                        (0..3)
                            .map(|axis| matrix[row][axis] * i64::from(contours[axis]))
                            .sum::<i64>() as i8
                    });
                    if canonical_contours(new_contours).is_err() {
                        continue;
                    }
                    let old_momenta = std::array::from_fn(|row| {
                        std::array::from_fn(|coordinate| {
                            (0..3)
                                .map(|axis| {
                                    i128::from(matrix[row][axis]) * momenta[axis][coordinate]
                                })
                                .sum()
                        })
                    });
                    let value = denominator_values(&old_momenta)[missing].pow(2)
                        * old_momenta[0][0]
                        * old_momenta[1][0].pow(2)
                        * old_momenta[2][0];
                    expected.insert((new_contours, value));
                }
                let actual: BTreeSet<_> = bubble_reflections(contours, point)
                    .unwrap()
                    .into_iter()
                    .map(|(new_contours, terms)| {
                        let value = terms
                            .into_iter()
                            .map(|(term, coefficient)| {
                                let mut value = i128::from(coefficient);
                                for axis in 0..6 {
                                    let power = base[axis] - term[axis];
                                    assert!(power >= 0);
                                    value *= values[axis].pow(power as u32);
                                }
                                for axis in 0..3 {
                                    value *=
                                        momenta[axis][0].pow(term[6 + axis].unsigned_abs().into());
                                }
                                value
                            })
                            .sum::<i128>();
                        (new_contours, value)
                    })
                    .collect();
                assert_eq!(actual, expected, "contours={contours:?}, missing={missing}");
            }
        }
    }

    #[test]
    fn triangle_energy_independent_numerator_has_an_exact_reflection_reduction() {
        // Q -> P+R-Q is a subtopology symmetry, rather than an automorphism of
        // every denominator in the full family. Its Jacobian has absolute
        // determinant one; + + + contours map back to + + + because 1+1-1=1.
        let reflection = [[1_i64, 0, 0], [1, -1, 1], [0, 0, 1]];
        let determinant = reflection[0][0]
            * (reflection[1][1] * reflection[2][2] - reflection[1][2] * reflection[2][1])
            - reflection[0][1]
                * (reflection[1][0] * reflection[2][2] - reflection[1][2] * reflection[2][0])
            + reflection[0][2]
                * (reflection[1][0] * reflection[2][1] - reflection[1][1] * reflection[2][0]);
        assert_eq!(determinant.abs(), 1);
        assert_eq!(reflection.map(|row| row.iter().sum::<i64>()), [1, 1, 1]);
        // Derive a squared linear momentum in the denominator basis from
        // 2P.Q=P²+Q²-(P-Q)², and its two other pairwise counterparts.
        let square = |v: [i64; 3]| {
            [
                v[0] * v[0] + v[0] * v[1] + v[0] * v[2],
                v[1] * v[1] + v[0] * v[1] + v[1] * v[2],
                v[2] * v[2] + v[0] * v[2] + v[1] * v[2],
                -v[0] * v[1],
                -v[0] * v[2],
                -v[1] * v[2],
            ]
        };
        let mut denominator_image = Vec::new();
        for &(a, b) in &EDGES {
            let vertex = |v: usize| if v == 0 { [0; 3] } else { reflection[v - 1] };
            let (a, b) = (vertex(a), vertex(b));
            denominator_image.push(square(std::array::from_fn(|axis| a[axis] - b[axis])));
        }
        let base = key([1, 0, 1, 1, 1, 1]);
        let mut transformed_support = BTreeSet::new();
        for edge in 0..6 {
            if base[edge] == 0 {
                continue;
            }
            let image = denominator_image[edge];
            assert_eq!(
                image
                    .iter()
                    .filter(|&&coefficient| coefficient != 0)
                    .count(),
                1
            );
            let target = image
                .iter()
                .position(|&coefficient| coefficient == 1)
                .unwrap();
            transformed_support.insert(target);
        }
        assert_eq!(transformed_support, BTreeSet::from([0, 2, 3, 4, 5]));
        let numerator = square(reflection[1]);
        // Moving the reflected -Q² term back to the left produces 2I(Q²).
        assert_eq!(1 - numerator[1], 2);
        let mut survivors = Vec::new();
        for (edge, coefficient) in numerator.into_iter().enumerate() {
            if edge == 1 || coefficient == 0 {
                continue;
            }
            let mut point = base;
            point[edge] -= 1;
            if !is_scaleless([1, 1, 1], point).unwrap() {
                survivors.push((point, coefficient));
            }
        }
        assert_eq!(survivors, vec![(key([1, 0, 1, 1, 0, 1]), -1)]);
        // Thus the surviving equation is 2I(Q²)=-M. Establish M's BB+
        // representative using the independently checked vertex routing.
        assert_eq!(
            canonicalize([1, 1, 1], survivors[0].0).unwrap(),
            canonicalize([0, 0, 1], key([1, 1, 0, 0, 1, 1])).unwrap(),
        );
    }
}
