//! Independent replay authority for persisted source circuits.
//!
//! All physical source columns participate in Symbolica's exact sparse product.
//! A discovery support, modular zero, graph edge or saved rank is never proof.

use super::*;
use symbolica::tensors::sparse::SparseMatrix;

/// The caller authenticates source identity/ordinal bounds and supplies exactly
/// the original ordered selected frame. Required conditions include the frozen
/// baseline, earlier accepted circuits, and the entire scanned source prefix's
/// domain, including every unselected/zero-weight/post-hit source.
pub(super) fn validate_circuit(
    session: &TerminalRelationSession,
    source_rows: &[&SourceRow],
    required_conditions: &[Coefficient],
    circuit: &TerminalRelationModularCircuit,
) -> Result<(), TerminalRelationError> {
    if source_rows.is_empty() || source_rows.len() != circuit.sources.len() {
        return Err(invalid("circuit source frame has the wrong length"));
    }
    if source_rows
        .iter()
        .zip(&circuit.sources)
        .any(|(row, identity)| &row.identity != identity)
    {
        return Err(invalid(
            "circuit original-source identity differs from its selected frame",
        ));
    }
    let context = session.family.coefficient_context();
    let limits = session.limits.algebra.exact_algebra;
    for guard in &circuit.nonzero_conditions {
        context
            .validate_with_limits(guard, limits)
            .map_err(algebra)?;
        if guard.is_zero() {
            return Err(invalid(
                "circuit carries an identically zero domain condition",
            ));
        }
    }
    for guard in required_conditions {
        if !guard.is_constant() && !circuit.nonzero_conditions.contains(guard) {
            return Err(invalid(
                "circuit dropped an original source/domain condition",
            ));
        }
    }
    if circuit
        .weights
        .iter()
        .any(|(&slot, value)| slot >= source_rows.len() || value.is_zero())
    {
        return Err(invalid(
            "circuit contains an invalid or explicit-zero source-weight slot",
        ));
    }
    for value in circuit.weights.values().chain(circuit.image.values()) {
        context
            .validate_with_limits(value, limits)
            .map_err(algebra)?;
        if !value.denominator.is_constant() {
            let pole: Coefficient = value.denominator.clone().into();
            if !circuit.nonzero_conditions.contains(&pole) {
                return Err(invalid(
                    "circuit dropped a reconstructed source-weight/output pole",
                ));
            }
        }
    }
    if circuit.image.is_empty()
        || circuit
            .image
            .iter()
            .any(|(key, value)| !session.terminals.contains(key) || value.is_zero())
    {
        return Err(invalid(
            "circuit image is not a nonempty terminal-only equation",
        ));
    }
    let mut image_keys: Vec<_> = circuit.image.keys().cloned().collect();
    session.sort_existing_columns(&mut image_keys)?;
    if !circuit.image[&image_keys[0]].is_one() {
        return Err(invalid(
            "circuit does not use the existing unit terminal-pivot convention",
        ));
    }
    let mut columns: BTreeSet<_> = circuit.image.keys().cloned().collect();
    for row in source_rows {
        for (key, value) in &row.terms {
            if key.powers().len() != session.family.denominator_count() || value.is_zero() {
                return Err(invalid(
                    "circuit source has an invalid physical column or explicit zero",
                ));
            }
            context
                .validate_with_limits(value, limits)
                .map_err(algebra)?;
            columns.insert(key.clone());
        }
    }
    let width = u32::try_from(columns.len()).map_err(invalid)?;
    let count = u32::try_from(source_rows.len()).map_err(invalid)?;
    let registry: BTreeMap<_, _> = columns
        .into_iter()
        .enumerate()
        .map(|(index, key)| (key, index as u32))
        .collect();
    let field = RationalPolynomialField::new(Z);
    let mut sources = SparseMatrix::new(0, width, field.clone());
    for row in source_rows {
        let (ids, values) = row
            .terms
            .iter()
            .map(|(key, value)| (registry[key], value.clone()))
            .unzip();
        sources.add_row(values, ids);
    }
    let mut weights = SparseMatrix::new(0, count, field.clone());
    let (ids, values) = circuit
        .weights
        .iter()
        .map(|(&slot, value)| (slot as u32, value.clone()))
        .unzip();
    weights.add_row(values, ids);
    let product = &weights * &sources;
    let mut expected = SparseMatrix::new(0, width, field);
    let (ids, values) = circuit
        .image
        .iter()
        .map(|(key, value)| (registry[key], value.clone()))
        .unzip();
    expected.add_row(values, ids);
    if product != expected {
        return Err(invalid(
            "exact full-column circuit product differs from the terminal image",
        ));
    }
    Ok(())
}
