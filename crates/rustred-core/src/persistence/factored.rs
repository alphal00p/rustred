//! coef-form prototype: a factored encoding of a coefficient-table section.
//!
//! The section written by [`super::CoefficientTableBuilder`] stores every
//! coefficient as one native Symbolica atom: numerator and denominator fully
//! expanded, no sharing below whole-coefficient equality. This encoding keeps
//! the same Symbolica state section and replaces the atom section by
//!
//! - [`FACTORED_TABLE_MAGIC`] (never a valid legacy entry count) and
//!   [`FACTORED_TABLE_VERSION`];
//! - one native template atom per distinct variable map (the zero coefficient
//!   on that map: Symbolica owns symbol identity);
//! - a dictionary of polynomials (the irreducible factors Symbolica's
//!   `factor` returns; a polynomial above [`FACTOR_TERM_CAP`] terms, or one
//!   whose factors do not multiply back exactly, is one entry as a whole);
//! - per coefficient: its map, then numerator and denominator as
//!   integer content times a product of dictionary entries with exponents.
//!
//! The SHARED form ([`encode_shared_table`], `p-import-table --coef-form
//! shared`) is the same container with factoring switched off: every
//! non-constant numerator and denominator is one whole dictionary entry,
//! numbered in encounter order (numerator before denominator, coefficient by
//! coefficient), a constant is the part's integer content. It shares the
//! repeated denominators of a table and drops the native atom framing at no
//! factoring cost; the bytes depend only on the coefficient table. Readers
//! cannot tell the two forms apart (no new version: a whole entry with
//! exponent 1 has always been valid).
//!
//! Decoding multiplies every part back. The encoder checks, per polynomial,
//! that the product is the stored polynomial itself, so a decoded table is
//! IDENTICAL (same normalised numerator and denominator, same variable map)
//! to the table the section replaced: the engine sees no difference.
//! Integers are zigzag varints up to 2^61, else their decimal text.

use std::collections::HashMap;
use std::sync::Arc;

use symbolica::atom::Atom;
use symbolica::coefficient::Coefficient as NativeCoefficient;
use symbolica::poly::factor::Factorize;
use symbolica::prelude::{Integer, PolyVariable};

use crate::algebra::{Coefficient, CoefficientPolynomial, validate_coefficient_on_map};

use super::native::NATIVE_ATOM_HEADER_BYTES;
use super::{BinaryIoError, BinaryIoLimits};

/// First bytes of a factored coefficient section. As a little-endian u64
/// legacy entry count it exceeds every addressable table, so the two
/// encodings cannot be confused.
pub const FACTORED_TABLE_MAGIC: [u8; 8] = *b"RRFCTAB\xff";
/// Hand-bumped format version of the factored coefficient section.
pub const FACTORED_TABLE_VERSION: u32 = 1;
/// Polynomials above this many terms are stored whole (one dictionary entry).
pub const FACTOR_TERM_CAP: usize = 4000;

/// Whether `atoms` is a factored coefficient section.
pub fn is_factored_table(atoms: &[u8]) -> bool {
    atoms.len() >= 8 && atoms[..8] == FACTORED_TABLE_MAGIC
}

// ----- byte helpers -----

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn byte(&mut self) -> Result<u8, BinaryIoError> {
        let b = *self.bytes.get(self.at).ok_or(BinaryIoError::Invalid(
            "truncated factored coefficient table",
        ))?;
        self.at += 1;
        Ok(b)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], BinaryIoError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(BinaryIoError::Invalid(
                "truncated factored coefficient table",
            ))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn varint(&mut self) -> Result<u64, BinaryIoError> {
        let (mut v, mut shift) = (0u64, 0u32);
        loop {
            let b = self.byte()?;
            if shift >= 64 {
                return Err(BinaryIoError::Invalid("factored table varint overflow"));
            }
            v |= u64::from(b & 0x7f) << shift;
            if b < 0x80 {
                return Ok(v);
            }
            shift += 7;
        }
    }
    fn count(
        &mut self,
        limits: BinaryIoLimits,
        what: &'static str,
    ) -> Result<usize, BinaryIoError> {
        let n = usize::try_from(self.varint()?)
            .map_err(|_| BinaryIoError::Invalid("factored table count exceeds usize"))?;
        limits.check_count(what, n, limits.max_collection_entries)?;
        // every counted record takes at least one byte
        if n > self.bytes.len() - self.at {
            return Err(BinaryIoError::Invalid(
                "factored table count exceeds remaining bytes",
            ));
        }
        Ok(n)
    }
}

fn put_integer(out: &mut Vec<u8>, c: &Integer) {
    match c {
        Integer::Single(i) if i.unsigned_abs() < (1u64 << 61) => {
            let z = ((*i << 1) ^ (*i >> 63)) as u64;
            varint(out, z << 1);
        }
        other => {
            let s = other.to_string();
            varint(out, ((s.len() as u64) << 1) | 1);
            out.extend_from_slice(s.as_bytes());
        }
    }
}

fn get_integer(src: &mut Cursor<'_>) -> Result<Integer, BinaryIoError> {
    let tag = src.varint()?;
    if tag & 1 == 0 {
        let z = tag >> 1;
        Ok(Integer::from(((z >> 1) as i64) ^ -((z & 1) as i64)))
    } else {
        let n = usize::try_from(tag >> 1)
            .map_err(|_| BinaryIoError::Invalid("factored table integer length"))?;
        let text = std::str::from_utf8(src.take(n)?)
            .map_err(|_| BinaryIoError::Invalid("factored table integer text"))?;
        text.parse::<Integer>()
            .map_err(|_| BinaryIoError::Invalid("factored table integer text"))
    }
}

/// nterms, then per term: coefficient, number of nonzero exponents, (var, exp).
fn put_poly(out: &mut Vec<u8>, p: &CoefficientPolynomial) {
    varint(out, p.nterms() as u64);
    for i in 0..p.nterms() {
        put_integer(out, &p.coefficients[i]);
        let e = p.exponents(i);
        varint(out, e.iter().filter(|&&x| x != 0).count() as u64);
        for (v, &x) in e.iter().enumerate() {
            if x != 0 {
                varint(out, v as u64);
                varint(out, u64::from(x));
            }
        }
    }
}

fn get_poly(
    src: &mut Cursor<'_>,
    template: &CoefficientPolynomial,
    limits: BinaryIoLimits,
) -> Result<CoefficientPolynomial, BinaryIoError> {
    let n = src.count(limits, "factored table polynomial terms")?;
    let nvars = template.nvars();
    let mut p = template.zero_with_capacity(n);
    let mut exps = vec![0u16; nvars];
    for _ in 0..n {
        let c = get_integer(src)?;
        exps.iter_mut().for_each(|x| *x = 0);
        let nz = src.varint()?;
        for _ in 0..nz {
            let v = usize::try_from(src.varint()?)
                .ok()
                .filter(|&v| v < nvars)
                .ok_or(BinaryIoError::Invalid("factored table variable index"))?;
            exps[v] = u16::try_from(src.varint()?)
                .map_err(|_| BinaryIoError::Invalid("factored table exponent"))?;
        }
        p.append_monomial(c, &exps);
    }
    Ok(p)
}

// ----- encoder -----

struct Part {
    content: Integer,
    factors: Vec<(u32, u32)>,
}

struct Dictionary {
    entries: Vec<(u32, CoefficientPolynomial)>,
    index: HashMap<(u32, CoefficientPolynomial), u32>,
}

impl Dictionary {
    fn id(&mut self, map: u32, p: CoefficientPolynomial) -> u32 {
        if let Some(&i) = self.index.get(&(map, p.clone())) {
            return i;
        }
        let i = self.entries.len() as u32;
        self.entries.push((map, p.clone()));
        self.index.insert((map, p), i);
        i
    }
}

fn factor_part(p: &CoefficientPolynomial, map: u32, dict: &mut Dictionary) -> Part {
    if p.is_constant() {
        return Part {
            content: if p.is_zero() {
                Integer::zero()
            } else {
                p.lcoeff()
            },
            factors: Vec::new(),
        };
    }
    let whole = |dict: &mut Dictionary| Part {
        content: Integer::one(),
        factors: vec![(dict.id(map, p.clone()), 1)],
    };
    if p.nterms() > FACTOR_TERM_CAP {
        return whole(dict);
    }
    let mut content = Integer::one();
    let mut factors = Vec::new();
    let mut check = p.one();
    for (f, e) in p.factor() {
        check = check * &f.pow(e);
        if f.is_constant() {
            for _ in 0..e {
                content = &content * &f.lcoeff();
            }
        } else {
            factors.push((f, e as u32));
        }
    }
    if check != *p {
        return whole(dict);
    }
    let mut part = Part {
        content,
        factors: factors
            .into_iter()
            .map(|(f, e)| (dict.id(map, f), e))
            .collect(),
    };
    part.factors.sort_unstable();
    // the decoder multiplies in this order: verify that order exactly
    if multiply(&part, |id| &dict.entries[id as usize].1, p) != *p {
        return whole(dict);
    }
    part
}

fn multiply<'a>(
    part: &Part,
    entry: impl Fn(u32) -> &'a CoefficientPolynomial,
    template: &CoefficientPolynomial,
) -> CoefficientPolynomial {
    let mut acc = template.one().mul_coeff(part.content.clone());
    for &(id, e) in &part.factors {
        acc = acc * &entry(id).pow(e as usize);
    }
    acc
}

fn put_part(out: &mut Vec<u8>, part: &Part) {
    put_integer(out, &part.content);
    varint(out, part.factors.len() as u64);
    for &(f, e) in &part.factors {
        varint(out, u64::from(f));
        varint(out, u64::from(e));
    }
}

/// The factored coefficient section of `coefficients` (in table order; ids
/// unchanged). The Symbolica state section of the table it replaces must stay
/// with it: it carries every symbol of the template atoms.
/// The variable map index of every coefficient (first occurrence order) and
/// one zero template per map.
fn maps_and_templates(
    coefficients: &[Coefficient],
) -> Result<(Vec<u32>, Vec<Coefficient>), BinaryIoError> {
    let mut maps: Vec<Arc<Vec<PolyVariable>>> = Vec::new();
    let mut templates: Vec<Coefficient> = Vec::new();
    let mut map_of = Vec::with_capacity(coefficients.len());
    for c in coefficients {
        super::resource_checkpoint()?;
        let vars = c.numerator.variables();
        let map = match maps.iter().position(|m| m == vars) {
            Some(i) => i as u32,
            None => {
                maps.push(vars.clone());
                let mut zero = c.clone();
                zero.numerator = c.numerator.zero();
                zero.denominator = c.numerator.one();
                templates.push(zero);
                (maps.len() - 1) as u32
            }
        };
        map_of.push(map);
    }
    Ok((map_of, templates))
}

fn write_section<'a>(
    templates: &[Coefficient],
    entries: impl ExactSizeIterator<Item = (u32, &'a CoefficientPolynomial)>,
    parts: &[(u32, Part, Part)],
) -> Result<Vec<u8>, BinaryIoError> {
    let mut out = Vec::new();
    out.extend_from_slice(&FACTORED_TABLE_MAGIC);
    out.extend_from_slice(&FACTORED_TABLE_VERSION.to_le_bytes());
    varint(&mut out, templates.len() as u64);
    for template in templates {
        let mut atom = Atom::new();
        atom.to_num(NativeCoefficient::RationalPolynomial(template.clone()));
        let mut frame = Vec::new();
        bincode::encode_into_std_write(&atom, &mut frame, bincode::config::standard())
            .map_err(super::native_encode_error)?;
        varint(&mut out, frame.len() as u64);
        out.extend_from_slice(&frame);
    }
    varint(&mut out, entries.len() as u64);
    for (map, p) in entries {
        varint(&mut out, u64::from(map));
        put_poly(&mut out, p);
    }
    varint(&mut out, parts.len() as u64);
    for (map, num, den) in parts {
        varint(&mut out, u64::from(*map));
        put_part(&mut out, num);
        put_part(&mut out, den);
    }
    Ok(out)
}

pub fn encode_factored_table(coefficients: &[Coefficient]) -> Result<Vec<u8>, BinaryIoError> {
    let (map_of, templates) = maps_and_templates(coefficients)?;
    let mut dict = Dictionary {
        entries: Vec::new(),
        index: HashMap::new(),
    };
    let mut parts = Vec::with_capacity(coefficients.len());
    for (c, &map) in coefficients.iter().zip(&map_of) {
        super::resource_checkpoint()?;
        let num = factor_part(&c.numerator, map, &mut dict);
        let den = factor_part(&c.denominator, map, &mut dict);
        parts.push((map, num, den));
    }
    write_section(
        &templates,
        dict.entries.iter().map(|(m, p)| (*m, p)),
        &parts,
    )
}

/// coef-form `shared`: the factored container without factoring (module
/// docs). The dictionary borrows the coefficients' polynomials, so the
/// encoder holds no second copy of them.
pub fn encode_shared_table(coefficients: &[Coefficient]) -> Result<Vec<u8>, BinaryIoError> {
    let (map_of, templates) = maps_and_templates(coefficients)?;
    let mut index: HashMap<(u32, &CoefficientPolynomial), u32> = HashMap::new();
    let mut entries: Vec<(u32, &CoefficientPolynomial)> = Vec::new();
    let mut parts = Vec::with_capacity(coefficients.len());
    for (c, &map) in coefficients.iter().zip(&map_of) {
        super::resource_checkpoint()?;
        let num = shared_part(&c.numerator, map, &mut index, &mut entries);
        let den = shared_part(&c.denominator, map, &mut index, &mut entries);
        parts.push((map, num, den));
    }
    write_section(&templates, entries.into_iter(), &parts)
}

fn shared_part<'a>(
    p: &'a CoefficientPolynomial,
    map: u32,
    index: &mut HashMap<(u32, &'a CoefficientPolynomial), u32>,
    entries: &mut Vec<(u32, &'a CoefficientPolynomial)>,
) -> Part {
    if p.is_constant() {
        return Part {
            content: if p.is_zero() {
                Integer::zero()
            } else {
                p.lcoeff()
            },
            factors: Vec::new(),
        };
    }
    let id = *index.entry((map, p)).or_insert_with(|| {
        entries.push((map, p));
        (entries.len() - 1) as u32
    });
    Part {
        content: Integer::one(),
        factors: vec![(id, 1)],
    }
}

// ----- streaming shared writer (vk5l WP6) -----

/// vk5l WP6: the SHARED form ([`encode_shared_table`]) written without holding
/// the coefficient table: each unique coefficient is pushed once, in table
/// order ([`Self::intern`] deduplicates and returns its id); its new
/// dictionary polynomials and its parts are appended to two scratch files
/// (the caller removes them afterwards), only a hash index and the variable
/// maps stay in memory. [`Self::write_section`] writes the section
/// [`encode_shared_table`] writes for the same coefficients in the same order.
pub struct SharedSpillWriter {
    entries: std::fs::File,
    entry_frames: Vec<(u64, u32)>,
    entry_end: u64,
    entry_index: HashMap<u64, Vec<u32>>,
    parts: std::fs::File,
    parts_end: u64,
    coefficient_index: HashMap<u64, Vec<(u32, u64, u32)>>,
    count: u32,
    maps: Vec<Arc<Vec<PolyVariable>>>,
    templates: Vec<Coefficient>,
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn read_at(file: &std::fs::File, offset: u64, len: u32) -> Result<Vec<u8>, BinaryIoError> {
    use std::os::unix::fs::FileExt;
    let mut out = vec![0; len as usize];
    file.read_exact_at(&mut out, offset)
        .map_err(super::native_io_error)?;
    Ok(out)
}

fn append_at(
    file: &std::fs::File,
    end: &mut u64,
    bytes: &[u8],
) -> Result<(u64, u32), BinaryIoError> {
    use std::os::unix::fs::FileExt;
    let offset = *end;
    file.write_all_at(bytes, offset)
        .map_err(super::native_io_error)?;
    let len = u32::try_from(bytes.len())
        .map_err(|_| BinaryIoError::Invalid("shared table record exceeds u32 bytes"))?;
    *end += u64::from(len);
    Ok((offset, len))
}

impl SharedSpillWriter {
    pub fn new(entries: std::fs::File, parts: std::fs::File) -> Self {
        Self {
            entries,
            entry_frames: Vec::new(),
            entry_end: 0,
            entry_index: HashMap::new(),
            parts,
            parts_end: 0,
            coefficient_index: HashMap::new(),
            count: 0,
            maps: Vec::new(),
            templates: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.count as usize
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn map_of(&mut self, c: &Coefficient) -> u32 {
        let vars = c.numerator.variables();
        match self.maps.iter().position(|m| m == vars) {
            Some(i) => i as u32,
            None => {
                self.maps.push(vars.clone());
                let mut zero = c.clone();
                zero.numerator = c.numerator.zero();
                zero.denominator = c.numerator.one();
                self.templates.push(zero);
                (self.maps.len() - 1) as u32
            }
        }
    }

    fn part(&mut self, p: &CoefficientPolynomial, map: u32) -> Result<Part, BinaryIoError> {
        if p.is_constant() {
            return Ok(Part {
                content: if p.is_zero() {
                    Integer::zero()
                } else {
                    p.lcoeff()
                },
                factors: Vec::new(),
            });
        }
        let mut bytes = Vec::new();
        varint(&mut bytes, u64::from(map));
        put_poly(&mut bytes, p);
        let hash = hash_bytes(&bytes);
        if let Some(bucket) = self.entry_index.get(&hash) {
            for &id in bucket {
                let (offset, len) = self.entry_frames[id as usize];
                if read_at(&self.entries, offset, len)? == bytes {
                    return Ok(Part {
                        content: Integer::one(),
                        factors: vec![(id, 1)],
                    });
                }
            }
        }
        let id = u32::try_from(self.entry_frames.len())
            .map_err(|_| BinaryIoError::Invalid("shared table dictionary exceeds u32"))?;
        let frame = append_at(&self.entries, &mut self.entry_end, &bytes)?;
        self.entry_frames.push(frame);
        self.entry_index.entry(hash).or_default().push(id);
        Ok(Part {
            content: Integer::one(),
            factors: vec![(id, 1)],
        })
    }

    /// The table id of `c`: an earlier one when an equal coefficient (same
    /// variable map, numerator and denominator) was pushed, else the next.
    pub fn intern(&mut self, c: &Coefficient) -> Result<u32, BinaryIoError> {
        super::resource_checkpoint()?;
        let map = self.map_of(c);
        // The parts of the coefficient; a repeated coefficient adds no
        // dictionary entry (its polynomials are found), so computing them
        // first keeps the encounter order of encode_shared_table.
        let num = self.part(&c.numerator, map)?;
        let den = self.part(&c.denominator, map)?;
        let mut record = Vec::new();
        varint(&mut record, u64::from(map));
        put_part(&mut record, &num);
        put_part(&mut record, &den);
        let hash = hash_bytes(&record);
        if let Some(bucket) = self.coefficient_index.get(&hash) {
            for &(id, offset, len) in bucket {
                if read_at(&self.parts, offset, len)? == record {
                    return Ok(id);
                }
            }
        }
        let id = self.count;
        let (offset, len) = append_at(&self.parts, &mut self.parts_end, &record)?;
        self.coefficient_index
            .entry(hash)
            .or_default()
            .push((id, offset, len));
        self.count = self.count.checked_add(1).ok_or(BinaryIoError::Invalid(
            "shared table exceeds u32 coefficients",
        ))?;
        Ok(id)
    }

    fn head(&self) -> Result<Vec<u8>, BinaryIoError> {
        let mut out = Vec::new();
        out.extend_from_slice(&FACTORED_TABLE_MAGIC);
        out.extend_from_slice(&FACTORED_TABLE_VERSION.to_le_bytes());
        varint(&mut out, self.templates.len() as u64);
        for template in &self.templates {
            let mut atom = Atom::new();
            atom.to_num(NativeCoefficient::RationalPolynomial(template.clone()));
            let mut frame = Vec::new();
            bincode::encode_into_std_write(&atom, &mut frame, bincode::config::standard())
                .map_err(super::native_encode_error)?;
            varint(&mut out, frame.len() as u64);
            out.extend_from_slice(&frame);
        }
        varint(&mut out, self.entry_frames.len() as u64);
        Ok(out)
    }

    /// The byte length of the section [`Self::write_section`] writes.
    pub fn section_len(&self) -> Result<usize, BinaryIoError> {
        let mut parts_count = Vec::new();
        varint(&mut parts_count, u64::from(self.count));
        Ok(self.head()?.len()
            + self.entry_end as usize
            + parts_count.len()
            + self.parts_end as usize)
    }

    /// The section: header, templates, the dictionary and the parts copied
    /// from the scratch files.
    pub fn write_section(self, writer: &mut impl std::io::Write) -> Result<(), BinaryIoError> {
        use std::io::{Read, Seek};
        let io = super::native_io_error;
        writer.write_all(&self.head()?).map_err(io)?;
        let copy = |mut file: &std::fs::File, len: u64, writer: &mut dyn std::io::Write| {
            file.seek(std::io::SeekFrom::Start(0)).map_err(io)?;
            let copied = std::io::copy(&mut (&mut file).take(len), writer).map_err(io)?;
            if copied != len {
                return Err(BinaryIoError::Invalid(
                    "shared table scratch file truncated",
                ));
            }
            Ok(())
        };
        copy(&self.entries, self.entry_end, writer)?;
        let mut parts_count = Vec::new();
        varint(&mut parts_count, u64::from(self.count));
        writer.write_all(&parts_count).map_err(io)?;
        copy(&self.parts, self.parts_end, writer)?;
        Ok(())
    }
}

// ----- decoder -----

fn get_part(
    src: &mut Cursor<'_>,
    entries: usize,
    limits: BinaryIoLimits,
) -> Result<Part, BinaryIoError> {
    let content = get_integer(src)?;
    let n = src.count(limits, "factored table factor references")?;
    let mut factors = Vec::with_capacity(n);
    for _ in 0..n {
        let id = u32::try_from(src.varint()?)
            .ok()
            .filter(|&id| (id as usize) < entries)
            .ok_or(BinaryIoError::Invalid(
                "factored table dictionary reference",
            ))?;
        let e = u32::try_from(src.varint()?)
            .map_err(|_| BinaryIoError::Invalid("factored table exponent"))?;
        factors.push((id, e));
    }
    Ok(Part { content, factors })
}

/// Decode a factored coefficient section (`is_factored_table`) against the
/// imported Symbolica state of its program: the identical coefficients the
/// expanded section held, in table order.
pub(super) fn decode_factored_table(
    atoms: &[u8],
    state_map: &symbolica::state::StateMap,
    limits: BinaryIoLimits,
) -> Result<Vec<Coefficient>, BinaryIoError> {
    let section = parse_factored_table(atoms, state_map, limits)?;
    let mut coefficients = Vec::with_capacity(section.len());
    for i in 0..section.len() {
        super::resource_checkpoint()?;
        coefficients.push(section.expand(i)?);
    }
    Ok(coefficients)
}

/// Whether coefficients of a factored section are multiplied back on first
/// use (`RUSTRED_COEF_FORM_LAZY=1`, coef-form prototype) instead of at load.
pub fn factored_lazy_expansion() -> bool {
    std::env::var("RUSTRED_COEF_FORM_LAZY").is_ok_and(|v| v == "1")
}

/// A parsed factored section: dictionary and factor references, not yet
/// multiplied back. [`FactoredSection::expand`] gives coefficient `i`,
/// identical to the eager decoder's.
pub(super) struct FactoredSection {
    templates: Vec<Coefficient>,
    entries: Vec<CoefficientPolynomial>,
    parts: Vec<(usize, Part, Part)>,
    limits: BinaryIoLimits,
}

impl FactoredSection {
    pub(super) fn len(&self) -> usize {
        self.parts.len()
    }

    pub(super) fn expand(&self, i: usize) -> Result<Coefficient, BinaryIoError> {
        let (map, num, den) = self
            .parts
            .get(i)
            .ok_or(BinaryIoError::Invalid("coefficient ID is outside table"))?;
        let template = &self.templates[*map];
        let mut value = template.clone();
        value.numerator = multiply(num, |id| &self.entries[id as usize], &template.numerator);
        value.denominator = multiply(den, |id| &self.entries[id as usize], &template.numerator);
        if value.denominator.is_zero() {
            return Err(BinaryIoError::Invalid("factored table zero denominator"));
        }
        validate_coefficient_on_map(
            &value,
            value.numerator.variables(),
            self.limits.exact_algebra,
        )
        .map_err(super::native_error)?;
        Ok(value)
    }
}

// ----- on-demand reader (batch 12, engine-line) -----

fn skip_integer(src: &mut Cursor<'_>) -> Result<(), BinaryIoError> {
    let tag = src.varint()?;
    if tag & 1 == 1 {
        let n = usize::try_from(tag >> 1)
            .map_err(|_| BinaryIoError::Invalid("factored table integer length"))?;
        src.take(n)?;
    }
    Ok(())
}

fn skip_poly(src: &mut Cursor<'_>, limits: BinaryIoLimits) -> Result<(), BinaryIoError> {
    let n = src.count(limits, "factored table polynomial terms")?;
    for _ in 0..n {
        skip_integer(src)?;
        let nz = src.varint()?;
        for _ in 0..nz {
            src.varint()?;
            src.varint()?;
        }
    }
    Ok(())
}

fn skip_part(src: &mut Cursor<'_>, limits: BinaryIoLimits) -> Result<(), BinaryIoError> {
    skip_integer(src)?;
    let n = src.count(limits, "factored table factor references")?;
    for _ in 0..n {
        src.varint()?;
        src.varint()?;
    }
    Ok(())
}

/// A factored / shared section read ON DEMAND: one pass records the byte
/// offset of every dictionary entry and every coefficient's parts (no
/// polynomial is built); a dictionary entry is decoded the first time a
/// coefficient needs it and kept (shared by every later coefficient: the
/// shared form's repeated denominators), a coefficient is multiplied back
/// on each request. [`Self::expand`] is identical to the eager decoder's
/// coefficient (same parts, same entries, same multiplication).
pub(super) struct OnDemandFactoredSection {
    bytes: Arc<[u8]>,
    templates: Vec<Coefficient>,
    entry_offsets: Vec<u64>,
    part_offsets: Vec<u64>,
    entries: Vec<std::sync::OnceLock<Arc<CoefficientPolynomial>>>,
    limits: BinaryIoLimits,
}

impl OnDemandFactoredSection {
    pub(super) fn new(
        bytes: Arc<[u8]>,
        state_map: &symbolica::state::StateMap,
        limits: BinaryIoLimits,
    ) -> Result<Self, BinaryIoError> {
        if !is_factored_table(&bytes) {
            return Err(BinaryIoError::Invalid("not a factored coefficient table"));
        }
        let mut src = Cursor {
            bytes: &bytes,
            at: 8,
        };
        let version = u32::from_le_bytes(src.take(4)?.try_into().expect("4 bytes"));
        if version != FACTORED_TABLE_VERSION {
            return Err(BinaryIoError::Invalid(
                "unsupported factored coefficient table version",
            ));
        }
        let nmaps = src.count(limits, "factored table variable maps")?;
        let mut templates = Vec::with_capacity(nmaps);
        for _ in 0..nmaps {
            let n = src.count(limits, "factored table template bytes")?;
            if n <= NATIVE_ATOM_HEADER_BYTES {
                return Err(BinaryIoError::Invalid("invalid native atom frame"));
            }
            let frame = src.take(n)?;
            templates.push(super::atoms::decode_generated_frame(
                frame, state_map, limits, false,
            )?);
        }
        let nentries = src.count(limits, "factored table dictionary entries")?;
        super::resource_array::<u64>(nentries)?;
        let mut entry_offsets = Vec::with_capacity(nentries);
        for _ in 0..nentries {
            entry_offsets.push(src.at as u64);
            let map = usize::try_from(src.varint()?)
                .ok()
                .filter(|&m| m < templates.len())
                .ok_or(BinaryIoError::Invalid("factored table map reference"))?;
            let _ = map;
            skip_poly(&mut src, limits)?;
        }
        let count = src.count(limits, "coefficient table entries")?;
        if count > 0 {
            super::CoefficientId::try_from_index(count - 1)?;
        }
        super::resource_array::<u64>(count)?;
        let mut part_offsets = Vec::with_capacity(count);
        for _ in 0..count {
            part_offsets.push(src.at as u64);
            let map = usize::try_from(src.varint()?)
                .ok()
                .filter(|&m| m < templates.len())
                .ok_or(BinaryIoError::Invalid("factored table map reference"))?;
            let _ = map;
            skip_part(&mut src, limits)?;
            skip_part(&mut src, limits)?;
        }
        if src.at != bytes.len() {
            return Err(BinaryIoError::Invalid("trailing coefficient table bytes"));
        }
        let entries = (0..nentries).map(|_| std::sync::OnceLock::new()).collect();
        Ok(Self {
            bytes,
            templates,
            entry_offsets,
            part_offsets,
            entries,
            limits,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.part_offsets.len()
    }

    /// Dictionary entries decoded so far.
    #[cfg(test)]
    pub(super) fn decoded_entries(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.get().is_some())
            .count()
    }

    fn entry(&self, id: u32) -> Result<Arc<CoefficientPolynomial>, BinaryIoError> {
        let cell = self.entries.get(id as usize).ok_or(BinaryIoError::Invalid(
            "factored table dictionary reference",
        ))?;
        if let Some(entry) = cell.get() {
            return Ok(Arc::clone(entry));
        }
        let mut src = Cursor {
            bytes: &self.bytes,
            at: self.entry_offsets[id as usize] as usize,
        };
        let map = src.varint()? as usize;
        let polynomial = Arc::new(get_poly(
            &mut src,
            &self.templates[map].numerator,
            self.limits,
        )?);
        Ok(Arc::clone(cell.get_or_init(|| polynomial)))
    }

    pub(super) fn expand(&self, i: usize) -> Result<Coefficient, BinaryIoError> {
        let at = *self
            .part_offsets
            .get(i)
            .ok_or(BinaryIoError::Invalid("coefficient ID is outside table"))?;
        let mut src = Cursor {
            bytes: &self.bytes,
            at: at as usize,
        };
        let map = src.varint()? as usize;
        let num = get_part(&mut src, self.entries.len(), self.limits)?;
        let den = get_part(&mut src, self.entries.len(), self.limits)?;
        let used: HashMap<u32, Arc<CoefficientPolynomial>> = num
            .factors
            .iter()
            .chain(&den.factors)
            .map(|&(id, _)| Ok((id, self.entry(id)?)))
            .collect::<Result<_, BinaryIoError>>()?;
        let template = &self.templates[map];
        let mut value = template.clone();
        value.numerator = multiply(&num, |id| &*used[&id], &template.numerator);
        value.denominator = multiply(&den, |id| &*used[&id], &template.numerator);
        if value.denominator.is_zero() {
            return Err(BinaryIoError::Invalid("factored table zero denominator"));
        }
        validate_coefficient_on_map(
            &value,
            value.numerator.variables(),
            self.limits.exact_algebra,
        )
        .map_err(super::native_error)?;
        Ok(value)
    }
}

/// Parse a factored section without multiplying any coefficient back.
pub(super) fn parse_factored_table(
    atoms: &[u8],
    state_map: &symbolica::state::StateMap,
    limits: BinaryIoLimits,
) -> Result<FactoredSection, BinaryIoError> {
    if !is_factored_table(atoms) {
        return Err(BinaryIoError::Invalid("not a factored coefficient table"));
    }
    let mut src = Cursor {
        bytes: atoms,
        at: 8,
    };
    let version = u32::from_le_bytes(src.take(4)?.try_into().expect("4 bytes"));
    if version != FACTORED_TABLE_VERSION {
        return Err(BinaryIoError::Invalid(
            "unsupported factored coefficient table version",
        ));
    }
    let nmaps = src.count(limits, "factored table variable maps")?;
    let mut templates = Vec::with_capacity(nmaps);
    for _ in 0..nmaps {
        let n = src.count(limits, "factored table template bytes")?;
        if n <= NATIVE_ATOM_HEADER_BYTES {
            return Err(BinaryIoError::Invalid("invalid native atom frame"));
        }
        let frame = src.take(n)?;
        templates.push(super::atoms::decode_generated_frame(
            frame, state_map, limits, false,
        )?);
    }
    let nentries = src.count(limits, "factored table dictionary entries")?;
    let mut entries = Vec::with_capacity(nentries);
    for _ in 0..nentries {
        super::resource_checkpoint()?;
        let map = usize::try_from(src.varint()?)
            .ok()
            .filter(|&m| m < templates.len())
            .ok_or(BinaryIoError::Invalid("factored table map reference"))?;
        entries.push(get_poly(&mut src, &templates[map].numerator, limits)?);
    }
    let count = src.count(limits, "coefficient table entries")?;
    if count > 0 {
        super::CoefficientId::try_from_index(count - 1)?;
    }
    super::resource_array::<(usize, Part, Part)>(count)?;
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        super::resource_checkpoint()?;
        let map = usize::try_from(src.varint()?)
            .ok()
            .filter(|&m| m < templates.len())
            .ok_or(BinaryIoError::Invalid("factored table map reference"))?;
        let num = get_part(&mut src, entries.len(), limits)?;
        let den = get_part(&mut src, entries.len(), limits)?;
        parts.push((map, num, den));
    }
    if src.at != atoms.len() {
        return Err(BinaryIoError::Invalid("trailing coefficient table bytes"));
    }
    Ok(FactoredSection {
        templates,
        entries,
        parts,
        limits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{CoefficientTableBuilder, DecodedCoefficientTable};
    use symbolica::atom::AtomCore;
    use symbolica::parse;
    use symbolica::prelude::Z;

    fn coefficient(text: &str, vars: &Arc<Vec<PolyVariable>>) -> Coefficient {
        parse!(text).to_rational_polynomial(&Z, &Z, Some(vars.clone()))
    }

    /// Value identity: a table re-encoded factored decodes to exactly the
    /// coefficients the expanded section holds (same numerator, denominator
    /// and variable map), ids unchanged.
    #[test]
    fn factored_table_decodes_to_identical_coefficients() {
        let vars: Arc<Vec<PolyVariable>> = Arc::new(
            ["d", "n0", "n1", "n2"]
                .iter()
                .map(|s| PolyVariable::Symbol(symbolica::symbol!(*s)))
                .collect(),
        );
        let small: Arc<Vec<PolyVariable>> =
            Arc::new(vec![PolyVariable::Symbol(symbolica::symbol!("d"))]);
        let texts = [
            "-n0/(-1+n2)",
            "n1/(-1+n2)",
            "(d-4)*(n0+n1-2)^2*(3*d-2*n2+1)/((d-2*n0-1)*(n1-1)^2*(2*d-7))",
            "123456789012345678901234567890*(n0-n1)^3/((d-3)*(d-5)^2)",
            "-(2^62+5)*n2/(d+1)",
            "0",
            "-5",
            "n0^2/(2*d-7)^3",
        ];
        let mut values: Vec<Coefficient> = texts.iter().map(|t| coefficient(t, &vars)).collect();
        values.push(coefficient("(d-4)/(2*d-7)", &small));
        let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
        for value in &values {
            builder.intern(value).unwrap();
        }
        let encoded = builder.finish().unwrap();
        let expanded = DecodedCoefficientTable::import_generated(
            &encoded.state,
            &encoded.atoms,
            BinaryIoLimits::default(),
        )
        .unwrap();
        let originals: Vec<Coefficient> = (0..expanded.len())
            .map(|i| {
                expanded
                    .coefficient(crate::persistence::CoefficientId::try_from_index(i).unwrap())
                    .unwrap()
                    .clone()
            })
            .collect();
        let factored = encode_factored_table(&originals).unwrap();
        assert!(is_factored_table(&factored));
        let decoded = DecodedCoefficientTable::import_generated(
            &encoded.state,
            &factored,
            BinaryIoLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.len(), originals.len());
        for (i, original) in originals.iter().enumerate() {
            let value = decoded
                .coefficient(crate::persistence::CoefficientId::try_from_index(i).unwrap())
                .unwrap();
            assert_eq!(value.numerator, original.numerator, "numerator {i}");
            assert_eq!(value.denominator, original.denominator, "denominator {i}");
            assert_eq!(value.get_variables(), original.get_variables(), "map {i}");
        }
        // batch 12: the ON-DEMAND reader of both forms gives the identical coefficients, in any order,
        // decoding only the dictionary entries the requested coefficients use
        let state_map = super::super::atoms::import_state(&encoded.state).unwrap();
        for form in [factored.clone(), encode_shared_table(&originals).unwrap()] {
            let on_demand = OnDemandFactoredSection::new(
                Arc::from(&form[..]),
                &state_map,
                BinaryIoLimits::default(),
            )
            .unwrap();
            assert_eq!(on_demand.len(), originals.len());
            assert_eq!(on_demand.decoded_entries(), 0);
            let last = originals.len() - 1;
            let value = on_demand.expand(last).unwrap();
            assert_eq!(value.numerator, originals[last].numerator);
            assert!(
                on_demand.decoded_entries() <= 2,
                "only the last coefficient's entries"
            );
            for (i, original) in originals.iter().enumerate().rev() {
                let value = on_demand.expand(i).unwrap();
                assert_eq!(
                    value.numerator, original.numerator,
                    "on-demand numerator {i}"
                );
                assert_eq!(
                    value.denominator, original.denominator,
                    "on-demand denominator {i}"
                );
                assert_eq!(
                    value.get_variables(),
                    original.get_variables(),
                    "on-demand map {i}"
                );
            }
        }
        // per-coefficient expansion (RUSTRED_COEF_FORM_LAZY path), any order
        let section =
            parse_factored_table(&factored, &state_map, BinaryIoLimits::default()).unwrap();
        assert_eq!(section.len(), originals.len());
        for (i, original) in originals.iter().enumerate().rev() {
            let value = section.expand(i).unwrap();
            assert_eq!(value.numerator, original.numerator, "lazy numerator {i}");
            assert_eq!(
                value.denominator, original.denominator,
                "lazy denominator {i}"
            );
            assert_eq!(
                value.get_variables(),
                original.get_variables(),
                "lazy map {i}"
            );
        }
        let lazy = crate::persistence::LazyDecodedCoefficientTable::new(
            encoded.state.clone().into(),
            factored.clone().into(),
            BinaryIoLimits::default(),
        )
        .unwrap();
        assert_eq!(lazy.len(), originals.len());
        for (i, original) in originals.iter().enumerate() {
            let value = lazy
                .coefficient(crate::persistence::CoefficientId::try_from_index(i).unwrap())
                .unwrap();
            assert_eq!(*value, *original, "lazy {i}");
        }
    }

    /// coef-form `shared`: the factored container without factoring. Same
    /// coefficients through the eager and the lazy reader, no factoring
    /// vk5l WP6: the streaming shared writer (scratch files, one coefficient
    /// at a time, repeats deduplicated) and the spilled builder mode write
    /// the section encode_shared_table writes for the same table.
    #[test]
    fn the_streaming_shared_writer_writes_the_shared_section() {
        let vars: Arc<Vec<PolyVariable>> = Arc::new(
            ["d", "n0", "n1", "n2"]
                .iter()
                .map(|s| PolyVariable::Symbol(symbolica::symbol!(*s)))
                .collect(),
        );
        let small: Arc<Vec<PolyVariable>> =
            Arc::new(vec![PolyVariable::Symbol(symbolica::symbol!("d"))]);
        let texts = [
            "-n0/(-1+n2)",
            "n1/(-1+n2)",
            "(d-4)*(n0+n1-2)^2*(3*d-2*n2+1)/((d-2*n0-1)*(n1-1)^2*(2*d-7))",
            "-n0/(-1+n2)",
            "0",
            "-5",
            "n0^2/((d-2*n0-1)*(n1-1)^2*(2*d-7))",
            "n1/(-1+n2)",
        ];
        let mut values: Vec<Coefficient> = texts.iter().map(|t| coefficient(t, &vars)).collect();
        values.push(coefficient("(d-4)/(2*d-7)", &small));
        values.push(coefficient("(d-4)/(2*d-7)", &small));
        // The reference: the in-memory builder, its table decoded, encode_shared_table.
        let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
        let ids: Vec<_> = values.iter().map(|v| builder.intern(v).unwrap()).collect();
        let encoded = builder.finish().unwrap();
        let expanded = DecodedCoefficientTable::import_generated(
            &encoded.state,
            &encoded.atoms,
            BinaryIoLimits::default(),
        )
        .unwrap();
        let originals: Vec<Coefficient> = (0..expanded.len())
            .map(|i| {
                expanded
                    .coefficient(crate::persistence::CoefficientId::try_from_index(i).unwrap())
                    .unwrap()
                    .clone()
            })
            .collect();
        let reference = encode_shared_table(&originals).unwrap();
        let scratch = |name: &str| {
            let path = std::env::temp_dir().join(format!(
                "rustred-shared-writer-{}-{name}",
                std::process::id()
            ));
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            (path, file)
        };
        // The writer alone.
        let ((p1, e1), (p2, f1)) = (scratch("e1"), scratch("p1"));
        let mut writer = SharedSpillWriter::new(e1, f1);
        let writer_ids: Vec<u32> = values.iter().map(|v| writer.intern(v).unwrap()).collect();
        assert_eq!(
            writer_ids,
            ids.iter().map(|id| id.index() as u32).collect::<Vec<_>>()
        );
        let expected_len = writer.section_len().unwrap();
        let mut section = Vec::new();
        writer.write_section(&mut section).unwrap();
        assert_eq!(section.len(), expected_len);
        assert_eq!(section, reference);
        // The spilled builder mode.
        let ((p3, e2), (p4, f2)) = (scratch("e2"), scratch("p2"));
        let mut builder =
            CoefficientTableBuilder::with_shared_spill(BinaryIoLimits::default(), e2, f2);
        let builder_ids: Vec<_> = values.iter().map(|v| builder.intern(v).unwrap()).collect();
        assert_eq!(builder_ids, ids);
        assert_eq!(builder.encoded_atom_bytes(), reference.len());
        let state = builder.encoded_state().unwrap();
        let mut section = Vec::new();
        builder.write_atoms(&mut section).unwrap();
        assert_eq!(section, reference);
        // the exported Symbolica state is GLOBAL: tests running in parallel may register symbols between the two
        // exports (batch 12a: flaky `state == encoded.state`), so the builder's state is checked by what it decodes
        let decoded =
            DecodedCoefficientTable::import_generated(&state, &section, BinaryIoLimits::default())
                .unwrap();
        assert_eq!(decoded.len(), originals.len());
        for (i, original) in originals.iter().enumerate() {
            let value = decoded
                .coefficient(crate::persistence::CoefficientId::try_from_index(i).unwrap())
                .unwrap();
            assert_eq!(value.numerator, original.numerator, "numerator {i}");
            assert_eq!(value.denominator, original.denominator, "denominator {i}");
        }
        for path in [p1, p2, p3, p4] {
            std::fs::remove_file(path).unwrap();
        }
    }

    /// (every non-constant polynomial is one entry with exponent 1, shared
    /// between coefficients), and the bytes depend only on the table.
    #[test]
    fn shared_table_round_trips_unfactored_and_reproducibly() {
        let vars: Arc<Vec<PolyVariable>> = Arc::new(
            ["d", "n0", "n1", "n2"]
                .iter()
                .map(|s| PolyVariable::Symbol(symbolica::symbol!(*s)))
                .collect(),
        );
        let small: Arc<Vec<PolyVariable>> =
            Arc::new(vec![PolyVariable::Symbol(symbolica::symbol!("d"))]);
        let texts = [
            "-n0/(-1+n2)",
            "n1/(-1+n2)",
            "(d-4)*(n0+n1-2)^2*(3*d-2*n2+1)/((d-2*n0-1)*(n1-1)^2*(2*d-7))",
            "123456789012345678901234567890*(n0-n1)^3/((d-3)*(d-5)^2)",
            "-(2^62+5)*n2/(d+1)",
            "0",
            "-5",
            "n0^2/((d-2*n0-1)*(n1-1)^2*(2*d-7))",
        ];
        let mut values: Vec<Coefficient> = texts.iter().map(|t| coefficient(t, &vars)).collect();
        values.push(coefficient("(d-4)/(2*d-7)", &small));
        values.push(coefficient("(d+1)/(2*d-7)", &small));
        let mut builder = CoefficientTableBuilder::new(BinaryIoLimits::default());
        for value in &values {
            builder.intern(value).unwrap();
        }
        let encoded = builder.finish().unwrap();
        let expanded = DecodedCoefficientTable::import_generated(
            &encoded.state,
            &encoded.atoms,
            BinaryIoLimits::default(),
        )
        .unwrap();
        let originals: Vec<Coefficient> = (0..expanded.len())
            .map(|i| {
                expanded
                    .coefficient(crate::persistence::CoefficientId::try_from_index(i).unwrap())
                    .unwrap()
                    .clone()
            })
            .collect();
        let shared = encode_shared_table(&originals).unwrap();
        assert!(is_factored_table(&shared));
        assert_eq!(
            shared,
            encode_shared_table(&originals).unwrap(),
            "reproducible"
        );
        assert_eq!(
            shared,
            encode_shared_table(&originals.clone()).unwrap(),
            "independent of the caller's copy"
        );
        let state_map = super::super::atoms::import_state(&encoded.state).unwrap();
        let section = parse_factored_table(&shared, &state_map, BinaryIoLimits::default()).unwrap();
        // no factoring: whole entries, exponent 1, shared (two coefficients
        // here have the same denominator on each map)
        let mut distinct = std::collections::HashSet::new();
        for c in &originals {
            for p in [&c.numerator, &c.denominator] {
                if !p.is_constant() {
                    distinct.insert((p.variables().clone(), p.clone()));
                }
            }
        }
        assert_eq!(
            section.entries.len(),
            distinct.len(),
            "one entry per distinct polynomial"
        );
        for (_, num, den) in &section.parts {
            for part in [num, den] {
                assert!(part.factors.len() <= 1);
                assert!(part.factors.iter().all(|&(_, e)| e == 1));
                if !part.factors.is_empty() {
                    assert_eq!(part.content, Integer::one());
                }
            }
        }
        let decoded = DecodedCoefficientTable::import_generated(
            &encoded.state,
            &shared,
            BinaryIoLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.len(), originals.len());
        let lazy = crate::persistence::LazyDecodedCoefficientTable::new(
            encoded.state.clone().into(),
            shared.clone().into(),
            BinaryIoLimits::default(),
        )
        .unwrap();
        assert_eq!(lazy.len(), originals.len());
        for (i, original) in originals.iter().enumerate().rev() {
            let id = crate::persistence::CoefficientId::try_from_index(i).unwrap();
            let eager = decoded.coefficient(id).unwrap();
            assert_eq!(eager, original, "eager {i}");
            assert_eq!(
                eager.get_variables(),
                original.get_variables(),
                "eager map {i}"
            );
            let value = lazy.coefficient(id).unwrap();
            assert_eq!(*value, *original, "lazy {i}");
            assert_eq!(
                value.get_variables(),
                original.get_variables(),
                "lazy map {i}"
            );
        }
    }
}
