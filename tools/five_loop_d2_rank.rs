//! Exact-orbit finite-field rank probe for the five-loop banana D=2 shell.
//!
//! Off-diagonal numerator moments are represented as marked edges of the six
//! oriented physical lines, not as permutations of RustRed's nine auxiliary
//! basis entries.  The probe adds momentum-conservation and diagonal
//! `l_i^2=D_i-m2` relations explicitly.  It is a discovery oracle, not a
//! production reduction certificate.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

const P: i64 = 1_000_003;
const D: i64 = 17;
const M: i64 = 19;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Column {
    Scalar([i32; 6]),
    Moment { powers: [i32; 6], edge: [usize; 2] },
}

impl Column {
    fn hardness(&self) -> (u64, u64, u8, [i32; 6], [usize; 2]) {
        match self {
            Self::Scalar(powers) => {
                let dots = powers
                    .iter()
                    .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
                    .sum();
                (dots, dots, 0, *powers, [0, 0])
            }
            Self::Moment { powers, edge } => {
                let dots = powers
                    .iter()
                    .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
                    .sum();
                (dots + 1, dots, 1, *powers, *edge)
            }
        }
    }
}

impl Ord for Column {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hardness().cmp(&other.hardness())
    }
}

impl PartialOrd for Column {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

type Row = BTreeMap<Column, i64>;

fn modulo(value: i64) -> i64 {
    value.rem_euclid(P)
}

fn power_mod(mut base: i64, mut exponent: i64) -> i64 {
    let mut result = 1;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = result * base % P;
        }
        base = base * base % P;
        exponent >>= 1;
    }
    result
}

fn inverse(value: i64) -> i64 {
    power_mod(modulo(value), P - 2)
}

fn next_permutation(values: &mut [usize; 6]) -> bool {
    let Some(left) = (0..5).rfind(|position| values[*position] < values[*position + 1]) else {
        return false;
    };
    let right = (left + 1..6)
        .rfind(|position| values[left] < values[*position])
        .unwrap();
    values.swap(left, right);
    values[left + 1..].reverse();
    true
}

fn canonical_scalar(powers: [i32; 6]) -> Column {
    let mut powers = powers;
    powers.sort_by(|left, right| right.cmp(left));
    Column::Scalar(powers)
}

fn canonical_moment(powers: [i32; 6], edge: [usize; 2]) -> Column {
    let mut permutation = [0, 1, 2, 3, 4, 5];
    let mut best = None;
    loop {
        let mapped_powers = std::array::from_fn(|target| powers[permutation[target]]);
        let mut inverse = [0; 6];
        for (target, source) in permutation.into_iter().enumerate() {
            inverse[source] = target;
        }
        let mut mapped_edge = [inverse[edge[0]], inverse[edge[1]]];
        mapped_edge.sort();
        let candidate = Column::Moment {
            powers: mapped_powers,
            edge: mapped_edge,
        };
        if best.as_ref().is_none_or(|current| candidate > *current) {
            best = Some(candidate);
        }
        if !next_permutation(&mut permutation) {
            return best.unwrap();
        }
    }
}

fn add(row: &mut Row, column: Column, coefficient: i64) {
    let coefficient = modulo(coefficient);
    let updated = modulo(row.get(&column).copied().unwrap_or(0) + coefficient);
    if updated == 0 {
        row.remove(&column);
    } else {
        row.insert(column, updated);
    }
}

fn raw_rows(seed: [i32; 6]) -> Vec<Row> {
    let mut rows = Vec::with_capacity(25);
    for differentiated in 0..5 {
        for contracted in 0..5 {
            let mut row = Row::new();
            if differentiated == contracted {
                add(&mut row, canonical_scalar(seed), D);
            }
            let mut dotted = seed;
            dotted[differentiated] += 1;
            add(
                &mut row,
                canonical_moment(dotted, [differentiated, contracted]),
                -2 * i64::from(seed[differentiated]),
            );
            let mut dotted_sixth = seed;
            dotted_sixth[5] += 1;
            add(
                &mut row,
                canonical_moment(dotted_sixth, [5, contracted]),
                2 * i64::from(seed[5]),
            );
            rows.push(row);
        }
    }
    rows
}

fn diagonal_relation(powers: [i32; 6], line: usize) -> Row {
    let mut row = Row::new();
    add(&mut row, canonical_moment(powers, [line, line]), 1);
    let mut lowered = powers;
    lowered[line] -= 1;
    add(&mut row, canonical_scalar(lowered), -1);
    add(&mut row, canonical_scalar(powers), M);
    row
}

fn momentum_relation(powers: [i32; 6], line: usize) -> Row {
    let mut row = Row::new();
    for other in 0..6 {
        add(&mut row, canonical_moment(powers, [line, other]), 1);
    }
    row
}

fn compositions(total: u32, parts: usize, prefix: &mut Vec<u32>, output: &mut Vec<Vec<u32>>) {
    if parts == 1 {
        let mut composition = prefix.clone();
        composition.push(total);
        output.push(composition);
        return;
    }
    for value in 0..=total {
        prefix.push(value);
        compositions(total - value, parts - 1, prefix, output);
        prefix.pop();
    }
}

fn seeds(max_dots: u32) -> Vec<[i32; 6]> {
    let mut result = Vec::new();
    for dots in 0..=max_dots {
        let mut values = Vec::new();
        compositions(dots, 6, &mut Vec::new(), &mut values);
        result.extend(values.into_iter().map(|dots| {
            std::array::from_fn(|position| 1 + i32::try_from(dots[position]).unwrap())
        }));
    }
    result
}

fn reduce(mut row: Row, pivots: &BTreeMap<Column, Row>) -> Row {
    while let Some((leading, coefficient)) = row.last_key_value().map(|(c, a)| (c.clone(), *a)) {
        let Some(pivot) = pivots.get(&leading) else {
            break;
        };
        row.remove(&leading);
        for (column, pivot_coefficient) in pivot {
            add(&mut row, column.clone(), -coefficient * pivot_coefficient);
        }
    }
    row
}

fn eliminate(rows: Vec<Row>) -> BTreeMap<Column, Row> {
    let mut pivots = BTreeMap::new();
    for row in rows {
        let mut row = reduce(row, &pivots);
        let Some((leading, coefficient)) = row.last_key_value().map(|(c, a)| (c.clone(), *a))
        else {
            continue;
        };
        row.remove(&leading);
        let normalization = inverse(coefficient);
        for coefficient in row.values_mut() {
            *coefficient = *coefficient * normalization % P;
        }
        pivots.insert(leading, row);
    }
    pivots
}

fn main() {
    let max_seed_dots = std::env::args()
        .nth(1)
        .map_or(1, |value| value.parse::<u32>().unwrap());
    let seeds = seeds(max_seed_dots);
    let mut rows = Vec::new();
    let mut moment_powers = BTreeSet::new();
    for seed in &seeds {
        let seed_rows = raw_rows(*seed);
        for row in &seed_rows {
            for column in row.keys() {
                if let Column::Moment { powers, .. } = column {
                    moment_powers.insert(*powers);
                }
            }
        }
        rows.extend(seed_rows);
    }
    for powers in moment_powers {
        for line in 0..6 {
            rows.push(diagonal_relation(powers, line));
            rows.push(momentum_relation(powers, line));
        }
    }
    let columns = rows
        .iter()
        .flat_map(|row| row.keys().cloned())
        .collect::<BTreeSet<_>>();
    let pivots = eliminate(rows.clone());
    println!(
        "Dseed={max_seed_dots}: seeds={} rows={} columns={} rank={} nullity={}",
        seeds.len(),
        rows.len(),
        columns.len(),
        pivots.len(),
        columns.len() - pivots.len()
    );
    for target in [
        canonical_scalar([3, 1, 1, 1, 1, 1]),
        canonical_scalar([2, 2, 1, 1, 1, 1]),
        canonical_moment([2, 2, 1, 1, 1, 1], [0, 1]),
    ] {
        println!(
            "  {target:?}: {}",
            if pivots.contains_key(&target) {
                "pivot"
            } else {
                "free"
            }
        );
    }
    let free_scalar_columns = columns
        .iter()
        .filter_map(|column| match column {
            Column::Scalar(powers) if !pivots.contains_key(column) => Some(*powers),
            _ => None,
        })
        .collect::<Vec<_>>();
    println!(
        "  free scalar columns ({}): {free_scalar_columns:?}",
        free_scalar_columns.len()
    );
    let free_top_columns = free_scalar_columns
        .iter()
        .copied()
        .filter(|powers| powers.iter().all(|power| *power > 0))
        .collect::<Vec<_>>();
    println!(
        "  free six-line scalar columns ({}): {free_top_columns:?}",
        free_top_columns.len()
    );
}
