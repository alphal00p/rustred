//! Public coordinate projection for capacity-backed campaign diagnostics.
//! Only named coordinate fields are projected; arbitrary collection lengths,
//! strings and symbolic expressions retain their meaning.
use serde_json::Value;

pub(super) fn project(value: &mut Value, physical_arity: usize) {
    let capacity = rustred::campaign_storage_arity(physical_arity);
    if capacity == physical_arity || !cfg!(feature = "capacity-dispatch") {
        return;
    }
    project_fields(value, physical_arity, capacity);
}

/// Private wire codecs may store either physical coordinates or capacity
/// coordinates. Shrinking is permitted only for authenticated zero padding.
pub(super) fn compatible_width(left: usize, right: usize) -> bool {
    rustred::fits_storage(left, right) || rustred::fits_storage(right, left)
}
pub(super) fn restore_array<T: Copy + PartialEq, const N: usize>(
    values: &[T],
    padding: T,
) -> Option<[T; N]> {
    if values.len() <= N {
        return rustred::storage_array(values, padding);
    }
    if rustred::fits_storage(N, values.len()) && values[N..].iter().all(|value| *value == padding) {
        Some(std::array::from_fn(|axis| values[axis]))
    } else {
        None
    }
}

fn project_fields(value: &mut Value, n: usize, capacity: usize) {
    match value {
        Value::Array(values) => {
            for value in values {
                project_fields(value, n, capacity);
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields.iter_mut() {
                if matches!(
                    key.as_str(),
                    "owner"
                        | "owner_mask"
                        | "source_mask"
                        | "target_mask"
                        | "sector"
                        | "source_sector"
                        | "target_sector"
                        | "target_owner"
                        | "support"
                        | "source_support"
                        | "target_support"
                        | "root_sector"
                        | "incomplete_owner"
                ) {
                    if let Value::String(mask) = value {
                        if mask.len() == capacity
                            && mask.bytes().all(|byte| matches!(byte, b'0' | b'1'))
                        {
                            debug_assert!(mask.as_bytes()[n..].iter().all(|byte| *byte == b'0'));
                            mask.truncate(n);
                        }
                    }
                }
                if matches!(
                    key.as_str(),
                    "lower"
                        | "upper"
                        | "powers"
                        | "target"
                        | "shift"
                        | "shifts"
                        | "argument_shift"
                        | "fixed"
                        | "source_lower"
                        | "source_upper"
                        | "target_lower"
                        | "target_upper"
                        | "source_local_lower"
                        | "source_local_upper"
                        | "target_local_lower"
                        | "target_local_upper"
                ) {
                    if let Value::Array(coordinates) = value {
                        if coordinates.len() == capacity
                            && coordinates.iter().all(|value| {
                                value.is_number() || value.is_null() || value.is_boolean()
                            })
                        {
                            coordinates.truncate(n);
                        }
                    }
                }
                if matches!(
                    key.as_str(),
                    "source_inactive_axes"
                        | "target_inactive_axes"
                        | "active_axes"
                        | "inactive_axes"
                ) {
                    if let Value::Array(axes) = value {
                        axes.retain(|axis| axis.as_u64().is_none_or(|axis| axis < n as u64));
                    }
                }
                project_fields(value, n, capacity);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    #[cfg(feature = "capacity-dispatch")]
    fn restored_padding_is_checked_before_conversion() {
        assert_eq!(restore_array::<_, 4>(&[2u64], 0), Some([2, 0, 0, 0]));
        assert_eq!(restore_array::<_, 1>(&[2u64, 0, 0, 0], 0), Some([2]));
        assert_eq!(restore_array::<_, 1>(&[2u64, 0, 1, 0], 0), None);
        assert_eq!(
            restore_array::<_, 1>(&[Some(2u64), Some(0), None, Some(0)], Some(0)),
            None
        );
        assert_eq!(
            restore_array::<_, 1>(&[true, false, true, false], false),
            None
        );
        assert_eq!(restore_array::<_, 4>(&[2u64, 0], 0), Some([2, 0, 0, 0]));
        assert_eq!(restore_array::<_, 8>(&[2u64], 0), None);
    }
    #[test]
    #[cfg(feature = "capacity-dispatch")]
    fn projection_preserves_collections_and_display_text() {
        let mut value = json!({"owner":"1000", "lower":[2,0,0,0], "upper":[null,0,0,0],
            "records":[1,2,3,4], "id":"1000", "expression":"1000", "nested":{"target":[3,0,0,0]}});
        project(&mut value, 1);
        assert_eq!(
            value,
            json!({"owner":"1", "lower":[2], "upper":[null], "records":[1,2,3,4],
            "id":"1000", "expression":"1000", "nested":{"target":[3]}})
        );
    }
}
