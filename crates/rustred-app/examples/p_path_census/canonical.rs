//! M5 R0 WP0 golden format (T-VAL), shared verbatim by vakint's
//! `examples/m5_r0_goldens/canonical.rs` and rustred's
//! `crates/rustred-app/examples/p_path_census/canonical.rs`: keep the two
//! files byte-identical below this header line.
// ---- shared body ----
//
// `rustred.p-reduction.canonical.v1`: the values of a `PReduction` (no
// statistics), one line per item, independent of Symbolica's printer:
//   dimension <L>
//   term <basis> <tag> | <numerator> | <denominator>
//   condition <numerator> | <denominator>
// with <basis> = U, M:<name> or R:<terminal id>, <tag> the Gamma tag's
// (power, slot) pairs, and a polynomial in `d` as space-separated
// `<exponents>:<integer coefficient>` monomials, ascending. Terms keep the
// reduction's order (sorted by basis and tag); conditions are sorted.
use rustred::algebra::Coefficient;
use rustred::propagator::library::{PBasis, PReduction};

pub const CANONICAL_SCHEMA: &str = "rustred.p-reduction.canonical.v1";

fn polynomial<I, C>(monomials: I) -> String
where
    I: IntoIterator<Item = (Vec<u16>, C)>,
    C: std::fmt::Display,
{
    let mut monomials: Vec<(Vec<u16>, String)> = monomials
        .into_iter()
        .map(|(e, c)| (e, c.to_string()))
        .collect();
    monomials.sort();
    let parts: Vec<String> = monomials
        .into_iter()
        .map(|(e, c)| {
            let e: Vec<String> = e.iter().map(u16::to_string).collect();
            format!("{}:{c}", e.join(","))
        })
        .collect();
    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join(" ")
    }
}

fn coefficient(c: &Coefficient) -> String {
    let numerator = (&c.numerator)
        .into_iter()
        .map(|m| (m.exponents.to_vec(), m.coefficient.clone()));
    let denominator = (&c.denominator)
        .into_iter()
        .map(|m| (m.exponents.to_vec(), m.coefficient.clone()));
    format!("{} | {}", polynomial(numerator), polynomial(denominator))
}

pub fn canonical_text(reduction: &PReduction) -> String {
    let mut out = format!("{CANONICAL_SCHEMA}\ndimension {}\n", reduction.dimension);
    for term in &reduction.terms {
        let basis = match &term.basis {
            PBasis::Unit => "U".to_owned(),
            PBasis::Master(name) => format!("M:{name}"),
            PBasis::Residual(id) => format!("R:{}", id.0),
        };
        let tag = format!("{:?}", term.tag).replace(' ', "");
        out.push_str(&format!(
            "term {basis} {tag} | {}\n",
            coefficient(&term.coefficient)
        ));
    }
    let mut conditions: Vec<String> = reduction
        .nonzero_conditions
        .iter()
        .map(coefficient)
        .collect();
    conditions.sort();
    for condition in conditions {
        out.push_str(&format!("condition {condition}\n"));
    }
    out
}
