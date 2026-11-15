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
use std::io::Write;
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
    BorrowedStateMap, LENGTH_BYTES, NATIVE_ATOM_HEADER_BYTES, preflight_native_frame,
    preflight_state_export, read_length, write_length,
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
    /// vk5l WP6 ([`Self::with_spill`]): unique atoms kept as their encoded
    /// frames in a scratch file instead of `atoms`.
    spill: Option<AtomSpill>,
}

/// The unique atoms of a spilled builder: each atom's native encoding (the
/// bytes [`CoefficientTableBuilder::finish`] writes for it) appended to a
/// scratch file, with its offset and length. Equality on a hash hit compares
/// the encodings, which for a native atom is its complete data.
struct AtomSpill {
    file: std::fs::File,
    frames: Vec<(u64, u32)>,
    end: u64,
}

impl AtomSpill {
    fn read(&self, index: usize, out: &mut Vec<u8>) -> Result<(), BinaryIoError> {
        use std::os::unix::fs::FileExt;
        let (offset, len) = self.frames[index];
        out.clear();
        out.resize(len as usize, 0);
        self.file
            .read_exact_at(out, offset)
            .map_err(super::native_io_error)
    }
}

impl CoefficientTableBuilder {
    pub fn new(limits: BinaryIoLimits) -> Self {
        Self {
            limits,
            atoms: Vec::new(),
            buckets: HashMap::new(),
            variable_maps: HashSet::new(),
            encoded_atom_bytes: LENGTH_BYTES,
            spill: None,
        }
    }

    /// vk5l WP6: a builder whose unique atoms live in `scratch` (an empty,
    /// readable and writable file the caller removes afterwards), so its
    /// memory is the hash index and the variable maps only, not the
    /// coefficients. Same ids and the same encoded table as [`Self::new`].
    pub fn with_spill(limits: BinaryIoLimits, scratch: std::fs::File) -> Self {
        let mut builder = Self::new(limits);
        builder.spill = Some(AtomSpill {
            file: scratch,
            frames: Vec::new(),
            end: 0,
        });
        builder
    }

    pub fn len(&self) -> usize {
        match &self.spill {
            Some(spill) => spill.frames.len(),
            None => self.atoms.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn intern(&mut self, coefficient: &Coefficient) -> Result<CoefficientId, BinaryIoError> {
        super::resource_checkpoint()?;
        validate_coefficient_on_map(
            coefficient,
            coefficient.numerator.variables(),
            self.limits.exact_algebra,
        )
        .map_err(super::native_error)?;
        let mut atom = Atom::new();
        // Unlike Atom::num, to_num retains the RP variant and its map for zero.
        atom.to_num(NativeCoefficient::RationalPolynomial(coefficient.clone()));
        let mut hasher = DefaultHasher::new();
        atom.hash(&mut hasher);
        let hash = hasher.finish();
        // Spilled: the atom's encoding is both the equality key and the bytes
        // appended to the scratch file.
        let encoded = match &self.spill {
            Some(_) => Some(
                bincode::encode_to_vec(&atom, bincode::config::standard())
                    .map_err(super::native_encode_error)?,
            ),
            None => None,
        };
        if let Some(bucket) = self.buckets.get(&hash) {
            match (&self.spill, &encoded) {
                (Some(spill), Some(encoded)) => {
                    let mut stored = Vec::new();
                    for id in bucket {
                        spill.read(id.index(), &mut stored)?;
                        if &stored == encoded {
                            return Ok(*id);
                        }
                    }
                }
                _ => {
                    for id in bucket {
                        if self.atoms[id.index()] == atom {
                            return Ok(*id);
                        }
                    }
                }
            }
        }

        let count = checked_add(self.len(), 1)?;
        self.limits.check_count(
            "coefficient table entries",
            count,
            self.limits.max_collection_entries,
        )?;
        let id = CoefficientId::try_from_index(self.len())?;
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
        super::resource_array::<Atom>(1)?;
        super::resource_array::<(u64, Vec<CoefficientId>)>(1)?;
        super::resource_array::<Arc<Vec<PolyVariable>>>(1)?;
        super::resource_array::<CoefficientId>(1)?;
        if self.spill.is_none() {
            self.atoms
                .try_reserve(1)
                .map_err(|_| BinaryIoError::Allocation {
                    resource: "coefficient table atoms",
                    requested: count,
                })?;
        }
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
        match (&mut self.spill, encoded) {
            (Some(spill), Some(encoded)) => {
                use std::os::unix::fs::FileExt;
                if encoded.len() != frame_bytes {
                    return Err(BinaryIoError::Invalid(
                        "native atom encoding length changed",
                    ));
                }
                spill
                    .file
                    .write_all_at(&encoded, spill.end)
                    .map_err(super::native_io_error)?;
                let len = u32::try_from(encoded.len())
                    .map_err(|_| BinaryIoError::Invalid("spilled atom exceeds u32 bytes"))?;
                spill.frames.push((spill.end, len));
                spill.end += u64::from(len);
            }
            _ => self.atoms.push(atom),
        }
        self.variable_maps
            .insert(coefficient.get_variables().clone());
        self.encoded_atom_bytes = total;
        Ok(id)
    }

    /// Complete encoded atom-section size, including count and frame lengths.
    pub fn encoded_atom_bytes(&self) -> usize {
        self.encoded_atom_bytes
    }

    /// Compact unique atom payload retained by the interner, excluding framing.
    /// This is a memory floor, not a bound on allocator or native scratch RSS.
    pub fn retained_atom_payload_bytes(&self) -> usize {
        self.encoded_atom_bytes
            - LENGTH_BYTES
            - self.len() * (LENGTH_BYTES + NATIVE_ATOM_HEADER_BYTES)
    }

    /// Known interner container storage, excluding allocator overhead, hash
    /// control bytes, variable-map payloads and native scratch. This is a lower
    /// bound only; interning remains proportional to all unique coefficients.
    pub fn retained_index_lower_bound_bytes(&self) -> Result<usize, BinaryIoError> {
        let overflow = || BinaryIoError::Invalid("coefficient interner index size overflow");
        let mut bytes = self
            .atoms
            .capacity()
            .checked_mul(std::mem::size_of::<Atom>())
            .and_then(|n| {
                n.checked_add(
                    self.buckets
                        .capacity()
                        .checked_mul(std::mem::size_of::<(u64, Vec<CoefficientId>)>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.variable_maps
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<Vec<PolyVariable>>>())?,
                )
            })
            .ok_or_else(overflow)?;
        for bucket in self.buckets.values() {
            bytes = bytes
                .checked_add(
                    bucket
                        .capacity()
                        .checked_mul(std::mem::size_of::<CoefficientId>())
                        .ok_or_else(overflow)?,
                )
                .ok_or_else(overflow)?;
        }
        Ok(bytes)
    }

    pub(crate) fn encoded_state(&self) -> Result<Vec<u8>, BinaryIoError> {
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
            super::resource_checkpoint()?;
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
        State::export_partial(&mut state, symbols).map_err(super::native_io_error)?;
        let state = state.finish();
        check_limit(
            "coefficient program bytes",
            checked_add(state.len(), self.encoded_atom_bytes)?,
            self.limits.max_program_bytes,
        )?;
        Ok(state)
    }

    pub(crate) fn write_atoms(self, writer: &mut impl Write) -> Result<(), BinaryIoError> {
        let count = self.len();
        // No more equality lookups occur after all IDs have been assigned.
        // Drop indexes before writing; consumed atoms are then freed one by one.
        drop(self.buckets);
        drop(self.variable_maps);
        write_length(writer, count)?;
        if let Some(spill) = &self.spill {
            let mut frame = Vec::new();
            for index in 0..spill.frames.len() {
                super::resource_checkpoint()?;
                spill.read(index, &mut frame)?;
                write_length(writer, frame.len())?;
                writer.write_all(&frame).map_err(super::native_io_error)?;
            }
            return Ok(());
        }
        for atom in self.atoms {
            super::resource_checkpoint()?;
            let frame_bytes =
                checked_add(atom.as_view().get_data().len(), NATIVE_ATOM_HEADER_BYTES)?;
            write_length(writer, frame_bytes)?;
            let written =
                bincode::encode_into_std_write(&atom, writer, bincode::config::standard())
                    .map_err(super::native_encode_error)?;
            if written != frame_bytes {
                return Err(BinaryIoError::Invalid(
                    "native atom encoding length changed",
                ));
            }
        }
        Ok(())
    }

    /// Stream the two length-framed native sections in their existing order:
    /// Symbolica state, then one shared atom table. Only the bounded state export
    /// is buffered; no serialized atom-table copy is retained. All atoms must be
    /// interned first so the state contains every encountered variable map.
    /// A failure leaves partial output; callers must publish only on success.
    pub fn finish_into(self, writer: &mut impl Write) -> Result<(), BinaryIoError> {
        let state = self.encoded_state()?;
        let atom_bytes = self.encoded_atom_bytes;
        write_length(writer, state.len())?;
        super::resource_checkpoint()?;
        writer.write_all(&state).map_err(super::native_io_error)?;
        drop(state);
        write_length(writer, atom_bytes)?;
        self.write_atoms(writer)
    }

    pub fn finish(self) -> Result<EncodedCoefficientTable, BinaryIoError> {
        let state = self.encoded_state()?;
        let mut atoms =
            CappedWriter::new("coefficient table bytes", self.limits.max_total_atom_bytes);
        self.write_atoms(&mut atoms)?;
        Ok(EncodedCoefficientTable {
            state,
            atoms: atoms.finish(),
        })
    }
}

/// Once-decoded coefficients owned by a loaded program.
pub struct DecodedCoefficientTable {
    coefficients: Vec<Coefficient>,
    /// coef-form prototype (`RUSTRED_COEF_FORM_LAZY=1`): a factored section
    /// kept unexpanded; each coefficient is multiplied back on first use.
    lazy: Option<LazyFactored>,
}

struct LazyFactored {
    section: super::factored::FactoredSection,
    cells: Vec<std::sync::OnceLock<Result<Coefficient, BinaryIoError>>>,
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
        if super::factored::is_factored_table(atoms) {
            // coef-form: factored section (format FACTORED_TABLE_VERSION);
            // decodes to the identical expanded coefficients
            let state_map = import_state(state)?;
            if super::factored::factored_lazy_expansion() {
                let section = catch_unwind(AssertUnwindSafe(|| {
                    super::factored::parse_factored_table(atoms, &state_map, limits)
                }))
                .map_err(|_| {
                    BinaryIoError::Native("factored coefficient import panicked".into())
                })??;
                let cells = (0..section.len())
                    .map(|_| std::sync::OnceLock::new())
                    .collect();
                return Ok(Self {
                    coefficients: Vec::new(),
                    lazy: Some(LazyFactored { section, cells }),
                });
            }
            let coefficients = catch_unwind(AssertUnwindSafe(|| {
                super::factored::decode_factored_table(atoms, &state_map, limits)
            }))
            .map_err(|_| BinaryIoError::Native("factored coefficient import panicked".into()))??;
            return Ok(Self {
                coefficients,
                lazy: None,
            });
        }
        let mut source = atoms;
        let count = read_length(&mut source)?;
        limits.check_count(
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

        let state_map = import_state(state)?;
        let mut coefficients = Vec::new();
        super::resource_array::<Coefficient>(count)?;
        coefficients
            .try_reserve_exact(count)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "decoded coefficients",
                requested: count,
            })?;
        let mut source = &atoms[LENGTH_BYTES..];
        for _ in 0..count {
            let frame = take_frame(&mut source, limits.max_atom_bytes)?;
            coefficients.push(decode_generated_frame(
                frame,
                &state_map,
                limits,
                normalize_unique,
            )?);
        }
        Ok(Self {
            coefficients,
            lazy: None,
        })
    }

    pub fn coefficient(&self, id: CoefficientId) -> Result<&Coefficient, BinaryIoError> {
        if let Some(lazy) = &self.lazy {
            let cell = lazy
                .cells
                .get(id.index())
                .ok_or(BinaryIoError::Invalid("coefficient ID is outside table"))?;
            return cell
                .get_or_init(|| {
                    catch_unwind(AssertUnwindSafe(|| lazy.section.expand(id.index())))
                        .unwrap_or_else(|_| {
                            Err(BinaryIoError::Native(
                                "factored coefficient expansion panicked".into(),
                            ))
                        })
                })
                .as_ref()
                .map_err(Clone::clone);
        }
        self.coefficients
            .get(id.index())
            .ok_or(BinaryIoError::Invalid("coefficient ID is outside table"))
    }

    pub fn len(&self) -> usize {
        self.lazy
            .as_ref()
            .map_or(self.coefficients.len(), |lazy| lazy.cells.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Import a generated Symbolica state section (preflighted, panics caught).
pub(super) fn import_state(state: &[u8]) -> Result<symbolica::state::StateMap, BinaryIoError> {
    preflight_state_export(state)?;
    super::resource_allocation(state.len())?;
    catch_unwind(AssertUnwindSafe(|| {
        let mut source = super::structure::ResourceReader::new(state);
        let map = State::import(&mut source, None).map_err(super::native_io_error)?;
        if source.remaining() != 0 {
            return Err(BinaryIoError::Invalid("trailing Symbolica state bytes"));
        }
        Ok(map)
    }))
    .map_err(|_| BinaryIoError::Native("native Symbolica state import panicked".into()))?
}

pub(super) fn decode_generated_frame(
    frame: &[u8],
    state_map: &symbolica::state::StateMap,
    limits: BinaryIoLimits,
    normalize_unique: bool,
) -> Result<Coefficient, BinaryIoError> {
    super::resource_allocation(frame.len())?;
    catch_unwind(AssertUnwindSafe(|| {
        let (atom, consumed): (Atom, usize) = bincode::decode_from_slice_with_context(
            frame,
            bincode::config::standard(),
            BorrowedStateMap(state_map),
        )
        .map_err(super::native_error)?;
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
        super::resource_checkpoint()?;
        validate_coefficient_on_map(
            &coefficient,
            coefficient.numerator.variables(),
            limits.exact_algebra,
        )
        .map_err(|error| BinaryIoError::Native(error.to_string()))?;
        if normalize_unique {
            // Optional native normalization is per unique record,
            // never on every subsequent ID use.
            let normalized =
                Coefficient::from_num_den(coefficient.numerator, coefficient.denominator, &Z, true);
            // Exact division can increase the sparse term count, e.g.
            // (x^m-1)/(x-1), despite lowering the polynomial degree.
            validate_coefficient_on_map(
                &normalized,
                normalized.numerator.variables(),
                limits.exact_algebra,
            )
            .map_err(super::native_error)?;
            Ok(normalized)
        } else {
            Ok(coefficient)
        }
    }))
    .map_err(|_| BinaryIoError::Native("native coefficient import panicked".into()))?
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
