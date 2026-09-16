use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

fn target(context: &CoefficientContext) -> AffineApplicationDomain {
    let equation = context.coefficient_fixture("2*a-b").numerator;
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::<2>::generic(),
        &[equation],
        &[0, 1],
        &[false; 2],
    )
    .unwrap() else {
        panic!("expected a coupled rational chart")
    };
    AffineApplicationDomain::from_case(&case, &[false; 2]).unwrap()
}

fn output(render: impl FnOnce(&mut BoundedText) -> fmt::Result) -> String {
    let mut bytes = Vec::new();
    emit_to(true, &mut bytes, render);
    String::from_utf8(bytes).unwrap()
}

#[test]
fn disabled_diagnostics_do_not_render_or_write() {
    struct ForbiddenWriter;
    impl Write for ForbiddenWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            panic!("disabled diagnostics wrote output")
        }
        fn flush(&mut self) -> std::io::Result<()> {
            panic!("disabled diagnostics flushed output")
        }
    }
    emit_to(false, &mut ForbiddenWriter, |_| {
        panic!("disabled diagnostics rendered")
    });
}

#[test]
fn outer_guard_reports_exact_context_without_changing_the_error() {
    let context = CoefficientContext::new(["a", "b"]);
    let polynomial = context.coefficient_fixture("a^3+2*b+1").numerator;
    let polynomial_before = polynomial.clone();
    let target = target(&context);
    let origins = [
        ParametricGuardOrigin::FinalTargetCoefficient,
        ParametricGuardOrigin::OriginalDomainCondition {
            condition_ordinal: 41,
        },
    ];
    let exclusions = [Arc::new(target.clone())];
    let piece = LatticeBox::try_new([0, 2], [None, Some(5)]).unwrap();
    let error = SourcePortAuditError::message("original exact failure");
    let before = error.to_string();
    let text = output(|out| {
        GuardFailure {
            ordinal: 7,
            polynomial: &polynomial,
            origins: &origins,
            sector: &[false; 2],
            piece: &piece,
            target: Some(&target),
            exclusions: &exclusions,
            error: &error,
        }
        .render(out)
    });
    for expected in [
        "outer_guard ordinal=7 origins=2 exclusions=1 sector=[false, false]",
        "original_error=original exact failure",
        "local_piece lower=[0, 2] upper=[None, Some(5)]",
        "target: sector=[false, false] fixed=[None, None] index_variables=[0, 1] integral_chart=Some(false)",
        "target.equation[0]=",
        "exclusion_ordinal=0",
        "exclusion.equation[0]=",
        "guard_origin[0]=FinalTargetCoefficient",
        "condition_ordinal: 41",
        "[end affine-guard diagnostic]",
    ] {
        assert!(text.contains(expected), "missing {expected:?}: {text}");
    }
    assert!(text.contains(&format!("guard_polynomial={polynomial}")));
    assert!(!text.contains("truncated"));
    assert!(!text.contains("inner_restriction"));
    assert_eq!(polynomial, polynomial_before);
    assert_eq!(error.to_string(), before);
}

#[test]
fn inner_restriction_reports_actual_polynomial_and_term_or_bit_estimates() {
    let context = CoefficientContext::new(["a", "b"]);
    let target = target(&context);
    let polynomial = context.coefficient_fixture("b^3+2*b+1").numerator;
    let estimate = RestrictionEstimate {
        variables: 10,
        index_degree: 3,
        input_terms: 3,
        expansion: 1331,
        prospective_terms: 3993,
        chart_bits: Some(80),
        input_coefficient_bits: Some(2),
        prospective_coefficient_bits: Some(309),
        prospective_total_bits: Some(1233837),
        max_input_terms: 65536,
        max_replay_terms: 16000000,
        max_total_bits: 65536,
        max_coefficient_bits: 1048576,
    };
    for (reason, estimate) in [
        ("prospective bit failure", estimate),
        (
            "prospective term failure",
            RestrictionEstimate {
                chart_bits: None,
                input_coefficient_bits: None,
                prospective_coefficient_bits: None,
                prospective_total_bits: None,
                ..estimate
            },
        ),
    ] {
        let text = output(|out| {
            RestrictionFailure {
                reason,
                polynomial: &polynomial,
                target: &target,
                estimate,
            }
            .render(out)
        });
        assert!(text.contains(&format!("inner_restriction reason={reason}")));
        assert!(text.contains(&format!("restriction_polynomial={polynomial}")));
        assert!(text.contains("variables=10 index_degree=3 input_terms=3 expansion=1331"));
        assert!(text.contains("prospective_terms=3993"));
        assert!(text.contains(&format!("chart_bits={:?}", estimate.chart_bits)));
        assert!(text.contains("max_total_bits=65536 max_coefficient_bits=1048576"));
        assert!(!text.contains("outer_guard"));
        assert!(!text.contains("truncated"));
    }
}

#[test]
fn entire_diagnostic_block_is_bounded_and_utf8_safe() {
    let text = output(|out| {
        for _ in 0..MAX_BLOCK_BYTES {
            out.write_str("é🙂")?;
        }
        Ok(())
    });
    assert!(text.len() <= MAX_BLOCK_BYTES);
    assert!(text.contains("truncated at 16384-byte block limit"));
    assert!(text.contains("context is incomplete"));
    assert!(!text.contains("[end affine-guard diagnostic]"));
    assert!(!text.contains("formatting failed"));
}

#[test]
fn native_polynomial_formatting_and_context_share_one_block_cap() {
    let context = CoefficientContext::new(["a", "b"]);
    let polynomial = context.coefficient_fixture("a^3+2*b+1").numerator;
    let text = output(|out| {
        for ordinal in 0..4096 {
            writeln!(out, "equation[{ordinal}]={polynomial}")?;
        }
        Ok(())
    });
    assert!(text.len() <= MAX_BLOCK_BYTES);
    assert!(text.contains("equation[0]="));
    assert!(!text.contains("equation[4095]="));
    assert!(text.contains("truncated"));
}

#[test]
fn formatter_failure_is_not_reported_as_intentional_truncation() {
    let text = output(|out| {
        out.write_str("partial")?;
        Err(fmt::Error)
    });
    assert!(text.contains("partial"));
    assert!(text.contains("formatting failed; context is incomplete"));
    assert!(!text.contains("truncated"));
    assert!(!text.contains("[end affine-guard diagnostic]"));
}

#[test]
fn broken_output_stream_does_not_escape_or_change_original_failure() {
    struct BrokenWriter;
    impl Write for BrokenWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }
    let error = SourcePortAuditError::message("unchanged mathematical failure");
    let before = error.to_string();
    emit_to(true, &mut BrokenWriter, |out| write!(out, "{error}"));
    assert_eq!(error.to_string(), before);
}

#[test]
fn formatter_panic_is_diagnostic_only() {
    let text = output(|_| panic!("injected diagnostic formatter failure"));
    assert!(text.contains("formatter panicked; context is incomplete"));
    assert!(!text.contains("[end affine-guard diagnostic]"));
}
