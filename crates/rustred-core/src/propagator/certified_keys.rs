//! The CERTIFIED KEY SET of a boxed node (vk5l WP1, plan VK5L §3.4): the
//! explicit, finite set of keys of the node's chart on which its table is
//! certified to close (the forward closure of its box under first match, e.g.
//! T162: 65,908 keys of which 10,204 lie outside box B). A layer box cannot
//! express it, so the boxed record carries both: the user box (messages, the
//! early request check) and this set (the dispatch domain).
//!
//! Encoding [`CERTIFIED_KEYS_SCHEMA`] (version [`P_CERTIFIED_KEYS_VERSION`],
//! a simple version constant, no digest): the magic line, one JSON header
//! line (`schema`, `version`, `node`, `arity`, `count`, `frame`), then the
//! sorted keys, each as the number of leading slots it shares with the
//! previous key (one byte) followed by the remaining slots as zigzag LEB128
//! varints. Sorted, deduplicated, at most 255 slots. The same bytes are a
//! standalone `.rrkeys` file and, base64-encoded, the `certified_keys` line of
//! a boxed payload's provenance ([`super::table_import`]); the set is decoded
//! on the first use of its owner (lazy), never at library load.
//!
//! In memory the set is one flat `i8` row array (T178: 335,306 keys x 20
//! slots = 6.7 MB), searched by binary search.
use std::fmt;

use super::error::PError;

/// Schema of a certified key set.
pub const CERTIFIED_KEYS_SCHEMA: &str = "rustred.certified-keys.v1";
/// Version of the encoding and of its meaning (the forward closure of a
/// boxed node's box under first match, terminals included).
pub const P_CERTIFIED_KEYS_VERSION: u32 = 1;
/// First line of the encoding.
const MAGIC: &[u8] = b"RRCK1\n";

/// A sorted, deduplicated set of keys of one arity (every power in `i8`).
#[derive(Clone, PartialEq, Eq)]
pub struct CertifiedKeySet {
    node: String,
    arity: usize,
    /// The keys' frame: `"presentation"` (slots of the owner presentation) or
    /// `"table"` (the table's own slot order, before an import).
    frame: String,
    /// `count * arity` powers, row-major, rows ascending.
    rows: Vec<i8>,
}

impl fmt::Debug for CertifiedKeySet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CertifiedKeySet")
            .field("node", &self.node)
            .field("arity", &self.arity)
            .field("frame", &self.frame)
            .field("keys", &self.len())
            .finish()
    }
}

fn bad(detail: impl fmt::Display) -> PError {
    PError::MalformedLibrary(format!("certified key set: {detail}"))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Header {
    schema: String,
    version: u32,
    node: String,
    arity: usize,
    count: usize,
    frame: String,
}

impl CertifiedKeySet {
    /// The set of `keys` (any order, duplicates merged). Every key has
    /// `arity` powers in `-128..=127`.
    pub fn new(
        node: impl Into<String>,
        arity: usize,
        frame: impl Into<String>,
        keys: impl IntoIterator<Item = Vec<i64>>,
    ) -> Result<Self, PError> {
        if arity == 0 || arity > 255 {
            return Err(bad(format!("arity {arity}")));
        }
        let mut packed: Vec<Vec<i8>> = Vec::new();
        for key in keys {
            if key.len() != arity {
                return Err(bad(format!("the key {key:?} has not {arity} slots")));
            }
            let row = key
                .iter()
                .map(|&power| i8::try_from(power))
                .collect::<Result<Vec<i8>, _>>()
                .map_err(|_| bad(format!("the key {key:?} has a power outside i8")))?;
            packed.push(row);
        }
        packed.sort_unstable();
        packed.dedup();
        Ok(Self {
            node: node.into(),
            arity,
            frame: frame.into(),
            rows: packed.concat(),
        })
    }

    /// The keys of a key list in text: one key per line, either a bare key
    /// `I(1,1,...,0)` or a JSON array whose first element is the key text (the
    /// rows of a closure statement, `keys.jsonl` of T162 or `statement.jsonl`
    /// of T178). Blank lines are skipped.
    pub fn parse_key_lines(
        node: impl Into<String>,
        arity: usize,
        frame: impl Into<String>,
        text: &str,
    ) -> Result<Self, PError> {
        let mut keys = Vec::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let key_text: String = if line.starts_with('[') {
                let row: serde_json::Value = serde_json::from_str(line)
                    .map_err(|error| bad(format!("line {}: {error}", number + 1)))?;
                row.get(0)
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| bad(format!("line {}: no key text first", number + 1)))?
                    .to_owned()
            } else {
                line.trim_matches('"').to_owned()
            };
            keys.push(
                parse_numeric_key(&key_text, arity)
                    .map_err(|detail| bad(format!("line {}: {detail}", number + 1)))?,
            );
        }
        Self::new(node, arity, frame, keys)
    }

    pub fn node(&self) -> &str {
        &self.node
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn frame(&self) -> &str {
        &self.frame
    }

    pub fn len(&self) -> usize {
        self.rows.len() / self.arity
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Bytes held in memory by the rows.
    pub fn heap_bytes(&self) -> usize {
        self.rows.capacity() + self.node.capacity() + self.frame.capacity()
    }

    fn row(&self, index: usize) -> &[i8] {
        &self.rows[index * self.arity..(index + 1) * self.arity]
    }

    /// `powers` is one of the keys (binary search).
    pub fn contains(&self, powers: &[i64]) -> bool {
        if powers.len() != self.arity {
            return false;
        }
        let mut probe = Vec::with_capacity(self.arity);
        for &power in powers {
            match i8::try_from(power) {
                Ok(power) => probe.push(power),
                Err(_) => return false,
            }
        }
        let (mut low, mut high) = (0, self.len());
        while low < high {
            let middle = (low + high) / 2;
            match self.row(middle).cmp(&probe[..]) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return true,
            }
        }
        false
    }

    /// The keys, ascending.
    pub fn keys(&self) -> impl Iterator<Item = Vec<i64>> + '_ {
        (0..self.len()).map(|index| self.row(index).iter().map(|&p| i64::from(p)).collect())
    }

    /// The same set with slot `s` moved to `slot_map[s]` (a table frame onto
    /// the presentation frame), named `frame`.
    pub fn permuted(&self, slot_map: &[usize], frame: impl Into<String>) -> Result<Self, PError> {
        let mut seen = vec![false; self.arity];
        if slot_map.len() != self.arity
            || slot_map
                .iter()
                .any(|&to| to >= self.arity || std::mem::replace(&mut seen[to], true))
        {
            return Err(bad(format!(
                "the slot map {slot_map:?} is no permutation of {} slots",
                self.arity
            )));
        }
        let keys = self.keys().map(|key| {
            let mut out = vec![0; self.arity];
            for (slot, power) in key.into_iter().enumerate() {
                out[slot_map[slot]] = power;
            }
            out
        });
        Self::new(
            self.node.clone(),
            self.arity,
            frame,
            keys.collect::<Vec<_>>(),
        )
    }

    /// The encoding (see the module documentation).
    pub fn encode(&self) -> Vec<u8> {
        let header = Header {
            schema: CERTIFIED_KEYS_SCHEMA.to_owned(),
            version: P_CERTIFIED_KEYS_VERSION,
            node: self.node.clone(),
            arity: self.arity,
            count: self.len(),
            frame: self.frame.clone(),
        };
        let mut out = MAGIC.to_vec();
        out.extend(serde_json::to_vec(&header).expect("a header serialises"));
        out.push(b'\n');
        let mut previous: &[i8] = &[];
        for index in 0..self.len() {
            let row = self.row(index);
            let shared = row.iter().zip(previous).take_while(|(a, b)| a == b).count();
            out.push(shared as u8);
            for &power in &row[shared..] {
                let mut zigzag = ((i64::from(power) << 1) ^ (i64::from(power) >> 63)) as u64;
                loop {
                    let byte = (zigzag & 0x7f) as u8;
                    zigzag >>= 7;
                    if zigzag == 0 {
                        out.push(byte);
                        break;
                    }
                    out.push(byte | 0x80);
                }
            }
            previous = row;
        }
        out
    }

    /// Reads [`Self::encode`]'s bytes; a wrong schema, version, order or
    /// count is refused.
    pub fn decode(bytes: &[u8]) -> Result<Self, PError> {
        let body = bytes
            .strip_prefix(MAGIC)
            .ok_or_else(|| bad("no RRCK1 magic line"))?;
        let end = body
            .iter()
            .position(|&byte| byte == b'\n')
            .ok_or_else(|| bad("no header line"))?;
        let header: Header = serde_json::from_slice(&body[..end])
            .map_err(|error| bad(format!("header: {error}")))?;
        if header.schema != CERTIFIED_KEYS_SCHEMA {
            return Err(bad(format!("schema {:?}", header.schema)));
        }
        if header.version != P_CERTIFIED_KEYS_VERSION {
            return Err(PError::VersionMismatch {
                name: "P_CERTIFIED_KEYS_VERSION".to_owned(),
                found: header.version.to_string(),
                expected: P_CERTIFIED_KEYS_VERSION.to_string(),
            });
        }
        let arity = header.arity;
        if arity == 0 || arity > 255 {
            return Err(bad(format!("arity {arity}")));
        }
        let mut data = &body[end + 1..];
        let mut rows: Vec<i8> = Vec::with_capacity(header.count.saturating_mul(arity).min(1 << 30));
        let mut row = vec![0_i8; arity];
        let next = |data: &mut &[u8]| -> Result<u8, PError> {
            let (&byte, rest) = data.split_first().ok_or_else(|| bad("truncated"))?;
            *data = rest;
            Ok(byte)
        };
        for index in 0..header.count {
            let shared = usize::from(next(&mut data)?);
            if shared > arity || (index == 0 && shared != 0) {
                return Err(bad(format!("row {index} shares {shared} slots")));
            }
            for slot in row.iter_mut().skip(shared) {
                let (mut value, mut shift) = (0_u64, 0);
                loop {
                    let byte = next(&mut data)?;
                    if shift > 63 {
                        return Err(bad("a varint overflows"));
                    }
                    value |= u64::from(byte & 0x7f) << shift;
                    shift += 7;
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                let power = ((value >> 1) as i64) ^ -((value & 1) as i64);
                *slot = i8::try_from(power).map_err(|_| bad("a power outside i8"))?;
            }
            if index > 0 && rows[(index - 1) * arity..] >= row[..] {
                return Err(bad(format!("row {index} is not ascending")));
            }
            rows.extend_from_slice(&row);
        }
        if !data.is_empty() {
            return Err(bad(format!("{} trailing bytes", data.len())));
        }
        Ok(Self {
            node: header.node,
            arity,
            frame: header.frame,
            rows,
        })
    }

    /// Reads a `.rrkeys` file ([`Self::decode`]) or, when it does not start
    /// with the magic line, a key list ([`Self::parse_key_lines`] with
    /// `node`, `arity` and `frame`).
    pub fn read(bytes: &[u8], node: &str, arity: usize, frame: &str) -> Result<Self, PError> {
        if bytes.starts_with(MAGIC) {
            let set = Self::decode(bytes)?;
            if set.arity != arity {
                return Err(bad(format!(
                    "the set has {} slots, the table {arity}",
                    set.arity
                )));
            }
            return Ok(set);
        }
        let text = std::str::from_utf8(bytes).map_err(|error| bad(error))?;
        Self::parse_key_lines(node, arity, frame, text)
    }
}

/// `I(a,b,...)` with integer slots only.
fn parse_numeric_key(text: &str, arity: usize) -> Result<Vec<i64>, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(|| format!("key {text:?}"))?;
    let key = inner
        .split(',')
        .map(|part| part.trim().parse::<i64>())
        .collect::<Result<Vec<i64>, _>>()
        .map_err(|_| format!("key {text:?} is not numeric"))?;
    if key.len() != arity {
        return Err(format!("key {text:?} has {} slots, not {arity}", key.len()));
    }
    Ok(key)
}

/// Standard base64 (RFC 4648, padded): the certified set's line in a payload's
/// provenance text.
pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(ALPHABET[(n >> shift & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub(crate) fn base64_decode(text: &str) -> Result<Vec<u8>, PError> {
    let value = |byte: u8| -> Result<u32, PError> {
        Ok(match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(bad("base64")),
        } as u32)
    };
    let text = text.trim().as_bytes();
    if text.len() % 4 != 0 {
        return Err(bad("base64 length"));
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for chunk in text.chunks(4) {
        let pad = chunk.iter().rev().take_while(|&&byte| byte == b'=').count();
        if pad > 2 {
            return Err(bad("base64 padding"));
        }
        let mut n = 0;
        for &byte in &chunk[..4 - pad] {
            n = n << 6 | value(byte)?;
        }
        n <<= 6 * pad as u32;
        out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8][..3 - pad]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CertifiedKeySet {
        CertifiedKeySet::new(
            "T0",
            4,
            "presentation",
            vec![
                vec![1, 1, 2, 0],
                vec![1, 1, 1, 0],
                vec![1, 1, 1, -1],
                vec![2, 1, 1, -3],
                vec![1, 1, 1, 0],
                vec![1, 127, 1, -128],
            ],
        )
        .unwrap()
    }

    #[test]
    fn a_certified_set_round_trips_and_answers_membership() {
        let set = sample();
        assert_eq!(set.len(), 5);
        let bytes = set.encode();
        let back = CertifiedKeySet::decode(&bytes).unwrap();
        assert_eq!(back, set);
        assert!(back.contains(&[1, 1, 1, -1]) && back.contains(&[1, 127, 1, -128]));
        assert!(!back.contains(&[1, 1, 1, -2]) && !back.contains(&[1, 1, 1]));
        assert!(!back.contains(&[1, 1, 1, 1000]));
        let text = base64_encode(&bytes);
        assert_eq!(base64_decode(&text).unwrap(), bytes);
        for n in 0..7 {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(base64_decode(&base64_encode(&data)).unwrap(), data);
        }
    }

    #[test]
    fn a_certified_set_is_read_from_statement_rows_and_permuted() {
        let text = "[\"I(1,1,0,-1)\", \"terminal\", null, true, [0, 0]]\n\n\
                    [\"I(2,1,0,0)\", true, \"face\", \"F1#0\", 0]\nI(1,1,-1,0)\n";
        let set = CertifiedKeySet::read(text.as_bytes(), "T0", 4, "table").unwrap();
        assert_eq!(set.len(), 3);
        assert!(set.contains(&[2, 1, 0, 0]));
        let moved = set.permuted(&[1, 0, 2, 3], "presentation").unwrap();
        assert!(moved.contains(&[1, 2, 0, 0]) && !moved.contains(&[2, 1, 0, 0]));
        assert_eq!(moved.frame(), "presentation");
        assert!(set.permuted(&[0, 0, 1, 2], "x").is_err());
        // A .rrkeys file is read as such; a wrong arity is refused.
        let bytes = moved.encode();
        assert_eq!(
            CertifiedKeySet::read(&bytes, "T0", 4, "table").unwrap(),
            moved
        );
        assert!(CertifiedKeySet::read(&bytes, "T0", 5, "table").is_err());
        assert!(CertifiedKeySet::read(b"I(1,n1,0,0)\n", "T0", 4, "table").is_err());
    }

    #[test]
    fn a_tampered_encoding_is_refused() {
        let bytes = sample().encode();
        assert!(CertifiedKeySet::decode(&bytes[..bytes.len() - 1]).is_err());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(CertifiedKeySet::decode(&longer).is_err());
        let needle = b"\"version\":1";
        let at = bytes
            .windows(needle.len())
            .position(|w| w == needle)
            .unwrap();
        let mut other = bytes.clone();
        other[at + needle.len() - 1] = b'9';
        assert!(matches!(
            CertifiedKeySet::decode(&other),
            Err(PError::VersionMismatch { .. })
        ));
    }
}
