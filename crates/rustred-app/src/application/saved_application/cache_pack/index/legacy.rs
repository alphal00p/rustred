//! Bounded structural import of index02; native chunk payloads are unchanged.
//! Only tests write this layout. New checkpoints and copies always write03.
#[cfg(test)]
use super::*;

pub(super) const MAGIC: &[u8; 16] = b"RR-CACHE-PACK-02";

#[cfg(test)]
#[derive(bincode::Encode)]
struct Chunk02 {
    digest: [u8; 32],
    bytes: u64,
    decoded_digest: [u8; 32],
    decoded_bytes: u64,
    records: usize,
    first: ApplicationCacheKey,
    last: ApplicationCacheKey,
}

#[cfg(test)]
#[derive(bincode::Encode)]
struct Manifest02 {
    binding: ApplicationCacheBinding,
    bundle_managed: bool,
    generation: u64,
    runs: Vec<Vec<Chunk02>>,
}

#[cfg(test)]
pub(super) fn encode_for_test(manifest: &Manifest) -> Vec<u8> {
    let old = Manifest02 {
        binding: manifest.binding.clone(),
        bundle_managed: manifest.bundle_managed,
        generation: manifest.generation,
        runs: manifest
            .runs
            .iter()
            .map(|run| {
                run.iter()
                    .map(|chunk| Chunk02 {
                        digest: chunk.digest,
                        bytes: chunk.bytes,
                        decoded_digest: chunk.decoded_digest,
                        decoded_bytes: chunk.decoded_bytes,
                        records: chunk.records,
                        first: chunk.first.clone(),
                        last: chunk.last.clone(),
                    })
                    .collect()
            })
            .collect(),
    };
    let mut bytes = MAGIC.to_vec();
    bincode::encode_into_std_write(old, &mut bytes, bincode::config::standard()).unwrap();
    let hash = blake3::hash(&bytes);
    bytes.extend_from_slice(hash.as_bytes());
    bytes
}
