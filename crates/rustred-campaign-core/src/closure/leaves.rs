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

/// Version of the leaf-set FORMAT (audit r2-planners L1): 2 = the binding records the PRODUCER's required evidence level,
/// tier and strictness ([`LeafProducer`]). A version-1 file (no such fields) is still read, its producer UNKNOWN: its
/// leaves count at the lowest level only ([`LeafSet::shortfall`]), never as sufficient above it.
pub const LEAFSET_FORMAT_VERSION: u32 = 2;
/// Schema of the leaf-set file (format [`LEAFSET_FORMAT_VERSION`]).
pub const LEAFSET_SCHEMA: &str = "rustred.campaign.leafset.v2";
/// Schema of a version-1 leaf-set file (read only; written by builds before format 2).
pub const LEAFSET_SCHEMA_V1: &str = "rustred.campaign.leafset.v1";
/// The evidence-level words of a leaf, LOWEST FIRST (the import's `Level` words; a word not listed ranks below all).
pub const LEAF_LEVEL_ORDER: &[&str] = &["imported", "replay", "per-key"];
/// The level word of a declared terminal (certified at every level: a terminal is not evidence-levelled).
pub const LEAF_LEVEL_TERMINAL: &str = "terminal";
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
    /// Format 2 (audit r2-planners L1): the PRODUCER's certification basis. `None` (every version-1 file, or a
    /// producer that does not state it): unknown = the lowest level, tier unknown, not strict.
    #[serde(default)]
    pub producer: Option<LeafProducer>,
}

/// The certification basis of the run that produced a leaf set (format 2): its `certified` flags mean "certified at
/// `required_level`, tier `tier`, on a strict basis iff `strict`".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeafProducer {
    /// The producer's required evidence level word ([`LEAF_LEVEL_ORDER`]).
    pub required_level: String,
    /// The tier its walk ran at: `C` (exact facts) | `K` (keys only).
    pub tier: String,
    /// Its basis was strict exact (Tier C, strict dispatcher, nothing diagnostic).
    pub strict: bool,
    /// audit r2-zeros F9: the ZERO-ROUTE guarantees the producer enforced before calling a key certified (sorted words
    /// of the producer's own vocabulary, e.g. the closing loop's `all_zeros`, `decisive_zeros`, `elided_zeros`). A
    /// consumer requiring a guarantee not listed here refuses the leaf (`leaf_zero_route_weaker`). Empty: none stated.
    #[serde(default)]
    pub zero_route: Vec<String>,
}

impl LeafProducer {
    pub fn new(required_level: &str, tier: super::Tier, strict: bool) -> Self {
        LeafProducer {
            required_level: required_level.to_string(),
            tier: tier_word(tier).to_string(),
            strict,
            zero_route: vec![],
        }
    }
    /// The same producer with its zero-route guarantees (sorted, deduplicated).
    pub fn with_zero_route(mut self, words: &[String]) -> Self {
        let mut w = words.to_vec();
        w.sort();
        w.dedup();
        self.zero_route = w;
        self
    }
    /// The basis of a set composed of several parts: the WEAKEST part's (lowest required level, tier `C` only if every
    /// part is, strict only if every part is). `None` when there is no part or any part's producer is unknown (or names
    /// a level that is no level word): the composed set certifies no more than its weakest part.
    pub fn weakest<'a>(parts: impl IntoIterator<Item = Option<&'a LeafProducer>>) -> Option<Self> {
        let mut out: Option<(usize, bool, bool, Vec<String>)> = None;
        for p in parts {
            let p = p?;
            let r = level_rank(&p.required_level)?;
            let c = p.tier == tier_word(super::Tier::C);
            out = Some(match out {
                None => (r, c, p.strict, p.zero_route.clone()),
                Some((r0, c0, s0, z0)) => (
                    r0.min(r),
                    c0 && c,
                    s0 && p.strict,
                    z0.into_iter()
                        .filter(|w| p.zero_route.contains(w))
                        .collect(),
                ),
            });
        }
        let (r, c, strict, zero_route) = out?;
        Some(LeafProducer {
            required_level: LEAF_LEVEL_ORDER[r].to_string(),
            tier: tier_word(if c { super::Tier::C } else { super::Tier::K }).to_string(),
            strict,
            zero_route,
        })
    }
}

/// The tier word of a leaf binding.
pub fn tier_word(t: super::Tier) -> &'static str {
    match t {
        super::Tier::C => "C",
        super::Tier::K => "K",
    }
}

/// Rank of a level word in [`LEAF_LEVEL_ORDER`] (None: not a level word).
pub fn level_rank(word: &str) -> Option<usize> {
    LEAF_LEVEL_ORDER.iter().position(|w| *w == word)
}

/// What the CONSUMING run requires of a leaf it stops at (audit r2-planners L1): its required level, its tier and
/// whether it requires a strict exact basis. A certified leaf below any of the three BLOCKS the consuming walk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeafRequirement {
    /// The consuming run's required evidence level word ([`LEAF_LEVEL_ORDER`]).
    pub required_level: String,
    pub tier: super::Tier,
    pub strict: bool,
    /// audit r2-zeros F9: the zero-route guarantees the consumer enforces itself (its producer vocabulary); a leaf whose
    /// producer did not enforce every one of them is refused (`leaf_zero_route_weaker`). Empty: none required.
    pub zero_route: Vec<String>,
}

/// Why a CERTIFIED leaf does not meet a [`LeafRequirement`] (blocking words of the consuming walk).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeafShortfall {
    /// `leaf_below_required_evidence`: its certified level (the lower of its level word and the producer's required
    /// level; unknown producer = the lowest level) is below the consumer's required level.
    BelowRequiredEvidence,
    /// `leaf_below_required_tier`: certified keys-only (or tier unknown) while the consumer runs the exact tier.
    BelowRequiredTier,
    /// `leaf_basis_not_strict`: the producer's basis was not strict exact (or unknown) while the consumer requires it.
    BasisNotStrict,
    /// `leaf_zero_route_weaker` (audit r2-zeros F9): the producer did not enforce a zero-route guarantee the consumer
    /// enforces (e.g. closed with `all_zeros: off` under a default run), or states none.
    ZeroRouteWeaker,
}

impl LeafShortfall {
    pub fn word(self) -> &'static str {
        match self {
            LeafShortfall::BelowRequiredEvidence => "leaf_below_required_evidence",
            LeafShortfall::BelowRequiredTier => "leaf_below_required_tier",
            LeafShortfall::BasisNotStrict => "leaf_basis_not_strict",
            LeafShortfall::ZeroRouteWeaker => "leaf_zero_route_weaker",
        }
    }
    pub const ALL: [LeafShortfall; 4] = [
        LeafShortfall::BelowRequiredEvidence,
        LeafShortfall::BelowRequiredTier,
        LeafShortfall::BasisNotStrict,
        LeafShortfall::ZeroRouteWeaker,
    ];
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
    /// The file format version (2 for everything built here; 1 for an old file read back).
    format: u32,
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
    format: u32,
) -> String {
    let mut b = serde_json::to_value(binding).unwrap_or(serde_json::Value::Null);
    if format < 2
        && let Some(o) = b.as_object_mut()
    {
        // a version-1 binding had no producer member: its digest is computed as it was written
        o.remove("producer");
    }
    let v = serde_json::json!({"arity": arity, "classes": classes, "levels": levels, "binding": b});
    crate::schema::canon::canonical_json(&v, false).unwrap_or_else(|_| v.to_string())
}

fn schema_of(format: u32) -> &'static str {
    if format < 2 {
        LEAFSET_SCHEMA_V1
    } else {
        LEAFSET_SCHEMA
    }
}

/// The canonical digest stream of a leaf set (independent of the codec bits).
struct LeafDigest {
    h: Sha256,
    buf: Vec<u8>,
}

impl LeafDigest {
    fn new(
        arity: usize,
        classes: &[String],
        levels: &[String],
        binding: &LeafBinding,
        format: u32,
    ) -> Self {
        let mut h = Sha256::new();
        h.update(schema_of(format).as_bytes());
        h.update(b"\n");
        h.update(header_canon(arity, classes, levels, binding, format).as_bytes());
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
    /// The file format version ([`LEAFSET_FORMAT_VERSION`]; 1 for an old file read back).
    pub fn format_version(&self) -> u32 {
        self.format
    }
    /// The producer's certification basis (None: unknown, e.g. a version-1 file).
    pub fn producer(&self) -> Option<&LeafProducer> {
        if self.format < 2 {
            return None;
        }
        self.binding.producer.as_ref()
    }
    /// Why the CERTIFIED leaf `i` does not meet `req` (None: it does, or it is not certified: an uncertified leaf
    /// blocks as `leaf_open` whatever the requirement). A terminal meets every level; the certified level of any other
    /// leaf is the LOWER of its level word and the producer's required level (unknown producer: the lowest level, tier
    /// unknown, not strict); a level word that is no level word ranks below all.
    pub fn shortfall(&self, i: usize, req: &LeafRequirement) -> Option<LeafShortfall> {
        if !self.is_certified(i) {
            return None;
        }
        let p = self.producer();
        let terminal = self.level_of(i) == LEAF_LEVEL_TERMINAL;
        if !terminal {
            let want = level_rank(&req.required_level).unwrap_or(LEAF_LEVEL_ORDER.len());
            let own = level_rank(self.level_of(i));
            let prod = p.map_or(Some(0), |p| level_rank(&p.required_level));
            let have = match (own, prod) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            };
            if have.is_none_or(|h| h < want) {
                return Some(LeafShortfall::BelowRequiredEvidence);
            }
        }
        if req.tier == super::Tier::C && p.is_none_or(|p| p.tier != tier_word(super::Tier::C)) {
            return Some(LeafShortfall::BelowRequiredTier);
        }
        if req.strict && !p.is_some_and(|p| p.strict) {
            return Some(LeafShortfall::BasisNotStrict);
        }
        if !req.zero_route.is_empty()
            && !p.is_some_and(|p| req.zero_route.iter().all(|w| p.zero_route.contains(w)))
        {
            return Some(LeafShortfall::ZeroRouteWeaker);
        }
        None
    }
    /// The SET-level part of [`LeafSet::shortfall`] (the producer's basis against `req`, ignoring the per-key level
    /// words): e.g. whether a frozen layer may stay frozen under a run's requirement. None: the producer meets it.
    pub fn producer_shortfall(&self, req: &LeafRequirement) -> Option<LeafShortfall> {
        let p = self.producer();
        let want = level_rank(&req.required_level).unwrap_or(LEAF_LEVEL_ORDER.len());
        if p.and_then(|p| level_rank(&p.required_level)).unwrap_or(0) < want {
            return Some(LeafShortfall::BelowRequiredEvidence);
        }
        if req.tier == super::Tier::C && p.is_none_or(|p| p.tier != tier_word(super::Tier::C)) {
            return Some(LeafShortfall::BelowRequiredTier);
        }
        if req.strict && !p.is_some_and(|p| p.strict) {
            return Some(LeafShortfall::BasisNotStrict);
        }
        if !req.zero_route.is_empty()
            && !p.is_some_and(|p| req.zero_route.iter().all(|w| p.zero_route.contains(w)))
        {
            return Some(LeafShortfall::ZeroRouteWeaker);
        }
        None
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
        let mut s = b.finish(self.binding.clone())?;
        if s.format != self.format {
            s.format = self.format;
            s.digest = s.compute_digest();
        }
        debug_assert_eq!(s.digest, self.digest);
        Ok(s)
    }
    fn compute_digest(&self) -> String {
        let arity = self.codec.arity();
        let mut d = LeafDigest::new(
            arity,
            &self.classes,
            &self.levels,
            &self.binding,
            self.format,
        );
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
            "schema": schema_of(self.format), "arity": self.codec.arity(), "bits": self.codec.bits(),
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
        let format = if h["schema"] == LEAFSET_SCHEMA {
            LEAFSET_FORMAT_VERSION
        } else if h["schema"] == LEAFSET_SCHEMA_V1 {
            1
        } else {
            return Err(LeafError::Format(format!("schema {}", h["schema"])));
        };
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
        let mut binding: LeafBinding = serde_json::from_value(h["binding"].clone())
            .map_err(|e| LeafError::Format(format!("binding: {e}")))?;
        if format < 2 {
            // a version-1 file states no producer basis (and must not gain one by a stray member)
            binding.producer = None;
        }
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
            format,
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
            format: LEAFSET_FORMAT_VERSION,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closure::Tier;

    fn set(format: u32, producer: Option<LeafProducer>, level: &str) -> LeafSet {
        let mut b = LeafSetBuilder::new(KeyCodec::new(2, 6).unwrap(), vec![], vec![]);
        b.push(&[2, 1], "table", level, true).unwrap();
        b.push(&[3, 1], "table", "terminal", true).unwrap();
        let mut s = b
            .finish(LeafBinding {
                source_verdict: "closed".into(),
                producer,
                ..Default::default()
            })
            .unwrap();
        if format != s.format {
            // a file of an older build (format 1 has no producer member)
            s.format = format;
            s.binding.producer = None;
            s.digest = s.compute_digest();
        }
        s
    }

    fn req(level: &str, tier: Tier, strict: bool) -> LeafRequirement {
        LeafRequirement {
            required_level: level.into(),
            tier,
            strict,
            zero_route: vec![],
        }
    }

    #[test]
    fn format_two_round_trips_its_producer_and_format_one_reads_as_unknown() {
        let p = LeafProducer::new("per-key", Tier::C, true);
        let s2 = set(2, Some(p.clone()), "per-key");
        let mut buf = vec![];
        s2.write(&mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains(LEAFSET_SCHEMA));
        let r2 = LeafSet::read(&buf[..]).unwrap();
        assert_eq!(r2.format_version(), LEAFSET_FORMAT_VERSION);
        assert_eq!(r2.producer(), Some(&p));
        assert_eq!(r2.digest(), s2.digest());
        // a version-1 file reads back (its own digest), producer unknown
        let s1 = set(1, None, "per-key");
        let mut buf = vec![];
        s1.write(&mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains(LEAFSET_SCHEMA_V1));
        let r1 = LeafSet::read(&buf[..]).unwrap();
        assert_eq!(r1.format_version(), 1);
        assert_eq!(r1.producer(), None);
        assert_ne!(r1.digest(), s2.digest());
        // recode keeps the format and the digest
        let rc = r1.recode(KeyCodec::new(2, 7).unwrap()).unwrap();
        assert_eq!((rc.format_version(), rc.digest()), (1, r1.digest()));
    }

    #[test]
    fn a_leaf_meets_a_requirement_only_at_its_certified_level_tier_and_basis() {
        let i = 0; // [2,1]
        let pk = set(
            2,
            Some(LeafProducer::new("per-key", Tier::C, true)),
            "per-key",
        );
        assert_eq!(pk.shortfall(i, &req("per-key", Tier::C, true)), None);
        // certified level = min(level word, producer's required level)
        let imp = set(
            2,
            Some(LeafProducer::new("imported", Tier::C, true)),
            "per-key",
        );
        assert_eq!(
            imp.shortfall(i, &req("per-key", Tier::C, false)),
            Some(LeafShortfall::BelowRequiredEvidence)
        );
        assert_eq!(imp.shortfall(i, &req("imported", Tier::C, true)), None);
        let k = set(
            2,
            Some(LeafProducer::new("per-key", Tier::K, false)),
            "per-key",
        );
        assert_eq!(
            k.shortfall(i, &req("per-key", Tier::C, false)),
            Some(LeafShortfall::BelowRequiredTier)
        );
        assert_eq!(k.shortfall(i, &req("per-key", Tier::K, false)), None);
        let ns = set(
            2,
            Some(LeafProducer::new("per-key", Tier::C, false)),
            "per-key",
        );
        assert_eq!(
            ns.shortfall(i, &req("per-key", Tier::C, true)),
            Some(LeafShortfall::BasisNotStrict)
        );
        // unknown producer (format 1, or none stated): the lowest level only, tier unknown, not strict
        for old in [set(1, None, "per-key"), set(2, None, "per-key")] {
            assert_eq!(old.shortfall(i, &req("imported", Tier::K, false)), None);
            assert_eq!(
                old.shortfall(i, &req("replay", Tier::K, false)),
                Some(LeafShortfall::BelowRequiredEvidence)
            );
            assert_eq!(
                old.shortfall(i, &req("imported", Tier::C, false)),
                Some(LeafShortfall::BelowRequiredTier)
            );
            // a terminal meets every LEVEL (not the tier of an unknown producer)
            assert_eq!(old.shortfall(1, &req("per-key", Tier::K, false)), None);
        }
        // a word that is no level word is below everything
        let odd = set(2, Some(LeafProducer::new("per-key", Tier::C, true)), "none");
        assert_eq!(
            odd.shortfall(i, &req("imported", Tier::K, false)),
            Some(LeafShortfall::BelowRequiredEvidence)
        );
    }

    #[test]
    fn the_weakest_part_bounds_a_composed_producer() {
        let a = LeafProducer::new("per-key", Tier::C, true);
        let b = LeafProducer::new("replay", Tier::C, false);
        let w = LeafProducer::weakest([Some(&a), Some(&b)]).unwrap();
        assert_eq!(w, LeafProducer::new("replay", Tier::C, false));
        assert_eq!(LeafProducer::weakest([Some(&a), None]), None);
        assert_eq!(LeafProducer::weakest(std::iter::empty()), None);
        let k = LeafProducer::new("per-key", Tier::K, true);
        assert_eq!(
            LeafProducer::weakest([Some(&a), Some(&k)]).unwrap().tier,
            "K"
        );
    }

    #[test]
    fn the_level_words_are_the_imports() {
        use crate::import::Level;
        let words: Vec<&str> = [Level::Imported, Level::Replay, Level::PerKey]
            .iter()
            .map(|l| l.word())
            .collect();
        assert_eq!(words, LEAF_LEVEL_ORDER);
        assert!(Level::Imported < Level::Replay && Level::Replay < Level::PerKey);
    }
}

#[cfg(test)]
mod zero_route_tests {
    use super::*;
    use crate::closure::Tier;

    /// audit r2-zeros F9: the producer's zero-route guarantees bound the leaf; a consumer enforcing more refuses it.
    #[test]
    fn a_leaf_closed_with_a_weaker_zero_route_is_refused() {
        let words = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mk = |z: &[&str]| {
            let mut b = LeafSetBuilder::new(KeyCodec::new(2, 6).unwrap(), vec![], vec![]);
            b.push(&[2, 1], "table", "per-key", true).unwrap();
            b.finish(LeafBinding {
                source_verdict: "closed".into(),
                producer: Some(
                    LeafProducer::new("per-key", Tier::C, true).with_zero_route(&words(z)),
                ),
                ..Default::default()
            })
            .unwrap()
        };
        let req = |z: &[&str]| LeafRequirement {
            required_level: "per-key".into(),
            tier: Tier::C,
            strict: true,
            zero_route: words(z),
        };
        let full = ["all_zeros", "decisive_zeros", "elided_zeros"];
        let off = mk(&["decisive_zeros", "elided_zeros"]);
        assert_eq!(
            off.shortfall(0, &req(&full)),
            Some(LeafShortfall::ZeroRouteWeaker)
        );
        assert_eq!(
            off.producer_shortfall(&req(&full)),
            Some(LeafShortfall::ZeroRouteWeaker)
        );
        assert_eq!(off.shortfall(0, &req(&["decisive_zeros"])), None);
        let on = mk(&full);
        assert_eq!(on.shortfall(0, &req(&full)), None);
        assert_eq!(on.producer_shortfall(&req(&full)), None);
        // the guarantees survive the file round trip and enter the digest
        let mut buf = vec![];
        on.write(&mut buf).unwrap();
        let back = LeafSet::read(&buf[..]).unwrap();
        assert_eq!(back.producer().unwrap().zero_route, words(&full));
        assert_ne!(on.digest(), off.digest());
        // the weakest part: the intersection
        let w = LeafProducer::weakest([on.producer(), off.producer()]).unwrap();
        assert_eq!(w.zero_route, words(&["decisive_zeros", "elided_zeros"]));
    }
}
