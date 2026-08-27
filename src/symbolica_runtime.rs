//! Narrow, reusable boundary for Symbolica-owned representation details.
//!
//! Higher layers may serialize RustRed results without depending directly on
//! Symbolica or importing its extension traits.  Algebra remains owned by the
//! existing RustRed/Symbolica domain types; this module only exposes runtime
//! metadata and bounded representation observations needed at an application
//! serialization boundary.

use symbolica::LicenseManager;
use symbolica::prelude::{Atom, AtomCore, Integer};

/// Return the version of the vendored Symbolica runtime used by RustRed.
pub fn symbolica_runtime_version() -> &'static str {
    LicenseManager::get_version()
}

/// Render one Symbolica atom using Symbolica's canonical string format.
pub fn canonical_symbolica_atom(atom: &Atom) -> String {
    atom.to_canonical_string()
}

/// Return the packed native byte size reported by Symbolica for one atom.
///
/// This is a representation census, not a bound on expansion, conversion, or
/// rendered output.  Callers must apply their own checked resource policy.
pub fn symbolica_atom_packed_byte_size(atom: &Atom) -> usize {
    atom.as_view().get_byte_size()
}

/// Return the exact significant-bit count for a Symbolica integer when it is
/// representable by this target's `usize`.
pub fn symbolica_integer_significant_bits(value: &Integer) -> Option<usize> {
    let bits = match value {
        Integer::Single(value) => u64::BITS - value.unsigned_abs().leading_zeros(),
        Integer::Double(value) => u128::BITS - value.unsigned_abs().leading_zeros(),
        Integer::Large(value) => return usize::try_from(value.significant_bits()).ok(),
    };
    usize::try_from(bits).ok()
}

/// Return the in-memory enum size charged for each retained Symbolica integer.
pub const fn symbolica_integer_structural_byte_size() -> usize {
    std::mem::size_of::<Integer>()
}
