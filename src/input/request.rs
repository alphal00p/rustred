//! Unvalidated construction requests accepted by the input compiler.

use symbolica::atom::Atom;

/// Construction input shared by compact and explicit frontends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomPropagator {
    pub id: String,
    pub expression: Atom,
    pub target_power: i64,
    pub power_shift: Option<Atom>,
}

/// One upper-triangular external Gram entry supplied as authenticated atoms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomGramEntry {
    pub left: String,
    pub right: String,
    pub value: Atom,
}

/// Fully typed, but not yet validated, common project input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomProject {
    pub name: Option<String>,
    /// `None` requests deterministic inference. `Some` is a strict ordered
    /// allowlist. Extra entries are retained as frontend/application metadata,
    /// but only parameters actually discovered in family-defining fields enter
    /// the derived family's coefficient field.
    pub parameters: Option<Vec<String>>,
    pub loop_momenta: Vec<String>,
    pub external_momenta: Vec<String>,
    pub dimension: Atom,
    pub propagators: Vec<AtomPropagator>,
    pub external_gram: Vec<AtomGramEntry>,
    pub numerator: Option<Atom>,
}

/// Textual propagator accepted by an explicit frontend.
///
/// Expression strings are parsed only by
/// [`super::Compiler::compile_text_parts`], under the same namespace,
/// resource limits, and panic boundary as compact `I(...)` input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPropagator {
    pub id: String,
    pub expression: String,
    pub target_power: i64,
    pub power_shift: Option<String>,
}

/// Textual upper-triangular external Gram entry for an explicit frontend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextGramEntry {
    pub left: String,
    pub right: String,
    pub value: String,
}

/// Fully textual explicit-project seam used by application adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextProject {
    pub name: Option<String>,
    pub parameters: Option<Vec<String>>,
    pub loop_momenta: Vec<String>,
    pub external_momenta: Vec<String>,
    pub dimension: String,
    pub propagators: Vec<TextPropagator>,
    pub external_gram: Vec<TextGramEntry>,
    pub numerator: Option<String>,
}
