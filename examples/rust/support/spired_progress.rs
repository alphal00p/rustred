//! Opt-in, line-oriented case/phase diagnostics. No solver policy lives here.

use std::io::{Result, Write};
use std::time::Duration;

use rustred::solver::{MaterializationEvent, SearchEvent, SectorEvent, SectorPhase};

pub fn write_event<const N: usize>(
    output: &mut impl Write,
    sector: &str,
    elapsed: Duration,
    event: SectorEvent<'_, N>,
) -> Result<()> {
    write!(
        output,
        "sector={sector} elapsed_us={} ",
        elapsed.as_micros()
    )?;
    match event {
        SectorEvent::CaseStarted { case, pending } => {
            write!(
                output,
                "case={} pending={pending} required=",
                case.integral()
            )?;
            if let Some(affine) = case.affine() {
                for (ordinal, equation) in affine.equations().iter().enumerate() {
                    if ordinal != 0 {
                        write!(output, " AND ")?;
                    }
                    write!(output, "({equation})=0")?;
                }
                writeln!(output)
            } else {
                writeln!(output, "true")
            }
        }
        SectorEvent::Search { case, event } => {
            write!(output, "case={} ", case.integral())?;
            match event {
                SearchEvent::DiscoveryProgress {
                    depth,
                    seeds,
                    rows,
                    discovery,
                } => {
                    let sizes = discovery.unwrap_or_default();
                    writeln!(
                        output,
                        "phase=discovery depth={depth} seeds={seeds} rows={rows} pivots={} columns={} u_nnz={} l_entries={}",
                        sizes.independent_rows,
                        sizes.columns,
                        sizes.reducer_nonzeros,
                        sizes.retained_l_entries
                    )
                }
                SearchEvent::ExactStarted {
                    pivot,
                    trace_rows,
                    discovery,
                } => writeln!(
                    output,
                    "phase=exact-materialization pivot={pivot} trace_rows={trace_rows} pivots={} columns={} u_nnz={} l_entries={}",
                    discovery.independent_rows,
                    discovery.columns,
                    discovery.reducer_nonzeros,
                    discovery.retained_l_entries
                ),
                SearchEvent::CanonicalizationStarted { terms, direct_hit } => writeln!(
                    output,
                    "phase=canonicalization terms={terms} direct_hit={direct_hit}"
                ),
                SearchEvent::ExactProgress(event) => match event {
                    MaterializationEvent::FramePrepared {
                        source_rows,
                        integral_columns,
                        target_column,
                        input_terms,
                        coefficient_variables,
                        active_variables,
                    } => writeln!(
                        output,
                        "phase=exact-frame source_rows={source_rows} integral_columns={integral_columns} target_column={target_column} input_terms={input_terms} coefficient_variables={coefficient_variables} active_variables={active_variables}"
                    ),
                    MaterializationEvent::DenseFractionFreeStarted {
                        rows,
                        columns,
                        reduction_columns,
                        rational_coefficients,
                    } => writeln!(
                        output,
                        "phase=dense-fraction-free-start rows={rows} columns={columns} reduction_columns={reduction_columns} rational_coefficients={rational_coefficients}"
                    ),
                    MaterializationEvent::DenseFractionFreeFinished { rank } => {
                        writeln!(output, "phase=dense-fraction-free-finish rank={rank}")
                    }
                    MaterializationEvent::TargetBlockStarted { columns } => {
                        writeln!(output, "phase=target-block-start columns={columns}")
                    }
                    MaterializationEvent::TargetWeightsStarted {
                        rows,
                        lower_nonzeros,
                    } => writeln!(
                        output,
                        "phase=target-weights-start rows={rows} lower_nnz={lower_nonzeros}"
                    ),
                    MaterializationEvent::TargetWeightsFinished { nonzero_weights } => writeln!(
                        output,
                        "phase=target-weights-finish weights_nnz={nonzero_weights}"
                    ),
                    MaterializationEvent::TargetReconstructionStarted { rows, columns } => {
                        writeln!(
                            output,
                            "phase=target-reconstruction-start rows={rows} columns={columns}"
                        )
                    }
                    MaterializationEvent::TargetReconstructionFinished { output_terms } => {
                        writeln!(
                            output,
                            "phase=target-reconstruction-finish output_terms={output_terms}"
                        )
                    }
                    MaterializationEvent::RowStarted {
                        row,
                        input_nonzeros,
                        reducer_rows,
                        reducer_nonzeros,
                    } => writeln!(
                        output,
                        "phase=exact-row-start row={row} input_nnz={input_nonzeros} exact_u_rows={reducer_rows} exact_u_nnz={reducer_nonzeros}"
                    ),
                    MaterializationEvent::RowFinished {
                        row,
                        pivot,
                        reducer_rows,
                        reducer_nonzeros,
                    } => {
                        write!(output, "phase=exact-row-finish row={row} pivot=")?;
                        if let Some(pivot) = pivot {
                            write!(output, "{pivot}")?;
                        } else {
                            write!(output, "none")?;
                        }
                        writeln!(
                            output,
                            " exact_u_rows={reducer_rows} exact_u_nnz={reducer_nonzeros}"
                        )
                    }
                },
            }
        }
        SectorEvent::PhaseStarted { case, phase } => writeln!(
            output,
            "case={} phase={}",
            case.integral(),
            match phase {
                SectorPhase::GuardExtraction => "guard-extraction",
                SectorPhase::ExceptionalGeometry => "exceptional-geometry",
            }
        ),
        SectorEvent::RuleFound { rule, pending } => writeln!(
            output,
            "solved={} rhs_terms={} rows={} trace_rows={} search_us={} gplu_exact_and_canonicalization_us={} pending={pending}",
            rule.candidate.target,
            rule.candidate.rhs.len(),
            rule.candidate.stats.rows,
            rule.candidate.stats.exact_trace_rows,
            rule.candidate.stats.elapsed.as_micros(),
            rule.candidate.stats.exact_materialization.as_micros()
        ),
        SectorEvent::NumericalStarted { cases } => {
            writeln!(output, "numerical_cases={}", cases.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::{algebra::CoefficientContext, solver::Case};

    #[test]
    fn case_progress_retains_every_coupled_equality_in_one_atomic_line() {
        let context = CoefficientContext::try_new(["a", "b", "c"]).unwrap();
        let a = context.parameter("a").unwrap();
        let b = context.parameter("b").unwrap();
        let c = context.parameter("c").unwrap();
        let case = Case::<3>::generic()
            .intersect(
                &[(&(&a + &a) - &b).numerator, (&b - &c).numerator],
                &[0, 1, 2],
                &[true; 3],
            )
            .unwrap()
            .unwrap();
        let mut output = Vec::new();
        write_event(
            &mut output,
            "111",
            Duration::from_micros(7),
            SectorEvent::CaseStarted {
                case: case.clone(),
                pending: 85,
            },
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("elapsed_us=7"));
        assert!(text.contains("pending=85 required="));
        for equation in case.affine().unwrap().equations() {
            assert!(text.contains(&format!("({equation})=0")), "{text}");
        }
    }

    #[test]
    fn exact_milestone_reports_native_sizes_without_a_closure_claim() {
        let case = Case::<1>::generic();
        let mut output = Vec::new();
        write_event(
            &mut output,
            "1",
            Duration::ZERO,
            SectorEvent::Search {
                case: &case,
                event: SearchEvent::ExactStarted {
                    pivot: case.integral(),
                    trace_rows: 3,
                    discovery: rustred::solver::DiscoveryStats {
                        independent_rows: 11,
                        columns: 20,
                        reducer_nonzeros: 48,
                        retained_l_entries: 29,
                        ..Default::default()
                    },
                },
            },
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("phase=exact-materialization"));
        assert!(text.contains("trace_rows=3 pivots=11 columns=20 u_nnz=48 l_entries=29"));
        assert!(!text.contains("closed"));
    }

    #[test]
    fn compact_frame_and_exact_rows_have_unambiguous_size_labels() {
        let case = Case::<1>::generic();
        let mut output = Vec::new();
        for event in [
            MaterializationEvent::FramePrepared {
                source_rows: 3,
                integral_columns: 4,
                target_column: 1,
                input_terms: 7,
                coefficient_variables: 17,
                active_variables: 6,
            },
            MaterializationEvent::RowStarted {
                row: 2,
                input_nonzeros: 2,
                reducer_rows: 1,
                reducer_nonzeros: 2,
            },
            MaterializationEvent::RowFinished {
                row: 2,
                pivot: None,
                reducer_rows: 1,
                reducer_nonzeros: 2,
            },
        ] {
            write_event(
                &mut output,
                "1",
                Duration::ZERO,
                SectorEvent::Search {
                    case: &case,
                    event: SearchEvent::ExactProgress(event),
                },
            )
            .unwrap();
        }
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.lines().count(), 3);
        assert!(text.contains("coefficient_variables=17 active_variables=6"));
        assert!(text.contains(
            "phase=exact-frame source_rows=3 integral_columns=4 target_column=1 input_terms=7"
        ));
        assert!(
            text.contains("phase=exact-row-start row=2 input_nnz=2 exact_u_rows=1 exact_u_nnz=2")
        );
        assert!(
            text.contains("phase=exact-row-finish row=2 pivot=none exact_u_rows=1 exact_u_nnz=2")
        );
    }

    #[test]
    fn target_only_native_phases_are_separately_observable() {
        let case = Case::<1>::generic();
        let mut output = Vec::new();
        for event in [
            MaterializationEvent::TargetBlockStarted { columns: 405 },
            MaterializationEvent::TargetWeightsStarted {
                rows: 298,
                lower_nonzeros: 1200,
            },
            MaterializationEvent::TargetWeightsFinished {
                nonzero_weights: 240,
            },
            MaterializationEvent::TargetReconstructionStarted {
                rows: 298,
                columns: 482,
            },
            MaterializationEvent::TargetReconstructionFinished { output_terms: 20 },
        ] {
            write_event(
                &mut output,
                "1",
                Duration::ZERO,
                SectorEvent::Search {
                    case: &case,
                    event: SearchEvent::ExactProgress(event),
                },
            )
            .unwrap();
        }
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.lines().count(), 5);
        for expected in [
            "phase=target-block-start columns=405",
            "phase=target-weights-start rows=298 lower_nnz=1200",
            "phase=target-weights-finish weights_nnz=240",
            "phase=target-reconstruction-start rows=298 columns=482",
            "phase=target-reconstruction-finish output_terms=20",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        assert!(!text.contains("closed"));
    }
}
