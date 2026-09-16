//! Failure-only observations; they never replace an error or certify a guard.

use std::fmt::{self, Write as _};
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::algebra::CoefficientPolynomial;
use crate::diagnostic::BoundedText;
use crate::foundry::completion::LatticeBox;
use crate::foundry::parametric::{AffineApplicationDomain, ParametricGuardOrigin};

use super::SourcePortAuditError;

const MAX_BLOCK_BYTES: usize = 16_384;
const STATUS_RESERVE: usize = 256;

pub(super) struct GuardFailure<'a> {
    pub ordinal: usize,
    pub polynomial: &'a CoefficientPolynomial,
    pub origins: &'a [ParametricGuardOrigin],
    pub sector: &'a [bool],
    pub piece: &'a LatticeBox,
    pub target: Option<&'a AffineApplicationDomain>,
    pub exclusions: &'a [Arc<AffineApplicationDomain>],
    pub error: &'a SourcePortAuditError,
}

/// Scalars from the actual failed prospective check, not recomputed estimates.
#[derive(Clone, Copy)]
pub(super) struct RestrictionEstimate {
    pub variables: usize,
    pub index_degree: usize,
    pub input_terms: usize,
    pub expansion: usize,
    pub prospective_terms: usize,
    pub chart_bits: Option<usize>,
    pub input_coefficient_bits: Option<usize>,
    pub prospective_coefficient_bits: Option<usize>,
    pub prospective_total_bits: Option<usize>,
    pub max_input_terms: usize,
    pub max_replay_terms: usize,
    pub max_total_bits: usize,
    pub max_coefficient_bits: usize,
}

pub(super) struct RestrictionFailure<'a> {
    pub reason: &'static str,
    pub polynomial: &'a CoefficientPolynomial,
    pub target: &'a AffineApplicationDomain,
    pub estimate: RestrictionEstimate,
}

pub(super) fn guard_failure(failure: GuardFailure<'_>) {
    emit_stderr(|output| failure.render(output));
}

pub(super) fn restriction_failure(failure: RestrictionFailure<'_>) {
    emit_stderr(|output| failure.render(output));
}

impl GuardFailure<'_> {
    fn render(&self, output: &mut BoundedText) -> fmt::Result {
        writeln!(
            output,
            "outer_guard ordinal={} origins={} exclusions={} sector={:?}",
            self.ordinal,
            self.origins.len(),
            self.exclusions.len(),
            self.sector
        )?;
        writeln!(output, "guard_polynomial={}", self.polynomial)?;
        writeln!(output, "original_error={}", self.error)?;
        writeln!(
            output,
            "local_piece lower={:?} upper={:?}; x=n-1 if active, x=-n otherwise; None is infinity",
            self.piece.lower(),
            self.piece.upper()
        )?;
        if let Some(target) = self.target {
            domain(output, "target", target)?;
        } else {
            output.write_str("target=coordinate_only\n")?;
        }
        for (ordinal, exclusion) in self.exclusions.iter().enumerate() {
            writeln!(output, "exclusion_ordinal={ordinal}")?;
            domain(output, "exclusion", exclusion)?;
        }
        for (ordinal, origin) in self.origins.iter().enumerate() {
            writeln!(output, "guard_origin[{ordinal}]={origin:?}")?;
        }
        Ok(())
    }
}

impl RestrictionFailure<'_> {
    fn render(&self, output: &mut BoundedText) -> fmt::Result {
        writeln!(output, "inner_restriction reason={}", self.reason)?;
        writeln!(output, "restriction_polynomial={}", self.polynomial)?;
        self.estimate.render(output)?;
        domain(output, "target", self.target)
    }
}

impl RestrictionEstimate {
    fn render(&self, output: &mut BoundedText) -> fmt::Result {
        writeln!(
            output,
            "prospective_estimate: variables={} index_degree={} input_terms={} expansion={} prospective_terms={} chart_bits={:?} input_coefficient_bits={:?} prospective_coefficient_bits={:?} prospective_total_bits={:?} max_input_terms={} max_replay_terms={} max_total_bits={} max_coefficient_bits={}",
            self.variables,
            self.index_degree,
            self.input_terms,
            self.expansion,
            self.prospective_terms,
            self.chart_bits,
            self.input_coefficient_bits,
            self.prospective_coefficient_bits,
            self.prospective_total_bits,
            self.max_input_terms,
            self.max_replay_terms,
            self.max_total_bits,
            self.max_coefficient_bits,
        )
    }
}

fn domain(output: &mut BoundedText, label: &str, domain: &AffineApplicationDomain) -> fmt::Result {
    writeln!(
        output,
        "{label}: sector={:?} fixed={:?} index_variables={:?} integral_chart={:?} equations={}",
        domain.sector(),
        domain.fixed(),
        domain.indices(),
        domain.has_integral_chart(),
        domain.equations().len()
    )?;
    for (ordinal, equation) in domain.equations().iter().enumerate() {
        writeln!(output, "{label}.equation[{ordinal}]={equation}")?;
    }
    Ok(())
}

fn emit_stderr(render: impl FnOnce(&mut BoundedText) -> fmt::Result) {
    // Read only on failure. Successful guards do not inspect the environment,
    // lock stderr, render native polynomials, or allocate diagnostic state.
    if std::env::var_os("RUSTRED_AFFINE_GUARD_DIAGNOSTICS").as_deref()
        != Some(std::ffi::OsStr::new("1"))
    {
        return;
    }
    emit_to(true, &mut std::io::stderr().lock(), render);
}

fn emit_to(
    enabled: bool,
    sink: &mut impl Write,
    render: impl FnOnce(&mut BoundedText) -> fmt::Result,
) {
    if !enabled {
        return;
    }
    // Formatting or a custom writer must not change the original proof error.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let mut output = BoundedText::new(MAX_BLOCK_BYTES - STATUS_RESERVE);
        let rendered = catch_unwind(AssertUnwindSafe(|| {
            output.write_str(
                "[rustred affine-guard diagnostic; observational, not a closure certificate]\n",
            )?;
            render(&mut output)
        }));
        let status = match rendered {
            Err(_) => "\n[affine-guard diagnostic formatter panicked; context is incomplete]\n",
            Ok(Err(_)) if !output.truncated => {
                "\n[affine-guard diagnostic formatting failed; context is incomplete]\n"
            }
            _ if output.truncated => {
                "\n[affine-guard diagnostic truncated at 16384-byte block limit; context is incomplete]\n"
            }
            Ok(Ok(())) => "[end affine-guard diagnostic]\n",
            Ok(Err(_)) => unreachable!("intentional cutoff handled above"),
        };
        debug_assert!(status.len() <= STATUS_RESERVE);
        output.text.push_str(status);
        let _ = sink.write_all(output.text.as_bytes());
    }));
}

#[cfg(test)]
mod tests;
