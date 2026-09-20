//! Explicitly ignored, external frozen-frame benchmark. No search or source generation.

use std::cell::Cell;
use std::fs;
use std::path::Path;
use std::time::Instant;

use symbolica::domains::factorized_rational_polynomial::FactorizedRationalPolynomialField;

use crate::persistence::{CoefficientId, DecodedCoefficientTable};
use crate::solver::{CoefficientVariableOrder, Power};

use super::*;

fn columns<const N: usize>(path: &Path) -> Vec<Integral<N>> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| {
            let parts: Vec<_> = line
                .strip_prefix("I(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split(',')
                .collect();
            assert_eq!(parts.len(), N);
            let powers = std::array::from_fn(|axis| {
                let name = format!("n{axis}");
                let (symbolic, number) = if let Some(suffix) = parts[axis].strip_prefix(&name) {
                    (true, if suffix.is_empty() { "0" } else { suffix })
                } else {
                    (false, parts[axis])
                };
                Power::new(symbolic, number.parse().unwrap()).unwrap()
            });
            let result = Integral::new(powers);
            assert_eq!(format!("{result}"), line);
            result
        })
        .collect()
}

fn rows<const N: usize>(path: &Path, columns: &[Integral<N>]) -> Vec<ExactRow<N>> {
    let table = DecodedCoefficientTable::import_generated(
        &fs::read(path.join("state.bin")).unwrap(),
        &fs::read(path.join("coefficients.bin")).unwrap(),
        Default::default(),
    )
    .unwrap();
    fs::read_to_string(path.join("rows.txt"))
        .unwrap()
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|term| {
                    let (column, coefficient) = term.split_once(':').unwrap();
                    Term {
                        integral: columns[column.parse::<usize>().unwrap()],
                        coefficient: table
                            .coefficient(
                                CoefficientId::try_from_index(coefficient.parse().unwrap())
                                    .unwrap(),
                            )
                            .unwrap()
                            .clone(),
                    }
                })
                .collect()
        })
        .collect()
}

#[derive(Default)]
struct Timings {
    block_start: Option<Instant>,
    row_start: Option<Instant>,
    weights_start: Option<Instant>,
    reconstruction_start: Option<Instant>,
    block_ns: u128,
    native_rows_ns: u128,
    weights_ns: u128,
    reconstruction_ns: u128,
    reconstruction_import_start: u128,
    reconstruction_import_ns: u128,
    retained_u_nonzeros: usize,
    retained_l_nonzeros: usize,
    nonzero_weights: usize,
    pivots: Vec<Option<u32>>,
}

impl Timings {
    fn observe<const N: usize>(
        &mut self,
        event: MaterializationEvent<N>,
        columns: &[Integral<N>],
        order: &IntegralOrder<N>,
        import_ns: u128,
    ) {
        match event {
            MaterializationEvent::TargetBlockStarted { .. } => {
                self.block_start = Some(Instant::now())
            }
            MaterializationEvent::RowStarted { .. } => self.row_start = Some(Instant::now()),
            MaterializationEvent::RowFinished {
                pivot,
                reducer_nonzeros,
                ..
            } => {
                self.native_rows_ns += self.row_start.take().unwrap().elapsed().as_nanos();
                self.retained_u_nonzeros = reducer_nonzeros;
                self.pivots.push(pivot.map(|key| {
                    columns
                        .binary_search_by(|column| order.compare(column, &key))
                        .unwrap() as u32
                }));
            }
            MaterializationEvent::TargetWeightsStarted { lower_nonzeros, .. } => {
                self.block_ns = self.block_start.take().unwrap().elapsed().as_nanos();
                self.retained_l_nonzeros = lower_nonzeros;
                self.weights_start = Some(Instant::now());
            }
            MaterializationEvent::TargetWeightsFinished { nonzero_weights } => {
                self.weights_ns = self.weights_start.take().unwrap().elapsed().as_nanos();
                self.nonzero_weights = nonzero_weights;
            }
            MaterializationEvent::TargetReconstructionStarted { .. } => {
                self.reconstruction_start = Some(Instant::now());
                self.reconstruction_import_start = import_ns;
            }
            MaterializationEvent::TargetReconstructionFinished { .. } => {
                self.reconstruction_ns = self
                    .reconstruction_start
                    .take()
                    .unwrap()
                    .elapsed()
                    .as_nanos();
                self.reconstruction_import_ns = import_ns - self.reconstruction_import_start;
            }
            _ => {}
        }
    }
}

/// Test-only dimensions describe this external fixture, never an algorithm dispatch.
#[test]
#[ignore = "requires externally frozen frame and an explicitly bounded release process"]
fn frozen_external_frame_three_native_schedules() {
    let directory = std::env::var("RUSTRED_TARGET_FRAME").expect("RUSTRED_TARGET_FRAME");
    let mode = std::env::var("RUSTRED_TARGET_MODE").expect("target-rp | target-frp | full-frp");
    let sector = std::env::var("RUSTRED_TARGET_SECTOR").expect("external sector bits");
    assert_eq!(sector.len(), 15);
    assert!(sector.bytes().all(|b| b == b'0' || b == b'1'));
    let path = Path::new(&directory);
    let columns = columns::<15>(&path.join("columns.txt"));
    let input = rows(path, &columns);
    let expected = rows(&path.join("expected"), &columns);
    assert_eq!(expected.len(), 1);
    let metadata: Vec<usize> = fs::read_to_string(path.join("metadata.txt"))
        .unwrap()
        .split_whitespace()
        .map(|x| x.parse().unwrap())
        .collect();
    assert_eq!(metadata.len(), 2);
    assert_eq!(metadata[0], columns.len());
    let target_column = metadata[1];
    let order = IntegralOrder::new(
        std::array::from_fn(|axis| sector.as_bytes()[axis] == b'1'),
        [false; 15],
    );
    assert!(
        columns
            .windows(2)
            .all(|w| order.compare(&w[0], &w[1]).is_lt())
    );
    assert!(input.iter().all(|row| {
        row.windows(2)
            .all(|w| order.compare(&w[0].integral, &w[1].integral).is_lt())
    }));
    assert_eq!(expected[0][0].integral, columns[target_column]);
    assert!(expected[0][0].coefficient.is_one());
    // This frozen external fixture was previously calibrated against all saved
    // RHS keys/coefficients and its complete ordered original-source trace.
    assert_eq!(
        (input.len(), columns.len(), target_column, expected[0].len()),
        (997, 3458, 1296, 1490)
    );
    let original = input[0][0].coefficient.numerator.variables().clone();
    for term in input.iter().flatten().chain(expected[0].iter()) {
        assert_eq!(term.coefficient.numerator.variables(), &original);
        assert_eq!(term.coefficient.denominator.variables(), &original);
        assert!(!term.coefficient.denominator.is_zero());
    }
    println!(
        "FRAME mode={mode} rows={} columns={} target={target_column} input_nonzeros={} variables={}",
        input.len(),
        columns.len(),
        input.iter().map(Vec::len).sum::<usize>(),
        original.len()
    );
    let import_ns = Cell::new(0u128);
    let export_ns = Cell::new(0u128);
    let mut times = Timings::default();
    let start = Instant::now();
    let variables =
        FrameVariables::try_new(&input, CoefficientVariableOrder::Original, &[]).unwrap();
    let setup_ns = start.elapsed().as_nanos();
    let active = variables.active_variables();
    let observe = |event| times.observe(event, &columns, &order, import_ns.get());
    let result = match mode.as_str() {
        "target-rp" => materialize_in_field(
            &input,
            &columns,
            &order,
            target_column,
            ExactField::new(Z),
            &|value| {
                let t = Instant::now();
                let value = variables.map_coefficient(value);
                import_ns.set(import_ns.get() + t.elapsed().as_nanos());
                value
            },
            &|value| {
                let t = Instant::now();
                let value = variables.restore_coefficient(value);
                export_ns.set(export_ns.get() + t.elapsed().as_nanos());
                value
            },
            false,
            observe,
        ),
        "target-frp" => materialize_in_field(
            &input,
            &columns,
            &order,
            target_column,
            FactorizedRationalPolynomialField::<_, u16>::new(Z, active.clone()),
            &|value| {
                let t = Instant::now();
                let value = variables
                    .map_coefficient(value)
                    .and_then(|value| factorized::factor(value, &active));
                import_ns.set(import_ns.get() + t.elapsed().as_nanos());
                value
            },
            &|value| {
                let t = Instant::now();
                let value = factorized::ordinary(value, &active)
                    .and_then(|value| variables.restore_coefficient(&value));
                export_ns.set(export_ns.get() + t.elapsed().as_nanos());
                value
            },
            true,
            observe,
        ),
        "full-frp" => {
            factorized::materialize(&input, &columns, &order, target_column, &variables, observe)
        }
        _ => panic!("unknown mode {mode}"),
    }
    .unwrap();
    let total_ns = start.elapsed().as_nanos();
    assert_eq!(result, expected[0]);
    for term in &result {
        assert_eq!(term.coefficient.numerator.variables(), &original);
        assert_eq!(term.coefficient.denominator.variables(), &original);
    }
    assert_eq!(
        format!("{:?}", times.pivots),
        fs::read_to_string(path.join("pivots.txt")).unwrap()
    );
    assert_eq!(times.pivots.len(), input.len());
    assert!(times.pivots.iter().all(Option::is_some));
    println!(
        "EXACT_PASS mode={mode} output_terms={} rows_consumed={} both_maps=true target=true ordered_pivots=true total_ns={total_ns} setup_ns={setup_ns} native_rows_ns={} retained_u_nonzeros={}",
        result.len(),
        times.pivots.len(),
        times.native_rows_ns,
        times.retained_u_nonzeros
    );
    if mode != "full-frp" {
        // This remainder includes native multiplication and structural assembly,
        // not only kernel time. Conversion timers themselves add small overhead.
        let product_and_structure_ns = times
            .reconstruction_ns
            .checked_sub(times.reconstruction_import_ns + export_ns.get())
            .unwrap();
        println!(
            "TARGET_PHASES import_ns={} output_ns={} block_inclusive_ns={} weights_ns={} reconstruction_inclusive_ns={} product_and_structure_ns={product_and_structure_ns} retained_l_nonzeros={} nonzero_weights={}",
            import_ns.get(),
            export_ns.get(),
            times.block_ns,
            times.weights_ns,
            times.reconstruction_ns,
            times.retained_l_nonzeros,
            times.nonzero_weights
        );
    } else {
        println!("FULL_PHASES conversion_and_output_included_in_total=true separately_timed=false");
    }
}
