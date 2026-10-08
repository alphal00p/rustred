//! Bind stored finite sources to the supplied sessions' complete retained span.
use super::*;
use symbolica::{
    domains::rational_polynomial::RationalPolynomialField,
    prelude::Z,
    tensors::sparse::{LuLMode, SparseRowReducer},
};

pub(in crate::reduction::terminal_relations::collection::saved) fn authenticate(
    feedbacks: &[Arc<FiniteFeedback>],
    sessions: &[&TerminalRelationSession],
    limits: TerminalCollectionLimits,
) -> Result<()> {
    let sessions: BTreeMap<_, _> = sessions
        .iter()
        .map(|s| (s.family_owner().fingerprint(), *s))
        .collect();
    for feedback in feedbacks {
        for (id, source) in &feedback.local {
            let session = sessions
                .get(id.as_str())
                .ok_or(VacuumCollectionError::WrongFamily)?;
            if source.binding.as_deref() != session.assistance_binding() {
                return Err(VacuumCollectionError::ReplayFailed);
            }
            let current = retained_rows(session)?;
            // Identical cold checkpoints need no second whole-basis reduction.
            // Normalization bridges are proved independently below, so a
            // forged hidden row cannot hide behind unchanged final maps.
            if source.rows.starts_with(&current) {
                let bridges = &source.rows[current.len()..];
                if bridges
                    .iter()
                    .map(|r| session.normalized_row(r))
                    .collect::<std::result::Result<Vec<_>, _>>()?
                    .iter()
                    .all(BTreeMap::is_empty)
                {
                    continue;
                }
            }
            let normalized = source
                .rows
                .iter()
                .map(|r| session.normalized_row(r))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let support: BTreeSet<_> = current
                .iter()
                .chain(&normalized)
                .flat_map(|r| r.keys())
                .cloned()
                .collect();
            check(
                "feedback authentication columns",
                support.len(),
                limits.finite_feedback.max_columns,
            )?;
            let offsets: BTreeMap<_, _> = support
                .into_iter()
                .enumerate()
                .map(|(i, k)| (k, i as u32))
                .collect();
            let mut reducer = SparseRowReducer::new(
                u32::try_from(offsets.len())
                    .map_err(|_| VacuumCollectionError::ArithmeticOverflow)?,
                RationalPolynomialField::new(Z),
                LuLMode::None,
            );
            let insert =
                |reducer: &mut SparseRowReducer<_>, row: &TerminalRelationRow| -> Result<bool> {
                    check(
                        "feedback authentication nonzeros",
                        reducer.u().nvalues().saturating_add(offsets.len()),
                        limits.finite_feedback.max_reducer_nonzeros,
                    )?;
                    let mut entries: Vec<_> =
                        row.iter().map(|(k, v)| (offsets[k], v.clone())).collect();
                    entries.sort_by_key(|(i, _)| *i);
                    let values: Vec<_> = entries.iter().map(|(_, v)| v.clone()).collect();
                    let columns: Vec<_> = entries.iter().map(|(i, _)| *i).collect();
                    Ok(reducer.add_row(&values, &columns).is_some())
                };
            for row in &current {
                insert(&mut reducer, row)?;
            }
            for row in &normalized {
                if insert(&mut reducer, row)? {
                    return Err(VacuumCollectionError::ReplayFailed);
                }
            }
        }
    }
    Ok(())
}
