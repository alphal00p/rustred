//! Necessary owner-root column exclusions on explicitly fixed coordinates.
//! This selects projection columns before solving; it never edits an RHS.
use super::{Result, require};
use rustred::identity::IndexShift;
use serde_json::Value;
use std::collections::BTreeSet;

pub fn enabled(request: &Value) -> Result<bool> {
    match request.get("forbid_fixed_outside_root_activations") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err("forbid_fixed_outside_root_activations must be boolean".into()),
    }
}

pub fn activates_fixed_outside_root(
    root: &[bool],
    fixed: &[(usize, i64)],
    shift: &[i64],
) -> Result<bool> {
    require(root.len() == shift.len(), "root/shift arity differs")?;
    let mut forbidden = false;
    for &(axis, value) in fixed {
        require(axis < root.len(), "fixed axis outside root arity")?;
        // Widen before addition: two signed physical coordinates fit i128.
        // Keep a checked operation rather than machine clipping/wrapping.
        let child = i128::from(value)
            .checked_add(i128::from(shift[axis]))
            .ok_or("fixed child power overflow")?;
        forbidden |= !root[axis] && child > 0;
    }
    Ok(forbidden)
}

pub fn columns(
    root: &[bool],
    fixed: &[(usize, i64)],
    universe: &BTreeSet<IndexShift>,
) -> Result<BTreeSet<IndexShift>> {
    let mut result = BTreeSet::new();
    for shift in universe {
        if activates_fixed_outside_root(root, fixed, shift.values())? {
            result.insert(shift.clone());
        }
    }
    Ok(result)
}
