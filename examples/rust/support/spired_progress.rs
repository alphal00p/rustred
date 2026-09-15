//! Opt-in, line-oriented case/phase diagnostics. No solver policy lives here.

use std::io::{Result, Write};
use std::time::Duration;

use rustred::solver::{SearchEvent, SectorEvent, SectorPhase};

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
}
