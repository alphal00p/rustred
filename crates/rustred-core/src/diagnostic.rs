//! Shared bounded rendering for observational diagnostics, never proof data.

use std::fmt::{self, Write};

pub(crate) struct BoundedText {
    pub(crate) text: String,
    remaining: usize,
    pub(crate) truncated: bool,
}

impl BoundedText {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            text: String::new(),
            remaining: limit,
            truncated: false,
        }
    }
}

impl Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() <= self.remaining {
            self.text.push_str(value);
            self.remaining -= value.len();
            return Ok(());
        }
        let mut end = self.remaining;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        self.remaining = 0;
        self.truncated = true;
        // Stop native formatting at the limit instead of allocating an
        // unbounded expression string and truncating afterwards.
        Err(fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_limit_and_zero_limit_are_distinguished_from_truncation() {
        let mut empty = BoundedText::new(0);
        assert!(empty.write_str("").is_ok());
        assert!(!empty.truncated);
        assert!(empty.write_str("x").is_err());
        assert!(empty.truncated);
        assert!(empty.text.is_empty());

        let mut exact = BoundedText::new(2);
        assert!(exact.write_str("é").is_ok());
        assert!(!exact.truncated);
        assert!(exact.write_str("x").is_err());
        assert!(exact.truncated);
        assert_eq!(exact.text, "é");
    }
}
