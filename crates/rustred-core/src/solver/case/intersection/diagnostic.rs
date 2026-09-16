//! Bounded rendering of the exact error payload, never geometry evidence.

use std::fmt::{self, Write};

use crate::algebra::CoefficientPolynomial;
use crate::diagnostic::BoundedText;

use super::{Case, CaseIntersectionError};

const MAX_CONTEXT_BYTES: usize = 16_384;

fn equations(output: &mut BoundedText, values: &[CoefficientPolynomial]) -> fmt::Result {
    output.write_char('[')?;
    for (position, equation) in values.iter().enumerate() {
        if position != 0 {
            output.write_str("; ")?;
        }
        write!(output, "{equation}")?;
    }
    output.write_char(']')
}

fn case<const N: usize>(output: &mut BoundedText, parent: &Case<N>) -> fmt::Result {
    write!(output, "fixed={:?}", parent.fixed())?;
    if let Some(parent) = parent.affine() {
        write!(
            output,
            ", index_variables={:?}, equalities=",
            parent.index_variables()
        )?;
        equations(output, parent.equations())?;
    }
    Ok(())
}

pub(super) fn write_context<const N: usize>(
    error: &CaseIntersectionError<N>,
    output: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let mut context = BoundedText::new(MAX_CONTEXT_BYTES);
    let rendered = (|| {
        // Put the final unresolved branch first: this is the small exact
        // diagnostic most useful when the original conjunction was large.
        context.write_str("\nunresolved_parent: ")?;
        case(&mut context, &error.unresolved_parent)?;
        context.write_str("\nunresolved_conjunction: ")?;
        equations(&mut context, &error.unresolved_conjunction)?;
        context.write_str("\noriginal_parent: ")?;
        case(&mut context, &error.original_parent)?;
        context.write_str("\noriginal_conjunction: ")?;
        equations(&mut context, &error.original_conjunction)
    })();
    if rendered.is_err() && !context.truncated {
        return Err(fmt::Error);
    }
    output.write_str(&context.text)?;
    if context.truncated {
        output.write_str(
            "\n[case diagnostic truncated at 16384 bytes; the typed error retains the complete exact payload]",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::solver::{CaseIntersectionFailure, CaseIntersectionStats};

    fn example() -> CaseIntersectionError<2> {
        let context = CoefficientContext::new(["a", "b"]);
        let equations = [context.coefficient_fixture("a^2+b^2+1").numerator];
        CaseIntersectionError {
            original_parent: Case::generic(),
            original_conjunction: equations.to_vec().into(),
            unresolved_parent: Case::generic(),
            unresolved_conjunction: equations.to_vec().into(),
            failure: CaseIntersectionFailure::UnsupportedGeometry,
            stats: CaseIntersectionStats::default(),
        }
    }

    #[test]
    fn unsupported_error_preserves_exact_context_without_changing_the_payload() {
        let error = example();
        let before = error.clone();
        let text = error.to_string();
        assert!(text.starts_with(
            "incomplete exact case intersection: a branch retains unsupported nonlinear equalities"
        ));
        assert!(text.contains("unresolved_parent: fixed=[None, None]"));
        assert!(text.contains("unresolved_conjunction: ["));
        assert!(text.contains(&error.unresolved_conjunction[0].to_string()));
        assert!(text.contains("original_conjunction: ["));
        assert!(!text.contains("truncated"));
        assert_eq!(error, before);
    }

    #[test]
    fn oversized_context_is_explicitly_truncated_but_not_the_typed_error() {
        let mut error = example();
        error.unresolved_conjunction = vec![error.unresolved_conjunction[0].clone(); 4096].into();
        let before = error.clone();
        let text = error.to_string();
        assert!(text.contains("case diagnostic truncated at 16384 bytes"));
        assert!(text.len() <= MAX_CONTEXT_BYTES + 256);
        assert_eq!(error, before);
    }

    #[test]
    fn context_limit_is_utf8_safe_and_stops_formatting() {
        let mut output = BoundedText::new(3);
        assert!(output.write_str("éé").is_err());
        assert_eq!(output.text, "é");
        assert!(output.truncated);
        assert!(output.write_str("extra").is_err());
        assert_eq!(output.text, "é");
    }
}
