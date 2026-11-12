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
pub fn encode_factored_table(coefficients: &[Coefficient]) -> Result<Vec<u8>, BinaryIoError> {
    let mut maps: Vec<Arc<Vec<PolyVariable>>> = Vec::new();
    let mut map_of = |c: &Coefficient| -> u32 {
        let vars = c.numerator.variables();
        match maps.iter().position(|m| m == vars) {
            Some(i) => i as u32,
            None => {
                maps.push(vars.clone());
                (maps.len() - 1) as u32
            }
        }
    };
    let mut dict = Dictionary {
        entries: Vec::new(),
        index: HashMap::new(),
    };
    let mut parts = Vec::with_capacity(coefficients.len());
    let mut templates: Vec<Coefficient> = Vec::new();
    for c in coefficients {
        super::resource_checkpoint()?;
        let map = map_of(c);
        if templates.len() <= map as usize {
            let mut zero = c.clone();
            zero.numerator = c.numerator.zero();
            zero.denominator = c.numerator.one();
            templates.push(zero);
        }
        let num = factor_part(&c.numerator, map, &mut dict);
        let den = factor_part(&c.denominator, map, &mut dict);
        parts.push((map, num, den));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&FACTORED_TABLE_MAGIC);
    out.extend_from_slice(&FACTORED_TABLE_VERSION.to_le_bytes());
    varint(&mut out, templates.len() as u64);
    for template in &templates {
        let mut atom = Atom::new();
        atom.to_num(NativeCoefficient::RationalPolynomial(template.clone()));
        let mut frame = Vec::new();
        bincode::encode_into_std_write(&atom, &mut frame, bincode::config::standard())
            .map_err(super::native_encode_error)?;
        varint(&mut out, frame.len() as u64);
        out.extend_from_slice(&frame);
    }
    varint(&mut out, dict.entries.len() as u64);
    for (map, p) in &dict.entries {
        varint(&mut out, u64::from(*map));
        put_poly(&mut out, p);
    }
    varint(&mut out, parts.len() as u64);
    for (map, num, den) in &parts {
        varint(&mut out, u64::from(*map));
        put_part(&mut out, num);
        put_part(&mut out, den);
    }
    Ok(out)
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
    super::resource_array::<Coefficient>(count)?;
    let mut coefficients = Vec::with_capacity(count);
    for _ in 0..count {
        super::resource_checkpoint()?;
        let map = usize::try_from(src.varint()?)
            .ok()
            .filter(|&m| m < templates.len())
            .ok_or(BinaryIoError::Invalid("factored table map reference"))?;
        let num = get_part(&mut src, entries.len(), limits)?;
        let den = get_part(&mut src, entries.len(), limits)?;
        let template = &templates[map];
        let mut value = template.clone();
        value.numerator = multiply(&num, |id| &entries[id as usize], &template.numerator);
        value.denominator = multiply(&den, |id| &entries[id as usize], &template.numerator);
        if value.denominator.is_zero() {
            return Err(BinaryIoError::Invalid("factored table zero denominator"));
        }
        validate_coefficient_on_map(&value, value.numerator.variables(), limits.exact_algebra)
            .map_err(super::native_error)?;
        coefficients.push(value);
    }
    if src.at != atoms.len() {
        return Err(BinaryIoError::Invalid("trailing coefficient table bytes"));
    }
    Ok(coefficients)
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
}
