//! Explicit generation work selection, not a new mathematical or closure scope.

use crate::AppError;

use super::preparation::Prepared;

pub(super) fn parse_masks(masks: &[String]) -> Result<Vec<Vec<bool>>, AppError> {
    if masks.is_empty() || masks.len() > (1 << 16) {
        return Err(AppError::input(
            "selected_sectors requires a nonempty list of at most 65536 binary masks",
        ));
    }
    masks
        .iter()
        .map(|mask| {
            if !(1..=16).contains(&mask.len())
                || !mask.bytes().all(|byte| matches!(byte, b'0' | b'1'))
            {
                return Err(AppError::input(
                    "selected_sectors masks must contain 1 through 16 binary digits",
                ));
            }
            Ok(mask.bytes().map(|byte| byte == b'1').collect())
        })
        .collect()
}

/// Keep the complete prepared zero census and sources. Only pending nonzero
/// jobs are selected. Returning canonical masks binds report/checkpoint identity
/// independently of user-supplied list order or integral coordinate priorities.
pub(super) fn apply<const N: usize, F>(
    prepared: &mut Prepared<N, F>,
    selected: Option<&[Vec<bool>]>,
    max_entries: usize,
) -> Result<Option<Vec<Vec<bool>>>, AppError> {
    let selected = canonical_masks(
        &prepared.root[..prepared.sources.active_arity()],
        selected,
        max_entries,
    )?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let mut sectors = Vec::with_capacity(selected.len());
    for sector in &selected {
        let sector: [bool; N] = rustred::storage_array(&sector, false).expect("validated arity");
        if prepared.sectors.binary_search(&sector).is_err() {
            return Err(AppError::input(
                "selected_sectors mask is proved zero, not a nonzero generation job",
            ));
        }
        sectors.push(sector);
    }
    prepared.sectors = sectors;
    Ok(Some(selected))
}

/// Shape/root canonicalization is also used by checkpoint identity and the
/// existing single-sector exporter, which deliberately does no zero census.
pub(super) fn canonical_masks(
    root: &[bool],
    selected: Option<&[Vec<bool>]>,
    max_entries: usize,
) -> Result<Option<Vec<Vec<bool>>>, AppError> {
    let Some(selected) = selected else {
        return Ok(None);
    };
    let arity = root.len();
    if !(1..=16).contains(&arity) {
        return Err(AppError::input(
            "selected_sectors family arity must be 1 through 16",
        ));
    }
    if selected.is_empty() {
        return Err(AppError::input("selected_sectors must not be empty"));
    }
    if selected.len() > (1usize << arity)
        || selected
            .len()
            .checked_mul(arity + 1)
            .is_none_or(|count| count > max_entries)
    {
        return Err(AppError::limit(
            "selected_sectors exceeds its collection limit",
        ));
    }
    let mut canonical = Vec::with_capacity(selected.len());
    for sector in selected {
        if sector.len() != arity {
            return Err(AppError::input(
                "selected_sectors mask has the wrong family arity",
            ));
        }
        if sector
            .iter()
            .zip(root)
            .any(|(&active, &allowed)| active && !allowed)
        {
            return Err(AppError::input(
                "selected_sectors mask lies outside the root",
            ));
        }
        canonical.push(sector.clone());
    }
    canonical.sort_unstable();
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(AppError::input(
            "selected_sectors contains a duplicate mask",
        ));
    }
    Ok(Some(canonical))
}
