//! Architecture-aware resource defaults. This never converts mathematical
//! values or silently truncates a caller's requested budget.

/// Preserve native defaults, capped by the target's addressable counter range.
pub(crate) const fn default_usize_limit(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_saturate_instead_of_truncating_on_narrow_targets() {
        assert_eq!(default_usize_limit(37), 37);
        assert_eq!(default_usize_limit(u64::MAX), usize::MAX);
        if usize::BITS == 32 {
            assert_eq!(default_usize_limit(16_000_000_000), usize::MAX);
        } else {
            assert_eq!(default_usize_limit(16_000_000_000) as u64, 16_000_000_000);
        }
    }
}
