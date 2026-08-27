/// Parse a base-two byte quantity used by application resource policies.
///
/// The caller decides whether zero is accepted for the particular field.
pub(crate) fn parse_memory_bytes(value: &str) -> Option<u64> {
    let (digits, multiplier) = [
        ("TiB", 1_u64 << 40),
        ("GiB", 1_u64 << 30),
        ("MiB", 1_u64 << 20),
        ("KiB", 1_u64 << 10),
        ("B", 1_u64),
    ]
    .into_iter()
    .find_map(|(suffix, multiplier)| {
        value
            .strip_suffix(suffix)
            .map(|digits| (digits, multiplier))
    })?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u64>().ok()?.checked_mul(multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bounded_binary_byte_quantities() {
        assert_eq!(parse_memory_bytes("0B"), Some(0));
        assert_eq!(parse_memory_bytes("2KiB"), Some(2 * 1024));
        assert_eq!(parse_memory_bytes("3MiB"), Some(3 * 1024 * 1024));
        assert_eq!(parse_memory_bytes("1TiB"), Some(1_u64 << 40));
        for invalid in ["", "1", "-1B", "+1B", "1KB", "1.5GiB"] {
            assert_eq!(parse_memory_bytes(invalid), None, "accepted {invalid:?}");
        }
    }

    #[test]
    fn rejects_overflow() {
        assert_eq!(parse_memory_bytes("18446744073709551615TiB"), None);
    }
}
