//! Shared authentication for process-global Symbolica scalar symbols.

use symbolica::{atom::UserData, prelude::Symbol};

/// Whether `symbol` is exactly the plain scalar symbol RustRed requested.
///
/// Symbolica's registry is process-global and returns an existing symbol when
/// its name is already registered. Every RustRed algebra boundary must inspect
/// that returned symbol before using it as a polynomial variable.
pub(crate) fn is_exact_plain_symbol(symbol: Symbol, expected_name: &str) -> bool {
    symbol.get_name() == expected_name
        && symbol.get_wildcard_level() == 0
        // `has_attributes` includes Symbolica tags.
        && !symbol.has_attributes()
        && symbol.is_exportable()
        && symbol.get_aliases().is_empty()
        && matches!(symbol.get_data(), UserData::None)
}
