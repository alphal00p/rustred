//! Explicit caller scope, independent of names, target powers and ordering.

use crate::application::error::AppError;

/// The downset of this mask admits positive powers only on true coordinates.
/// An empty list deliberately preserves unrestricted family closure.
pub(super) fn root_sector(
    arity: usize,
    nonpositive_indices: &[usize],
) -> Result<Vec<bool>, AppError> {
    let mut root = vec![true; arity];
    for &axis in nonpositive_indices {
        let permitted = root.get_mut(axis).ok_or_else(|| {
            AppError::input(format!(
                "family close nonpositive index {axis} lies outside 0..{arity}"
            ))
        })?;
        if !*permitted {
            return Err(AppError::input(format!(
                "family close nonpositive index {axis} is repeated"
            )));
        }
        *permitted = false;
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_is_explicit_and_coordinate_order_independent() {
        assert_eq!(root_sector(3, &[]).unwrap(), [true; 3]);
        assert_eq!(root_sector(3, &[2]).unwrap(), [true, true, false]);
        assert_eq!(
            root_sector(4, &[3, 1]).unwrap(),
            root_sector(4, &[1, 3]).unwrap()
        );
        assert_eq!(root_sector(1, &[0]).unwrap(), [false]);
    }

    #[test]
    fn invalid_scope_indices_fail_before_search() {
        assert!(root_sector(3, &[3]).is_err());
        assert!(root_sector(3, &[1, 1]).is_err());
        assert!(root_sector(3, &[usize::MAX]).is_err());
    }
}
