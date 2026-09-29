//! The persisted edge run log (§10.1): per merged native one run
//! `(source, n, targets...)`, per alias one run of length 1, anchor targets
//! inside the anchored node's run. `append_run` asserts strictly increasing
//! targets (sorted and deduplicated in P3, F6) and an unsealed source. The
//! edge digest is a blake3 chain over `(source, n, targets)` per run and the
//! records digest a chain over `(id, tag, distinct edge count)` per merged
//! native, both folded in P3 in merge order (§4.4). The closure monitor of
//! S2 is the legacy Tracker (fed by the same runs); the monitor graph of
//! S5 replaces it.
pub(super) struct EdgeStore {
    log: Vec<u32>,
    runs: u64,
    edges: u64,
    self_edges: u64,
    edge_digest: blake3::Hasher,
    records_digest: blake3::Hasher,
}

/// Feed the existing byte stream in bounded pieces, not one hasher call per
/// word. Explicit LE conversion is portable; no allocation or native-endian
/// view of the log is involved. The tail never includes unused scratch bytes.
fn feed_words(digest: &mut blake3::Hasher, words: &[u32]) {
    let mut bytes = [0u8; 1024];
    for chunk in words.chunks(256) {
        let used = chunk.len() * 4;
        for (word, slot) in chunk.iter().zip(bytes[..used].chunks_exact_mut(4)) {
            slot.copy_from_slice(&word.to_le_bytes());
        }
        digest.update(&bytes[..used]);
    }
}

fn feed_record(digest: &mut blake3::Hasher, id: u32, tag: u8, distinct_edges: u32) {
    let mut bytes = [0u8; 9];
    bytes[..4].copy_from_slice(&id.to_le_bytes());
    bytes[4] = tag;
    bytes[5..].copy_from_slice(&distinct_edges.to_le_bytes());
    digest.update(&bytes);
}

#[derive(Clone)]
pub(super) struct Runs<'a> {
    remaining: &'a [u32],
}

impl<'a> Iterator for Runs<'a> {
    type Item = (u32, &'a [u32]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        // Only an append-validated or restore-validated log creates Runs.
        let source = self.remaining[0];
        let end = 2 + self.remaining[1] as usize;
        let targets = &self.remaining[2..end];
        self.remaining = &self.remaining[end..];
        Some((source, targets))
    }
}

impl EdgeStore {
    pub fn new() -> Self {
        Self {
            log: Vec::new(),
            runs: 0,
            edges: 0,
            self_edges: 0,
            edge_digest: blake3::Hasher::new(),
            records_digest: blake3::Hasher::new(),
        }
    }
    pub fn try_reserve(&mut self, words: usize) -> Result<(), &'static str> {
        self.log
            .try_reserve(words)
            .map_err(|_| "edge log allocation")
    }
    /// Take the final runtime log, not a copy of its serialized section.
    /// This checks run geometry and rebuilds the edge digest; ledger/run
    /// uniqueness, record tags and the records digest remain cross-section
    /// restore checks before this provisional store can reach a worker.
    pub fn from_owned_log(log: Vec<u32>, watermark: u32) -> Result<Self, String> {
        let mut store = Self::new();
        let mut remaining = log.as_slice();
        while !remaining.is_empty() {
            let [source, count, ..] = remaining else {
                return Err("truncated checkpoint edge run".into());
            };
            let count = *count as usize;
            let targets = remaining
                .get(2..2usize.checked_add(count).ok_or("edge run overflow")?)
                .ok_or("truncated checkpoint edge targets")?;
            if *source >= watermark
                || targets.iter().any(|&target| target >= watermark)
                || targets.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err("invalid checkpoint edge run".into());
            }
            store.runs += 1;
            store.edges += count as u64;
            store.self_edges += u64::from(targets.binary_search(source).is_ok());
            feed_words(&mut store.edge_digest, &remaining[..2 + count]);
            remaining = &remaining[2 + count..];
        }
        store.log = log;
        Ok(store)
    }

    pub fn run_iter(&self) -> Runs<'_> {
        Runs {
            remaining: &self.log,
        }
    }

    /// Repeatable borrowed traversal, used to rebuild the closure CSR in
    /// two passes without materializing all dependency pairs.
    pub fn pairs(&self) -> impl Iterator<Item = (u32, u32)> + Clone + '_ {
        self.run_iter()
            .flat_map(|(source, targets)| targets.iter().map(move |&target| (source, target)))
    }
    /// Append one run. Err on a violated precondition (the caller treats it
    /// as a P3 internal failure, C5).
    pub fn append_run(
        &mut self,
        source: u32,
        targets: &[u32],
        source_sealed: bool,
    ) -> Result<(), String> {
        if source_sealed {
            return Err(format!("edge run from sealed source {source}"));
        }
        if targets.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(format!("edge run of {source} not strictly increasing"));
        }
        let n = u32::try_from(targets.len()).map_err(|_| "edge run length")?;
        let start = self.log.len();
        self.log.push(source);
        self.log.push(n);
        self.log.extend_from_slice(targets);
        self.runs += 1;
        self.edges += u64::from(n);
        self.self_edges += u64::from(targets.binary_search(&source).is_ok());
        feed_words(&mut self.edge_digest, &self.log[start..]);
        Ok(())
    }
    /// Fold one merged native into the records digest (out-degree check).
    pub fn fold_record(&mut self, id: u32, tag: u8, distinct_edges: u32) {
        feed_record(&mut self.records_digest, id, tag, distinct_edges);
    }
    pub fn log(&self) -> &[u32] {
        &self.log
    }
    pub fn runs(&self) -> u64 {
        self.runs
    }
    pub fn edges(&self) -> u64 {
        self.edges
    }
    pub fn self_edges(&self) -> u64 {
        self.self_edges
    }
    pub fn edge_digest(&self) -> String {
        self.edge_digest.finalize().to_hex().to_string()
    }
    pub fn records_digest(&self) -> String {
        self.records_digest.finalize().to_hex().to_string()
    }

    /// Restore the appendable records hash from authenticated runs and ledger
    /// tags. Run uniqueness/completeness is a separate cross-state prerequisite;
    /// this commits the reconstructed hasher only after the expected digest agrees.
    pub fn restore_records_digest(
        &mut self,
        ledger: &super::ledger6::Ledger6,
        expected: &str,
    ) -> Result<(), String> {
        use super::ledger6::Tag;
        let mut digest = blake3::Hasher::new();
        for (source, targets) in self.run_iter() {
            match ledger.tag(source) {
                Some(
                    tag @ (Tag::Native | Tag::NativeFrontier | Tag::NativeError | Tag::Abandoned),
                ) => {
                    feed_record(&mut digest, source, tag as u8, targets.len() as u32);
                }
                Some(Tag::Alias) => {}
                _ => return Err("epoch record digest run from unmerged source".into()),
            }
        }
        if digest.finalize().to_hex().as_str() != expected {
            return Err("epoch records digest differs from ledger/runs".into());
        }
        self.records_digest = digest;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
