//! Leaf sets (TRICKS item 10, first half; interfaces/closure.md section 18): close a box LAYER BY LAYER, every earlier
//! layer's closed result serving as LEAVES of the later walks. A leaf set is a compact SORTED KEY TABLE (packed codes of
//! the run's codec + one class byte, one level byte and one flag byte per key, plus the AUX keys its dispatch depended on:
//! the images of its credited keys) with a BINDING: the statement it was taken from, the clause's verdict, and the walk
//! components that decide what serves a key (tables, order, engine, and the relation sets / terminals / exclusions
//! RESTRICTED to its keys). The walk stops at a leaf (no dispatch, no expansion); the binding is verified inside the run
//! before the walk and a mismatch refuses the run. Nothing here knows a node, a count or an arity (L8).

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::keyset::{KeyCodec, KeySet, KeySetError, MAX_WORDS, sort_dedup};
use super::relations::RelationSet;
use super::sha256::Sha256;
use super::{ConfigIdentity, KeyExclusion};

/// Schema of the leaf-set file.
pub const LEAFSET_SCHEMA: &str = "rustred.campaign.leafset.v1";
/// Version of the leaf semantics (bump on any change of what a leaf means to the walk).
pub const LEAF_SEMANTICS: &str = "leaf-semantics.v1";
/// Flag bit: the key is certified in its clause (served at the required level on a strict basis, or a declared terminal).
pub const LEAF_CERTIFIED: u8 = 1;

/// Identity components copied verbatim into a leaf set's walk binding (what decides what serves a key). The components
/// `relations`, `terminals` and `exclusions` enter RESTRICTED to the leaf set's keys (see [`walk_components`]); facts,
/// start set, inputs, tier, hand-down mode and round limit never enter. The witness-absent policy enters as CONFIGURED
/// (`witness_absent_policy`), not as the identity's tier-effective word (Tier K always reports `block`): a leaf set of
/// the exact tier serves the keys-only walk too.
pub const WALK_COMPONENTS: &[&str] = &[
    "tool",
    "frame",
    "bits",
    "lines",
    "comparator",
    "dispatch",
    "bodies",
    "table_sources",
    "credits",
    "zero_sectors",
    "per_key_lift",
    "dispatcher_summary",
    "diagnostic",
];

/// What a leaf set is bound to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeafBinding {
    /// sha256 (hex) of the statement rows the leaves were taken from (the closed clause's statement).
    pub statement_digest: String,
    /// The clause's verdict: `closed` | `open` | ... (only `closed` lets a later walk close relative to it).
    pub source_verdict: String,
    /// Producer node and clause / layer id (data, free names).
    pub node: String,
    pub clause: String,
    /// The clause's start-set digest (closure key-set digest).
    pub start_digest: String,
    /// The walk components (name -> digest) of the producing run for these keys ([`walk_components`]).
    pub walk: BTreeMap<String, String>,
    /// Leaf sets the clause was itself closed relative to (digests, in order).
    pub relative_to: Vec<String>,
    /// Free provenance (never compared): table-set digest, engine binary, ...
    pub provenance: BTreeMap<String, String>,
}

/// Errors of leaf sets (format, ranges, duplicates).
#[derive(Debug)]
pub enum LeafError {
    KeySet(KeySetError),
    Format(String),
    Io(io::Error),
}

impl std::fmt::Display for LeafError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LeafError::KeySet(e) => write!(f, "{e:?}"),
            LeafError::Format(s) => write!(f, "leaf set: {s}"),
            LeafError::Io(e) => write!(f, "leaf set io: {e}"),
        }
    }
}
impl std::error::Error for LeafError {}
impl From<KeySetError> for LeafError {
    fn from(e: KeySetError) -> Self {
        LeafError::KeySet(e)
    }
}
impl From<io::Error> for LeafError {
    fn from(e: io::Error) -> Self {
        LeafError::Io(e)
    }
}

/// One leaf, materialised.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeafEntry {
    pub key: Vec<i32>,
    pub class: String,
    pub level: String,
    pub certified: bool,
}

/// A leaf set (see the module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeafSet {
    codec: KeyCodec,
    codes: Vec<u64>,
    class: Vec<u8>,
    level: Vec<u8>,
    flags: Vec<u8>,
    aux: Vec<u64>,
    classes: Vec<String>,
    levels: Vec<String>,
    binding: LeafBinding,
    digest: String,
}

fn position(codes: &[u64], w: usize, code: &[u64]) -> Result<usize, usize> {
    let (mut lo, mut hi) = (0usize, codes.len() / w);
    while lo < hi {
        let mid = (lo + hi) / 2;
        match codes[mid * w..(mid + 1) * w].cmp(code) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return Ok(mid),
        }
    }
    Err(lo)
}

fn header_canon(
    arity: usize,
    classes: &[String],
    levels: &[String],
    binding: &LeafBinding,
) -> String {
    let v = serde_json::json!({"arity": arity, "classes": classes, "levels": levels, "binding": binding});
    crate::schema::canon::canonical_json(&v, false).unwrap_or_else(|_| v.to_string())
}

/// The canonical digest stream of a leaf set (independent of the codec bits).
struct LeafDigest {
    h: Sha256,
    buf: Vec<u8>,
}

impl LeafDigest {
    fn new(arity: usize, classes: &[String], levels: &[String], binding: &LeafBinding) -> Self {
        let mut h = Sha256::new();
        h.update(LEAFSET_SCHEMA.as_bytes());
        h.update(b"\n");
        h.update(header_canon(arity, classes, levels, binding).as_bytes());
        LeafDigest {
            h,
            buf: Vec::with_capacity(1 << 16),
        }
    }
    fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
        if self.buf.len() >= 1 << 16 {
            self.h.update(&self.buf);
            self.buf.clear();
        }
    }
    fn key(&mut self, k: &[i32]) {
        for v in k {
            self.bytes(&v.to_le_bytes());
        }
    }
    fn finish(mut self) -> String {
        self.h.update(&self.buf);
        format!("sha256:{}", self.h.finish_hex())
    }
}

impl LeafSet {
    pub fn codec(&self) -> &KeyCodec {
        &self.codec
    }
    pub fn len(&self) -> usize {
        self.codes.len() / self.codec.words()
    }
    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }
    pub fn binding(&self) -> &LeafBinding {
        &self.binding
    }
    pub fn classes(&self) -> &[String] {
        &self.classes
    }
    pub fn levels(&self) -> &[String] {
        &self.levels
    }
    /// `"sha256:<hex>"` over the canonical stream (closure.md 18).
    pub fn digest(&self) -> &str {
        &self.digest
    }
    /// Position of `key` (lexicographic order), if a leaf.
    pub fn lookup(&self, key: &[i32]) -> Option<usize> {
        let mut c = [0u64; MAX_WORDS];
        if key.len() != self.codec.arity() || !self.codec.encode(key, &mut c) {
            return None;
        }
        position(&self.codes, self.codec.words(), &c[..self.codec.words()]).ok()
    }
    pub fn contains(&self, key: &[i32]) -> bool {
        self.lookup(key).is_some()
    }
    /// Whether `key` is an AUX key (an image the set's dispatch depended on).
    pub fn aux_contains(&self, key: &[i32]) -> bool {
        let mut c = [0u64; MAX_WORDS];
        if key.len() != self.codec.arity() || !self.codec.encode(key, &mut c) {
            return false;
        }
        position(&self.aux, self.codec.words(), &c[..self.codec.words()]).is_ok()
    }
    pub fn key(&self, i: usize) -> Vec<i32> {
        let w = self.codec.words();
        self.codec.decode_vec(&self.codes[i * w..(i + 1) * w])
    }
    pub fn is_certified(&self, i: usize) -> bool {
        self.flags[i] & LEAF_CERTIFIED != 0
    }
    pub fn class_of(&self, i: usize) -> &str {
        &self.classes[self.class[i] as usize]
    }
    pub fn level_of(&self, i: usize) -> &str {
        &self.levels[self.level[i] as usize]
    }
    pub fn entry(&self, i: usize) -> LeafEntry {
        LeafEntry {
            key: self.key(i),
            class: self.class_of(i).to_string(),
            level: self.level_of(i).to_string(),
            certified: self.is_certified(i),
        }
    }
    /// Number of keys that are not certified.
    pub fn n_uncertified(&self) -> usize {
        self.flags
            .iter()
            .filter(|&&f| f & LEAF_CERTIFIED == 0)
            .count()
    }
    /// Closed: the clause's verdict is `closed` and every key is certified.
    pub fn is_closed(&self) -> bool {
        self.binding.source_verdict == "closed" && self.n_uncertified() == 0
    }
    pub fn aux_len(&self) -> usize {
        self.aux.len() / self.codec.words()
    }
    /// The keys as a key set (same codec).
    pub fn keyset(&self) -> KeySet {
        KeySet::from_codes(self.codec, self.codes.clone())
    }
    /// Resident bytes (codes, three bytes per key, aux codes).
    pub fn memory_bytes(&self) -> usize {
        self.codes.capacity() * 8
            + self.class.capacity()
            + self.level.capacity()
            + self.flags.capacity()
            + self.aux.capacity() * 8
            + std::mem::size_of::<Self>()
    }
    /// Re-pack under another codec of the same arity (the digest is codec-independent and unchanged).
    pub fn recode(&self, codec: KeyCodec) -> Result<LeafSet, LeafError> {
        if codec == self.codec {
            return Ok(self.clone());
        }
        if codec.arity() != self.codec.arity() {
            return Err(LeafError::Format(format!(
                "arity {} != {}",
                codec.arity(),
                self.codec.arity()
            )));
        }
        let mut b = LeafSetBuilder::new(codec, self.classes.clone(), self.levels.clone());
        for i in 0..self.len() {
            b.push_raw(&self.key(i), self.class[i], self.level[i], self.flags[i])?;
        }
        let w = self.codec.words();
        for c in self.aux.chunks_exact(w) {
            b.aux(&self.codec.decode_vec(c))?;
        }
        let s = b.finish(self.binding.clone())?;
        debug_assert_eq!(s.digest, self.digest);
        Ok(s)
    }
    fn compute_digest(&self) -> String {
        let arity = self.codec.arity();
        let mut d = LeafDigest::new(arity, &self.classes, &self.levels, &self.binding);
        let w = self.codec.words();
        let mut k = vec![0i32; arity];
        for i in 0..self.len() {
            self.codec.decode(&self.codes[i * w..(i + 1) * w], &mut k);
            d.key(&k);
            d.bytes(&[self.class[i], self.level[i], self.flags[i]]);
        }
        d.bytes(b"aux");
        d.bytes(&(self.aux_len() as u32).to_le_bytes());
        for c in self.aux.chunks_exact(w) {
            self.codec.decode(c, &mut k);
            d.key(&k);
        }
        d.finish()
    }
    /// Write the leaf-set file; returns the digest.
    pub fn write(&self, mut w: impl Write) -> io::Result<String> {
        let header = serde_json::json!({
            "schema": LEAFSET_SCHEMA, "arity": self.codec.arity(), "bits": self.codec.bits(),
            "words": self.codec.words(), "count": self.len(), "aux_count": self.aux_len(),
            "classes": self.classes, "levels": self.levels, "binding": self.binding, "digest": self.digest,
        });
        writeln!(w, "{header}")?;
        let mut buf = Vec::with_capacity(1 << 16);
        let flush = |buf: &mut Vec<u8>, w: &mut dyn Write, force: bool| -> io::Result<()> {
            if force || buf.len() >= 1 << 16 {
                w.write_all(buf)?;
                buf.clear();
            }
            Ok(())
        };
        for x in &self.codes {
            buf.extend_from_slice(&x.to_le_bytes());
            flush(&mut buf, &mut w, false)?;
        }
        for v in [&self.class, &self.level, &self.flags] {
            buf.extend_from_slice(v);
            flush(&mut buf, &mut w, false)?;
        }
        for x in &self.aux {
            buf.extend_from_slice(&x.to_le_bytes());
            flush(&mut buf, &mut w, false)?;
        }
        flush(&mut buf, &mut w, true)?;
        w.flush()?;
        Ok(self.digest.clone())
    }
    /// Read a leaf-set file (streaming): header, codes strictly increasing, class / level indices in range, aux codes
    /// strictly increasing, no trailing bytes; the digest is recomputed and must equal the header's.
    pub fn read(mut r: impl BufRead) -> Result<LeafSet, LeafError> {
        let mut line = String::new();
        r.read_line(&mut line)?;
        let h: serde_json::Value = serde_json::from_str(line.trim_end())
            .map_err(|e| LeafError::Format(format!("header: {e}")))?;
        if h["schema"] != LEAFSET_SCHEMA {
            return Err(LeafError::Format(format!("schema {}", h["schema"])));
        }
        let field = |f: &str| {
            h[f].as_u64()
                .ok_or_else(|| LeafError::Format(format!("header field {f}")))
        };
        let codec = KeyCodec::new(field("arity")? as usize, field("bits")? as u32)?;
        if field("words")? as usize != codec.words() {
            return Err(LeafError::Format(
                "words do not match arity and bits".into(),
            ));
        }
        let count = field("count")? as usize;
        let aux_count = field("aux_count")? as usize;
        let strs = |f: &str| -> Result<Vec<String>, LeafError> {
            serde_json::from_value(h[f].clone())
                .map_err(|e| LeafError::Format(format!("header field {f}: {e}")))
        };
        let classes = strs("classes")?;
        let levels = strs("levels")?;
        if classes.len() > 256 || levels.len() > 256 {
            return Err(LeafError::Format("more than 256 classes or levels".into()));
        }
        let binding: LeafBinding = serde_json::from_value(h["binding"].clone())
            .map_err(|e| LeafError::Format(format!("binding: {e}")))?;
        let want = h["digest"].as_str().unwrap_or("").to_string();
        let w = codec.words();
        let read_codes =
            |r: &mut dyn BufRead, n: usize, what: &str| -> Result<Vec<u64>, LeafError> {
                let mut data = Vec::with_capacity(n * w);
                let mut b8 = [0u8; 8];
                for i in 0..n {
                    for _ in 0..w {
                        r.read_exact(&mut b8).map_err(|e| {
                            LeafError::Format(format!("{what}: truncated at key {i}: {e}"))
                        })?;
                        data.push(u64::from_le_bytes(b8));
                    }
                    if i > 0 && data[(i - 1) * w..i * w] >= data[i * w..(i + 1) * w] {
                        return Err(LeafError::Format(format!(
                            "{what}: key {i} not strictly increasing"
                        )));
                    }
                }
                Ok(data)
            };
        let codes = read_codes(&mut r, count, "codes")?;
        let mut bytes = |n: usize, what: &str| -> Result<Vec<u8>, LeafError> {
            let mut v = vec![0u8; n];
            r.read_exact(&mut v)
                .map_err(|e| LeafError::Format(format!("{what}: truncated: {e}")))?;
            Ok(v)
        };
        let class = bytes(count, "classes")?;
        let level = bytes(count, "levels")?;
        let flags = bytes(count, "flags")?;
        let aux = read_codes(&mut r, aux_count, "aux")?;
        let mut extra = [0u8; 1];
        if r.read(&mut extra)? != 0 {
            return Err(LeafError::Format("trailing bytes".into()));
        }
        if class.iter().any(|&c| c as usize >= classes.len())
            || level.iter().any(|&c| c as usize >= levels.len())
        {
            return Err(LeafError::Format("class / level index out of range".into()));
        }
        if flags.iter().any(|&f| f & !LEAF_CERTIFIED != 0) {
            return Err(LeafError::Format("unknown flag bits".into()));
        }
        let mut s = LeafSet {
            codec,
            codes,
            class,
            level,
            flags,
            aux,
            classes,
            levels,
            binding,
            digest: String::new(),
        };
        let got = s.compute_digest();
        if got != want {
            return Err(LeafError::Format(format!("digest {got} != header {want}")));
        }
        s.digest = got;
        Ok(s)
    }
    /// Read from a file path.
    pub fn read_path(p: &std::path::Path) -> Result<LeafSet, LeafError> {
        let f = std::fs::File::open(p)
            .map_err(|e| LeafError::Format(format!("{}: {e}", p.display())))?;
        LeafSet::read(io::BufReader::with_capacity(1 << 20, f))
    }
}

/// Builder of a [`LeafSet`]: push keys (any order) with their class / level words and certification, aux keys, then
/// `finish(binding)`. A key pushed twice with different data is an error; vocabularies grow as words are first seen.
#[derive(Clone, Debug)]
pub struct LeafSetBuilder {
    codec: KeyCodec,
    classes: Vec<String>,
    levels: Vec<String>,
    /// (code words, class, level, flags) records, `words + 1` u64 each (the last packs the three bytes)
    rec: Vec<u64>,
    aux: Vec<u64>,
}

impl LeafSetBuilder {
    /// `classes` / `levels`: initial vocabularies (may be empty; extended on first use, at most 256 each).
    pub fn new(codec: KeyCodec, classes: Vec<String>, levels: Vec<String>) -> Self {
        LeafSetBuilder {
            codec,
            classes,
            levels,
            rec: vec![],
            aux: vec![],
        }
    }
    fn word_index(v: &mut Vec<String>, s: &str) -> Result<u8, LeafError> {
        if let Some(i) = v.iter().position(|x| x == s) {
            return Ok(i as u8);
        }
        if v.len() >= 256 {
            return Err(LeafError::Format("more than 256 classes or levels".into()));
        }
        v.push(s.to_string());
        Ok((v.len() - 1) as u8)
    }
    fn encode(&self, key: &[i32]) -> Result<[u64; MAX_WORDS], LeafError> {
        if key.len() != self.codec.arity() {
            return Err(KeySetError::Arity {
                expected: self.codec.arity(),
                got: key.len(),
            }
            .into());
        }
        let mut c = [0u64; MAX_WORDS];
        if !self.codec.encode(key, &mut c) {
            return Err(KeySetError::Range(key.to_vec()).into());
        }
        Ok(c)
    }
    fn push_raw(&mut self, key: &[i32], class: u8, level: u8, flags: u8) -> Result<(), LeafError> {
        let c = self.encode(key)?;
        self.rec.extend_from_slice(&c[..self.codec.words()]);
        self.rec
            .push(((class as u64) << 16) | ((level as u64) << 8) | flags as u64);
        Ok(())
    }
    /// One leaf.
    pub fn push(
        &mut self,
        key: &[i32],
        class: &str,
        level: &str,
        certified: bool,
    ) -> Result<(), LeafError> {
        let ci = Self::word_index(&mut self.classes, class)?;
        let li = Self::word_index(&mut self.levels, level)?;
        self.push_raw(key, ci, li, if certified { LEAF_CERTIFIED } else { 0 })
    }
    /// One aux key (an image the set's dispatch depended on).
    pub fn aux(&mut self, key: &[i32]) -> Result<(), LeafError> {
        let c = self.encode(key)?;
        self.aux.extend_from_slice(&c[..self.codec.words()]);
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.rec.len() / (self.codec.words() + 1)
    }
    pub fn is_empty(&self) -> bool {
        self.rec.is_empty()
    }
    /// Sort, check duplicates, compute the digest.
    pub fn finish(self, binding: LeafBinding) -> Result<LeafSet, LeafError> {
        let w = self.codec.words();
        let rw = w + 1;
        let n = self.rec.len() / rw;
        let mut idx: Vec<usize> = (0..n).collect();
        idx.sort_unstable_by(|&a, &b| {
            self.rec[a * rw..a * rw + w].cmp(&self.rec[b * rw..b * rw + w])
        });
        let mut codes = Vec::with_capacity(n * w);
        let (mut class, mut level, mut flags) = (vec![], vec![], vec![]);
        let mut last: Option<usize> = None;
        for &i in &idx {
            let r = &self.rec[i * rw..(i + 1) * rw];
            if let Some(l) = last {
                let p = &self.rec[l * rw..(l + 1) * rw];
                if p[..w] == r[..w] {
                    if p[w] != r[w] {
                        return Err(LeafError::Format(format!(
                            "key {:?} pushed twice with different class / level / flags",
                            self.codec.decode_vec(&r[..w])
                        )));
                    }
                    continue;
                }
            }
            codes.extend_from_slice(&r[..w]);
            class.push((r[w] >> 16) as u8);
            level.push((r[w] >> 8) as u8);
            flags.push(r[w] as u8);
            last = Some(i);
        }
        let mut aux = self.aux;
        sort_dedup(w, &mut aux);
        let mut s = LeafSet {
            codec: self.codec,
            codes,
            class,
            level,
            flags,
            aux,
            classes: self.classes,
            levels: self.levels,
            binding,
            digest: String::new(),
        };
        s.digest = s.compute_digest();
        Ok(s)
    }
}

fn digest_of(parts: &[String]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update(b"\n");
    }
    h.finish_hex()
}

/// The walk components of a run for a key set given by `member` (the leaf keys + aux keys): the identity components
/// [`WALK_COMPONENTS`] (absent ones as `absent`), `leaf_semantics`, and `relations@keys` / `terminals@keys` /
/// `exclusions@keys` restricted to the member keys (closure.md 18). Cost: one membership test per certified relation
/// key, per terminal and per exclusion (never per leaf).
pub fn walk_components(
    identity: &ConfigIdentity,
    witness_absent_policy: &str,
    rsets: &[(u32, Arc<RelationSet>)],
    terminals: (&Option<KeySet>, &Option<KeySet>),
    exclusions: &[KeyExclusion],
    member: &dyn Fn(&[i32]) -> bool,
) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for &c in WALK_COMPONENTS {
        m.insert(
            c.to_string(),
            identity
                .components
                .get(c)
                .cloned()
                .unwrap_or_else(|| "absent".into()),
        );
    }
    m.insert("leaf_semantics".into(), LEAF_SEMANTICS.into());
    m.insert(
        "witness_absent_policy".into(),
        witness_absent_policy.to_string(),
    );
    // relation sets with a certified key among the members: header + per member key (in domain, relation, verdict)
    let mut rparts: Vec<String> = vec![];
    for (rank, s) in rsets {
        let st = &s.relations;
        let mut hits: Vec<String> = vec![];
        for i in 0..st.len() {
            let k = st.key(i);
            if member(&k) {
                let rd: String = st
                    .relation_digest(i)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                hits.push(format!("{k:?}|{}|{rd}|{}", s.in_domain(&k), st.verdict(i)));
            }
        }
        if !hits.is_empty() {
            rparts.push(format!(
                "{rank}:{}:{:?}:{}:{}",
                s.name,
                s.slot,
                s.recipe_digest,
                digest_of(&[format!("{:?}", s.domain)])
            ));
            rparts.extend(hits);
        }
    }
    m.insert(
        "relations@keys".into(),
        if rparts.is_empty() {
            "none".into()
        } else {
            digest_of(&rparts)
        },
    );
    let mut tparts: Vec<String> = vec![];
    for (name, t) in [("preferred", terminals.0), ("other", terminals.1)] {
        if let Some(t) = t {
            for k in t.iter() {
                if member(&k) {
                    tparts.push(format!("{name}:{k:?}"));
                }
            }
        }
    }
    m.insert(
        "terminals@keys".into(),
        if tparts.is_empty() {
            "none".into()
        } else {
            digest_of(&tparts)
        },
    );
    let mut eparts: Vec<String> = exclusions
        .iter()
        .filter(|e| member(&e.key))
        .map(|e| format!("{e:?}"))
        .collect();
    eparts.sort();
    eparts.dedup();
    m.insert(
        "exclusions@keys".into(),
        if eparts.is_empty() {
            "none".into()
        } else {
            digest_of(&eparts)
        },
    );
    m
}

/// The names of the components that differ between two walk bindings (missing on either side counts).
pub fn differing(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> Vec<String> {
    let mut names: Vec<String> = a.keys().chain(b.keys()).cloned().collect();
    names.sort();
    names.dedup();
    names.retain(|n| a.get(n) != b.get(n));
    names
}

/// A reached leaf (materialised for reports).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeafHit {
    /// Index of the leaf-set part (`ClosureConfig::leaves` order).
    pub part: usize,
    pub key: Vec<i32>,
    pub class: String,
    pub level: String,
    pub certified: bool,
    pub in_start: bool,
}

/// Per part: what the walk met.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeafPartSummary {
    pub digest: String,
    pub node: String,
    pub clause: String,
    pub source_verdict: String,
    pub keys: u64,
    pub reached: u64,
    pub in_start: u64,
    pub open_reached: u64,
}

/// The leaves a walk met (`ClosureSummary::leaves`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeafSummary {
    pub parts: Vec<LeafPartSummary>,
    pub reached: u64,
    pub in_start: u64,
    pub open_reached: u64,
    /// The digests of the reached parts: a `closed` verdict means "closed relative to" these.
    pub relative_to: Vec<String>,
}
