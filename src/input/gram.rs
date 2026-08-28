//! External-Gram validation and deterministic dense semantic assembly.

use symbolica::atom::Atom;

use super::error::Error;
use super::limits::{Limits, check_limit};
use super::request::AtomGramEntry;

pub(super) fn build_external_gram(
    external: &[String],
    entries: Vec<AtomGramEntry>,
    limits: Limits,
) -> Result<(Vec<Vec<Atom>>, Vec<Atom>), Error> {
    let expected = external
        .len()
        .checked_mul(
            external
                .len()
                .checked_add(1)
                .ok_or(Error::ResourceCountOverflow {
                    resource: "external Gram entries",
                })?,
        )
        .ok_or(Error::ResourceCountOverflow {
            resource: "external Gram entries",
        })?
        / 2;
    check_limit("external Gram entries", expected, limits.max_gram_entries)?;
    check_limit(
        "supplied external Gram entries",
        entries.len(),
        limits.max_gram_entries,
    )?;
    let mut supplied = Vec::<((usize, usize), Atom)>::new();
    supplied
        .try_reserve_exact(entries.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "supplied external Gram entries",
            requested: entries.len(),
        })?;
    for entry in entries {
        let left = external
            .iter()
            .position(|name| name == &entry.left)
            .ok_or_else(|| Error::UnknownExternalGramMomentum {
                momentum: entry.left.clone(),
            })?;
        let right = external
            .iter()
            .position(|name| name == &entry.right)
            .ok_or_else(|| Error::UnknownExternalGramMomentum {
                momentum: entry.right.clone(),
            })?;
        let key = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        if supplied.iter().any(|(candidate, _)| *candidate == key) {
            return Err(Error::DuplicateExternalGram {
                left: external[key.0].clone(),
                right: external[key.1].clone(),
            });
        }
        supplied.push((key, entry.value));
    }
    let mut matrix = Vec::<Vec<Atom>>::new();
    matrix
        .try_reserve_exact(external.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "external Gram matrix rows",
            requested: external.len(),
        })?;
    for _ in external {
        let mut row = Vec::<Atom>::new();
        row.try_reserve_exact(external.len())
            .map_err(|_| Error::AllocationFailure {
                resource: "external Gram matrix row",
                requested: external.len(),
            })?;
        for _ in external {
            row.push(Atom::num(0));
        }
        matrix.push(row);
    }
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(expected)
        .map_err(|_| Error::AllocationFailure {
            resource: "ordered external Gram",
            requested: expected,
        })?;
    for left in 0..external.len() {
        for right in left..external.len() {
            let position = supplied
                .iter()
                .position(|(candidate, _)| *candidate == (left, right))
                .ok_or_else(|| Error::MissingExternalGram {
                    left: external[left].clone(),
                    right: external[right].clone(),
                })?;
            let (_, value) = supplied.remove(position);
            matrix[left][right] = value.clone();
            matrix[right][left] = value.clone();
            ordered.push(value);
        }
    }
    debug_assert!(supplied.is_empty());
    Ok((matrix, ordered))
}
