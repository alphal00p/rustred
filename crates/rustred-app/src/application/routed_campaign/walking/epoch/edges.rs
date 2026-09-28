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
        self.log.push(source);
        self.log.push(n);
        self.log.extend_from_slice(targets);
        self.runs += 1;
        self.edges += u64::from(n);
        self.self_edges += u64::from(targets.binary_search(&source).is_ok());
        self.edge_digest.update(&source.to_le_bytes());
        self.edge_digest.update(&n.to_le_bytes());
        for target in targets {
            self.edge_digest.update(&target.to_le_bytes());
        }
        Ok(())
    }
    /// Fold one merged native into the records digest (out-degree check).
    pub fn fold_record(&mut self, id: u32, tag: u8, distinct_edges: u32) {
        self.records_digest.update(&id.to_le_bytes());
        self.records_digest.update(&[tag]);
        self.records_digest.update(&distinct_edges.to_le_bytes());
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
}
