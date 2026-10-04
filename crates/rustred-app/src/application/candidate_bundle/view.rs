//! Lazy, paged views of trusted generated candidates. Structural inspection is
//! not algebraic validation, original-source replay or closure certification.

use super::{
    CandidateBundleLimits, CandidateCoefficientInspection, CandidateRuleInspection, codec,
    inspection,
    model::{ProgramRecord, SectorRecord},
};
use crate::AppError;
use rustred::persistence::{CoefficientId, LazyDecodedCoefficientTable, SectionTag};
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;
use symbolica::prelude::AtomCore;
use symbolica::printer::PrintOptions;

/// Immutable structural records plus indexed encoded coefficient frames. Open
/// does not import Symbolica state or deserialize any coefficient polynomial.
/// First selected coefficient imports the state and caches only that native
/// coefficient. This owns encoded bytes; it is not a zero-copy/mmap reader.
pub struct CandidateArtifact {
    creator_pid: u32,
    records: ProgramRecord,
    coefficients: LazyDecodedCoefficientTable,
    encoded_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateArtifactPage<T> {
    pub total: usize,
    pub start: usize,
    pub items: Vec<T>,
}

impl CandidateArtifact {
    pub fn open(bytes: &[u8], limits: CandidateBundleLimits) -> Result<Self, AppError> {
        let (envelope, records, _family) = codec::read_structure(bytes, limits)?;
        let coefficients = LazyDecodedCoefficientTable::new(
            Arc::from(
                envelope
                    .section(SectionTag::SYMBOLICA_STATE)
                    .expect("checked section"),
            ),
            Arc::from(
                envelope
                    .section(SectionTag::COEFFICIENTS)
                    .expect("checked section"),
            ),
            limits.binary_limits(),
        )
        .map_err(codec::binary_error)?;
        codec::validate_ids(&records, coefficients.len())?;
        Ok(Self {
            creator_pid: std::process::id(),
            records,
            coefficients,
            encoded_bytes: bytes.len(),
        })
    }
    pub fn metadata(&self) -> Result<Value, AppError> {
        self.check_process()?;
        Ok(
            json!({"schema":"rustred.candidate-artifact-view.v1", "candidate_schema": self.records.schema,
            "status": self.records.status, "family_fingerprint":self.records.family_fingerprint,
            "arity":self.records.root_sector.len(), "root_sector":self.records.root_sector,
            "integral_order":self.records.integral_order, "priority_slots":self.records.permutation,
            "encoded_bytes":self.encoded_bytes, "total_sectors":self.records.sectors.len(),
            "total_rules":self.records.sectors.iter().map(|s|s.rules.len()).sum::<usize>(),
            "total_terminals":self.records.sectors.iter().map(|s|s.finite_residuals.len()).sum::<usize>(),
            "total_coefficients":self.coefficients.len(),
            "decoded_coefficients":self.coefficients.decoded_count().map_err(codec::binary_error)?,
            "source_replay_claim":false,"closure_claim":false,
            "authority":"trusted-generated structural inspection; coefficient text is display-only"}),
        )
    }
    pub fn sectors(
        &self,
        start: usize,
        limit: usize,
    ) -> Result<CandidateArtifactPage<Value>, AppError> {
        self.check_process()?;
        let range = page(self.records.sectors.len(), start, limit)?;
        Ok(CandidateArtifactPage {
            total: self.records.sectors.len(),
            start,
            items: range
                .map(|ordinal| {
                    let s = &self.records.sectors[ordinal];
                    json!({
                "ordinal":ordinal,"sector":s.sector,"total_rules":s.rules.len(),
                "total_terminals":s.finite_residuals.len()})
                })
                .collect(),
        })
    }
    fn sector(&self, ordinal: usize) -> Result<&SectorRecord, AppError> {
        self.check_process()?;
        self.records
            .sectors
            .get(ordinal)
            .ok_or_else(|| AppError::input("sector ordinal is outside artifact"))
    }
    pub fn rules(
        &self,
        sector: usize,
        start: usize,
        limit: usize,
    ) -> Result<CandidateArtifactPage<Value>, AppError> {
        let s = self.sector(sector)?;
        let range = page(s.rules.len(), start, limit)?;
        Ok(CandidateArtifactPage { total:s.rules.len(),start,items:range.map(|ordinal| {
            let r=&s.rules[ordinal]; json!({"ordinal":ordinal,
                "case":{"kind":r.case.kind,"fixed":r.case.fixed_axes.iter().zip(&r.case.fixed_values)
                    .map(|(axis,value)|json!({"axis":axis,"value":value})).collect::<Vec<_>>(),
                        "affine_equation_count":r.case.equations.len()},
                "target":inspection::integral_view(&r.target),"rhs_terms":r.rhs.len(),
                "retained_source_count":r.sources.len(),
                "guard_count":r.exclusions.iter().map(Vec::len).sum::<usize>()})
        }).collect() })
    }
    pub fn terminals(
        &self,
        sector: usize,
        start: usize,
        limit: usize,
    ) -> Result<CandidateArtifactPage<Vec<i16>>, AppError> {
        let s = self.sector(sector)?;
        let range = page(s.finite_residuals.len(), start, limit)?;
        Ok(CandidateArtifactPage {
            total: s.finite_residuals.len(),
            start,
            items: range
                .map(|i| s.finite_residuals[i].values.clone())
                .collect(),
        })
    }
    pub fn rule(
        &self,
        sector: usize,
        ordinal: usize,
        max_output_bytes: usize,
    ) -> Result<CandidateRuleInspection, AppError> {
        output_budget(max_output_bytes)?;
        let r = self
            .sector(sector)?
            .rules
            .get(ordinal)
            .ok_or_else(|| AppError::input("rule ordinal is outside sector"))?;
        let entries = r
            .rhs
            .len()
            .saturating_add(r.case.equations.len())
            .saturating_add(r.exclusions.iter().map(Vec::len).sum::<usize>());
        if entries > max_output_bytes {
            return Err(AppError::limit("rule exceeds detail output budget"));
        }
        let mut view = inspection::rule_view(ordinal, r);
        if codec::dispatch::policy(&self.records, sector, ordinal)
            == rustred::solver::RuleDispatchPolicy::AfterBaselinePartitionWholePiece
        {
            view.dispatch_policy = Some("AfterBaselinePartitionWholePiece");
        }
        bounded(&view, max_output_bytes)?;
        Ok(view)
    }
    pub fn coefficient(
        &self,
        id: usize,
        max_output_bytes: usize,
    ) -> Result<CandidateCoefficientInspection, AppError> {
        self.check_process()?;
        output_budget(max_output_bytes)?;
        let key = CoefficientId::try_from_index(id).map_err(codec::binary_error)?;
        let value = self
            .coefficients
            .coefficient(key)
            .map_err(codec::binary_error)?;
        let mut remaining = max_output_bytes;
        let mut render = |atom: symbolica::atom::Atom| -> Result<String, AppError> {
            let text = render_bounded(atom.printer(PrintOptions::file()), remaining)?;
            remaining -= text.len();
            Ok(text)
        };
        let variables = value
            .numerator
            .variables()
            .iter()
            .map(|v| render(v.to_atom()))
            .collect::<Result<Vec<_>, _>>()?;
        let numerator = render(value.numerator.to_expression())?;
        let denominator = render(value.denominator.to_expression())?;
        let view = CandidateCoefficientInspection {
            id: u32::try_from(id).map_err(|_| AppError::input("coefficient ID overflow"))?,
            variables,
            numerator,
            denominator,
            numerator_terms: value.numerator.nterms(),
            denominator_terms: value.denominator.nterms(),
        };
        bounded(&view, max_output_bytes)?;
        Ok(view)
    }
    pub fn check_process(&self) -> Result<(), AppError> {
        if self.creator_pid != std::process::id() {
            return Err(AppError::execution(
                "candidate artifact views cannot be reused after fork; reopen in a fresh process",
            ));
        }
        Ok(())
    }
}

// Use Symbolica's streaming native printer. Its canonical-string API allocates
// the entire string first and is intentionally NOT used for bounded UI text.
// Atom conversion itself remains native and is not a hard algebra/RAM budget.
fn render_bounded(value: impl std::fmt::Display, limit: usize) -> Result<String, AppError> {
    use std::fmt::Write;
    struct Text {
        text: String,
        limit: usize,
    }
    impl Write for Text {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            if s.len() > self.limit - self.text.len() {
                return Err(std::fmt::Error);
            }
            self.text.push_str(s);
            Ok(())
        }
    }
    let mut output = Text {
        text: String::new(),
        limit,
    };
    write!(&mut output, "{value}")
        .map_err(|_| AppError::limit("coefficient text exceeds output budget"))?;
    Ok(output.text)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_native_text_stops_the_producer_at_the_limit() {
        struct Huge<'a>(&'a std::cell::Cell<usize>);
        impl std::fmt::Display for Huge<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                for _ in 0..1_000_000 {
                    self.0.set(self.0.get() + 1);
                    f.write_str("x")?;
                }
                Ok(())
            }
        }
        let calls = std::cell::Cell::new(0);
        assert!(super::render_bounded(Huge(&calls), 8).is_err());
        assert_eq!(calls.get(), 9);
    }
}

fn page(total: usize, start: usize, limit: usize) -> Result<std::ops::Range<usize>, AppError> {
    if limit == 0 || limit > 1000 {
        return Err(AppError::input("page limit must be from 1 to 1000"));
    }
    if start > total {
        return Err(AppError::input("page start is beyond collection"));
    }
    Ok(start..start.saturating_add(limit).min(total))
}
fn output_budget(bytes: usize) -> Result<(), AppError> {
    if bytes == 0 || bytes > crate::MAX_OUTPUT_BYTES {
        Err(AppError::input("invalid detail output budget"))
    } else {
        Ok(())
    }
}
fn bounded<T: Serialize>(value: &T, bytes: usize) -> Result<(), AppError> {
    struct Counter {
        count: usize,
        limit: usize,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if buf.len() > self.limit - self.count {
                return Err(std::io::Error::other("detail output budget exceeded"));
            }
            self.count += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(
        Counter {
            count: 0,
            limit: bytes,
        },
        value,
    )
    .map_err(|e| AppError::limit(e.to_string()))
}
