//! Stable private naming and authentication identities for indexed fields.

use crate::algebra::CoefficientContext;

use super::error::IndexedAlgebraError;

const BASE_CONTEXT_PREFIX: &str = "rustred-base-context-v1|parameters=";
const INDEX_SYMBOL_PREFIX: &str = "rustred::indexed_coefficient_s";
const INDEX_SYMBOL_POSITION_SEPARATOR: &str = "::n";
const INDEXED_CONTEXT_PREFIX: &str = "rustred-indexed-coefficient-context-v1|base=";
const INDEXED_CONTEXT_SCOPE_SEPARATOR: &str = "|scope=";
const INDEXED_CONTEXT_INDEX_SEPARATOR: &str = "|indices=";

fn checked_string_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, IndexedAlgebraError> {
    left.checked_add(right)
        .ok_or(IndexedAlgebraError::ResourceCountOverflow { resource })
}

fn checked_string_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, IndexedAlgebraError> {
    left.checked_mul(right)
        .ok_or(IndexedAlgebraError::ResourceCountOverflow { resource })
}

fn try_string_with_capacity(
    resource: &'static str,
    requested: usize,
) -> Result<String, IndexedAlgebraError> {
    let mut result = String::new();
    result
        .try_reserve_exact(requested)
        .map_err(|_| IndexedAlgebraError::AllocationFailure {
            resource,
            requested,
        })?;
    Ok(result)
}

const fn decimal_digits(value: usize) -> usize {
    if value == 0 {
        return 1;
    }
    let mut remaining = value;
    let mut digits = 0;
    while remaining != 0 {
        remaining /= 10;
        digits += 1;
    }
    digits
}

fn push_decimal(target: &mut String, value: usize) {
    // A usize's base-10 digit count is bounded by its binary bit width, so
    // this stack buffer is sufficient on every supported platform.
    let mut reversed = [0_u8; usize::BITS as usize];
    let mut remaining = value;
    let mut used = 0;
    loop {
        reversed[used] = (remaining % 10) as u8;
        used += 1;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    for digit in reversed[..used].iter().rev() {
        target.push(char::from(b'0' + *digit));
    }
}

pub(super) fn encode_symbol_component(bytes: &[u8]) -> Result<String, IndexedAlgebraError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let requested = checked_string_mul("encoded indexed coefficient scope bytes", bytes.len(), 2)?;
    let mut encoded =
        try_string_with_capacity("encoded indexed coefficient scope bytes", requested)?;
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(encoded)
}

fn qualified_index_symbol_len(
    encoded_scope_len: usize,
    position: usize,
) -> Result<usize, IndexedAlgebraError> {
    let resource = "indexed coefficient Symbolica name bytes";
    let requested = checked_string_add(resource, INDEX_SYMBOL_PREFIX.len(), encoded_scope_len)?;
    let requested = checked_string_add(resource, requested, INDEX_SYMBOL_POSITION_SEPARATOR.len())?;
    checked_string_add(resource, requested, decimal_digits(position))
}

/// Keep the total generated-name workload representable before registration;
/// Symbolica retains the identities even though Rust builds one name at a time.
pub(super) fn preflight_qualified_index_symbols(
    encoded_scope_len: usize,
    index_count: usize,
) -> Result<(), IndexedAlgebraError> {
    debug_assert_ne!(index_count, 0);
    let resource = "indexed coefficient aggregate Symbolica name bytes";
    let fixed_len = checked_string_add(resource, INDEX_SYMBOL_PREFIX.len(), encoded_scope_len)?;
    let fixed_len = checked_string_add(resource, fixed_len, INDEX_SYMBOL_POSITION_SEPARATOR.len())?;
    let mut total = checked_string_mul(resource, fixed_len, index_count)?;

    // Exact decimal digit sum for positions 0..index_count, in O(log n).
    let mut range_start = 0usize;
    let mut digits = 1usize;
    while range_start < index_count {
        let range_end = if range_start == 0 {
            index_count.min(10)
        } else {
            range_start
                .checked_mul(10)
                .unwrap_or(index_count)
                .min(index_count)
        };
        let positions = range_end - range_start;
        total = checked_string_add(
            resource,
            total,
            checked_string_mul(resource, positions, digits)?,
        )?;
        if range_end == index_count {
            break;
        }
        range_start = range_end;
        digits = checked_string_add(resource, digits, 1)?;
    }

    // Also validate the exact largest individual allocation explicitly.
    let _ = qualified_index_symbol_len(encoded_scope_len, index_count - 1)?;
    Ok(())
}

pub(super) fn qualified_index_symbol(
    encoded_scope: &str,
    position: usize,
) -> Result<String, IndexedAlgebraError> {
    let resource = "indexed coefficient Symbolica name bytes";
    let requested = qualified_index_symbol_len(encoded_scope.len(), position)?;
    let mut result = try_string_with_capacity(resource, requested)?;
    result.push_str(INDEX_SYMBOL_PREFIX);
    result.push_str(encoded_scope);
    result.push_str(INDEX_SYMBOL_POSITION_SEPARATOR);
    push_decimal(&mut result, position);
    debug_assert_eq!(result.len(), requested);
    Ok(result)
}

pub(super) fn base_context_fingerprint(
    base: &CoefficientContext,
) -> Result<String, IndexedAlgebraError> {
    let resource = "indexed coefficient base-context fingerprint bytes";
    let mut requested = checked_string_add(
        resource,
        BASE_CONTEXT_PREFIX.len(),
        decimal_digits(base.parameter_names().len()),
    )?;
    for name in base.parameter_names() {
        requested = checked_string_add(resource, requested, 1)?;
        requested = checked_string_add(resource, requested, decimal_digits(name.len()))?;
        requested = checked_string_add(resource, requested, 1)?;
        requested = checked_string_add(resource, requested, name.len())?;
    }

    let mut result = try_string_with_capacity(resource, requested)?;
    result.push_str(BASE_CONTEXT_PREFIX);
    push_decimal(&mut result, base.parameter_names().len());
    for name in base.parameter_names() {
        result.push('|');
        push_decimal(&mut result, name.len());
        result.push(':');
        result.push_str(name);
    }
    debug_assert_eq!(result.len(), requested);
    Ok(result)
}

pub(super) fn indexed_context_fingerprint(
    base_fingerprint: &str,
    scope: &str,
    index_count: usize,
) -> Result<String, IndexedAlgebraError> {
    let resource = "indexed coefficient context fingerprint bytes";
    let mut requested = checked_string_add(
        resource,
        INDEXED_CONTEXT_PREFIX.len(),
        base_fingerprint.len(),
    )?;
    requested = checked_string_add(resource, requested, INDEXED_CONTEXT_SCOPE_SEPARATOR.len())?;
    requested = checked_string_add(resource, requested, decimal_digits(scope.len()))?;
    requested = checked_string_add(resource, requested, 1)?;
    requested = checked_string_add(resource, requested, scope.len())?;
    requested = checked_string_add(resource, requested, INDEXED_CONTEXT_INDEX_SEPARATOR.len())?;
    requested = checked_string_add(resource, requested, decimal_digits(index_count))?;

    let mut result = try_string_with_capacity(resource, requested)?;
    result.push_str(INDEXED_CONTEXT_PREFIX);
    result.push_str(base_fingerprint);
    result.push_str(INDEXED_CONTEXT_SCOPE_SEPARATOR);
    push_decimal(&mut result, scope.len());
    result.push(':');
    result.push_str(scope);
    result.push_str(INDEXED_CONTEXT_INDEX_SEPARATOR);
    push_decimal(&mut result, index_count);
    debug_assert_eq!(result.len(), requested);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::algebra::CoefficientContext;

    use super::{
        INDEX_SYMBOL_POSITION_SEPARATOR, INDEX_SYMBOL_PREFIX, base_context_fingerprint,
        encode_symbol_component, indexed_context_fingerprint, preflight_qualified_index_symbols,
        qualified_index_symbol, qualified_index_symbol_len,
    };
    use crate::algebra::indexed::IndexedAlgebraError;

    #[test]
    fn checked_builders_preserve_the_indexed_identity_encoding() {
        let encoded = encode_symbol_component(b"a/\0").unwrap();
        assert_eq!(encoded, "612f00");
        assert_eq!(
            qualified_index_symbol(&encoded, 10).unwrap(),
            "rustred::indexed_coefficient_s612f00::n10"
        );

        let base = CoefficientContext::new(["d", "m2"]);
        let base_fingerprint = base_context_fingerprint(&base).unwrap();
        assert_eq!(
            base_fingerprint,
            "rustred-base-context-v1|parameters=2|1:d|2:m2"
        );
        assert_eq!(
            indexed_context_fingerprint(&base_fingerprint, "s|x", 10).unwrap(),
            "rustred-indexed-coefficient-context-v1|base=rustred-base-context-v1|parameters=2|1:d|2:m2|scope=3:s|x|indices=10"
        );
    }

    #[test]
    fn generated_name_lengths_are_checked_before_allocation_or_registration() {
        let overhead = INDEX_SYMBOL_PREFIX.len() + INDEX_SYMBOL_POSITION_SEPARATOR.len() + 1;
        let exact_scope_len = usize::MAX - overhead;
        assert_eq!(
            qualified_index_symbol_len(exact_scope_len, 0).unwrap(),
            usize::MAX
        );
        assert!(matches!(
            qualified_index_symbol_len(exact_scope_len + 1, 0),
            Err(IndexedAlgebraError::ResourceCountOverflow {
                resource: "indexed coefficient Symbolica name bytes",
            })
        ));
        assert!(matches!(
            preflight_qualified_index_symbols(usize::MAX, 1),
            Err(IndexedAlgebraError::ResourceCountOverflow {
                resource: "indexed coefficient aggregate Symbolica name bytes",
            })
        ));
    }
}
