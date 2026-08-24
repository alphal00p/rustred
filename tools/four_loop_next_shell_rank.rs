//! Pure-std finite-field discovery probe for the next four-loop genuine shell.
//!
//! Compile directly, without Cargo or Symbolica:
//! `rustc --edition=2024 -O tools/four_loop_next_shell_rank.rs -o /tmp/four_loop_next_shell_rank`
//! Pass `p d --component-boundary-audit` to print the component dot split and
//! finite-field numerator/parity service inventory for the selected shell.
//! Pass `p d --seed-manifest` to print the frozen 123-seed production prefix.
//! Pass `p d --f5-d2n1-manifest` to print the concrete owned local F5 D2/N1
//! requests and their orbits under the integer-verified F5 stabilizer.
//!
//! This is discovery evidence, not a reduction certificate.  It reproduces
//! the completed corner matrix at a finite-field image, transports genuine
//! proper sectors through the same signed-GL(4,Z) normal form, closes the
//! scalar D1/N0 factorized boundary, and retains every higher factorized
//! target as a typed opaque boundary column.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;

const LOOPS: usize = 4;
const BASIS: usize = 10;
const NEXT_SEED_SCHEMA: &str = "rustred-equal-mass-euclidean-four-loop-next-seed-v1";
const CORNER_SCHEMA: &str = "rustred-equal-mass-euclidean-four-loop-corner-v1";
const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;
const FROZEN_NEXT_SEED_CHECKSUM: u64 = 0x0bff_80d5_dddb_4340;

const H_ROUTINGS: [[i64; LOOPS]; 9] = [
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [0, 0, 0, 1],
    [1, 0, -1, 0],
    [0, 1, -1, 0],
    [-1, 0, 1, 1],
    [0, -1, 1, 1],
    [0, 0, 1, 1],
];

const X_ROUTINGS: [[i64; LOOPS]; 9] = [
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [0, 0, 0, 1],
    [1, 0, -1, 0],
    [0, 1, -1, 0],
    [-1, 0, 1, 1],
    [0, -1, 1, 1],
    [-1, -1, 1, 1],
];

const THREE_LOOP_ROUTINGS: [[i64; 3]; 6] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [-1, 0, 1],
    [1, -1, 0],
    [0, 1, -1],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Topology {
    H,
    X,
}

impl Topology {
    fn routings(self) -> &'static [[i64; LOOPS]; 9] {
        match self {
            Self::H => &H_ROUTINGS,
            Self::X => &X_ROUTINGS,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::H => "H",
            Self::X => "X",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CornerType {
    V5,
    V6a,
    V6b,
    V7a,
    V7b,
    V7c,
    V8a,
    V8b,
    H9,
    X9,
}

impl CornerType {
    const ALL: [Self; 10] = [
        Self::V5,
        Self::V6a,
        Self::V6b,
        Self::V7a,
        Self::V7b,
        Self::V7c,
        Self::V8a,
        Self::V8b,
        Self::H9,
        Self::X9,
    ];

    fn topology(self) -> Topology {
        if self == Self::X9 {
            Topology::X
        } else {
            Topology::H
        }
    }

    fn mask(self) -> u16 {
        match self {
            Self::V5 => 0x06b,
            Self::V6a => 0x06f,
            Self::V6b => 0x0cf,
            Self::V7a => 0x13f,
            Self::V7b => 0x07f,
            Self::V7c => 0x0df,
            Self::V8a => 0x17f,
            Self::V8b => 0x0ff,
            Self::H9 | Self::X9 => 0x1ff,
        }
    }

    fn lines(self) -> usize {
        self.mask().count_ones() as usize
    }

    fn key(self) -> &'static str {
        match self {
            Self::V5 => "V5",
            Self::V6a => "V6a",
            Self::V6b => "V6b",
            Self::V7a => "V7a",
            Self::V7b => "V7b",
            Self::V7c => "V7c",
            Self::V8a => "V8a",
            Self::V8b => "V8b",
            Self::H9 => "H9",
            Self::X9 => "X9",
        }
    }
}

#[derive(Clone, Copy)]
struct Field {
    p: i64,
}

impl Field {
    fn n(self, value: i128) -> i64 {
        let p = i128::from(self.p);
        let mut value = value % p;
        if value < 0 {
            value += p;
        }
        value as i64
    }

    fn add(self, left: i64, right: i64) -> i64 {
        self.n(i128::from(left) + i128::from(right))
    }

    fn sub(self, left: i64, right: i64) -> i64 {
        self.n(i128::from(left) - i128::from(right))
    }

    fn mul(self, left: i64, right: i64) -> i64 {
        self.n(i128::from(left) * i128::from(right))
    }

    fn pow(self, mut base: i64, mut exponent: i64) -> i64 {
        base = self.n(i128::from(base));
        let mut value = 1;
        while exponent > 0 {
            if exponent & 1 == 1 {
                value = self.mul(value, base);
            }
            base = self.mul(base, base);
            exponent >>= 1;
        }
        value
    }

    fn inv(self, value: i64) -> i64 {
        assert_ne!(value, 0, "finite-field division by zero");
        self.pow(value, self.p - 2)
    }

    fn div(self, left: i64, right: i64) -> i64 {
        self.mul(left, self.inv(right))
    }
}

#[derive(Clone)]
struct Family {
    qforms: Vec<Vec<i64>>,
    shifts: Vec<i64>,
    inverse: Vec<Vec<i64>>,
}

impl Family {
    fn build(topology: Topology, field: Field) -> Self {
        let mut qforms = topology
            .routings()
            .iter()
            .map(|routing| routing_qform(routing, field))
            .collect::<Vec<_>>();
        let mut rank = matrix_rank(qforms.clone(), field);
        for scalar_product in 0..BASIS {
            if qforms.len() == BASIS {
                break;
            }
            let mut row = vec![0; BASIS];
            row[scalar_product] = 1;
            let mut trial = qforms.clone();
            trial.push(row.clone());
            let trial_rank = matrix_rank(trial, field);
            if trial_rank > rank {
                qforms.push(row);
                rank = trial_rank;
            }
        }
        assert_eq!(rank, BASIS);
        assert_eq!(qforms.len(), BASIS);
        let inverse = invert_field(&qforms, field).expect("completed basis is invertible");
        let shifts = (0..BASIS).map(|position| i64::from(position < 9)).collect();
        Self {
            qforms,
            shifts,
            inverse,
        }
    }

    fn contraction(
        &self,
        denominator: usize,
        differentiated: usize,
        contracted: usize,
        field: Field,
    ) -> (i64, Vec<i64>) {
        let mut scalar = vec![0; BASIS];
        for left in 0..LOOPS {
            for right in left..LOOPS {
                let coefficient = self.qforms[denominator][sp_index(left, right)];
                if coefficient == 0 {
                    continue;
                }
                if differentiated == left {
                    let multiplicity = if left == right { 2 } else { 1 };
                    let target = sp_index(right, contracted);
                    scalar[target] =
                        field.add(scalar[target], field.mul(coefficient, multiplicity));
                }
                if left != right && differentiated == right {
                    let target = sp_index(left, contracted);
                    scalar[target] = field.add(scalar[target], coefficient);
                }
            }
        }
        let mut denominator_coefficients = vec![0; BASIS];
        for source in 0..BASIS {
            for target in 0..BASIS {
                denominator_coefficients[target] = field.add(
                    denominator_coefficients[target],
                    field.mul(scalar[source], self.inverse[source][target]),
                );
            }
        }
        let constant = denominator_coefficients
            .iter()
            .zip(&self.shifts)
            .fold(0, |sum, (&coefficient, &shift)| {
                field.sub(sum, field.mul(coefficient, shift))
            });
        (constant, denominator_coefficients)
    }
}

fn sp_index(left: usize, right: usize) -> usize {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    (0..left).map(|row| LOOPS - row).sum::<usize>() + right - left
}

fn routing_qform(routing: &[i64; LOOPS], field: Field) -> Vec<i64> {
    let mut row = Vec::with_capacity(BASIS);
    for left in 0..LOOPS {
        for right in left..LOOPS {
            let symmetry = if left == right { 1 } else { 2 };
            row.push(field.n(i128::from(routing[left] * routing[right] * symmetry)));
        }
    }
    row
}

#[derive(Clone, Debug)]
struct SignatureLine {
    position: usize,
    value: Vec<i64>,
}

#[derive(Clone, Debug)]
struct Signature {
    basis: Vec<Vec<i64>>,
    signs: Vec<i64>,
    lines: Vec<SignatureLine>,
    values: Vec<Vec<i64>>,
}

fn determinant_integer(matrix: &[Vec<i64>]) -> i64 {
    match matrix.len() {
        0 => 1,
        1 => matrix[0][0],
        n => (0..n).fold(0_i64, |sum, column| {
            let minor = matrix[1..]
                .iter()
                .map(|row| {
                    row.iter()
                        .enumerate()
                        .filter_map(|(c, &v)| (c != column).then_some(v))
                        .collect()
                })
                .collect::<Vec<Vec<i64>>>();
            let term = matrix[0][column] * determinant_integer(&minor);
            if column % 2 == 0 {
                sum + term
            } else {
                sum - term
            }
        }),
    }
}

fn invert_unimodular(matrix: &[Vec<i64>]) -> Option<Vec<Vec<i64>>> {
    let determinant = determinant_integer(matrix);
    if determinant.abs() != 1 {
        return None;
    }
    let n = matrix.len();
    let mut inverse = vec![vec![0; n]; n];
    for row in 0..n {
        for column in 0..n {
            let minor = matrix
                .iter()
                .enumerate()
                .filter_map(|(source_row, values)| {
                    (source_row != column).then(|| {
                        values
                            .iter()
                            .enumerate()
                            .filter_map(|(source_column, &value)| {
                                (source_column != row).then_some(value)
                            })
                            .collect()
                    })
                })
                .collect::<Vec<Vec<i64>>>();
            let cofactor = if (row + column) % 2 == 0 {
                determinant_integer(&minor)
            } else {
                -determinant_integer(&minor)
            };
            inverse[row][column] = cofactor / determinant;
        }
    }
    Some(inverse)
}

fn row_times_integer(row: &[i64], matrix: &[Vec<i64>]) -> Vec<i64> {
    (0..matrix[0].len())
        .map(|column| {
            row.iter()
                .zip(matrix)
                .map(|(&left, right)| left * right[column])
                .sum()
        })
        .collect()
}

fn matrix_multiply_integer(left: &[Vec<i64>], right: &[Vec<i64>]) -> Vec<Vec<i64>> {
    left.iter()
        .map(|row| row_times_integer(row, right))
        .collect()
}

fn ordered_selections(length: usize, choose: usize) -> Vec<Vec<usize>> {
    fn visit(
        length: usize,
        choose: usize,
        used: &mut [bool],
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            output.push(current.clone());
            return;
        }
        for value in 0..length {
            if !used[value] {
                used[value] = true;
                current.push(value);
                visit(length, choose, used, current, output);
                current.pop();
                used[value] = false;
            }
        }
    }
    let mut output = Vec::new();
    visit(
        length,
        choose,
        &mut vec![false; length],
        &mut Vec::new(),
        &mut output,
    );
    output
}

fn canonical_signature(rows: &[(usize, Vec<i64>)], rank: usize) -> Signature {
    let mut best: Option<Signature> = None;
    for indices in ordered_selections(rows.len(), rank) {
        let basis = indices
            .iter()
            .map(|&index| rows[index].1.clone())
            .collect::<Vec<_>>();
        let Some(inverse) = invert_unimodular(&basis) else {
            continue;
        };
        let coordinates = rows
            .iter()
            .map(|(_, row)| row_times_integer(row, &inverse))
            .collect::<Vec<_>>();
        for sign_bits in 0..(1_usize << rank) {
            let signs = (0..rank)
                .map(|axis| if sign_bits & (1 << axis) == 0 { 1 } else { -1 })
                .collect::<Vec<_>>();
            let mut lines = coordinates
                .iter()
                .enumerate()
                .map(|(line, row)| {
                    let mut value = row
                        .iter()
                        .enumerate()
                        .map(|(axis, &v)| v * signs[axis])
                        .collect::<Vec<_>>();
                    let orientation = if value.iter().find(|&&v| v != 0).is_some_and(|&v| v < 0) {
                        -1
                    } else {
                        1
                    };
                    if orientation == -1 {
                        value.iter_mut().for_each(|v| *v = -*v);
                    }
                    SignatureLine {
                        position: rows[line].0,
                        value,
                    }
                })
                .collect::<Vec<_>>();
            lines.sort_by(|left, right| {
                left.value
                    .cmp(&right.value)
                    .then(left.position.cmp(&right.position))
            });
            let values = lines
                .iter()
                .map(|line| line.value.clone())
                .collect::<Vec<_>>();
            let candidate = Signature {
                basis: basis.clone(),
                signs,
                lines,
                values,
            };
            if best
                .as_ref()
                .is_none_or(|current| candidate.values < current.values)
            {
                best = Some(candidate);
            }
        }
    }
    best.expect("every genuine/component routing has a unimodular basis")
}

#[derive(Clone)]
struct GenuineMap {
    corner_type: CornerType,
    source_to_reference: Vec<(usize, usize)>,
    loop_map: Vec<Vec<i64>>,
}

struct Catalog {
    signatures: Vec<(CornerType, Signature)>,
    lower: BTreeMap<Master, Signature>,
}

impl Catalog {
    fn build() -> Self {
        let signatures = CornerType::ALL
            .into_iter()
            .map(|corner_type| {
                let rows = active_positions(corner_type.mask())
                    .into_iter()
                    .map(|position| {
                        (
                            position,
                            corner_type.topology().routings()[position].to_vec(),
                        )
                    })
                    .collect::<Vec<_>>();
                (corner_type, canonical_signature(&rows, LOOPS))
            })
            .collect();
        let lower = Master::ALL
            .into_iter()
            .map(|master| {
                let rows = lower_reference_rows(master);
                (master, canonical_signature(&rows, master.loops()))
            })
            .collect();
        Self { signatures, lower }
    }

    fn classify_genuine(&self, topology: Topology, mask: u16) -> GenuineMap {
        let rows = active_positions(mask)
            .into_iter()
            .map(|position| (position, topology.routings()[position].to_vec()))
            .collect::<Vec<_>>();
        let source = canonical_signature(&rows, LOOPS);
        let (corner_type, reference) = self
            .signatures
            .iter()
            .find(|(_, signature)| signature.values == source.values)
            .expect("genuine sector is in ten-type catalog");
        let source_inverse = invert_unimodular(&source.basis).unwrap();
        let diagonal = (0..LOOPS)
            .map(|row| {
                (0..LOOPS)
                    .map(|column| {
                        if row == column {
                            source.signs[row] * reference.signs[row]
                        } else {
                            0
                        }
                    })
                    .collect()
            })
            .collect::<Vec<Vec<i64>>>();
        let loop_map = matrix_multiply_integer(
            &matrix_multiply_integer(&source_inverse, &diagonal),
            &reference.basis,
        );
        let source_to_reference = source
            .lines
            .iter()
            .zip(&reference.lines)
            .map(|(source, reference)| (source.position, reference.position))
            .collect();
        GenuineMap {
            corner_type: *corner_type,
            source_to_reference,
            loop_map,
        }
    }
}

fn active_positions(mask: u16) -> Vec<usize> {
    (0..9)
        .filter(|&position| mask & (1 << position) != 0)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Master {
    T1,
    S2,
    B4,
    F5,
    M6,
}

impl Master {
    const ALL: [Self; 5] = [Self::T1, Self::S2, Self::B4, Self::F5, Self::M6];
    fn loops(self) -> usize {
        match self {
            Self::T1 => 1,
            Self::S2 => 2,
            _ => 3,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::T1 => "T1",
            Self::S2 => "S2",
            Self::B4 => "B4",
            Self::F5 => "F5",
            Self::M6 => "M6",
        }
    }
}

fn lower_reference_rows(master: Master) -> Vec<(usize, Vec<i64>)> {
    match master {
        Master::T1 => vec![(0, vec![1])],
        Master::S2 => vec![(0, vec![1, 0]), (1, vec![0, 1]), (2, vec![1, 1])],
        Master::B4 => [0, 1, 3, 5]
            .into_iter()
            .enumerate()
            .map(|(compact, position)| (compact, THREE_LOOP_ROUTINGS[position].to_vec()))
            .collect(),
        Master::F5 => THREE_LOOP_ROUTINGS[..5]
            .iter()
            .enumerate()
            .map(|(position, row)| (position, row.to_vec()))
            .collect(),
        Master::M6 => THREE_LOOP_ROUTINGS
            .iter()
            .enumerate()
            .map(|(position, row)| (position, row.to_vec()))
            .collect(),
    }
}

/// Exact integer witnesses for the stabilizer of the five-line F5 mask in
/// the completed six-line tetrahedron family.  A permutation maps each source
/// line position to its target position.  The missing F5 line stays at
/// position five.  For each permutation the stored unimodular matrix maps all
/// six routing vectors to the advertised targets, independently up to sign.
fn f5_stabilizer() -> BTreeMap<[usize; 6], Vec<Vec<i64>>> {
    fn visit_active_permutations(
        position: usize,
        used: &mut [bool; 5],
        current: &mut [usize; 6],
        output: &mut Vec<[usize; 6]>,
    ) {
        if position == 5 {
            current[5] = 5;
            output.push(*current);
            return;
        }
        for target in 0..5 {
            if used[target] {
                continue;
            }
            used[target] = true;
            current[position] = target;
            visit_active_permutations(position + 1, used, current, output);
            used[target] = false;
        }
    }

    fn equal_up_to_sign(left: &[i64], right: &[i64]) -> bool {
        left == right || left.iter().zip(right).all(|(&left, &right)| left == -right)
    }

    let mut permutations = Vec::new();
    visit_active_permutations(0, &mut [false; 5], &mut [0; 6], &mut permutations);
    let mut stabilizer = BTreeMap::new();
    for permutation in permutations {
        for sign_bits in 0..(1_usize << 3) {
            // The first three tetrahedron routings are the coordinate rows,
            // so their signed images are the rows of the loop map.
            let loop_map = (0..3)
                .map(|axis| {
                    let sign = if sign_bits & (1 << axis) == 0 { 1 } else { -1 };
                    THREE_LOOP_ROUTINGS[permutation[axis]]
                        .iter()
                        .map(|&value| sign * value)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            if determinant_integer(&loop_map).abs() != 1 {
                continue;
            }
            let maps_all_lines =
                THREE_LOOP_ROUTINGS
                    .iter()
                    .enumerate()
                    .all(|(source_position, source_routing)| {
                        let image = row_times_integer(source_routing, &loop_map);
                        equal_up_to_sign(&image, &THREE_LOOP_ROUTINGS[permutation[source_position]])
                    });
            if maps_all_lines {
                stabilizer.entry(permutation).or_insert(loop_map);
                break;
            }
        }
    }
    assert_eq!(
        stabilizer.len(),
        4,
        "the F5 mask stabilizer must be the four-element missing-edge stabilizer"
    );
    stabilizer
}

fn canonical_f5_powers(
    powers: [i32; 6],
    stabilizer: &BTreeMap<[usize; 6], Vec<Vec<i64>>>,
) -> [i32; 6] {
    stabilizer
        .keys()
        .map(|permutation| {
            let mut image = [0_i32; 6];
            for source in 0..6 {
                image[permutation[source]] = powers[source];
            }
            image
        })
        .min()
        .expect("the F5 stabilizer contains the identity")
}

fn complete_labelled_f5_d2n1_domain() -> BTreeSet<[i32; 6]> {
    let mut targets = BTreeSet::new();
    for first in 0..5 {
        let mut triple = [1_i32; 6];
        triple[5] = -1;
        triple[first] = 3;
        targets.insert(triple);
        for second in first + 1..5 {
            let mut double = [1_i32; 6];
            double[5] = -1;
            double[first] = 2;
            double[second] = 2;
            targets.insert(double);
        }
    }
    assert_eq!(targets.len(), 15);
    targets
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Product([u8; 5]);

impl Product {
    fn from_masters(masters: impl IntoIterator<Item = Master>) -> Self {
        let mut counts = [0; 5];
        for master in masters {
            counts[master as usize] += 1;
        }
        Self(counts)
    }
    fn multiply(&self, other: &Self) -> Self {
        Self(std::array::from_fn(|index| self.0[index] + other.0[index]))
    }
    fn remove(&self, master: Master) -> Self {
        let mut result = self.clone();
        result.0[master as usize] -= 1;
        result
    }
    fn stable_key(&self) -> String {
        let mut parts = Vec::new();
        for master in Master::ALL {
            let count = self.0[master as usize];
            if count != 0 {
                parts.push(format!("{}^{count}", master.name()));
            }
        }
        parts.join("*")
    }
}

#[derive(Clone)]
struct Factorization {
    product: Product,
    components: Vec<Component>,
    source_to_reference_loop_map: Vec<Vec<i64>>,
}

#[derive(Clone)]
struct Component {
    master: Master,
    global_basis_slots: Vec<usize>,
    physical_positions: Vec<usize>,
    source_signature: Signature,
    reference_offset: usize,
    loop_map: Vec<Vec<i64>>,
}

fn factorize(topology: Topology, mask: u16, catalog: &Catalog) -> Option<Factorization> {
    let active = active_positions(mask);
    let rows = active
        .iter()
        .map(|&position| (position, topology.routings()[position].to_vec()))
        .collect::<Vec<_>>();
    let basis_indices = combinations(rows.len(), LOOPS)
        .into_iter()
        .find(|indices| {
            determinant_integer(
                &indices
                    .iter()
                    .map(|&index| rows[index].1.clone())
                    .collect::<Vec<_>>(),
            )
            .abs()
                == 1
        })?;
    let basis = basis_indices
        .iter()
        .map(|&index| rows[index].1.clone())
        .collect::<Vec<_>>();
    let inverse = invert_unimodular(&basis).unwrap();
    let coordinates = rows
        .iter()
        .map(|(position, row)| (*position, row_times_integer(row, &inverse)))
        .collect::<Vec<_>>();
    let mut parent = [0, 1, 2, 3];
    for (_, coordinate) in &coordinates {
        let support = coordinate
            .iter()
            .enumerate()
            .filter_map(|(slot, &value)| (value != 0).then_some(slot))
            .collect::<Vec<_>>();
        if let Some((&first, rest)) = support.split_first() {
            for &other in rest {
                union(&mut parent, first, other);
            }
        }
    }
    let mut blocks = Vec::<Vec<usize>>::new();
    for slot in 0..LOOPS {
        let root = find(&mut parent, slot);
        if let Some(block) = blocks
            .iter_mut()
            .find(|block| find(&mut parent, block[0]) == root)
        {
            block.push(slot);
        } else {
            blocks.push(vec![slot]);
        }
    }
    blocks.sort_by_key(|block| block[0]);
    if blocks.len() == 1 {
        return None;
    }
    let mut components = Vec::new();
    for block in blocks {
        let reduced = coordinates
            .iter()
            .filter(|(_, coordinate)| block.iter().any(|&slot| coordinate[slot] != 0))
            .map(|(position, coordinate)| {
                (
                    *position,
                    block
                        .iter()
                        .map(|&slot| coordinate[slot])
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let master = match (block.len(), reduced.len()) {
            (1, 1) => Master::T1,
            (2, 3) => Master::S2,
            (3, 4) => Master::B4,
            (3, 5) => Master::F5,
            (3, 6) => Master::M6,
            other => panic!("unrecognized factor component {other:?}"),
        };
        let source_signature = canonical_signature(&reduced, block.len());
        let reference_signature = &catalog.lower[&master];
        assert_eq!(source_signature.values, reference_signature.values);
        let source_inverse = invert_unimodular(&source_signature.basis).unwrap();
        let diagonal = (0..block.len())
            .map(|row| {
                (0..block.len())
                    .map(|column| {
                        if row == column {
                            source_signature.signs[row] * reference_signature.signs[row]
                        } else {
                            0
                        }
                    })
                    .collect()
            })
            .collect::<Vec<Vec<i64>>>();
        let loop_map = matrix_multiply_integer(
            &matrix_multiply_integer(&source_inverse, &diagonal),
            &reference_signature.basis,
        );
        components.push(Component {
            master,
            global_basis_slots: block,
            physical_positions: reduced.iter().map(|(position, _)| *position).collect(),
            source_signature,
            reference_offset: 0,
            loop_map,
        });
    }
    let mut reference_offset = 0;
    let mut p_to_reference = vec![vec![0_i64; LOOPS]; LOOPS];
    for component in &mut components {
        component.reference_offset = reference_offset;
        for (local_row, &global_slot) in component.global_basis_slots.iter().enumerate() {
            for local_column in 0..component.master.loops() {
                p_to_reference[global_slot][reference_offset + local_column] =
                    component.loop_map[local_row][local_column];
            }
        }
        reference_offset += component.master.loops();
    }
    assert_eq!(reference_offset, LOOPS);
    let source_to_reference_loop_map = matrix_multiply_integer(&inverse, &p_to_reference);
    let product = Product::from_masters(components.iter().map(|component| component.master));
    Some(Factorization {
        product,
        components,
        source_to_reference_loop_map,
    })
}

fn combinations(length: usize, choose: usize) -> Vec<Vec<usize>> {
    fn visit(
        length: usize,
        choose: usize,
        start: usize,
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            output.push(current.clone());
            return;
        }
        let needed = choose - current.len();
        for value in start..=length.saturating_sub(needed) {
            current.push(value);
            visit(length, choose, value + 1, current, output);
            current.pop();
        }
    }
    let mut output = Vec::new();
    visit(length, choose, 0, &mut Vec::new(), &mut output);
    output
}

fn find(parent: &mut [usize; LOOPS], slot: usize) -> usize {
    if parent[slot] != slot {
        parent[slot] = find(parent, parent[slot]);
    }
    parent[slot]
}

fn union(parent: &mut [usize; LOOPS], left: usize, right: usize) {
    let left = find(parent, left);
    let right = find(parent, right);
    if left != right {
        parent[right] = left;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct GenuineColumn {
    corner_type: CornerType,
    powers: [i32; BASIS],
}

impl GenuineColumn {
    fn order_key(&self) -> (usize, u32, u32, &'static str, [i32; BASIS]) {
        let mask = self.corner_type.mask();
        let dots = self
            .powers
            .iter()
            .enumerate()
            .filter(|(position, _)| mask & (1 << position) != 0)
            .map(|(_, &power)| power.saturating_sub(1).max(0) as u32)
            .sum::<u32>();
        let numerators = self
            .powers
            .iter()
            .enumerate()
            .filter(|(position, _)| mask & (1 << position) == 0)
            .map(|(_, &power)| power.saturating_neg().max(0) as u32)
            .sum::<u32>();
        (
            self.corner_type.lines(),
            dots + numerators,
            dots,
            self.corner_type.key(),
            self.powers,
        )
    }
}

impl Ord for GenuineColumn {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_key().cmp(&other.order_key())
    }
}
impl PartialOrd for GenuineColumn {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Column {
    Product(String),
    Boundary {
        topology: Topology,
        product: String,
        powers: [i32; BASIS],
    },
    Genuine(GenuineColumn),
}

impl Ord for Column {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Product(left), Self::Product(right)) => left.cmp(right),
            (Self::Product(_), _) => Ordering::Less,
            (_, Self::Product(_)) => Ordering::Greater,
            (
                Self::Boundary {
                    topology: lt,
                    product: lp,
                    powers: lx,
                },
                Self::Boundary {
                    topology: rt,
                    product: rp,
                    powers: rx,
                },
            ) => (lt, lp, lx).cmp(&(rt, rp, rx)),
            (Self::Boundary { .. }, Self::Genuine(_)) => Ordering::Less,
            (Self::Genuine(_), Self::Boundary { .. }) => Ordering::Greater,
            (Self::Genuine(left), Self::Genuine(right)) => left.cmp(right),
        }
    }
}
impl PartialOrd for Column {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

type Row = BTreeMap<Column, i64>;

fn add(row: &mut Row, column: Column, coefficient: i64, field: Field) {
    if coefficient == 0 {
        return;
    }
    let value = field.add(row.get(&column).copied().unwrap_or(0), coefficient);
    if value == 0 {
        row.remove(&column);
    } else {
        row.insert(column, value);
    }
}

struct Normalizer {
    field: Field,
    d: i64,
    families: BTreeMap<Topology, Family>,
    catalog: Catalog,
    genuine_cache: HashMap<(Topology, u16), GenuineMap>,
    normalization_cache: HashMap<(Topology, [i32; BASIS]), Row>,
}

impl Normalizer {
    fn new(field: Field, d: i64) -> Self {
        let families = [Topology::H, Topology::X]
            .into_iter()
            .map(|topology| (topology, Family::build(topology, field)))
            .collect();
        Self {
            field,
            d: field.n(i128::from(d)),
            families,
            catalog: Catalog::build(),
            genuine_cache: HashMap::new(),
            normalization_cache: HashMap::new(),
        }
    }

    fn genuine_map(&mut self, topology: Topology, mask: u16) -> GenuineMap {
        if let Some(map) = self.genuine_cache.get(&(topology, mask)) {
            return map.clone();
        }
        let map = self.catalog.classify_genuine(topology, mask);
        self.genuine_cache.insert((topology, mask), map.clone());
        map
    }

    fn normalize(&mut self, topology: Topology, powers: [i32; BASIS]) -> Row {
        if let Some(row) = self.normalization_cache.get(&(topology, powers)) {
            return row.clone();
        }
        let row = self.normalize_uncached(topology, powers);
        self.normalization_cache
            .insert((topology, powers), row.clone());
        row
    }

    fn normalize_uncached(&mut self, topology: Topology, powers: [i32; BASIS]) -> Row {
        let mask = powers
            .iter()
            .take(9)
            .enumerate()
            .fold(0_u16, |mask, (position, &power)| {
                mask | (u16::from(power > 0) << position)
            });
        let active = active_positions(mask);
        if routing_rank(topology, &active) < LOOPS {
            return Row::new();
        }
        if let Some(factorization) = factorize(topology, mask, &self.catalog) {
            let dots = active
                .iter()
                .map(|&position| powers[position].saturating_sub(1).max(0) as u32)
                .sum::<u32>();
            let numerators = powers
                .iter()
                .map(|&power| power.saturating_neg().max(0) as u32)
                .sum::<u32>();
            if dots == 0 && numerators == 0 {
                return Row::from([(Column::Product(factorization.product.stable_key()), 1)]);
            }
            if dots == 1 && numerators == 0 {
                return self.close_d1_factorized(&factorization, &powers);
            }
            return Row::from([(
                Column::Boundary {
                    topology,
                    product: factorization.product.stable_key(),
                    powers,
                },
                1,
            )]);
        }

        let map = self.genuine_map(topology, mask);
        let target_topology = map.corner_type.topology();
        let reference_family = self.families[&target_topology].clone();
        let source_family = self.families[&topology].clone();
        let mut reference_powers = [0_i32; BASIS];
        for &(source, reference) in &map.source_to_reference {
            reference_powers[reference] = powers[source];
        }

        let mut polynomial = BTreeMap::<[i32; BASIS], i64>::from([(reference_powers, 1)]);
        for (source_position, &power) in powers.iter().enumerate() {
            for _ in 0..power.saturating_neg() {
                let (constant, coefficients) = affine_image(
                    &source_family,
                    &reference_family,
                    source_position,
                    &map.loop_map,
                    self.field,
                );
                let mut next = BTreeMap::new();
                for (current, coefficient) in polynomial {
                    if constant != 0 {
                        add_power_term(
                            &mut next,
                            current,
                            self.field.mul(coefficient, constant),
                            self.field,
                        );
                    }
                    for (position, &basis_coefficient) in coefficients.iter().enumerate() {
                        if basis_coefficient == 0 {
                            continue;
                        }
                        let mut shifted = current;
                        shifted[position] -= 1;
                        add_power_term(
                            &mut next,
                            shifted,
                            self.field.mul(coefficient, basis_coefficient),
                            self.field,
                        );
                    }
                }
                polynomial = next;
            }
        }

        let mut output = Row::new();
        for (branch, coefficient) in polynomial {
            let branch_mask = branch
                .iter()
                .take(9)
                .enumerate()
                .fold(0_u16, |mask, (position, &power)| {
                    mask | (u16::from(power > 0) << position)
                });
            if branch_mask == map.corner_type.mask() {
                add(
                    &mut output,
                    Column::Genuine(GenuineColumn {
                        corner_type: map.corner_type,
                        powers: branch,
                    }),
                    coefficient,
                    self.field,
                );
            } else {
                let reduced = self.normalize(target_topology, branch);
                for (column, value) in reduced {
                    add(
                        &mut output,
                        column,
                        self.field.mul(coefficient, value),
                        self.field,
                    );
                }
            }
        }
        output
    }

    fn close_d1_factorized(&self, factorization: &Factorization, powers: &[i32; BASIS]) -> Row {
        let dotted_position = powers
            .iter()
            .position(|&power| power == 2)
            .expect("D1 has one dot");
        let owner = factorization
            .components
            .iter()
            .find(|component| component.physical_positions.contains(&dotted_position))
            .expect("dot belongs to a component");
        let reference = &self.catalog.lower[&owner.master];
        let compact_position = owner
            .source_signature
            .lines
            .iter()
            .zip(&reference.lines)
            .find_map(|(source, target)| {
                (source.position == dotted_position).then_some(target.position)
            })
            .unwrap();
        let unaffected = factorization.product.remove(owner.master);
        let local = self.local_formula(owner.master, compact_position);
        let mut row = Row::new();
        for (product, coefficient) in local {
            add(
                &mut row,
                Column::Product(unaffected.multiply(&product).stable_key()),
                coefficient,
                self.field,
            );
        }
        row
    }

    fn local_formula(&self, master: Master, position: usize) -> Vec<(Product, i64)> {
        let d = self.d;
        let f = self.field;
        let ratio = |constant: i64, d_factor: i64, denominator: i64| {
            f.div(
                f.add(
                    f.n(i128::from(constant)),
                    f.mul(f.n(i128::from(d_factor)), d),
                ),
                f.n(i128::from(denominator)),
            )
        };
        let one = |master| Product::from_masters([master]);
        match (master, position) {
            (Master::T1, 0) => vec![(one(Master::T1), ratio(2, -1, 2))],
            (Master::S2, 0..=2) => vec![(one(Master::S2), ratio(3, -1, 3))],
            (Master::B4, 0..=3) => vec![(one(Master::B4), ratio(8, -3, 8))],
            (Master::F5, 0) => vec![
                (one(Master::B4), ratio(8, -3, 6)),
                (
                    Product::from_masters([Master::T1, Master::S2]),
                    ratio(-4, 2, 3),
                ),
                (one(Master::F5), ratio(6, -1, 6)),
            ],
            (Master::F5, 1..=4) => vec![
                (one(Master::B4), ratio(-8, 3, 24)),
                (
                    Product::from_masters([Master::T1, Master::S2]),
                    ratio(2, -1, 6),
                ),
                (one(Master::F5), ratio(3, -1, 3)),
            ],
            (Master::M6, 0..=5) => vec![(one(Master::M6), ratio(4, -1, 4))],
            _ => panic!("invalid lower component formula {master:?}/{position}"),
        }
    }

    fn raw_rows(&mut self, seed: &Seed) -> Vec<Row> {
        let family = self.families[&seed.corner_type.topology()].clone();
        let mut rows = Vec::with_capacity(16);
        for differentiated in 0..LOOPS {
            for contracted in 0..LOOPS {
                let mut raw = BTreeMap::<[i32; BASIS], i64>::new();
                if differentiated == contracted {
                    add_power_term(&mut raw, seed.powers, self.d, self.field);
                }
                for (denominator, &power) in seed.powers.iter().enumerate() {
                    if power == 0 {
                        continue;
                    }
                    let factor = self.field.n(-i128::from(power));
                    let (constant, coefficients) =
                        family.contraction(denominator, differentiated, contracted, self.field);
                    if constant != 0 {
                        let mut shifted = seed.powers;
                        shifted[denominator] += 1;
                        add_power_term(
                            &mut raw,
                            shifted,
                            self.field.mul(factor, constant),
                            self.field,
                        );
                    }
                    for (cancelled, coefficient) in coefficients.into_iter().enumerate() {
                        if coefficient == 0 {
                            continue;
                        }
                        let mut shifted = seed.powers;
                        shifted[denominator] += 1;
                        shifted[cancelled] -= 1;
                        add_power_term(
                            &mut raw,
                            shifted,
                            self.field.mul(factor, coefficient),
                            self.field,
                        );
                    }
                }
                let mut normalized = Row::new();
                for (powers, coefficient) in raw {
                    for (column, value) in self.normalize(seed.corner_type.topology(), powers) {
                        add(
                            &mut normalized,
                            column,
                            self.field.mul(coefficient, value),
                            self.field,
                        );
                    }
                }
                rows.push(normalized);
            }
        }
        rows
    }
}

fn add_power_term(
    terms: &mut BTreeMap<[i32; BASIS], i64>,
    powers: [i32; BASIS],
    coefficient: i64,
    field: Field,
) {
    if coefficient == 0 {
        return;
    }
    let value = field.add(terms.get(&powers).copied().unwrap_or(0), coefficient);
    if value == 0 {
        terms.remove(&powers);
    } else {
        terms.insert(powers, value);
    }
}

fn affine_image(
    source: &Family,
    reference: &Family,
    position: usize,
    loop_map: &[Vec<i64>],
    field: Field,
) -> (i64, Vec<i64>) {
    let mut transformed = vec![0; BASIS];
    for source_left in 0..LOOPS {
        for source_right in source_left..LOOPS {
            let coefficient = source.qforms[position][sp_index(source_left, source_right)];
            if coefficient == 0 {
                continue;
            }
            for reference_left in 0..LOOPS {
                for reference_right in reference_left..LOOPS {
                    let mapped = if reference_left == reference_right {
                        loop_map[source_left][reference_left]
                            * loop_map[source_right][reference_right]
                    } else {
                        loop_map[source_left][reference_left]
                            * loop_map[source_right][reference_right]
                            + loop_map[source_left][reference_right]
                                * loop_map[source_right][reference_left]
                    };
                    let target = sp_index(reference_left, reference_right);
                    transformed[target] = field.add(
                        transformed[target],
                        field.mul(coefficient, field.n(i128::from(mapped))),
                    );
                }
            }
        }
    }
    let mut coefficients = vec![0; BASIS];
    for scalar in 0..BASIS {
        for target in 0..BASIS {
            coefficients[target] = field.add(
                coefficients[target],
                field.mul(transformed[scalar], reference.inverse[scalar][target]),
            );
        }
    }
    let constant = coefficients
        .iter()
        .zip(&reference.shifts)
        .fold(source.shifts[position], |value, (&coefficient, &shift)| {
            field.sub(value, field.mul(coefficient, shift))
        });
    (constant, coefficients)
}

fn routing_rank(topology: Topology, active: &[usize]) -> usize {
    let rows = active
        .iter()
        .map(|&position| {
            topology.routings()[position]
                .iter()
                .map(|&value| value)
                .collect()
        })
        .collect::<Vec<Vec<i64>>>();
    rank_rational_integer(rows)
}

fn rank_rational_integer(mut matrix: Vec<Vec<i64>>) -> usize {
    if matrix.is_empty() {
        return 0;
    }
    let columns = matrix[0].len();
    let mut rank = 0;
    for column in 0..columns {
        let Some(pivot) = (rank..matrix.len()).find(|&row| matrix[row][column] != 0) else {
            continue;
        };
        matrix.swap(rank, pivot);
        let pivot_value = matrix[rank][column];
        for row in rank + 1..matrix.len() {
            let factor = matrix[row][column];
            if factor == 0 {
                continue;
            }
            for target in column..columns {
                matrix[row][target] =
                    matrix[row][target] * pivot_value - matrix[rank][target] * factor;
            }
            let divisor = matrix[row]
                .iter()
                .fold(0_i64, |gcd, &value| gcd_i64(gcd, value));
            if divisor > 1 {
                matrix[row].iter_mut().for_each(|value| *value /= divisor);
            }
        }
        rank += 1;
        if rank == matrix.len() {
            break;
        }
    }
    rank
}

fn gcd_i64(mut left: i64, mut right: i64) -> i64 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn is_prime(value: i64) -> bool {
    if value < 2 || value % 2 == 0 {
        return value == 2;
    }
    let mut divisor = 3_i64;
    while divisor <= value / divisor {
        if value % divisor == 0 {
            return false;
        }
        divisor += 2;
    }
    true
}

fn invert_field(matrix: &[Vec<i64>], field: Field) -> Option<Vec<Vec<i64>>> {
    let n = matrix.len();
    let mut augmented = matrix
        .iter()
        .enumerate()
        .map(|(row, values)| {
            let mut result = values.clone();
            result.extend((0..n).map(|column| i64::from(row == column)));
            result
        })
        .collect::<Vec<_>>();
    for column in 0..n {
        let pivot = (column..n).find(|&row| augmented[row][column] != 0)?;
        augmented.swap(column, pivot);
        let inverse = field.inv(augmented[column][column]);
        for value in &mut augmented[column] {
            *value = field.mul(*value, inverse);
        }
        for row in 0..n {
            if row == column {
                continue;
            }
            let factor = augmented[row][column];
            if factor == 0 {
                continue;
            }
            for target in column..2 * n {
                augmented[row][target] = field.sub(
                    augmented[row][target],
                    field.mul(factor, augmented[column][target]),
                );
            }
        }
    }
    Some(augmented.into_iter().map(|row| row[n..].to_vec()).collect())
}

fn matrix_rank(mut matrix: Vec<Vec<i64>>, field: Field) -> usize {
    if matrix.is_empty() {
        return 0;
    }
    let columns = matrix[0].len();
    let mut rank = 0;
    for column in 0..columns {
        let Some(pivot) = (rank..matrix.len()).find(|&row| matrix[row][column] != 0) else {
            continue;
        };
        matrix.swap(rank, pivot);
        let inverse = field.inv(matrix[rank][column]);
        for target in column..columns {
            matrix[rank][target] = field.mul(matrix[rank][target], inverse);
        }
        for row in rank + 1..matrix.len() {
            let factor = matrix[row][column];
            if factor == 0 {
                continue;
            }
            for target in column..columns {
                matrix[row][target] =
                    field.sub(matrix[row][target], field.mul(factor, matrix[rank][target]));
            }
        }
        rank += 1;
        if rank == matrix.len() {
            break;
        }
    }
    rank
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SeedKind {
    Corner,
    Dot,
    Numerator,
    Mixed,
}

#[derive(Clone, Debug)]
struct Seed {
    corner_type: CornerType,
    powers: [i32; BASIS],
    kind: SeedKind,
}

impl Seed {
    fn column(&self) -> GenuineColumn {
        GenuineColumn {
            corner_type: self.corner_type,
            powers: self.powers,
        }
    }
    fn label(&self) -> String {
        format!(
            "{}:{:?}:[{}]",
            self.corner_type.key(),
            self.kind,
            self.powers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn stable_manifest_key(&self, phase: &str, phase_index: usize) -> String {
        format!(
            "{NEXT_SEED_SCHEMA}:{phase}:{phase_index:03}:{CORNER_SCHEMA}:{}:[{}]",
            self.corner_type.key(),
            self.powers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn selected_seed_manifest_checksum(
    corners: &[Seed],
    dots: &[Seed],
    numerators: &[Seed],
    mixed: &[Seed],
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    for (phase, seeds) in [
        ("corner", corners),
        ("dot", dots),
        ("numerator", numerators),
        ("mixed-prefix-13", &mixed[..13]),
    ] {
        for (phase_index, seed) in seeds.iter().enumerate() {
            let stable_key = seed.stable_manifest_key(phase, phase_index);
            for byte in stable_key.bytes().chain([b'\n']) {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(FNV1A64_PRIME);
            }
        }
    }
    hash
}

fn seeds() -> (Vec<Seed>, Vec<Seed>, Vec<Seed>, Vec<Seed>) {
    let mut corners = Vec::new();
    let mut dots = Vec::new();
    let mut numerators = Vec::new();
    let mut mixed = Vec::new();
    for corner_type in CornerType::ALL {
        let corner =
            std::array::from_fn(|position| i32::from(corner_type.mask() & (1 << position) != 0));
        corners.push(Seed {
            corner_type,
            powers: corner,
            kind: SeedKind::Corner,
        });
        for active in active_positions(corner_type.mask()) {
            let mut powers = corner;
            powers[active] = 2;
            dots.push(Seed {
                corner_type,
                powers,
                kind: SeedKind::Dot,
            });
        }
        for inactive in (0..BASIS).filter(|&position| corner_type.mask() & (1 << position) == 0) {
            let mut powers = corner;
            powers[inactive] = -1;
            numerators.push(Seed {
                corner_type,
                powers,
                kind: SeedKind::Numerator,
            });
            for active in active_positions(corner_type.mask()) {
                let mut powers = powers;
                powers[active] = 2;
                mixed.push(Seed {
                    corner_type,
                    powers,
                    kind: SeedKind::Mixed,
                });
            }
        }
    }
    (corners, dots, numerators, mixed)
}

#[derive(Clone)]
struct Elimination {
    pivots: BTreeMap<Column, Row>,
    columns: BTreeSet<Column>,
    rows: usize,
    nonzero_rows: usize,
    input_terms: usize,
    max_input_width: usize,
}

impl Elimination {
    fn new() -> Self {
        Self {
            pivots: BTreeMap::new(),
            columns: BTreeSet::new(),
            rows: 0,
            nonzero_rows: 0,
            input_terms: 0,
            max_input_width: 0,
        }
    }

    fn add_row(&mut self, mut row: Row, field: Field) {
        self.rows += 1;
        self.input_terms += row.len();
        self.max_input_width = self.max_input_width.max(row.len());
        self.columns.extend(row.keys().cloned());
        loop {
            let Some(hardest) = row.last_key_value().map(|(column, _)| column.clone()) else {
                return;
            };
            let Some(pivot) = self.pivots.get(&hardest) else {
                break;
            };
            let factor = row[&hardest];
            for (column, &coefficient) in pivot {
                add(
                    &mut row,
                    column.clone(),
                    field.n(-i128::from(field.mul(factor, coefficient))),
                    field,
                );
            }
        }
        let (pivot, coefficient) = row
            .last_key_value()
            .map(|(column, &coefficient)| (column.clone(), coefficient))
            .unwrap();
        let inverse = field.inv(coefficient);
        row.values_mut()
            .for_each(|value| *value = field.mul(*value, inverse));
        self.pivots.insert(pivot, row);
        self.nonzero_rows += 1;
    }

    fn add_rows(&mut self, rows: impl IntoIterator<Item = Row>, field: Field) {
        for row in rows {
            self.add_row(row, field);
        }
    }
    fn free(&self) -> BTreeSet<Column> {
        self.columns
            .difference(&self.pivots.keys().cloned().collect())
            .cloned()
            .collect()
    }
}

fn column_census(columns: impl Iterator<Item = Column>) -> BTreeMap<String, usize> {
    let mut census = BTreeMap::new();
    for column in columns {
        let key = match column {
            Column::Product(_) => "product".to_owned(),
            Column::Boundary { .. } => "typed-boundary".to_owned(),
            Column::Genuine(genuine) => {
                let (_, degree, dots, _, _) = genuine.order_key();
                format!("genuine:D{dots}:N{}", degree - dots)
            }
        };
        *census.entry(key).or_insert(0) += 1;
    }
    census
}

fn boundary_census(columns: impl Iterator<Item = Column>) -> BTreeMap<String, usize> {
    let mut census = BTreeMap::new();
    for column in columns {
        if let Column::Boundary {
            topology,
            product,
            powers,
        } = column
        {
            let dots = powers
                .iter()
                .map(|power| power.saturating_sub(1).max(0) as u32)
                .sum::<u32>();
            let numerators = powers
                .iter()
                .map(|power| power.saturating_neg().max(0) as u32)
                .sum::<u32>();
            *census
                .entry(format!(
                    "{}:{product}:D{dots}:N{numerators}",
                    topology.key()
                ))
                .or_insert(0) += 1;
        }
    }
    census
}

/// Discovery-only component grading of the opaque boundary inventory.
///
/// This is deliberately weaker than production numerator transport: it
/// records where the scalar dots live after the exact matroid-factorization
/// split, but does not declare a numerator-bearing component closed.  The
/// production service must still replay the complete affine quadratic map and
/// prove the cross-component parity zeros over Q(d,m2).
fn component_boundary_census(
    columns: impl Iterator<Item = Column>,
    catalog: &Catalog,
) -> BTreeMap<String, usize> {
    let mut census = BTreeMap::new();
    for column in columns {
        let Column::Boundary {
            topology,
            product,
            powers,
        } = column
        else {
            continue;
        };
        let mask = powers
            .iter()
            .take(9)
            .enumerate()
            .fold(0_u16, |mask, (position, &power)| {
                mask | (u16::from(power > 0) << position)
            });
        let factorization =
            factorize(topology, mask, catalog).expect("a typed boundary factorizes");
        assert_eq!(factorization.product.stable_key(), product);
        let dots = powers
            .iter()
            .map(|power| power.saturating_sub(1).max(0) as u32)
            .sum::<u32>();
        let numerators = powers
            .iter()
            .map(|power| power.saturating_neg().max(0) as u32)
            .sum::<u32>();
        let mut component_grades = factorization
            .components
            .iter()
            .map(|component| {
                let component_dots = component
                    .physical_positions
                    .iter()
                    .map(|&position| powers[position].saturating_sub(1).max(0) as u32)
                    .sum::<u32>();
                format!("{}:D{component_dots}", component.master.name())
            })
            .collect::<Vec<_>>();
        component_grades.sort();
        *census
            .entry(format!(
                "{product}:D{dots}:N{numerators}:{}",
                component_grades.join("+")
            ))
            .or_insert(0) += 1;
    }
    census
}

#[derive(Clone, Copy, Debug)]
enum FactorizedReferenceColumn {
    Local {
        component: usize,
        position: usize,
    },
    Cross {
        left_component: usize,
        right_component: usize,
    },
}

fn complete_component_routings(master: Master) -> Vec<Vec<i64>> {
    match master {
        Master::T1 => vec![vec![1]],
        Master::S2 => lower_reference_rows(master)
            .into_iter()
            .map(|(_, routing)| routing)
            .collect(),
        Master::B4 | Master::F5 | Master::M6 => THREE_LOOP_ROUTINGS
            .iter()
            .map(|routing| routing.to_vec())
            .collect(),
    }
}

fn factorized_reference_family(
    factorization: &Factorization,
    field: Field,
) -> (Family, Vec<FactorizedReferenceColumn>) {
    let mut qforms = Vec::new();
    let mut shifts = Vec::new();
    let mut columns = Vec::new();
    let mut coordinate_owner = [usize::MAX; LOOPS];
    for (component_index, component) in factorization.components.iter().enumerate() {
        for coordinate in
            component.reference_offset..component.reference_offset + component.master.loops()
        {
            coordinate_owner[coordinate] = component_index;
        }
        for (position, routing) in complete_component_routings(component.master)
            .into_iter()
            .enumerate()
        {
            let mut embedded = [0_i64; LOOPS];
            for (local, value) in routing.into_iter().enumerate() {
                embedded[component.reference_offset + local] = value;
            }
            qforms.push(routing_qform(&embedded, field));
            shifts.push(1);
            columns.push(FactorizedReferenceColumn::Local {
                component: component_index,
                position,
            });
        }
    }
    assert!(
        coordinate_owner
            .into_iter()
            .all(|owner| owner != usize::MAX)
    );
    for left in 0..LOOPS {
        for right in left + 1..LOOPS {
            let left_component = coordinate_owner[left];
            let right_component = coordinate_owner[right];
            if left_component == right_component {
                continue;
            }
            let mut qform = vec![0_i64; BASIS];
            qform[sp_index(left, right)] = 1;
            qforms.push(qform);
            shifts.push(0);
            columns.push(FactorizedReferenceColumn::Cross {
                left_component,
                right_component,
            });
        }
    }
    assert_eq!(
        (qforms.len(), shifts.len(), columns.len()),
        (BASIS, BASIS, BASIS)
    );
    let inverse = invert_field(&qforms, field).expect("factorized reference basis is complete");
    (
        Family {
            qforms,
            shifts,
            inverse,
        },
        columns,
    )
}

fn component_full_position(master: Master, compact_position: usize) -> usize {
    match master {
        Master::B4 => [0, 1, 3, 5][compact_position],
        Master::T1 | Master::S2 | Master::F5 | Master::M6 => compact_position,
    }
}

fn component_base_powers(
    factorization: &Factorization,
    powers: &[i32; BASIS],
    catalog: &Catalog,
) -> Vec<Vec<i32>> {
    factorization
        .components
        .iter()
        .map(|component| {
            let reference = &catalog.lower[&component.master];
            let mut local = vec![0_i32; complete_component_routings(component.master).len()];
            for &physical_position in &component.physical_positions {
                let compact_position = component
                    .source_signature
                    .lines
                    .iter()
                    .zip(&reference.lines)
                    .find_map(|(source, target)| {
                        (source.position == physical_position).then_some(target.position)
                    })
                    .expect("every component line has a reference match");
                local[component_full_position(component.master, compact_position)] =
                    powers[physical_position];
            }
            local
        })
        .collect()
}

fn local_shape(master: Master, powers: &[i32]) -> String {
    let active = powers.iter().filter(|&&power| power > 0).count();
    let dots = powers
        .iter()
        .map(|power| power.saturating_sub(1).max(0) as u32)
        .sum::<u32>();
    let numerators = powers
        .iter()
        .map(|power| power.saturating_neg().max(0) as u32)
        .sum::<u32>();
    format!("{}:L{active}:D{dots}:N{numerators}", master.name())
}

/// Finite-field discovery audit of the exact component requests induced by a
/// sole parent numerator.  Counts are parent-key incidences, deduplicated
/// within each key.  Production must reconstruct the same support over exact
/// rationals and replay every discarded cross term as an odd-parity zero.
fn numerator_component_service_census(
    columns: impl Iterator<Item = Column>,
    catalog: &Catalog,
    field: Field,
) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let source_families = [Topology::H, Topology::X]
        .into_iter()
        .map(|topology| (topology, Family::build(topology, field)))
        .collect::<BTreeMap<_, _>>();
    let mut scalar_services = BTreeMap::new();
    let mut parity_services = BTreeMap::new();
    for column in columns {
        let Column::Boundary {
            topology,
            product,
            powers,
        } = column
        else {
            continue;
        };
        let numerator_positions = powers
            .iter()
            .enumerate()
            .filter_map(|(position, &power)| (power < 0).then_some((position, power)))
            .collect::<Vec<_>>();
        if numerator_positions.is_empty() {
            continue;
        }
        assert_eq!(numerator_positions.len(), 1);
        assert_eq!(numerator_positions[0].1, -1);
        let numerator_position = numerator_positions[0].0;
        let mask = powers
            .iter()
            .take(9)
            .enumerate()
            .fold(0_u16, |mask, (position, &power)| {
                mask | (u16::from(power > 0) << position)
            });
        let factorization = factorize(topology, mask, catalog).expect("typed boundary factorizes");
        assert_eq!(factorization.product.stable_key(), product);
        let (reference_family, reference_columns) =
            factorized_reference_family(&factorization, field);
        let (constant, coefficients) = affine_image(
            &source_families[&topology],
            &reference_family,
            numerator_position,
            &factorization.source_to_reference_loop_map,
            field,
        );
        let base = component_base_powers(&factorization, &powers, catalog);
        let parent_dots = powers
            .iter()
            .map(|power| power.saturating_sub(1).max(0) as u32)
            .sum::<u32>();
        let parent_prefix = format!("{product}:D{parent_dots}:N1");
        let mut scalar_labels = BTreeSet::new();
        let mut parity_labels = BTreeSet::new();
        if constant != 0 {
            for (component, local) in factorization.components.iter().zip(&base) {
                scalar_labels.insert(local_shape(component.master, local));
            }
        }
        for (column, coefficient) in reference_columns.iter().zip(coefficients) {
            if coefficient == 0 {
                continue;
            }
            match *column {
                FactorizedReferenceColumn::Local {
                    component,
                    position,
                } => {
                    for (other_component, local) in factorization.components.iter().zip(&base) {
                        scalar_labels.insert(local_shape(other_component.master, local));
                    }
                    let mut lowered = base[component].clone();
                    lowered[position] -= 1;
                    scalar_labels.insert(local_shape(
                        factorization.components[component].master,
                        &lowered,
                    ));
                }
                FactorizedReferenceColumn::Cross {
                    left_component,
                    right_component,
                } => {
                    let mut pair = [
                        factorization.components[left_component].master.name(),
                        factorization.components[right_component].master.name(),
                    ];
                    pair.sort();
                    parity_labels.insert(format!("{}x{}:rank1xrank1", pair[0], pair[1]));
                }
            }
        }
        for label in scalar_labels {
            *scalar_services
                .entry(format!("{parent_prefix}:{label}"))
                .or_insert(0) += 1;
        }
        for label in parity_labels {
            *parity_services
                .entry(format!("{parent_prefix}:{label}"))
                .or_insert(0) += 1;
        }
    }
    (scalar_services, parity_services)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct F5D2N1Incidence {
    topology: Topology,
    product: String,
    parent_powers: [i32; BASIS],
    numerator_position: usize,
    component_index: usize,
    component_reference_offset: usize,
    component_global_basis_slots: Vec<usize>,
    component_physical_positions: Vec<usize>,
    lowered_local_position: usize,
    labelled_target: [i32; 6],
    canonical_target: [i32; 6],
}

impl F5D2N1Incidence {
    fn support_key(&self) -> String {
        format!(
            "topology={} product={} parent={:?} numerator={} component={} reference_offset={} global_slots={:?} physical_positions={:?} lowered_local={} labelled={:?} canonical={:?}",
            self.topology.key(),
            self.product,
            self.parent_powers,
            self.numerator_position,
            self.component_index,
            self.component_reference_offset,
            self.component_global_basis_slots,
            self.component_physical_positions,
            self.lowered_local_position,
            self.labelled_target,
            self.canonical_target,
        )
    }
}

/// Concrete finite-field support of the targeted local F5 D2/N1 service.
///
/// An incidence retains the complete parent key and the owned factor component
/// instead of merging equal F5 targets too early.  The coefficient is used
/// only as a nonzero-support test at this image and is intentionally excluded
/// from the returned key.  Thus manifests from different prime images can be
/// compared literally.
fn f5_d2n1_manifest(
    columns: impl Iterator<Item = Column>,
    catalog: &Catalog,
    field: Field,
) -> (
    BTreeMap<[usize; 6], Vec<Vec<i64>>>,
    BTreeSet<F5D2N1Incidence>,
) {
    let source_families = [Topology::H, Topology::X]
        .into_iter()
        .map(|topology| (topology, Family::build(topology, field)))
        .collect::<BTreeMap<_, _>>();
    let stabilizer = f5_stabilizer();
    let mut incidences = BTreeSet::new();
    for column in columns {
        let Column::Boundary {
            topology,
            product,
            powers,
        } = column
        else {
            continue;
        };
        let numerator_positions = powers
            .iter()
            .enumerate()
            .filter_map(|(position, &power)| (power < 0).then_some((position, power)))
            .collect::<Vec<_>>();
        if numerator_positions.is_empty() {
            continue;
        }
        assert_eq!(numerator_positions.len(), 1);
        assert_eq!(numerator_positions[0].1, -1);
        let numerator_position = numerator_positions[0].0;
        let mask = powers
            .iter()
            .take(9)
            .enumerate()
            .fold(0_u16, |mask, (position, &power)| {
                mask | (u16::from(power > 0) << position)
            });
        let factorization = factorize(topology, mask, catalog).expect("typed boundary factorizes");
        assert_eq!(factorization.product.stable_key(), product);
        let (reference_family, reference_columns) =
            factorized_reference_family(&factorization, field);
        let (_, coefficients) = affine_image(
            &source_families[&topology],
            &reference_family,
            numerator_position,
            &factorization.source_to_reference_loop_map,
            field,
        );
        let base = component_base_powers(&factorization, &powers, catalog);
        for (reference_column, coefficient) in reference_columns.iter().zip(coefficients) {
            if coefficient == 0 {
                continue;
            }
            let FactorizedReferenceColumn::Local {
                component,
                position,
            } = *reference_column
            else {
                continue;
            };
            let owner = &factorization.components[component];
            if owner.master != Master::F5 {
                continue;
            }
            let mut lowered = base[component].clone();
            lowered[position] -= 1;
            let active = lowered.iter().filter(|&&power| power > 0).count();
            let dots = lowered
                .iter()
                .map(|power| power.saturating_sub(1).max(0) as u32)
                .sum::<u32>();
            let numerators = lowered
                .iter()
                .map(|power| power.saturating_neg().max(0) as u32)
                .sum::<u32>();
            if (active, dots, numerators) != (5, 2, 1) {
                continue;
            }
            let labelled_target: [i32; 6] = lowered
                .try_into()
                .expect("the completed F5 family has six entries");
            let canonical_target = canonical_f5_powers(labelled_target, &stabilizer);
            incidences.insert(F5D2N1Incidence {
                topology,
                product: product.clone(),
                parent_powers: powers,
                numerator_position,
                component_index: component,
                component_reference_offset: owner.reference_offset,
                component_global_basis_slots: owner.global_basis_slots.clone(),
                component_physical_positions: owner.physical_positions.clone(),
                lowered_local_position: position,
                labelled_target,
                canonical_target,
            });
        }
    }
    (stabilizer, incidences)
}

fn discovery_checksum(lines: &[String]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for line in lines {
        for byte in line.bytes().chain([b'\n']) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn print_f5_d2n1_manifest(columns: impl Iterator<Item = Column>, catalog: &Catalog, field: Field) {
    let (stabilizer, incidences) = f5_d2n1_manifest(columns, catalog, field);
    println!("f5_d2n1_stabilizer order={}", stabilizer.len());
    for (permutation, loop_map) in &stabilizer {
        println!(
            "  stabilizer permutation={permutation:?} loop_map={loop_map:?} determinant={}",
            determinant_integer(loop_map)
        );
    }

    let support_lines = incidences
        .iter()
        .map(F5D2N1Incidence::support_key)
        .collect::<Vec<_>>();
    for (index, line) in support_lines.iter().enumerate() {
        println!("  incidence={:02} {line}", index + 1);
    }

    let parent_keys = incidences
        .iter()
        .map(|incidence| {
            (
                incidence.topology,
                incidence.product.clone(),
                incidence.parent_powers,
            )
        })
        .collect::<BTreeSet<_>>();
    let labelled_targets = incidences
        .iter()
        .map(|incidence| incidence.labelled_target)
        .collect::<BTreeSet<_>>();
    let canonical_targets = incidences
        .iter()
        .map(|incidence| incidence.canonical_target)
        .collect::<BTreeSet<_>>();
    let complete_labelled_domain = complete_labelled_f5_d2n1_domain();
    let complete_canonical_domain = complete_labelled_domain
        .iter()
        .map(|&target| canonical_f5_powers(target, &stabilizer))
        .collect::<BTreeSet<_>>();
    let mut labelled_multiplicity = BTreeMap::<[i32; 6], usize>::new();
    let mut canonical_multiplicity = BTreeMap::<[i32; 6], usize>::new();
    let mut owner_multiplicity = BTreeMap::<String, usize>::new();
    let mut numerator_position_multiplicity = BTreeMap::<usize, usize>::new();
    for incidence in &incidences {
        *labelled_multiplicity
            .entry(incidence.labelled_target)
            .or_insert(0) += 1;
        *canonical_multiplicity
            .entry(incidence.canonical_target)
            .or_insert(0) += 1;
        *owner_multiplicity
            .entry(format!(
                "component={} reference_offset={} global_slots={:?}",
                incidence.component_index,
                incidence.component_reference_offset,
                incidence.component_global_basis_slots,
            ))
            .or_insert(0) += 1;
        *numerator_position_multiplicity
            .entry(incidence.numerator_position)
            .or_insert(0) += 1;
    }
    assert_eq!(
        (incidences.len(), parent_keys.len()),
        (31, 31),
        "the selected 123-seed shell must retain all 31 owned F5 D2/N1 parent incidences"
    );
    assert_eq!(
        canonical_targets, complete_canonical_domain,
        "the selected manifest must touch every F5 D2/N1 stabilizer orbit"
    );
    println!(
        "f5_d2n1_manifest_summary branch_incidences={} unique_parent_keys={} unique_labelled_targets={} unique_canonical_targets={} complete_labelled_domain={} complete_canonical_domain={} orbit_complete=true support_checksum=fnv1a64:{:016x}",
        incidences.len(),
        parent_keys.len(),
        labelled_targets.len(),
        canonical_targets.len(),
        complete_labelled_domain.len(),
        complete_canonical_domain.len(),
        discovery_checksum(&support_lines),
    );
    println!("  labelled_target_multiplicity={labelled_multiplicity:?}");
    println!("  canonical_target_multiplicity={canonical_multiplicity:?}");
    println!("  component_owner_multiplicity={owner_multiplicity:?}");
    println!("  parent_numerator_position_multiplicity={numerator_position_multiplicity:?}");
}

fn seed_hardness(seed: &Seed, base_free: &BTreeSet<Column>) -> (bool, GenuineColumn) {
    (
        base_free.contains(&Column::Genuine(seed.column())),
        seed.column(),
    )
}

fn report(name: &str, elimination: &Elimination, base_free: &BTreeSet<Column>) {
    let free = elimination.free();
    let pivoted_base = base_free
        .iter()
        .filter(|column| elimination.pivots.contains_key(*column))
        .count();
    let pivot_terms = elimination
        .pivots
        .values()
        .map(BTreeMap::len)
        .sum::<usize>();
    let max_pivot_width = elimination
        .pivots
        .values()
        .map(BTreeMap::len)
        .max()
        .unwrap_or(0);
    println!(
        "{name}: seeds={} rows={} nonzero_rows={} input_terms={} max_input_width={} columns={} rank={} nullity={} pivot_terms={} max_pivot_width={} base_free_pivoted={} base_free_remaining={}",
        elimination.rows / 16,
        elimination.rows,
        elimination.nonzero_rows,
        elimination.input_terms,
        elimination.max_input_width,
        elimination.columns.len(),
        elimination.pivots.len(),
        free.len(),
        pivot_terms,
        max_pivot_width,
        pivoted_base,
        base_free.len() - pivoted_base
    );
    println!(
        "  columns={:?}",
        column_census(elimination.columns.iter().cloned())
    );
    println!("  free={:?}", column_census(free.iter().cloned()));
    let boundaries = boundary_census(elimination.columns.iter().cloned());
    if !boundaries.is_empty() {
        println!("  boundaries={boundaries:?}");
    }
    let free_boundaries = boundary_census(free.into_iter());
    if !free_boundaries.is_empty() {
        println!("  free_boundaries={free_boundaries:?}");
    }
}

fn main() {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let prime = arguments
        .first()
        .map(|value| value.parse().unwrap())
        .unwrap_or(1_000_003_i64);
    let dimension = arguments
        .get(1)
        .map(|value| value.parse().unwrap())
        .unwrap_or(17_i64);
    let component_boundary_audit = arguments
        .iter()
        .any(|value| value == "--component-boundary-audit");
    let f5_d2n1_manifest = arguments.iter().any(|value| value == "--f5-d2n1-manifest");
    let seed_manifest = arguments.iter().any(|value| value == "--seed-manifest");
    assert!(is_prime(prime), "modulus must be prime");
    let field = Field { p: prime };
    let mut normalizer = Normalizer::new(field, dimension);
    let (corners, mut dots, mut numerators, mut mixed) = seeds();
    assert_eq!(
        (corners.len(), dots.len(), numerators.len(), mixed.len()),
        (10, 72, 28, 186)
    );

    let mut base = Elimination::new();
    for seed in &corners {
        base.add_rows(normalizer.raw_rows(seed), field);
    }
    let base_free = base.free();
    println!("image: p={prime} d={dimension} m2=1");
    report("corner", &base, &base_free);
    assert_eq!(
        (base.columns.len(), base.pivots.len(), base_free.len()),
        (223, 159, 64),
        "finite-field probe must first reproduce the exact corner certificate"
    );

    dots.sort_by_key(|seed| seed_hardness(seed, &base_free));
    dots.reverse();
    numerators.sort_by_key(|seed| seed_hardness(seed, &base_free));
    numerators.reverse();
    mixed.sort_by_key(|seed| seed_hardness(seed, &base_free));
    mixed.reverse();

    if seed_manifest {
        println!("frozen 123-seed manifest:");
        for (phase, seeds) in [
            ("corner", corners.as_slice()),
            ("dot", dots.as_slice()),
            ("numerator", numerators.as_slice()),
            ("mixed", &mixed[..13]),
        ] {
            for (index, seed) in seeds.iter().enumerate() {
                println!("  {phase}:{index:03}:{}", seed.label());
            }
        }
        let checksum = selected_seed_manifest_checksum(&corners, &dots, &numerators, &mixed);
        assert_eq!(
            checksum, FROZEN_NEXT_SEED_CHECKSUM,
            "the selected 123-seed stable-key manifest changed"
        );
        println!("seed_manifest_checksum=fnv1a64:{checksum:016x}");
    }

    // Discovery ordering: address a retained free coordinate first, then use
    // the production hardest-column order.  This is deterministic but is not
    // claimed globally cardinality-minimal.
    let mut free_first_candidates = dots
        .iter()
        .chain(&numerators)
        .chain(&mixed)
        .cloned()
        .collect::<Vec<_>>();
    free_first_candidates.sort_by_key(|seed| seed_hardness(seed, &base_free));
    free_first_candidates.reverse();
    let mut free_first = base.clone();
    let mut first_complete_prefix = None;
    for (index, seed) in free_first_candidates.iter().enumerate() {
        free_first.add_rows(normalizer.raw_rows(seed), field);
        let pivoted_nonterminal = base_free
            .iter()
            .filter(|column| match column {
                Column::Product(_) => false,
                Column::Genuine(genuine) => genuine.order_key().1 != 0,
                Column::Boundary { .. } => true,
            })
            .filter(|column| free_first.pivots.contains_key(*column))
            .count();
        if pivoted_nonterminal == 48 && first_complete_prefix.is_none() {
            first_complete_prefix = Some(index + 1);
            println!(
                "free-first closes all 48 nonterminal base-free columns after {} added seeds ({} total rows); last={}",
                index + 1,
                (corners.len() + index + 1) * 16,
                seed.label()
            );
            break;
        }
    }
    if let Some(prefix) = first_complete_prefix {
        report(
            &format!("free_first_prefix_{prefix}"),
            &free_first,
            &base_free,
        );
    } else {
        println!("free-first ordering did not close all 48 nonterminal base-free columns");
    }

    let mut axis = base.clone();
    let mut last_pivoted = 0;
    for (index, seed) in dots.iter().chain(&numerators).enumerate() {
        axis.add_rows(normalizer.raw_rows(seed), field);
        let pivoted = base_free
            .iter()
            .filter(|column| axis.pivots.contains_key(*column))
            .count();
        if pivoted != last_pivoted {
            println!(
                "  axis milestone seed={} added={} base_free_pivoted={} label={}",
                index + 1,
                if index < dots.len() {
                    "dot"
                } else {
                    "numerator"
                },
                pivoted,
                seed.label()
            );
            last_pivoted = pivoted;
        }
    }
    report("axis_DplusN_le_1", &axis, &base_free);

    let mut full = axis.clone();
    let mut mixed_prefix = None;
    last_pivoted = base_free
        .iter()
        .filter(|column| full.pivots.contains_key(*column))
        .count();
    for (index, seed) in mixed.iter().enumerate() {
        full.add_rows(normalizer.raw_rows(seed), field);
        let pivoted = base_free
            .iter()
            .filter(|column| full.pivots.contains_key(*column))
            .count();
        if pivoted != last_pivoted {
            println!(
                "  mixed milestone seed={} base_free_pivoted={} label={}",
                index + 1,
                pivoted,
                seed.label()
            );
            last_pivoted = pivoted;
        }
        if index + 1 == 13 {
            mixed_prefix = Some(full.clone());
        }
    }
    println!("selected mixed prefix (13 seeds):");
    for (index, seed) in mixed.iter().take(13).enumerate() {
        println!("  {} {}", index + 1, seed.label());
    }
    report(
        "axis_plus_mixed_prefix_13",
        mixed_prefix.as_ref().unwrap(),
        &base_free,
    );
    if component_boundary_audit {
        println!(
            "axis_plus_mixed_prefix_13 component_boundaries={:?}",
            component_boundary_census(
                mixed_prefix.as_ref().unwrap().columns.iter().cloned(),
                &normalizer.catalog,
            )
        );
        let (numerator_scalar_services, numerator_parity_services) =
            numerator_component_service_census(
                mixed_prefix.as_ref().unwrap().columns.iter().cloned(),
                &normalizer.catalog,
                field,
            );
        println!(
            "axis_plus_mixed_prefix_13 numerator_scalar_services={numerator_scalar_services:?}"
        );
        println!(
            "axis_plus_mixed_prefix_13 numerator_parity_services={numerator_parity_services:?}"
        );
    }
    if f5_d2n1_manifest {
        print_f5_d2n1_manifest(
            mixed_prefix.as_ref().unwrap().columns.iter().cloned(),
            &normalizer.catalog,
            field,
        );
    }
    report("full_D1_N1", &full, &base_free);

    println!("base free columns (hardest first):");
    for column in base_free.iter().rev() {
        println!("  {column:?}");
    }
}
