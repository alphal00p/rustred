//! Deterministic binary primitives of the step-cache files.
//!
//! Integers are unsigned LEB128 varints, signed values zigzag varints.
//! Arbitrary-precision integers keep Symbolica's canonical small/large split:
//! a value that fits `i64` is always written and read as the small form.
//! Polynomials are written in their native monomial order with exponents in
//! the coefficient context's variable order; the variable map itself is part
//! of the cache stamp and is never repeated per polynomial.
use symbolica::domains::backend::integer::{from_lsf_bytes, write_lsf_bytes};
use symbolica::domains::integer::Integer;

use super::factor::Polynomial;
use crate::family::IntegralKey;
use crate::reduction::OrderedKeyStage;

pub(crate) type CodecResult<T> = Result<T, String>;

#[derive(Default)]
pub(crate) struct Writer {
    pub bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.bytes.push((value as u8) | 0x80);
            value >>= 7;
        }
        self.bytes.push(value as u8);
    }

    pub(crate) fn len(&mut self, value: usize) {
        self.varint(value as u64);
    }

    pub(crate) fn zigzag(&mut self, value: i64) {
        self.varint(((value << 1) ^ (value >> 63)) as u64);
    }

    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub(crate) fn integer(&mut self, value: &Integer) {
        match value {
            Integer::Single(value) => {
                self.u8(0);
                self.zigzag(*value);
            }
            other => {
                self.u8(1);
                self.u8(u8::from(other.is_negative()));
                let mut magnitude = Vec::new();
                write_lsf_bytes(&other.clone().to_multi_prec(), &mut magnitude);
                self.len(magnitude.len());
                self.raw(&magnitude);
            }
        }
    }

    pub(crate) fn polynomial(&mut self, polynomial: &Polynomial) {
        self.len(polynomial.nterms());
        let nvars = polynomial.nvars();
        for (index, coefficient) in polynomial.coefficients.iter().enumerate() {
            self.integer(coefficient);
            for exponent in &polynomial.exponents[index * nvars..(index + 1) * nvars] {
                self.varint(u64::from(*exponent));
            }
        }
    }
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }

    pub(crate) fn take(&mut self, count: usize) -> CodecResult<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .filter(|&end| end <= self.bytes.len())
            .ok_or("truncated step-cache data")?;
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    pub(crate) fn u8(&mut self) -> CodecResult<u8> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn varint(&mut self) -> CodecResult<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            let low = u64::from(byte & 0x7f);
            if shift == 63 && low > 1 {
                return Err("step-cache varint overflows u64".into());
            }
            value |= low << shift;
            if byte & 0x80 == 0 {
                if byte == 0 && shift != 0 {
                    return Err("step-cache varint is not minimal".into());
                }
                return Ok(value);
            }
        }
        Err("step-cache varint is too long".into())
    }

    /// A count or index bounded by `limit`.
    pub(crate) fn len(&mut self, limit: usize) -> CodecResult<usize> {
        let value = self.varint()?;
        usize::try_from(value)
            .ok()
            .filter(|&value| value <= limit)
            .ok_or_else(|| format!("step-cache count {value} exceeds its bound {limit}"))
    }

    pub(crate) fn index(&mut self, len: usize) -> CodecResult<u32> {
        let value = self.varint()?;
        if value >= len as u64 {
            return Err(format!("step-cache index {value} is out of range {len}"));
        }
        Ok(value as u32)
    }

    pub(crate) fn zigzag(&mut self) -> CodecResult<i64> {
        let value = self.varint()?;
        Ok(((value >> 1) as i64) ^ -((value & 1) as i64))
    }

    pub(crate) fn integer(&mut self) -> CodecResult<Integer> {
        match self.u8()? {
            0 => Ok(Integer::Single(self.zigzag()?)),
            1 => {
                let negative = match self.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err("invalid step-cache integer sign".into()),
                };
                let length = self.len(self.bytes.len())?;
                let magnitude = self.take(length)?;
                if magnitude.last().is_none_or(|&byte| byte == 0) {
                    return Err("step-cache large integer is not minimal".into());
                }
                let magnitude = from_lsf_bytes(magnitude);
                let value = Integer::from(if negative { -magnitude } else { magnitude });
                if matches!(value, Integer::Single(_)) {
                    return Err("step-cache large integer fits the small form".into());
                }
                Ok(value)
            }
            _ => Err("invalid step-cache integer tag".into()),
        }
    }

    /// One polynomial in `template`'s variable map. Term order and nonzero
    /// coefficients are checked by the caller's coefficient validation.
    pub(crate) fn polynomial(&mut self, template: &Polynomial) -> CodecResult<Polynomial> {
        let terms = self.len(self.bytes.len())?;
        let nvars = template.nvars();
        let mut polynomial = template.zero_with_capacity(terms);
        polynomial.exponents.reserve(terms.saturating_mul(nvars));
        for _ in 0..terms {
            let coefficient = self.integer()?;
            if coefficient.is_zero() {
                return Err("step-cache polynomial has a zero coefficient".into());
            }
            polynomial.coefficients.push(coefficient);
            for _ in 0..nvars {
                let exponent = u16::try_from(self.varint()?)
                    .map_err(|_| "step-cache exponent exceeds u16".to_owned())?;
                polynomial.exponents.push(exponent);
            }
        }
        Ok(polynomial)
    }
}

/// Stage byte of an encoded scheduled identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeStage {
    Integral,
    Route,
    /// Owner sector as a bit mask (bit `i` is position `i`).
    Apply(u64),
    /// Propagator library: the presentation index.
    ClassRoute(u64),
    /// Propagator library: the owner class id.
    ClassApply(u64),
}

/// Bytes of one encoded identity: stage, owner mask (`u64`, little endian)
/// and one signed byte per power.
pub(crate) const NODE_HEADER: usize = 9;

/// Append one scheduled identity. Returns `false` (and leaves `out`
/// unchanged) when the identity has no compact form: an owner sector wider
/// than 64 slots or a power outside `i8`. Such steps are never recorded.
pub(crate) fn encode_node(
    stage: OrderedKeyStage<'_>,
    key: &IntegralKey,
    out: &mut Vec<u8>,
) -> bool {
    let start = out.len();
    let (tag, class) = match stage {
        OrderedKeyStage::Integral => (0u8, 0u64),
        OrderedKeyStage::Route => (1, 0),
        OrderedKeyStage::Apply { owner_sector } => {
            if owner_sector.len() > 64 {
                return false;
            }
            let mut mask = 0u64;
            for (position, &bit) in owner_sector.iter().enumerate() {
                if bit {
                    mask |= 1u64 << position;
                }
            }
            (2, mask)
        }
        OrderedKeyStage::ClassRoute { class } => (CLASS_ROUTE_TAG, class),
        OrderedKeyStage::ClassApply { class } => (CLASS_APPLY_TAG, class),
    };
    out.push(tag);
    out.extend_from_slice(&class.to_le_bytes());
    for &power in key.powers() {
        let Ok(power) = i8::try_from(power) else {
            out.truncate(start);
            return false;
        };
        out.push(power as u8);
    }
    true
}

/// Stage byte of a partially transported Route key in a tail table (never a
/// step-cache identity: such keys are never recorded there).
pub(crate) const PARTIAL_ROUTE_TAG: u8 = 3;

/// Stage byte of a propagator-library Route identity; the class field holds
/// the presentation index.
const CLASS_ROUTE_TAG: u8 = 4;
/// Stage byte of a propagator-library Apply identity; the class field holds
/// the owner class id.
const CLASS_APPLY_TAG: u8 = 5;

/// Append a partially transported Route key of source sector `source` with
/// `powers` (owner powers so far, then the source numerator powers not yet
/// expanded): stage [`PARTIAL_ROUTE_TAG`], the source mask (`u64`, little
/// endian), one signed byte per power. Returns `false` (and leaves `out`
/// unchanged) when the source is wider than 64 slots or a power is outside
/// `i8`.
pub(crate) fn encode_partial_route(source: &[bool], powers: &[i64], out: &mut Vec<u8>) -> bool {
    if source.len() > 64 {
        return false;
    }
    let start = out.len();
    let mask = source
        .iter()
        .enumerate()
        .filter(|&(_, &bit)| bit)
        .fold(0u64, |mask, (position, _)| mask | 1u64 << position);
    out.push(PARTIAL_ROUTE_TAG);
    out.extend_from_slice(&mask.to_le_bytes());
    for &power in powers {
        let Ok(power) = i8::try_from(power) else {
            out.truncate(start);
            return false;
        };
        out.push(power as u8);
    }
    true
}

/// Check one tail-table identity: a step-cache identity
/// ([`decode_node`]) or a partially transported Route key
/// ([`encode_partial_route`]).
pub(crate) fn check_tail_identity(bytes: &[u8]) -> CodecResult<()> {
    if bytes.first() == Some(&PARTIAL_ROUTE_TAG) {
        if bytes.len() <= NODE_HEADER {
            return Err("truncated partial Route identity".into());
        }
        return Ok(());
    }
    decode_node(bytes).map(|_| ())
}

pub(crate) fn decode_node(bytes: &[u8]) -> CodecResult<(NodeStage, IntegralKey)> {
    if bytes.len() < NODE_HEADER {
        return Err("truncated step-cache identity".into());
    }
    let class = u64::from_le_bytes(bytes[1..NODE_HEADER].try_into().expect("eight bytes"));
    let stage = match (bytes[0], class) {
        (0, 0) => NodeStage::Integral,
        (1, 0) => NodeStage::Route,
        (2, mask) => NodeStage::Apply(mask),
        (CLASS_ROUTE_TAG, class) => NodeStage::ClassRoute(class),
        (CLASS_APPLY_TAG, class) => NodeStage::ClassApply(class),
        _ => return Err("invalid step-cache identity stage".into()),
    };
    let powers: Vec<i64> = bytes[NODE_HEADER..]
        .iter()
        .map(|&byte| i64::from(byte as i8))
        .collect();
    let key = IntegralKey::try_from_preallocated(powers)
        .map_err(|error| format!("invalid step-cache integral: {error:?}"))?;
    Ok((stage, key))
}

/// Owner sector of an encoded Apply identity, for an adapter of arity `N`.
pub(crate) fn owner_sector<const N: usize>(mask: u64) -> Option<[bool; N]> {
    if N < 64 && mask >> N != 0 {
        return None;
    }
    Some(std::array::from_fn(|position| {
        position < 64 && mask & (1u64 << position) != 0
    }))
}

/// Multiply-rotate hasher for process-local interning tables (identities
/// and coefficients). Never used for anything persisted.
#[derive(Default)]
pub(crate) struct FxHasher(u64);

impl std::hash::Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.write_u64(u64::from_le_bytes(chunk.try_into().expect("eight bytes")));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.write_u64(u64::from_le_bytes(word) ^ ((rest.len() as u64) << 59));
        }
    }
    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }
    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

pub(crate) type FxBuild = std::hash::BuildHasherDefault<FxHasher>;

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn parse_hex32(text: &str) -> CodecResult<[u8; 32]> {
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid step-cache digest {text:?}"));
    }
    let mut out = [0u8; 32];
    for (index, chunk) in text.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).expect("ascii hex");
        out[index] = u8::from_str_radix(pair, 16).expect("validated hex");
    }
    Ok(out)
}
