//! Compact key sets: packed keys (a few `u64` words per key, no per-key heap allocation), sorted canonical sets with
//! a codec-independent digest, streaming key-set files, and an append-only open-addressing table over packed codes.
//!
//! Packing ([`KeyCodec`]): every slot gets `bits` bits holding `v + 2^(bits-1)`; `64 / bits` slots per word, slot 0
//! in the most significant bits of word 0, so the order of codes (word by word) is the lexicographic order of keys.
//! A value outside the codec range is refused (`encode` returns false), never wrapped.

use std::io::{self, BufRead, Write};

use super::sha256::Sha256;

/// Maximum number of u64 words per packed key (e.g. 32 slots at 16 bits, 80 slots at 6 bits).
pub const MAX_WORDS: usize = 8;

pub const KEYSET_SCHEMA: &str = "rustred.campaign.keyset.v1";

/// Errors of key sets and key-set files.
#[derive(Debug)]
pub enum KeySetError {
    /// Codec parameters out of range (bits 4..=16, at most MAX_WORDS words).
    Codec(String),
    /// A key of the wrong length.
    Arity {
        expected: usize,
        got: usize,
    },
    /// A key outside the codec range.
    Range(Vec<i32>),
    /// Malformed file (header, order, count, digest).
    Format(String),
    Io(io::Error),
}

impl std::fmt::Display for KeySetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeySetError::Codec(s) => write!(f, "key codec: {s}"),
            KeySetError::Arity { expected, got } => {
                write!(f, "key of arity {got}, expected {expected}")
            }
            KeySetError::Range(k) => write!(f, "key {k:?} outside the codec range"),
            KeySetError::Format(s) => write!(f, "key-set file: {s}"),
            KeySetError::Io(e) => write!(f, "key-set io: {e}"),
        }
    }
}
impl std::error::Error for KeySetError {}
impl From<io::Error> for KeySetError {
    fn from(e: io::Error) -> Self {
        KeySetError::Io(e)
    }
}

/// Packing of keys of one arity into `words` u64.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyCodec {
    arity: usize,
    bits: u32,
    per_word: usize,
    words: usize,
}

impl KeyCodec {
    /// `bits` in 4..=16; the key must fit into at most [`MAX_WORDS`] words (else a typed `Codec` error).
    pub fn new(arity: usize, bits: u32) -> Result<KeyCodec, KeySetError> {
        if !(4..=16).contains(&bits) {
            return Err(KeySetError::Codec(format!("bits {bits} not in 4..=16")));
        }
        if arity == 0 {
            return Err(KeySetError::Codec("arity 0".into()));
        }
        let per_word = 64 / bits as usize;
        let words = arity.div_ceil(per_word);
        if words > MAX_WORDS {
            return Err(KeySetError::Codec(format!(
                "arity {arity} at {bits} bits needs {words} words (max {MAX_WORDS})"
            )));
        }
        Ok(KeyCodec {
            arity,
            bits,
            per_word,
            words,
        })
    }
    pub fn arity(&self) -> usize {
        self.arity
    }
    pub fn bits(&self) -> u32 {
        self.bits
    }
    pub fn words(&self) -> usize {
        self.words
    }
    /// Representable values per slot (inclusive).
    pub fn range(&self) -> (i32, i32) {
        let b = 1i32 << (self.bits - 1);
        (-b, b - 1)
    }
    #[inline]
    fn place(&self, s: usize) -> (usize, u32) {
        let w = s / self.per_word;
        let shift = 64 - self.bits * (s % self.per_word + 1) as u32;
        (w, shift)
    }
    /// Pack `key` into `out` (length `words`). False if a value is outside the range or the arity differs.
    #[inline]
    pub fn encode(&self, key: &[i32], out: &mut [u64]) -> bool {
        if key.len() != self.arity {
            return false;
        }
        let bias = 1i64 << (self.bits - 1);
        let max = (1i64 << self.bits) - 1;
        out[..self.words].iter_mut().for_each(|w| *w = 0);
        for (s, &v) in key.iter().enumerate() {
            let x = v as i64 + bias;
            if !(0..=max).contains(&x) {
                return false;
            }
            let (w, sh) = self.place(s);
            out[w] |= (x as u64) << sh;
        }
        true
    }
    /// Unpack `code` into `out` (length `arity`).
    #[inline]
    pub fn decode(&self, code: &[u64], out: &mut [i32]) {
        let bias = 1i64 << (self.bits - 1);
        let mask = (1u64 << self.bits) - 1;
        for (s, o) in out.iter_mut().enumerate().take(self.arity) {
            let (w, sh) = self.place(s);
            *o = (((code[w] >> sh) & mask) as i64 - bias) as i32;
        }
    }
    pub fn decode_vec(&self, code: &[u64]) -> Vec<i32> {
        let mut k = vec![0; self.arity];
        self.decode(code, &mut k);
        k
    }
}

/// Sort `data` (records of `w` words) and remove duplicates, in place.
pub(crate) fn sort_dedup(w: usize, data: &mut Vec<u64>) {
    fn go<const W: usize>(data: &mut Vec<u64>) {
        let (ch, rest) = data.as_chunks_mut::<W>();
        debug_assert!(rest.is_empty());
        ch.sort_unstable();
        let mut n = 0usize;
        for i in 0..ch.len() {
            if n == 0 || ch[i] != ch[n - 1] {
                ch[n] = ch[i];
                n += 1;
            }
        }
        data.truncate(n * W);
    }
    match w {
        1 => go::<1>(data),
        2 => go::<2>(data),
        3 => go::<3>(data),
        4 => go::<4>(data),
        5 => go::<5>(data),
        6 => go::<6>(data),
        7 => go::<7>(data),
        8 => go::<8>(data),
        _ => unreachable!("codec words <= MAX_WORDS"),
    }
}

/// Canonical digest stream of a key set (independent of the codec bits).
pub(crate) struct DigestStream {
    h: Sha256,
    buf: Vec<u8>,
}

impl DigestStream {
    pub(crate) fn new(arity: usize) -> Self {
        let mut h = Sha256::new();
        h.update(KEYSET_SCHEMA.as_bytes());
        h.update(b"\n");
        h.update(&(arity as u32).to_le_bytes());
        DigestStream {
            h,
            buf: Vec::with_capacity(1 << 16),
        }
    }
    pub(crate) fn key(&mut self, k: &[i32]) {
        for v in k {
            self.buf.extend_from_slice(&v.to_le_bytes());
        }
        if self.buf.len() >= 1 << 16 {
            self.h.update(&self.buf);
            self.buf.clear();
        }
    }
    pub(crate) fn finish(mut self) -> String {
        self.h.update(&self.buf);
        format!("sha256:{}", self.h.finish_hex())
    }
}

/// A sorted, de-duplicated set of packed keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySet {
    codec: KeyCodec,
    data: Vec<u64>,
}

impl KeySet {
    /// Empty set.
    pub fn empty(codec: KeyCodec) -> KeySet {
        KeySet {
            codec,
            data: vec![],
        }
    }
    /// Build from keys (any order, duplicates allowed). A key of the wrong length or outside the codec range is an
    /// error (fail closed).
    pub fn from_keys<'a>(
        codec: KeyCodec,
        keys: impl IntoIterator<Item = &'a [i32]>,
    ) -> Result<KeySet, KeySetError> {
        let w = codec.words;
        let mut data = vec![];
        let mut code = [0u64; MAX_WORDS];
        for k in keys {
            if k.len() != codec.arity {
                return Err(KeySetError::Arity {
                    expected: codec.arity,
                    got: k.len(),
                });
            }
            if !codec.encode(k, &mut code) {
                return Err(KeySetError::Range(k.to_vec()));
            }
            data.extend_from_slice(&code[..w]);
        }
        sort_dedup(w, &mut data);
        Ok(KeySet { codec, data })
    }
    /// From packed codes (any order, duplicates allowed).
    pub(crate) fn from_codes(codec: KeyCodec, mut data: Vec<u64>) -> KeySet {
        sort_dedup(codec.words, &mut data);
        KeySet { codec, data }
    }
    pub fn codec(&self) -> &KeyCodec {
        &self.codec
    }
    pub fn len(&self) -> usize {
        self.data.len() / self.codec.words
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub(crate) fn raw_codes(&self) -> &[u64] {
        &self.data
    }
    pub(crate) fn code(&self, i: usize) -> &[u64] {
        let w = self.codec.words;
        &self.data[i * w..(i + 1) * w]
    }
    /// Key number `i` (lexicographic order).
    pub fn get(&self, i: usize) -> Vec<i32> {
        self.codec.decode_vec(self.code(i))
    }
    pub fn iter(&self) -> impl Iterator<Item = Vec<i32>> + '_ {
        (0..self.len()).map(|i| self.get(i))
    }
    fn position(&self, code: &[u64]) -> Result<usize, usize> {
        let (mut lo, mut hi) = (0usize, self.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.code(mid).cmp(code) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Ok(mid),
            }
        }
        Err(lo)
    }
    pub fn contains(&self, key: &[i32]) -> bool {
        let mut code = [0u64; MAX_WORDS];
        self.codec.encode(key, &mut code) && self.position(&code[..self.codec.words]).is_ok()
    }
    /// The position of `key` in the set's (lexicographic) order, if present (lane import2: per-key marks of a
    /// streamed pass in bitsets indexed by this position).
    pub fn index_of(&self, key: &[i32]) -> Option<usize> {
        let mut code = [0u64; MAX_WORDS];
        if !self.codec.encode(key, &mut code) {
            return None;
        }
        self.position(&code[..self.codec.words]).ok()
    }
    /// `"sha256:<hex>"` over the canonical stream (module documentation of [`KEYSET_SCHEMA`] files).
    pub fn digest(&self) -> String {
        let mut d = DigestStream::new(self.codec.arity);
        let mut k = vec![0; self.codec.arity];
        for i in 0..self.len() {
            self.codec.decode(self.code(i), &mut k);
            d.key(&k);
        }
        d.finish()
    }
    pub fn memory_bytes(&self) -> usize {
        self.data.capacity() * 8 + std::mem::size_of::<Self>()
    }
    /// Re-pack under another codec (error if a key does not fit).
    pub fn recode(&self, codec: KeyCodec) -> Result<KeySet, KeySetError> {
        if codec.arity != self.codec.arity {
            return Err(KeySetError::Arity {
                expected: codec.arity,
                got: self.codec.arity,
            });
        }
        let keys: Vec<Vec<i32>> = self.iter().collect();
        KeySet::from_keys(codec, keys.iter().map(|k| k.as_slice()))
    }
    fn same_codec(&self, o: &KeySet) {
        assert_eq!(self.codec, o.codec, "key sets with different codecs");
    }
    /// Set union (same codec).
    pub fn union(&self, o: &KeySet) -> KeySet {
        self.same_codec(o);
        let mut d = self.data.clone();
        d.extend_from_slice(&o.data);
        KeySet::from_codes(self.codec, d)
    }
    /// Keys of `self` not in `o` (same codec).
    pub fn difference(&self, o: &KeySet) -> KeySet {
        self.same_codec(o);
        let mut d = vec![];
        for i in 0..self.len() {
            if o.position(self.code(i)).is_err() {
                d.extend_from_slice(self.code(i));
            }
        }
        KeySet {
            codec: self.codec,
            data: d,
        }
    }
    /// Write the key-set file; returns the digest.
    pub fn write(&self, mut w: impl Write) -> io::Result<String> {
        let digest = self.digest();
        let header = serde_json::json!({
            "schema": KEYSET_SCHEMA,
            "arity": self.codec.arity,
            "bits": self.codec.bits,
            "words": self.codec.words,
            "count": self.len(),
            "digest": digest,
        });
        writeln!(w, "{header}")?;
        let mut buf = Vec::with_capacity(1 << 16);
        for x in &self.data {
            buf.extend_from_slice(&x.to_le_bytes());
            if buf.len() >= 1 << 16 {
                w.write_all(&buf)?;
                buf.clear();
            }
        }
        w.write_all(&buf)?;
        w.flush()?;
        Ok(digest)
    }
    /// Read a key-set file (streaming): header, then exactly `count` strictly increasing codes; the digest is
    /// recomputed and must equal the header's.
    pub fn read(mut r: impl BufRead) -> Result<KeySet, KeySetError> {
        let mut line = String::new();
        r.read_line(&mut line)?;
        let h: serde_json::Value = serde_json::from_str(line.trim_end())
            .map_err(|e| KeySetError::Format(format!("header: {e}")))?;
        if h["schema"] != KEYSET_SCHEMA {
            return Err(KeySetError::Format(format!("schema {}", h["schema"])));
        }
        let field = |f: &str| {
            h[f].as_u64()
                .ok_or_else(|| KeySetError::Format(format!("header field {f}")))
        };
        let codec = KeyCodec::new(field("arity")? as usize, field("bits")? as u32)?;
        if field("words")? as usize != codec.words {
            return Err(KeySetError::Format(
                "words do not match arity and bits".into(),
            ));
        }
        let count = field("count")? as usize;
        let want = h["digest"].as_str().unwrap_or("").to_string();
        let w = codec.words;
        let mut data = Vec::with_capacity(count * w);
        let mut b8 = [0u8; 8];
        let mut dig = DigestStream::new(codec.arity);
        let mut k = vec![0; codec.arity];
        for i in 0..count {
            for _ in 0..w {
                r.read_exact(&mut b8)
                    .map_err(|e| KeySetError::Format(format!("truncated at key {i}: {e}")))?;
                data.push(u64::from_le_bytes(b8));
            }
            let cur = &data[i * w..];
            if i > 0 && data[(i - 1) * w..i * w] >= *cur {
                return Err(KeySetError::Format(format!(
                    "key {i} not strictly increasing"
                )));
            }
            codec.decode(cur, &mut k);
            dig.key(&k);
        }
        let mut extra = [0u8; 1];
        if r.read(&mut extra)? != 0 {
            return Err(KeySetError::Format("trailing bytes".into()));
        }
        let got = dig.finish();
        if got != want {
            return Err(KeySetError::Format(format!(
                "digest {got} != header {want}"
            )));
        }
        Ok(KeySet { codec, data })
    }
}

/// Streaming builder of a [`KeySet`] (no per-key allocation): push keys in any order, then `finish`.
#[derive(Clone, Debug)]
pub struct KeySetBuilder {
    codec: KeyCodec,
    data: Vec<u64>,
    code: [u64; MAX_WORDS],
}

impl KeySetBuilder {
    pub fn new(codec: KeyCodec) -> Self {
        KeySetBuilder {
            codec,
            data: vec![],
            code: [0; MAX_WORDS],
        }
    }
    /// Add one key (error if it does not fit the codec).
    pub fn push(&mut self, key: &[i32]) -> Result<(), KeySetError> {
        if key.len() != self.codec.arity {
            return Err(KeySetError::Arity {
                expected: self.codec.arity,
                got: key.len(),
            });
        }
        if !self.codec.encode(key, &mut self.code) {
            return Err(KeySetError::Range(key.to_vec()));
        }
        self.data.extend_from_slice(&self.code[..self.codec.words]);
        Ok(())
    }
    pub fn finish(self) -> KeySet {
        KeySet::from_codes(self.codec, self.data)
    }
}

#[inline]
fn hash_code(c: &[u64]) -> u64 {
    let mut h = 0x9e37_79b9_7f4a_7c15u64;
    for &x in c {
        h = (h ^ x).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 29;
    }
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 32)
}

/// Append-only table of packed codes: insertion index per key (u32), open addressing over u32 slots (load <= 1/2).
/// Lookups are lock-free reads (`&self`); inserts need `&mut self`.
#[derive(Clone, Debug)]
pub struct KeyTable {
    codec: KeyCodec,
    data: Vec<u64>,
    slots: Vec<u32>,
    mask: usize,
}

const EMPTY: u32 = u32::MAX;

impl KeyTable {
    pub fn new(codec: KeyCodec, capacity_hint: usize) -> KeyTable {
        let cap = (capacity_hint.max(8) * 2).next_power_of_two();
        KeyTable {
            codec,
            data: Vec::with_capacity(capacity_hint * codec.words),
            slots: vec![EMPTY; cap],
            mask: cap - 1,
        }
    }
    pub fn codec(&self) -> &KeyCodec {
        &self.codec
    }
    pub fn len(&self) -> usize {
        self.data.len() / self.codec.words
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    #[inline]
    pub fn code(&self, i: usize) -> &[u64] {
        let w = self.codec.words;
        &self.data[i * w..(i + 1) * w]
    }
    pub fn key(&self, i: usize) -> Vec<i32> {
        self.codec.decode_vec(self.code(i))
    }
    #[inline]
    pub fn get(&self, code: &[u64]) -> Option<u32> {
        let mut p = hash_code(code) as usize & self.mask;
        loop {
            let s = self.slots[p];
            if s == EMPTY {
                return None;
            }
            if self.code(s as usize) == code {
                return Some(s);
            }
            p = (p + 1) & self.mask;
        }
    }
    pub fn get_key(&self, key: &[i32]) -> Option<u32> {
        let mut c = [0u64; MAX_WORDS];
        if !self.codec.encode(key, &mut c) {
            return None;
        }
        self.get(&c[..self.codec.words])
    }
    fn grow(&mut self) {
        let cap = self.slots.len() * 2;
        self.slots = vec![EMPTY; cap];
        self.mask = cap - 1;
        for i in 0..self.len() {
            let mut p = hash_code(self.code(i)) as usize & self.mask;
            while self.slots[p] != EMPTY {
                p = (p + 1) & self.mask;
            }
            self.slots[p] = i as u32;
        }
    }
    /// Insert; returns (index, newly inserted).
    pub fn insert(&mut self, code: &[u64]) -> (u32, bool) {
        if let Some(i) = self.get(code) {
            return (i, false);
        }
        if (self.len() + 1) * 2 > self.slots.len() {
            self.grow();
        }
        assert!(
            self.len() < EMPTY as usize - 1,
            "key table full (u32 indices)"
        );
        let i = self.len() as u32;
        self.data.extend_from_slice(code);
        let mut p = hash_code(code) as usize & self.mask;
        while self.slots[p] != EMPTY {
            p = (p + 1) & self.mask;
        }
        self.slots[p] = i;
        (i, true)
    }
    pub fn memory_bytes(&self) -> usize {
        self.data.capacity() * 8 + self.slots.capacity() * 4 + std::mem::size_of::<Self>()
    }
    /// The keys as a canonical [`KeySet`].
    pub fn to_keyset(&self) -> KeySet {
        KeySet::from_codes(self.codec, self.data.clone())
    }
    /// The keys with insertion index in `range`, as a [`KeySet`].
    pub fn range_keyset(&self, range: std::ops::Range<usize>) -> KeySet {
        let w = self.codec.words;
        KeySet::from_codes(
            self.codec,
            self.data[range.start * w..range.end * w].to_vec(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_order_is_lexicographic() {
        let c = KeyCodec::new(20, 6).unwrap();
        assert_eq!(c.words(), 2);
        let a = [
            1, 2, 3, 0, -1, -32, 31, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        ];
        let mut b = a;
        b[11] = 2;
        let (mut ca, mut cb) = ([0u64; MAX_WORDS], [0u64; MAX_WORDS]);
        assert!(c.encode(&a, &mut ca) && c.encode(&b, &mut cb));
        assert!(ca[..2] < cb[..2]);
        assert_eq!(c.decode_vec(&ca[..2]), a.to_vec());
        let mut bad = a;
        bad[0] = 32;
        assert!(!c.encode(&bad, &mut ca));
    }

    #[test]
    fn file_round_trip_and_digest_independent_of_bits() {
        let keys: Vec<Vec<i32>> = (0..500).map(|i| vec![i % 7, -(i % 5), i % 3, 1]).collect();
        let s6 = KeySet::from_keys(
            KeyCodec::new(4, 6).unwrap(),
            keys.iter().map(|k| k.as_slice()),
        )
        .unwrap();
        let s9 = KeySet::from_keys(
            KeyCodec::new(4, 9).unwrap(),
            keys.iter().map(|k| k.as_slice()),
        )
        .unwrap();
        assert_eq!(s6.len(), 105);
        assert_eq!(s6.digest(), s9.digest());
        let mut buf = vec![];
        let d = s6.write(&mut buf).unwrap();
        assert_eq!(d, s6.digest());
        let back = KeySet::read(&buf[..]).unwrap();
        assert_eq!(back, s6);
        let n = buf.len();
        buf[n - 1] ^= 1;
        assert!(KeySet::read(&buf[..]).is_err());
    }

    #[test]
    fn table_insert_get() {
        let c = KeyCodec::new(3, 8).unwrap();
        let mut t = KeyTable::new(c, 2);
        let mut code = [0u64; MAX_WORDS];
        for i in 0..1000 {
            c.encode(&[i % 100, i / 100, 0], &mut code);
            let (ix, _) = t.insert(&code[..1]);
            assert_eq!(t.key(ix as usize), vec![i % 100, i / 100, 0]);
        }
        assert_eq!(t.len(), 1000);
        assert_eq!(t.get_key(&[5, 3, 0]), Some(305));
        assert_eq!(t.get_key(&[5, 3, 1]), None);
    }
}
