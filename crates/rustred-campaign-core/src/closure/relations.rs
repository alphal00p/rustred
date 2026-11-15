//! Relation sets (requirement R48, recipe rules): per-key relations certified by a recipe, as a dispatch entry beside
//! the tables (closure.md section 12). A [`RelationStore`] holds, per CERTIFIED key (sorted packed codes), the
//! right-hand-side keys (packed codes, one offset table), a relation digest (32 bytes) and verdict bits; one `Vec` per
//! section, no per-key heap allocation. File format `rustred.campaign.relations.v1` (streaming read, digest verified).

use std::io::{self, BufRead, Write};

use super::keyset::{KeyCodec, KeySetError, MAX_WORDS};
use super::sha256::Sha256;
use crate::domain::CaseDomain;

pub const RELATIONS_SCHEMA: &str = "rustred.campaign.relations.v1";
/// Verdict bit: the relation is regular at the key (no pole).
pub const VERDICT_REGULAR: u8 = 1;
/// Verdict bit: every right-hand-side key descends under the node comparator (as recorded by the certifier).
pub const VERDICT_DESCENDS: u8 = 2;
/// Verdict bit: the exact check of the product passed at the key.
pub const VERDICT_EXACT: u8 = 4;
/// All verdict bits: the relation is certified at the key.
pub const VERDICT_CERTIFIED: u8 = VERDICT_REGULAR | VERDICT_DESCENDS | VERDICT_EXACT;

/// Per-key relations of one recipe (certified keys only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationStore {
    codec: KeyCodec,
    keys: Vec<u64>,
    offs: Vec<u64>,
    rhs: Vec<u64>,
    digests: Vec<[u8; 32]>,
    verdicts: Vec<u8>,
}

/// (key code, rhs codes, relation digest, verdict bits).
type BuilderItem = (Vec<u64>, Vec<u64>, [u8; 32], u8);

/// Builder (keys in any order; duplicates are an error at `finish`).
#[derive(Debug)]
pub struct RelationStoreBuilder {
    codec: KeyCodec,
    items: Vec<BuilderItem>,
}

impl RelationStoreBuilder {
    pub fn new(codec: KeyCodec) -> Self {
        RelationStoreBuilder {
            codec,
            items: vec![],
        }
    }
    /// Add the relation at `key`: its right-hand-side keys, relation digest and verdict bits.
    pub fn push(
        &mut self,
        key: &[i32],
        rhs: &[Vec<i32>],
        digest: [u8; 32],
        verdict: u8,
    ) -> Result<(), KeySetError> {
        let w = self.codec.words();
        let mut c = [0u64; MAX_WORDS];
        let enc = |k: &[i32], c: &mut [u64; MAX_WORDS]| -> Result<Vec<u64>, KeySetError> {
            if k.len() != self.codec.arity() {
                return Err(KeySetError::Arity {
                    expected: self.codec.arity(),
                    got: k.len(),
                });
            }
            if !self.codec.encode(k, c) {
                return Err(KeySetError::Range(k.to_vec()));
            }
            Ok(c[..w].to_vec())
        };
        let kc = enc(key, &mut c)?;
        let mut rc = Vec::with_capacity(rhs.len() * w);
        for r in rhs {
            rc.extend(enc(r, &mut c)?);
        }
        self.items.push((kc, rc, digest, verdict));
        Ok(())
    }
    pub fn finish(mut self) -> Result<RelationStore, KeySetError> {
        self.items.sort_by(|a, b| a.0.cmp(&b.0));
        if self.items.windows(2).any(|p| p[0].0 == p[1].0) {
            return Err(KeySetError::Format("duplicate certified key".into()));
        }
        let mut s = RelationStore {
            codec: self.codec,
            keys: Vec::with_capacity(self.items.len() * self.codec.words()),
            offs: vec![0],
            rhs: vec![],
            digests: vec![],
            verdicts: vec![],
        };
        for (k, r, d, v) in self.items {
            s.keys.extend(k);
            s.rhs.extend(r);
            s.offs.push((s.rhs.len() / self.codec.words()) as u64);
            s.digests.push(d);
            s.verdicts.push(v);
        }
        Ok(s)
    }
}

impl RelationStore {
    pub fn codec(&self) -> &KeyCodec {
        &self.codec
    }
    pub fn len(&self) -> usize {
        self.verdicts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.verdicts.is_empty()
    }
    /// Index of a certified key (binary search).
    pub fn find(&self, key: &[i32]) -> Option<usize> {
        let w = self.codec.words();
        let mut c = [0u64; MAX_WORDS];
        if key.len() != self.codec.arity() || !self.codec.encode(key, &mut c) {
            return None;
        }
        let (mut lo, mut hi) = (0usize, self.len());
        while lo < hi {
            let m = (lo + hi) / 2;
            match self.keys[m * w..(m + 1) * w].cmp(&c[..w]) {
                std::cmp::Ordering::Less => lo = m + 1,
                std::cmp::Ordering::Greater => hi = m,
                std::cmp::Ordering::Equal => return Some(m),
            }
        }
        None
    }
    pub fn key(&self, i: usize) -> Vec<i32> {
        let w = self.codec.words();
        self.codec.decode_vec(&self.keys[i * w..(i + 1) * w])
    }
    /// The packed right-hand-side codes of relation `i` (`words` u64 per key).
    pub fn rhs_codes(&self, i: usize) -> &[u64] {
        let w = self.codec.words();
        &self.rhs[self.offs[i] as usize * w..self.offs[i + 1] as usize * w]
    }
    pub fn n_rhs(&self, i: usize) -> usize {
        (self.offs[i + 1] - self.offs[i]) as usize
    }
    pub fn relation_digest(&self, i: usize) -> &[u8; 32] {
        &self.digests[i]
    }
    pub fn verdict(&self, i: usize) -> u8 {
        self.verdicts[i]
    }
    /// lane symfix: OR verdict bits into relation `i` (the import's post-load verification of supplied composites,
    /// import/supplied_verify.rs; bits are only ever added once a check passed).
    pub fn add_verdict_bits(&mut self, i: usize, bits: u8) {
        self.verdicts[i] |= bits;
    }
    pub fn memory_bytes(&self) -> usize {
        (self.keys.capacity() + self.offs.capacity() + self.rhs.capacity()) * 8
            + self.digests.capacity() * 32
            + self.verdicts.capacity()
    }
    /// sha256 over the canonical stream (frame-independent of `bits`): schema line, arity, then per key in order:
    /// key (i32 LE), rhs count (u32 LE), rhs keys (i32 LE), relation digest, verdict byte.
    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(RELATIONS_SCHEMA.as_bytes());
        h.update(b"\n");
        h.update(&(self.codec.arity() as u32).to_le_bytes());
        let w = self.codec.words();
        let mut k = vec![0i32; self.codec.arity()];
        let mut buf: Vec<u8> = Vec::with_capacity(4096);
        for i in 0..self.len() {
            self.codec.decode(&self.keys[i * w..(i + 1) * w], &mut k);
            k.iter()
                .for_each(|v| buf.extend_from_slice(&v.to_le_bytes()));
            buf.extend_from_slice(&(self.n_rhs(i) as u32).to_le_bytes());
            for c in self.rhs_codes(i).chunks_exact(w) {
                self.codec.decode(c, &mut k);
                k.iter()
                    .for_each(|v| buf.extend_from_slice(&v.to_le_bytes()));
            }
            buf.extend_from_slice(&self.digests[i]);
            buf.push(self.verdicts[i]);
            if buf.len() >= 1 << 16 {
                h.update(&buf);
                buf.clear();
            }
        }
        h.update(&buf);
        format!("sha256:{}", h.finish_hex())
    }

    /// Write the file (header line + binary sections); returns the digest.
    pub fn write(&self, name: &str, recipe_digest: &str, mut w: impl Write) -> io::Result<String> {
        let d = self.digest();
        let hdr = serde_json::json!({"schema": RELATIONS_SCHEMA, "name": name, "recipe_digest": recipe_digest,
            "arity": self.codec.arity(), "bits": self.codec.bits(), "words": self.codec.words(),
            "count": self.len(), "total_rhs": self.rhs.len() / self.codec.words(), "digest": d});
        writeln!(w, "{hdr}")?;
        let mut put = |v: &[u64]| -> io::Result<()> {
            for x in v {
                w.write_all(&x.to_le_bytes())?;
            }
            Ok(())
        };
        put(&self.keys)?;
        put(&self.offs)?;
        put(&self.rhs)?;
        for d in &self.digests {
            w.write_all(d)?;
        }
        w.write_all(&self.verdicts)?;
        w.flush()?;
        Ok(d)
    }

    /// Read a file written by [`Self::write`]: returns (name, recipe digest, store); verifies order, offsets and the
    /// digest.
    pub fn read(mut r: impl BufRead) -> Result<(String, String, RelationStore), KeySetError> {
        let mut line = String::new();
        r.read_line(&mut line)
            .map_err(|e| KeySetError::Format(e.to_string()))?;
        let h: serde_json::Value =
            serde_json::from_str(line.trim()).map_err(|e| KeySetError::Format(e.to_string()))?;
        if h["schema"] != RELATIONS_SCHEMA {
            return Err(KeySetError::Format("not a relations.v1 file".into()));
        }
        let num = |k: &str| -> Result<usize, KeySetError> {
            h[k].as_u64()
                .map(|x| x as usize)
                .ok_or_else(|| KeySetError::Format(format!("header field {k}")))
        };
        let codec = KeyCodec::new(num("arity")?, num("bits")? as u32)?;
        let w = codec.words();
        if num("words")? != w {
            return Err(KeySetError::Format("words".into()));
        }
        let (n, nr) = (num("count")?, num("total_rhs")?);
        let mut get = |cnt: usize| -> Result<Vec<u64>, KeySetError> {
            let mut b = vec![0u8; cnt * 8];
            r.read_exact(&mut b)
                .map_err(|e| KeySetError::Format(e.to_string()))?;
            Ok(b.as_chunks::<8>()
                .0
                .iter()
                .map(|c| u64::from_le_bytes(*c))
                .collect())
        };
        let keys = get(n * w)?;
        let offs = get(n + 1)?;
        let rhs = get(nr * w)?;
        let mut db = vec![0u8; n * 32];
        r.read_exact(&mut db)
            .map_err(|e| KeySetError::Format(e.to_string()))?;
        let mut verdicts = vec![0u8; n];
        r.read_exact(&mut verdicts)
            .map_err(|e| KeySetError::Format(e.to_string()))?;
        if offs.first() != Some(&0)
            || offs.last() != Some(&(nr as u64))
            || offs.windows(2).any(|p| p[0] > p[1])
            || keys
                .chunks_exact(w)
                .collect::<Vec<_>>()
                .windows(2)
                .any(|p| p[0] >= p[1])
        {
            return Err(KeySetError::Format("unsorted keys or bad offsets".into()));
        }
        let s = RelationStore {
            codec,
            keys,
            offs,
            rhs,
            digests: db.as_chunks::<32>().0.to_vec(),
            verdicts,
        };
        let d = s.digest();
        if h["digest"].as_str() != Some(d.as_str()) {
            return Err(KeySetError::Format("digest mismatch".into()));
        }
        Ok((
            h["name"].as_str().unwrap_or_default().to_string(),
            h["recipe_digest"].as_str().unwrap_or_default().to_string(),
            s,
        ))
    }
}

/// Counts of [`read_solve_results`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SolveResultsStats {
    pub records: usize,
    pub solved: usize,
    /// Non-SOLVED records by verdict (not certified: not in the store).
    pub not_solved: std::collections::BTreeMap<String, usize>,
    pub recipe_digest: Option<String>,
    pub determinism_id: Option<String>,
}

fn parse_key_text(t: &str, arity: usize) -> Option<Vec<i32>> {
    let inner = t.trim().strip_prefix("I(")?.strip_suffix(')')?;
    let v: Option<Vec<i32>> = inner.split(',').map(|x| x.trim().parse().ok()).collect();
    v.filter(|v| v.len() == arity)
}

/// Build a relation store from the engine's `rr_node solve-recipe` results (`DIR/results.jsonl`,
/// coord-recipe-solve.md sections 7 and 10): every `SOLVED` relation record becomes a certified key with its right-hand
/// side keys (from `rhs[*][0]`, `I(..)` texts), its `relation_digest` (64 hex = 32 bytes) and the verdict bits
/// regular (SOLVED), descends (`all_lower`), exact (`proof.ok`); other records are counted, not stored. All records must
/// carry one recipe digest and one determinism id (else an error).
pub fn read_solve_results(
    r: impl BufRead,
    codec: KeyCodec,
) -> Result<(RelationStore, SolveResultsStats), KeySetError> {
    let mut b = RelationStoreBuilder::new(codec);
    let mut st = SolveResultsStats::default();
    let fmt = |m: String| KeySetError::Format(m);
    for line in r.lines() {
        let line = line.map_err(|e| fmt(e.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| fmt(e.to_string()))?;
        if v["type"] != "relation" {
            continue;
        }
        st.records += 1;
        for (field, slot) in [
            ("recipe_digest", &mut st.recipe_digest),
            ("determinism_id", &mut st.determinism_id),
        ] {
            let x = v[field].as_str().map(|s| s.to_string());
            match slot {
                None => *slot = x,
                Some(old) if Some(old.clone()) != x => {
                    return Err(fmt(format!("records with different {field}")));
                }
                _ => {}
            }
        }
        let verdict = v["verdict"].as_str().unwrap_or("").to_string();
        if verdict != "SOLVED" {
            *st.not_solved.entry(verdict).or_default() += 1;
            continue;
        }
        let key: Vec<i32> =
            serde_json::from_value(v["key"].clone()).map_err(|e| fmt(e.to_string()))?;
        let mut rhs = vec![];
        for t in v["rhs"]
            .as_array()
            .ok_or_else(|| fmt("SOLVED record without rhs".into()))?
        {
            let txt = t[0].as_str().ok_or_else(|| fmt("rhs entry".into()))?;
            rhs.push(
                parse_key_text(txt, codec.arity()).ok_or_else(|| fmt(format!("rhs key {txt}")))?,
            );
        }
        let hex = v["relation_digest"].as_str().unwrap_or("");
        if hex.len() != 64 {
            return Err(fmt(format!("relation_digest {hex}")));
        }
        let mut d = [0u8; 32];
        for (i, byte) in d.iter_mut().enumerate() {
            *byte =
                u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| fmt(e.to_string()))?;
        }
        let mut verdict_bits = VERDICT_REGULAR;
        if v["all_lower"] == true {
            verdict_bits |= VERDICT_DESCENDS;
        }
        if v["proof"]["ok"] == true {
            verdict_bits |= VERDICT_EXACT;
        }
        b.push(&key, &rhs, d, verdict_bits)?;
        st.solved += 1;
    }
    Ok((b.finish()?, st))
}

/// Where a relation set sits in the dispatch order (explicit data, part of the configuration identity).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DispatchSlot {
    /// Immediately before the table with this name (before all its records).
    BeforeTable(String),
    /// After every table.
    Last,
}

/// A relation set: the per-key relations of one recipe, its domain and its slot in the dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationSet {
    pub name: String,
    pub recipe_digest: String,
    pub domain: CaseDomain,
    pub relations: RelationStore,
    pub slot: DispatchSlot,
}

impl RelationSet {
    /// Whether `key` lies in the recipe's domain (stage `Applies`).
    pub fn in_domain(&self, key: &[i32]) -> bool {
        self.domain.applies(key).stage() == crate::domain::Stage::Applies
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let codec = KeyCodec::new(4, 6).unwrap();
        let mut b = RelationStoreBuilder::new(codec);
        b.push(
            &[2, 1, 0, 0],
            &[vec![1, 1, 0, 0], vec![0, 1, 0, 0]],
            [7; 32],
            VERDICT_CERTIFIED,
        )
        .unwrap();
        b.push(&[1, 2, 0, 0], &[], [1; 32], VERDICT_REGULAR)
            .unwrap();
        let s = b.finish().unwrap();
        assert_eq!(s.find(&[2, 1, 0, 0]).map(|i| s.n_rhs(i)), Some(2));
        assert_eq!(s.find(&[3, 1, 0, 0]), None);
        let mut buf = vec![];
        let d = s.write("r", "sha256:x", &mut buf).unwrap();
        let (n, rd, s2) = RelationStore::read(&buf[..]).unwrap();
        assert_eq!((n.as_str(), rd.as_str(), &s2), ("r", "sha256:x", &s));
        assert_eq!(s2.digest(), d);
        let mut bad = buf.clone();
        let l = bad.len();
        bad[l - 1] ^= 1;
        assert!(RelationStore::read(&bad[..]).is_err());
    }
    #[test]
    fn from_solve_results() {
        let d = "ab".repeat(32);
        let txt = format!(
            "{}\n{}\n{}\n",
            serde_json::json!({"type":"relation","key":[2,1,0,0],"verdict":"SOLVED","recipe_digest":"r","determinism_id":"x",
                "rhs":[["I(1,1,0,0)","d-4"],["I(0,1,0,0)","1"]],"relation_digest":d,"all_lower":true,"proof":{"ok":true}}),
            serde_json::json!({"type":"relation","key":[3,1,0,0],"verdict":"NO_RELATION_IN_SEEDS","recipe_digest":"r","determinism_id":"x",
                "rhs":null,"relation_digest":null}),
            serde_json::json!({"type":"end","records":2})
        );
        let (s, st) = read_solve_results(txt.as_bytes(), KeyCodec::new(4, 6).unwrap()).unwrap();
        assert_eq!((s.len(), st.solved, st.records), (1, 1, 2));
        assert_eq!(st.not_solved.get("NO_RELATION_IN_SEEDS"), Some(&1));
        let i = s.find(&[2, 1, 0, 0]).unwrap();
        assert_eq!(
            (s.n_rhs(i), s.verdict(i), s.relation_digest(i)[0]),
            (2, VERDICT_CERTIFIED, 0xab)
        );
    }
}
