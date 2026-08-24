//! Finite-field discovery oracle for coupled scalar/numerator shells of the
//! five-loop equal-mass banana.
//!
//! This standalone tool deliberately uses only `std`.  Six oriented physical
//! lines obey `l_0+...+l_5=0`; a numerator monomial is a multigraph whose edge
//! `[u,v]` denotes `l_u.l_v`.  It generates native `d/dk_i.k_j` rows at scalar
//! and one-numerator seeds, plus exact diagonal and momentum-conservation
//! relations for the complete emitted multigraph halo.  Full `S6` acts jointly
//! on physical powers and graph vertices.
//!
//! A finite-field pivot is discovery evidence only.  Production claims still
//! require reconstruction and exact replay over `Q(d,m2)`.
//!
//! Arguments are
//! `scalar_D numerator_D numerator_N prime d m2 d2_subset numerator_min_D`,
//! where the scalar selector is `all`, `a2`, `b2`, or `none`.  Setting
//! `numerator_N=0` disables numerator seeds.  The final bound defaults to zero;
//! setting it equal to `numerator_D` selects one exact numerator-dot layer.
//! `interpolate prime max_total_degree` reads `x y` modular samples from
//! standard input and is likewise discovery-only.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

#[derive(Clone, Copy, Debug)]
struct Field {
    prime: i64,
    dimension: i64,
    mass: i64,
}

impl Field {
    fn modulo(self, value: i64) -> i64 {
        value.rem_euclid(self.prime)
    }

    fn power(self, mut base: i64, mut exponent: i64) -> i64 {
        let mut result = 1;
        base = self.modulo(base);
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = result * base % self.prime;
            }
            base = base * base % self.prime;
            exponent >>= 1;
        }
        result
    }

    fn inverse(self, value: i64) -> i64 {
        self.power(value, self.prime - 2)
    }
}

type Edge = [u8; 2];

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Column {
    powers: [i32; 6],
    edges: Vec<Edge>,
}

impl Column {
    fn active_lines(&self) -> usize {
        self.powers.iter().filter(|power| **power > 0).count()
    }

    fn dot_degree(&self) -> u64 {
        self.powers
            .iter()
            .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
            .sum()
    }

    fn numerator_degree(&self) -> u64 {
        self.edges.len() as u64
    }

    fn hardness(&self) -> (usize, u64, u64, [i32; 6], &[Edge]) {
        let dots = self.dot_degree();
        (
            self.active_lines(),
            dots + self.numerator_degree(),
            dots,
            self.powers,
            &self.edges,
        )
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

#[derive(Clone, Debug)]
struct Symmetry {
    permutations: Vec<([usize; 6], [usize; 6])>,
}

impl Symmetry {
    fn new() -> Self {
        let mut source = [0, 1, 2, 3, 4, 5];
        let mut permutations = Vec::with_capacity(720);
        loop {
            let mut inverse = [0; 6];
            for (target, value) in source.into_iter().enumerate() {
                inverse[value] = target;
            }
            permutations.push((source, inverse));
            if !next_permutation(&mut source) {
                break;
            }
        }
        Self { permutations }
    }

    fn canonical(&self, powers: [i32; 6], edges: &[Edge]) -> Column {
        let mut best = None;
        for (source, inverse) in &self.permutations {
            let mapped_powers = std::array::from_fn(|target| powers[source[target]]);
            let mut mapped_edges = edges
                .iter()
                .map(|edge| {
                    normalized_edge(inverse[usize::from(edge[0])], inverse[usize::from(edge[1])])
                })
                .collect::<Vec<_>>();
            mapped_edges.sort_unstable();
            let candidate = Column {
                powers: mapped_powers,
                edges: mapped_edges,
            };
            if best.as_ref().is_none_or(|current| candidate > *current) {
                best = Some(candidate);
            }
        }
        best.expect("S6 contains the identity")
    }
}

fn normalized_edge(left: usize, right: usize) -> Edge {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    [u8::try_from(left).unwrap(), u8::try_from(right).unwrap()]
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

fn add(
    row: &mut Row,
    symmetry: &Symmetry,
    field: Field,
    powers: [i32; 6],
    edges: &[Edge],
    coefficient: i64,
) {
    let coefficient = field.modulo(coefficient);
    if coefficient == 0 {
        return;
    }
    let column = symmetry.canonical(powers, edges);
    // A polynomial numerator cannot restore a missing fifth loop scale.
    if column.active_lines() <= 4 {
        return;
    }
    let updated = field.modulo(row.get(&column).copied().unwrap_or(0) + coefficient);
    if updated == 0 {
        row.remove(&column);
    } else {
        row.insert(column, updated);
    }
}

fn line_derivative(line: usize, differentiated: usize) -> i64 {
    match line {
        0..=4 if line == differentiated => 1,
        0..=4 => 0,
        5 => -1,
        _ => unreachable!(),
    }
}

fn raw_rows(seed: &Column, symmetry: &Symmetry, field: Field) -> Vec<Row> {
    let mut rows = Vec::with_capacity(25);
    for differentiated in 0..5 {
        for contracted in 0..5 {
            let mut row = Row::new();
            if differentiated == contracted {
                add(
                    &mut row,
                    symmetry,
                    field,
                    seed.powers,
                    &seed.edges,
                    field.dimension,
                );
            }

            // Derivative of every numerator edge.  Removing one quadratic
            // factor and differentiating it replaces it by another quadratic
            // scalar product, so numerator degree is preserved.
            for edge_position in 0..seed.edges.len() {
                let edge = seed.edges[edge_position];
                let left = usize::from(edge[0]);
                let right = usize::from(edge[1]);
                let mut remaining = seed.edges.clone();
                remaining.remove(edge_position);
                for (coefficient, replacement) in [
                    (
                        line_derivative(left, differentiated),
                        normalized_edge(right, contracted),
                    ),
                    (
                        line_derivative(right, differentiated),
                        normalized_edge(left, contracted),
                    ),
                ] {
                    if coefficient == 0 {
                        continue;
                    }
                    let mut edges = remaining.clone();
                    edges.push(replacement);
                    edges.sort_unstable();
                    add(&mut row, symmetry, field, seed.powers, &edges, coefficient);
                }
            }

            // Only D_i and D_5 depend on k_i in the oriented-line basis.
            let mut dotted = seed.powers;
            dotted[differentiated] += 1;
            let mut edges = seed.edges.clone();
            edges.push(normalized_edge(differentiated, contracted));
            edges.sort_unstable();
            add(
                &mut row,
                symmetry,
                field,
                dotted,
                &edges,
                -2 * i64::from(seed.powers[differentiated]),
            );

            let mut dotted_sixth = seed.powers;
            dotted_sixth[5] += 1;
            let mut edges = seed.edges.clone();
            edges.push(normalized_edge(5, contracted));
            edges.sort_unstable();
            add(
                &mut row,
                symmetry,
                field,
                dotted_sixth,
                &edges,
                2 * i64::from(seed.powers[5]),
            );
            if !row.is_empty() {
                rows.push(row);
            }
        }
    }
    rows
}

fn diagonal_relation(
    powers: [i32; 6],
    base: &[Edge],
    line: usize,
    symmetry: &Symmetry,
    field: Field,
) -> Row {
    let mut row = Row::new();
    let mut with_loop = base.to_vec();
    with_loop.push(normalized_edge(line, line));
    with_loop.sort_unstable();
    add(&mut row, symmetry, field, powers, &with_loop, 1);
    let mut lowered = powers;
    lowered[line] -= 1;
    add(&mut row, symmetry, field, lowered, base, -1);
    add(&mut row, symmetry, field, powers, base, field.mass);
    row
}

fn momentum_relation(
    powers: [i32; 6],
    base: &[Edge],
    line: usize,
    symmetry: &Symmetry,
    field: Field,
) -> Row {
    let mut row = Row::new();
    for other in 0..6 {
        let mut edges = base.to_vec();
        edges.push(normalized_edge(line, other));
        edges.sort_unstable();
        add(&mut row, symmetry, field, powers, &edges, 1);
    }
    row
}

fn all_edges(include_loops: bool) -> Vec<Edge> {
    let mut edges = Vec::new();
    for left in 0..6 {
        for right in if include_loops { left } else { left + 1 }..6 {
            edges.push(normalized_edge(left, right));
        }
    }
    edges
}

fn graph_multisets(degree: usize, available: &[Edge]) -> Vec<Vec<Edge>> {
    fn recurse(
        remaining: usize,
        minimum: usize,
        available: &[Edge],
        current: &mut Vec<Edge>,
        output: &mut Vec<Vec<Edge>>,
    ) {
        if remaining == 0 {
            output.push(current.clone());
            return;
        }
        for position in minimum..available.len() {
            current.push(available[position]);
            recurse(remaining - 1, position, available, current, output);
            current.pop();
        }
    }

    let mut output = Vec::new();
    recurse(degree, 0, available, &mut Vec::new(), &mut output);
    output
}

fn labelled_orbit_size(column: &Column, symmetry: &Symmetry) -> usize {
    symmetry
        .permutations
        .iter()
        .map(|(source, inverse)| {
            let mapped_powers = std::array::from_fn(|target| column.powers[source[target]]);
            let mut mapped_edges = column
                .edges
                .iter()
                .map(|edge| {
                    normalized_edge(inverse[usize::from(edge[0])], inverse[usize::from(edge[1])])
                })
                .collect::<Vec<_>>();
            mapped_edges.sort_unstable();
            Column {
                powers: mapped_powers,
                edges: mapped_edges,
            }
        })
        .collect::<BTreeSet<_>>()
        .len()
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

fn scalar_seed_powers(max_dots: u32) -> Vec<[i32; 6]> {
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

fn scalar_seeds(max_dots: u32, symmetry: &Symmetry) -> BTreeSet<Column> {
    scalar_seed_powers(max_dots)
        .into_iter()
        .map(|powers| symmetry.canonical(powers, &[]))
        .collect()
}

fn numerator_seeds(
    max_dots: u32,
    max_numerator_degree: usize,
    symmetry: &Symmetry,
) -> BTreeSet<Column> {
    let numerator_edges = all_edges(false);
    scalar_seed_powers(max_dots)
        .into_iter()
        .flat_map(|powers| {
            (1..=max_numerator_degree).flat_map({
                let numerator_edges = numerator_edges.clone();
                move |degree| {
                    graph_multisets(degree, &numerator_edges)
                        .into_iter()
                        .map(move |edges| symmetry.canonical(powers, &edges))
                }
            })
        })
        .collect()
}

fn algebraic_closure(input_rows: &[Row], symmetry: &Symmetry, field: Field) -> BTreeSet<Row> {
    let mut power_degree = BTreeMap::<[i32; 6], usize>::new();
    for column in input_rows.iter().flat_map(|row| row.keys()) {
        if !column.edges.is_empty() {
            power_degree
                .entry(column.powers)
                .and_modify(|degree| *degree = (*degree).max(column.edges.len()))
                .or_insert(column.edges.len());
        }
    }

    let graph_edges = all_edges(true);
    let mut rows = BTreeSet::new();
    let mut completed = BTreeMap::<[i32; 6], usize>::new();
    loop {
        let pending = power_degree
            .iter()
            .filter_map(|(powers, degree)| {
                (completed.get(powers).copied().unwrap_or(0) < *degree)
                    .then_some((*powers, *degree))
            })
            .collect::<Vec<_>>();
        if pending.is_empty() {
            break;
        }
        for (powers, maximum_degree) in pending {
            let first_degree = completed.get(&powers).copied().unwrap_or(0) + 1;
            for degree in first_degree..=maximum_degree {
                for base in graph_multisets(degree - 1, &graph_edges) {
                    for line in 0..6 {
                        for row in [
                            diagonal_relation(powers, &base, line, symmetry, field),
                            momentum_relation(powers, &base, line, symmetry, field),
                        ] {
                            if row.is_empty() {
                                continue;
                            }
                            for column in row.keys() {
                                if !column.edges.is_empty() {
                                    power_degree
                                        .entry(column.powers)
                                        .and_modify(|known| {
                                            *known = (*known).max(column.edges.len())
                                        })
                                        .or_insert(column.edges.len());
                                }
                            }
                            rows.insert(row);
                        }
                    }
                }
            }
            completed.insert(powers, maximum_degree);
        }
    }
    rows
}

fn add_column(row: &mut Row, column: Column, coefficient: i64, field: Field) {
    let updated = field.modulo(row.get(&column).copied().unwrap_or(0) + coefficient);
    if updated == 0 {
        row.remove(&column);
    } else {
        row.insert(column, updated);
    }
}

fn reduce(mut row: Row, pivots: &BTreeMap<Column, Row>, field: Field) -> Row {
    while let Some((reducible, coefficient)) = row
        .iter()
        .rev()
        .find(|(column, _)| pivots.contains_key(*column))
        .map(|(column, coefficient)| (column.clone(), *coefficient))
    {
        let Some(pivot) = pivots.get(&reducible) else {
            break;
        };
        row.remove(&reducible);
        for (column, pivot_coefficient) in pivot {
            add_column(
                &mut row,
                column.clone(),
                -coefficient * pivot_coefficient,
                field,
            );
        }
    }
    row
}

#[derive(Clone, Debug)]
struct Elimination {
    pivots: BTreeMap<Column, Row>,
    selected_sorted_rows: Vec<usize>,
    pivot_columns: Vec<Column>,
    lower_bound_minor_determinant: i64,
}

fn dense_determinant(mut matrix: Vec<Vec<i64>>, field: Field) -> i64 {
    let mut determinant = 1;
    for column in 0..matrix.len() {
        let Some(pivot) = (column..matrix.len()).find(|row| matrix[*row][column] != 0) else {
            return 0;
        };
        if pivot != column {
            matrix.swap(pivot, column);
            determinant = field.modulo(-determinant);
        }
        let coefficient = matrix[column][column];
        determinant = determinant * coefficient % field.prime;
        let inverse = field.inverse(coefficient);
        let normalized = matrix[column][column..].to_vec();
        for row in &mut matrix[column + 1..] {
            let multiplier = row[column] * inverse % field.prime;
            for (entry, pivot_entry) in row[column..].iter_mut().zip(&normalized) {
                *entry = field.modulo(*entry - multiplier * pivot_entry);
            }
        }
    }
    determinant
}

fn eliminate(mut rows: Vec<Row>, field: Field) -> Elimination {
    rows.sort_by(|left, right| {
        right
            .last_key_value()
            .map(|entry| entry.0)
            .cmp(&left.last_key_value().map(|entry| entry.0))
            .then_with(|| left.len().cmp(&right.len()))
    });
    let mut pivots = BTreeMap::new();
    let mut selected_sorted_rows = Vec::new();
    let mut pivot_columns = Vec::new();
    for (sorted_row, row) in rows.iter().cloned().enumerate() {
        let mut row = reduce(row, &pivots, field);
        let Some((leading, coefficient)) = row.last_key_value().map(|(c, a)| (c.clone(), *a))
        else {
            continue;
        };
        selected_sorted_rows.push(sorted_row);
        pivot_columns.push(leading.clone());
        row.remove(&leading);
        let normalization = field.inverse(coefficient);
        for coefficient in row.values_mut() {
            *coefficient = *coefficient * normalization % field.prime;
        }
        pivots.insert(leading, row);
    }
    let lower_bound_minor_determinant = if pivot_columns.len() <= 256 {
        let witness_matrix = selected_sorted_rows
            .iter()
            .map(|row| {
                pivot_columns
                    .iter()
                    .map(|column| rows[*row].get(column).copied().unwrap_or(0))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let determinant = dense_determinant(witness_matrix, field);
        assert_ne!(
            determinant, 0,
            "selected modular rank minor must be nonzero"
        );
        determinant
    } else {
        0
    };
    Elimination {
        pivots,
        selected_sorted_rows,
        pivot_columns,
        lower_bound_minor_determinant,
    }
}

fn scalar_target(parts: &[i32], symmetry: &Symmetry) -> Column {
    let mut powers = [1; 6];
    for (position, dots) in parts.iter().enumerate() {
        powers[position] += dots;
    }
    symmetry.canonical(powers, &[])
}

fn status(target: &Column, pivots: &BTreeMap<Column, Row>, field: Field) -> (bool, Row) {
    let mut unit = Row::new();
    unit.insert(target.clone(), 1);
    (pivots.contains_key(target), reduce(unit, pivots, field))
}

fn column_census<'a>(
    columns: impl Iterator<Item = &'a Column>,
) -> BTreeMap<(usize, u64, u64), usize> {
    let mut census = BTreeMap::new();
    for column in columns {
        *census
            .entry((
                column.active_lines(),
                column.dot_degree(),
                column.numerator_degree(),
            ))
            .or_insert(0) += 1;
    }
    census
}

fn row_column_census(rows: &[Row]) -> BTreeMap<(usize, u64, u64), usize> {
    let columns = rows
        .iter()
        .flat_map(|row| row.keys().cloned())
        .collect::<BTreeSet<_>>();
    column_census(columns.iter())
}

fn symmetric_residue(value: i64, field: Field) -> i64 {
    let value = field.modulo(value);
    if value > field.prime / 2 {
        value - field.prime
    } else {
        value
    }
}

fn small_rational(value: i64, field: Field) -> Option<(i64, i64)> {
    fn gcd(mut left: i64, mut right: i64) -> i64 {
        while right != 0 {
            let remainder = left.rem_euclid(right);
            left = right;
            right = remainder;
        }
        left.abs()
    }

    let mut best = None;
    for denominator in 1..=512_i64 {
        let numerator = symmetric_residue(field.modulo(value * denominator), field);
        if numerator.abs() > 20_000 || gcd(numerator, denominator) != 1 {
            continue;
        }
        let score = numerator.abs() + denominator;
        if best
            .as_ref()
            .is_none_or(|(best_score, _, _)| score < *best_score)
        {
            best = Some((score, numerator, denominator));
        }
    }
    best.map(|(_, numerator, denominator)| (numerator, denominator))
}

fn polynomial(coefficients: &[i64], value: i64, field: Field) -> i64 {
    coefficients.iter().rev().fold(0, |result, coefficient| {
        field.modulo(result * field.modulo(value) + coefficient)
    })
}

fn fraction(numerator: i64, denominator: i64, field: Field) -> i64 {
    field.modulo(numerator) * field.inverse(denominator) % field.prime
}

fn d3_candidate(name: &str, field: Field, master: &Column, b2: &Column) -> Row {
    let d = field.dimension;
    let inverse_mass = field.inverse(field.mass);
    let inverse_mass_cubed = inverse_mass * inverse_mass % field.prime * inverse_mass % field.prime;
    let (master_numerator, master_denominator, b2_numerator, b2_denominator) = match name {
        "A3" => (
            polynomial(&[3864, -3830, 1225, -125], d, field),
            864,
            polynomial(&[-250, 55], d, field),
            72,
        ),
        "B3" => (
            polynomial(&[840, -986, 385, -50], d, field),
            288,
            polynomial(&[-46, 19], d, field),
            24,
        ),
        "C3" => (
            polynomial(&[-840, 986, -385, 50], d, field),
            288,
            polynomial(&[47, -17], d, field),
            12,
        ),
        _ => unreachable!(),
    };
    let mut row = Row::new();
    add_column(
        &mut row,
        master.clone(),
        fraction(master_numerator, master_denominator, field) * inverse_mass_cubed,
        field,
    );
    add_column(
        &mut row,
        b2.clone(),
        fraction(b2_numerator, b2_denominator, field) * inverse_mass,
        field,
    );
    row
}

fn solve_dense(mut matrix: Vec<Vec<i64>>, unknowns: usize, field: Field) -> Option<Vec<i64>> {
    let mut pivot_row = 0;
    for column in 0..unknowns {
        let pivot = (pivot_row..matrix.len()).find(|row| matrix[*row][column] != 0)?;
        matrix.swap(pivot_row, pivot);
        let inverse = field.inverse(matrix[pivot_row][column]);
        for entry in &mut matrix[pivot_row][column..=unknowns] {
            *entry = *entry * inverse % field.prime;
        }
        let normalized = matrix[pivot_row][column..=unknowns].to_vec();
        for (row_index, row) in matrix.iter_mut().enumerate() {
            if row_index == pivot_row || row[column] == 0 {
                continue;
            }
            let coefficient = row[column];
            for (entry, pivot_entry) in row[column..=unknowns].iter_mut().zip(&normalized) {
                *entry = field.modulo(*entry - coefficient * pivot_entry);
            }
        }
        pivot_row += 1;
    }
    if matrix
        .iter()
        .any(|row| row[..unknowns].iter().all(|entry| *entry == 0) && row[unknowns] != 0)
    {
        return None;
    }
    let mut solution = vec![0; unknowns];
    for row in matrix {
        if let Some(column) = row[..unknowns].iter().position(|entry| *entry == 1) {
            solution[column] = row[unknowns];
        }
    }
    Some(solution)
}

fn interpolate_mode(arguments: &[String]) {
    let prime = arguments
        .get(1)
        .map_or(1_000_003, |value| value.parse().unwrap());
    let maximum_total_degree = arguments.get(2).map_or(12, |value| value.parse().unwrap());
    let field = Field {
        prime,
        dimension: 0,
        mass: 1,
    };
    assert!(is_prime(prime), "modulus must be prime");
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let samples = input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let values = line
                .split_whitespace()
                .map(|value| value.parse::<i64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 2, "expected `x y` sample lines");
            (field.modulo(values[0]), field.modulo(values[1]))
        })
        .collect::<Vec<_>>();

    for total_degree in 0..=maximum_total_degree {
        for numerator_degree in 0..=total_degree {
            let denominator_degree = total_degree - numerator_degree;
            let unknowns = numerator_degree + 1 + denominator_degree;
            if samples.len() < unknowns + 2 {
                continue;
            }
            let matrix = samples
                .iter()
                .map(|(x, y)| {
                    let mut row = Vec::with_capacity(unknowns + 1);
                    let mut power = 1;
                    for _ in 0..=numerator_degree {
                        row.push(power);
                        power = power * x % prime;
                    }
                    power = *x;
                    for _ in 1..=denominator_degree {
                        row.push(field.modulo(-y * power));
                        power = power * x % prime;
                    }
                    row.push(*y);
                    row
                })
                .collect::<Vec<_>>();
            let Some(solution) = solve_dense(matrix, unknowns, field) else {
                continue;
            };
            let (numerator, denominator_tail) = solution.split_at(numerator_degree + 1);
            let mut denominator = vec![1];
            denominator.extend_from_slice(denominator_tail);
            let valid = samples.iter().all(|(x, y)| {
                let evaluate = |coefficients: &[i64]| {
                    coefficients.iter().rev().fold(0, |value, coefficient| {
                        field.modulo(value * x + coefficient)
                    })
                };
                evaluate(numerator) == *y * evaluate(&denominator) % prime
                    && evaluate(&denominator) != 0
            });
            if !valid {
                continue;
            }
            println!(
                "rational interpolation modulo {prime}: numerator_degree={numerator_degree} denominator_degree={denominator_degree} samples={}",
                samples.len()
            );
            for (name, coefficients) in [("numerator", numerator), ("denominator", &denominator)] {
                println!("  {name} residues={coefficients:?}");
                println!(
                    "  {name} small_rationals={:?}",
                    coefficients
                        .iter()
                        .map(|value| small_rational(*value, field))
                        .collect::<Vec<_>>()
                );
            }
            return;
        }
    }
    panic!("no unique rational interpolant within the requested degree bound");
}

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|value| value == "interpolate")
    {
        interpolate_mode(&arguments);
        return;
    }
    let scalar_seed_dots = arguments.first().map_or(2, |value| value.parse().unwrap());
    let numerator_seed_dots = arguments.get(1).map_or(1, |value| value.parse().unwrap());
    let numerator_seed_degree = arguments.get(2).map_or(1, |value| value.parse().unwrap());
    let field = Field {
        prime: arguments
            .get(3)
            .map_or(1_000_003, |value| value.parse().unwrap()),
        dimension: arguments.get(4).map_or(17, |value| value.parse().unwrap()),
        mass: arguments.get(5).map_or(19, |value| value.parse().unwrap()),
    };
    let d2_subset = arguments.get(6).map_or("all", String::as_str);
    let numerator_seed_minimum_dots = arguments.get(7).map_or(0, |value| value.parse().unwrap());
    assert!(is_prime(field.prime), "modulus must be prime");
    assert!(field.mass.rem_euclid(field.prime) != 0);

    let symmetry = Symmetry::new();
    let mut scalar_seeds = scalar_seeds(scalar_seed_dots, &symmetry);
    if scalar_seed_dots >= 2 && d2_subset != "all" {
        scalar_seeds.retain(|seed| {
            if seed.dot_degree() != 2 {
                return true;
            }
            match d2_subset {
                "a2" => seed.powers == [3, 1, 1, 1, 1, 1],
                "b2" => seed.powers == [2, 2, 1, 1, 1, 1],
                "none" => false,
                value => panic!("unknown D=2 scalar subset {value}"),
            }
        });
    }
    let mut numerator_seeds =
        numerator_seeds(numerator_seed_dots, numerator_seed_degree, &symmetry);
    numerator_seeds.retain(|seed| seed.dot_degree() >= numerator_seed_minimum_dots);
    let mut raw = Vec::new();
    for seed in scalar_seeds.iter().chain(&numerator_seeds) {
        raw.extend(raw_rows(seed, &symmetry, field));
    }
    let algebra = algebraic_closure(&raw, &symmetry, field);
    let mut rows = raw.clone();
    rows.extend(algebra.iter().cloned());
    let columns = rows
        .iter()
        .flat_map(|row| row.keys().cloned())
        .collect::<BTreeSet<_>>();
    let elimination = eliminate(rows.clone(), field);
    let pivots = &elimination.pivots;

    println!(
        "scalar_D={scalar_seed_dots} d2_subset={d2_subset} numerator_D={numerator_seed_dots} numerator_min_D={numerator_seed_minimum_dots} numerator_N={numerator_seed_degree} p={} d={} m2={}: scalar_seeds={} numerator_seeds={} raw_origins={} raw_rows={} algebra_rows={} rows={} columns={} rank={} nullity={}",
        field.prime,
        field.dimension,
        field.mass,
        scalar_seeds.len(),
        numerator_seeds.len(),
        25 * (scalar_seeds.len() + numerator_seeds.len()),
        raw.len(),
        algebra.len(),
        rows.len(),
        columns.len(),
        pivots.len(),
        columns.len() - pivots.len(),
    );
    println!(
        "  modular rank witness: order={} determinant={} sorted_rows={:?}",
        elimination.pivot_columns.len(),
        symmetric_residue(elimination.lower_bound_minor_determinant, field),
        elimination.selected_sorted_rows,
    );
    println!(
        "  seed census (active,dots,numerators): scalar={:?} numerator={:?}",
        column_census(scalar_seeds.iter()),
        column_census(numerator_seeds.iter()),
    );
    println!(
        "  canonical seeds with labelled orbit sizes: scalar={:?} numerator={:?}",
        scalar_seeds
            .iter()
            .map(|seed| (seed, labelled_orbit_size(seed, &symmetry)))
            .collect::<Vec<_>>(),
        numerator_seeds
            .iter()
            .map(|seed| (seed, labelled_orbit_size(seed, &symmetry)))
            .collect::<Vec<_>>(),
    );
    println!(
        "  column census (active,dots,numerators): raw={:?}",
        row_column_census(&raw),
    );
    println!(
        "  column census (active,dots,numerators): closed={:?}",
        column_census(columns.iter()),
    );

    let master = scalar_target(&[], &symmetry);
    let b2 = scalar_target(&[1, 1], &symmetry);
    for (name, target) in [
        ("M", master.clone()),
        ("A2", scalar_target(&[2], &symmetry)),
        ("B2", scalar_target(&[1, 1], &symmetry)),
        ("A3", scalar_target(&[3], &symmetry)),
        ("B3", scalar_target(&[2, 1], &symmetry)),
        ("C3", scalar_target(&[1, 1, 1], &symmetry)),
    ] {
        let (pivot, normal_form) = status(&target, pivots, field);
        let free_top = normal_form
            .keys()
            .filter(|column| column.active_lines() == 6)
            .cloned()
            .collect::<Vec<_>>();
        let master_coefficient = normal_form
            .get(&master)
            .copied()
            .map(|value| symmetric_residue(value, field));
        let b2_coefficient = normal_form
            .get(&b2)
            .copied()
            .map(|value| symmetric_residue(value, field));
        let master_rational = normal_form
            .get(&master)
            .copied()
            .and_then(|value| small_rational(value, field));
        let b2_rational = normal_form
            .get(&b2)
            .copied()
            .and_then(|value| small_rational(value, field));
        println!(
            "  {name}: {} normal_form_terms={} coeff_M={master_coefficient:?}/{master_rational:?} coeff_B2={b2_coefficient:?}/{b2_rational:?} free_top={free_top:?}",
            if pivot { "pivot" } else { "free" },
            normal_form.len(),
        );
    }

    let candidate_matches = [
        ("A3", scalar_target(&[3], &symmetry)),
        ("B3", scalar_target(&[2, 1], &symmetry)),
        ("C3", scalar_target(&[1, 1, 1], &symmetry)),
    ]
    .into_iter()
    .all(|(name, target)| {
        let (pivot, normal_form) = status(&target, pivots, field);
        pivot && normal_form == d3_candidate(name, field, &master, &b2)
    });
    println!(
        "  reconstructed D3 formulas match this specialization: {candidate_matches} (discovery only)"
    );

    let free_top_scalars = columns
        .iter()
        .filter(|column| {
            column.active_lines() == 6 && column.edges.is_empty() && !pivots.contains_key(*column)
        })
        .cloned()
        .collect::<Vec<_>>();
    println!(
        "  free top scalars ({}): {free_top_scalars:?}",
        free_top_scalars.len()
    );
}
