//! Generic selected-source frames over a bounded triangular chart support.
//!
//! This producer owns no evidence or closure authority. It expands an explicit
//! subset of sector-chart axes into exact translated-source requests,
//! canonicalizes those requests at the existing campaign boundary, and
//! materializes the existing
//! [`crate::foundry::completion::frame::SelectedSourceFrame`]. Different
//! ordinary sources may use different total-degree ceilings; coordinates
//! outside the selected axes remain exactly zero.

mod construction;
mod enumeration;
mod error;
mod resource;

pub(crate) use construction::try_build_triangular_support_frame;
pub(crate) use error::TriangularSupportError;

const ARITY: &str = "triangular-support request arity";
const DEGREE: &str = "triangular-support source degree";
const OFFSETS: &str = "triangular-support distinct offsets";
const OFFSET_COORDINATES: &str = "triangular-support distinct-offset coordinate cells";
const SUBMITTED_REQUESTS: &str = "triangular-support submitted requests";
const CANONICAL_REQUESTS: &str = "triangular-support canonical requests";
const ACCUMULATED_REQUESTS: &str = "triangular-support accumulated requests";
const REQUEST_COORDINATES: &str = "triangular-support request coordinate cells";
const SELECTED_TRANSLATION_REQUESTS: &str = "requested selected source translations";
const SELECTED_TRANSLATION_SOURCES: &str = "translated source rows";
const SELECTED_TRANSLATION_OFFSETS: &str = "canonical selected translation offsets";
const PHYSICAL_SOURCE_INSTANCES: &str = "physical-frame source instances";
const PHYSICAL_CSR_ROW_OFFSETS: &str = "physical-frame CSR row offsets";

#[cfg(test)]
mod tests;
