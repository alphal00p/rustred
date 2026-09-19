//! Shared native Symbolica coefficient records for generated programs.
//!
//! The framing below contains only counts and record lengths. Symbolica owns
//! every algebraic byte, state import, symbol remapping and normalization.
//! Import is deliberately named `import_generated`: native Symbolica readers
//! are not hardened parsers for hostile input. Framing limits prevent accidental
//! overreads and oversized outer records, not every allocation inside a corrupt
//! native state or expression. Import can also extend process-global symbols.

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::atom::{Atom, AtomCore, AtomView};
use symbolica::coefficient::{Coefficient as NativeCoefficient, CoefficientView};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{PolyVariable, Z};
use symbolica::state::State;

use crate::algebra::{Coefficient, validate_coefficient_on_map};

use super::error::check_limit;
use super::limits::CappedWriter;
use super::native::{
    BorrowedStateMap, LENGTH_BYTES, NATIVE_ATOM_HEADER_BYTES, ensure_native_word_size,
    preflight_native_frame, read_length, write_length,
};
use super::{BinaryIoError, BinaryIoLimits};

/// A first-occurrence index into one coefficient table, not a global identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoefficientId(u32);

impl CoefficientId {
    /// Construct a representable table index. Lookup checks table membership.
    pub fn try_from_index(index: usize) -> Result<Self, BinaryIoError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| BinaryIoError::Invalid("coefficient ID exceeds u32"))
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Independent sections for a structural container to frame and persist.
#[derive(Debug)]
pub struct EncodedCoefficientTable {
    pub state: Vec<u8>,
    pub atoms: Vec<u8>,
}

/// Deduplicate identical native coefficients without storing a second atom key.
///
/// Hash collisions are resolved by complete native atom equality. IDs follow
/// encounter order, never hash-map iteration order. Coefficients from different
/// base/indexed contexts may coexist; their native variable maps are retained.
/// Interning preserves the producer's normalization; structural validation is
/// not a proof that arbitrary supplied numerator/denominator pairs are coprime.
pub struct CoefficientTableBuilder {
    limits: BinaryIoLimits,
    atoms: Vec<Atom>,
    buckets: HashMap<u64, Vec<CoefficientId>>,
    variable_maps: HashSet<Arc<Vec<PolyVariable>>>,
    encoded_atom_bytes: usize,
}

impl CoefficientTableBuilder {
    pub fn new(limits: BinaryIoLimits) -> Self {
        Self {
            limits,
            atoms: Vec::new(),
            buckets: HashMap::new(),
            variable_maps: HashSet::new(),
            encoded_atom_bytes: LENGTH_BYTES,
        }
    }

    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    pub fn intern(&mut self, coefficient: &Coefficient) -> Result<CoefficientId, BinaryIoError> {
        validate_coefficient_on_map(
            coefficient,
            coefficient.numerator.variables(),
            self.limits.exact_algebra,
        )
        .map_err(|error| BinaryIoError::Native(error.to_string()))?;
        let mut atom = Atom::new();
        // Unlike Atom::num, to_num retains the RP variant and its map for zero.
        atom.to_num(NativeCoefficient::RationalPolynomial(coefficient.clone()));
        let mut hasher = DefaultHasher::new();
        atom.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(bucket) = self.buckets.get(&hash) {
            for id in bucket {
                if self.atoms[id.index()] == atom {
                    return Ok(*id);
                }
            }
        }

        let count = checked_add(self.atoms.len(), 1)?;
        check_limit(
            "coefficient table entries",
            count,
            self.limits.max_collection_entries,
        )?;
        let id = CoefficientId::try_from_index(self.atoms.len())?;
        let frame_bytes = checked_add(atom.as_view().get_data().len(), NATIVE_ATOM_HEADER_BYTES)?;
        check_limit(
            "coefficient atom bytes",
            frame_bytes,
            self.limits.max_atom_bytes,
        )?;
        let total = checked_add(
            self.encoded_atom_bytes,
            checked_add(LENGTH_BYTES, frame_bytes)?,
        )?;
        check_limit(
            "coefficient table bytes",
            total,
            self.limits.max_total_atom_bytes,
        )?;
        self.atoms
            .try_reserve(1)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "coefficient table atoms",
                requested: count,
            })?;
        self.buckets
            .try_reserve(1)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "coefficient hash buckets",
                requested: count,
            })?;
        self.variable_maps
            .try_reserve(1)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "coefficient variable maps",
                requested: self.variable_maps.len().saturating_add(1),
            })?;
        let bucket = self.buckets.entry(hash).or_default();
        bucket
            .try_reserve(1)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "coefficient hash collision bucket",
                requested: bucket.len().saturating_add(1),
            })?;
        bucket.push(id);
        self.atoms.push(atom);
        self.variable_maps
            .insert(coefficient.get_variables().clone());
        self.encoded_atom_bytes = total;
        Ok(id)
    }

    pub fn finish(self) -> Result<EncodedCoefficientTable, BinaryIoError> {
        ensure_native_word_size()?;
        check_limit(
            "coefficient table bytes",
            self.encoded_atom_bytes,
            self.limits.max_total_atom_bytes,
        )?;
        // Construct every RP atom before exporting: construction registers its
        // variable-list resource in Symbolica's state. A preceding export would
        // omit maps first encountered in a later coefficient.
        let mut symbols = Atom::new().get_all_symbols(true);
        // get_all_symbols on an RP atom decodes its complete sparse payload.
        // The retained variable maps provide this metadata without doing that
        // work once more for every unique coefficient.
        for variables in &self.variable_maps {
            for variable in variables.iter() {
                match variable {
                    PolyVariable::Symbol(symbol) => {
                        symbols.insert(*symbol);
                    }
                    PolyVariable::Function(symbol, atom) => {
                        symbols.insert(*symbol);
                        symbols.extend(atom.get_all_symbols(true));
                    }
                    PolyVariable::Power(atom) => {
                        symbols.extend(atom.get_all_symbols(true));
                    }
                    PolyVariable::Temporary(_) => {}
                }
            }
        }
        let mut state = CappedWriter::new("Symbolica state bytes", self.limits.max_state_bytes);
        State::export_partial(&mut state, symbols)
            .map_err(|error| BinaryIoError::Native(error.to_string()))?;
        let state = state.finish();
        check_limit(
            "coefficient program bytes",
            checked_add(state.len(), self.encoded_atom_bytes)?,
            self.limits.max_program_bytes,
        )?;
        let mut atoms =
            CappedWriter::new("coefficient table bytes", self.limits.max_total_atom_bytes);
        write_length(&mut atoms, self.atoms.len())?;
        for atom in &self.atoms {
            let frame_bytes =
                checked_add(atom.as_view().get_data().len(), NATIVE_ATOM_HEADER_BYTES)?;
            write_length(&mut atoms, frame_bytes)?;
            let written =
                bincode::encode_into_std_write(atom, &mut atoms, bincode::config::standard())
                    .map_err(|error| BinaryIoError::Native(error.to_string()))?;
            if written != frame_bytes {
                return Err(BinaryIoError::Invalid(
                    "native atom encoding length changed",
                ));
            }
        }
        Ok(EncodedCoefficientTable {
            state,
            atoms: atoms.finish(),
        })
    }
}

/// Once-decoded coefficients owned by a loaded program.
pub struct DecodedCoefficientTable {
    coefficients: Vec<Coefficient>,
}

impl DecodedCoefficientTable {
    /// Load records written by the matching native Symbolica generation stack.
    ///
    /// Coefficients must already have the normalized, coprime representation
    /// generated by Symbolica. This fast path checks sparse structural validity
    /// without repeating GCD computations. It does not prove source identities
    /// or closure. Callers must bind each
    /// coefficient use to its appropriate family/indexed variable map. All
    /// framing is preflighted before mutating Symbolica state; native state and
    /// algebra payloads still require trusted generated provenance.
    pub fn import_generated(
        state: &[u8],
        atoms: &[u8],
        limits: BinaryIoLimits,
    ) -> Result<Self, BinaryIoError> {
        Self::import_impl(state, atoms, limits, false)
    }

    /// Import generated native records and normalize each unique coefficient
    /// once with Symbolica. This is useful for auditing normalization cost or
    /// generated producers that do not guarantee coprimality. It does not make
    /// the underlying native decoder safe for hostile input.
    pub fn import_generated_normalized(
        state: &[u8],
        atoms: &[u8],
        limits: BinaryIoLimits,
    ) -> Result<Self, BinaryIoError> {
        Self::import_impl(state, atoms, limits, true)
    }

    fn import_impl(
        state: &[u8],
        atoms: &[u8],
        limits: BinaryIoLimits,
        normalize_unique: bool,
    ) -> Result<Self, BinaryIoError> {
        ensure_native_word_size()?;
        check_limit("Symbolica state bytes", state.len(), limits.max_state_bytes)?;
        check_limit(
            "coefficient table bytes",
            atoms.len(),
            limits.max_total_atom_bytes,
        )?;
        check_limit(
            "coefficient program bytes",
            checked_add(state.len(), atoms.len())?,
            limits.max_program_bytes,
        )?;
        let mut source = atoms;
        let count = read_length(&mut source)?;
        check_limit(
            "coefficient table entries",
            count,
            limits.max_collection_entries,
        )?;
        if count > 0 {
            CoefficientId::try_from_index(count - 1)?;
        }
        for _ in 0..count {
            preflight_native_frame(take_frame(&mut source, limits.max_atom_bytes)?)?;
        }
        if !source.is_empty() {
            return Err(BinaryIoError::Invalid("trailing coefficient table bytes"));
        }

        let state_map = catch_unwind(AssertUnwindSafe(|| {
            let mut source = state;
            let map = State::import(&mut source, None)
                .map_err(|error| BinaryIoError::Native(error.to_string()))?;
            if !source.is_empty() {
                return Err(BinaryIoError::Invalid("trailing Symbolica state bytes"));
            }
            Ok(map)
        }))
        .map_err(|_| BinaryIoError::Native("native Symbolica state import panicked".into()))??;
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(count)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "decoded coefficients",
                requested: count,
            })?;
        let mut source = &atoms[LENGTH_BYTES..];
        for _ in 0..count {
            let frame = take_frame(&mut source, limits.max_atom_bytes)?;
            let coefficient = catch_unwind(AssertUnwindSafe(|| {
                let (atom, consumed): (Atom, usize) = bincode::decode_from_slice_with_context(
                    frame,
                    bincode::config::standard(),
                    BorrowedStateMap(&state_map),
                )
                .map_err(|error| BinaryIoError::Native(error.to_string()))?;
                if consumed != frame.len() {
                    return Err(BinaryIoError::Invalid("trailing native atom bytes"));
                }
                let AtomView::Num(number) = atom.as_view() else {
                    return Err(BinaryIoError::Invalid("coefficient atom is not a number"));
                };
                let CoefficientView::RationalPolynomial(packed) = number.get_coeff_view() else {
                    return Err(BinaryIoError::Invalid(
                        "coefficient atom is not a rational polynomial",
                    ));
                };
                let coefficient = packed.deserialize();
                validate_coefficient_on_map(
                    &coefficient,
                    coefficient.numerator.variables(),
                    limits.exact_algebra,
                )
                .map_err(|error| BinaryIoError::Native(error.to_string()))?;
                if normalize_unique {
                    // Optional native normalization is per unique record,
                    // never on every subsequent ID use.
                    let normalized = Coefficient::from_num_den(
                        coefficient.numerator,
                        coefficient.denominator,
                        &Z,
                        true,
                    );
                    // Exact division can increase the sparse term count, e.g.
                    // (x^m-1)/(x-1), despite lowering the polynomial degree.
                    validate_coefficient_on_map(
                        &normalized,
                        normalized.numerator.variables(),
                        limits.exact_algebra,
                    )
                    .map_err(|error| BinaryIoError::Native(error.to_string()))?;
                    Ok(normalized)
                } else {
                    Ok(coefficient)
                }
            }))
            .map_err(|_| BinaryIoError::Native("native coefficient import panicked".into()))??;
            coefficients.push(coefficient);
        }
        Ok(Self { coefficients })
    }

    pub fn coefficient(&self, id: CoefficientId) -> Result<&Coefficient, BinaryIoError> {
        self.coefficients
            .get(id.index())
            .ok_or(BinaryIoError::Invalid("coefficient ID is outside table"))
    }

    pub fn len(&self) -> usize {
        self.coefficients.len()
    }

    pub fn is_empty(&self) -> bool {
        self.coefficients.is_empty()
    }
}

fn checked_add(left: usize, right: usize) -> Result<usize, BinaryIoError> {
    left.checked_add(right)
        .ok_or(BinaryIoError::Invalid("coefficient byte count overflow"))
}

fn take_frame<'a>(source: &mut &'a [u8], limit: usize) -> Result<&'a [u8], BinaryIoError> {
    let length = read_length(source)?;
    check_limit("coefficient atom bytes", length, limit)?;
    let frame = source
        .get(..length)
        .ok_or(BinaryIoError::Invalid("truncated coefficient atom"))?;
    *source = &source[length..];
    Ok(frame)
}

#[cfg(test)]
mod tests;
