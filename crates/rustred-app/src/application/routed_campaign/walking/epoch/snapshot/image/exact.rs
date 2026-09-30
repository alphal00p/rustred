//! Deterministic exact-key index compaction with bounded cancellation latency.
//! Old cohorts are already sorted. Hash/sort only the new tail, using bounded
//! standard-library sorts; then merge the sorted runs with std::BinaryHeap.
use super::{Image, Layer};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

const CHUNK: usize = 4096;

pub(super) fn merge<const N: usize>(
    image: &Image<N>,
    first: u32,
    end: u32,
    prior: &[Arc<Layer<N>>],
    checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
) -> Result<Vec<(u64, u32)>, &'static str> {
    let mut tail_first = first;
    for layer in prior {
        if layer.first != tail_first || layer.end > end {
            return Err("snapshot exact cohort partition");
        }
        tail_first = layer.end;
    }
    let mut tail = Vec::new();
    tail.try_reserve_exact((end - tail_first) as usize)
        .map_err(|_| "snapshot exact tail allocation")?;
    for id in tail_first..end {
        if (id - tail_first) as usize % CHUNK == 0 {
            checkpoint()?;
        }
        tail.push((image.domains[id as usize].digest().0, id));
    }
    for chunk in tail.chunks_mut(CHUNK) {
        checkpoint()?;
        chunk.sort_unstable();
    }
    // A single small fresh cohort needs no second output buffer or heap.
    if prior.is_empty() && tail.len() <= CHUNK {
        return Ok(tail);
    }
    let mut runs = Vec::new();
    runs.try_reserve_exact(prior.len() + tail.len().div_ceil(CHUNK))
        .map_err(|_| "snapshot exact merge runs allocation")?;
    runs.extend(prior.iter().map(|layer| layer.exact.as_slice()));
    runs.extend(tail.chunks(CHUNK));
    let mut heap = BinaryHeap::new();
    heap.try_reserve(runs.len())
        .map_err(|_| "snapshot exact merge heap allocation")?;
    for (run, values) in runs.iter().enumerate() {
        if let Some(&(digest, id)) = values.first() {
            heap.push(Reverse((digest, id, run, 0usize)));
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact((end - first) as usize)
        .map_err(|_| "snapshot exact layer allocation")?;
    while let Some(Reverse((digest, id, run, at))) = heap.pop() {
        if output.len() % CHUNK == 0 {
            checkpoint()?;
        }
        output.push((digest, id));
        if let Some(&(digest, id)) = runs[run].get(at + 1) {
            heap.push(Reverse((digest, id, run, at + 1)));
        }
    }
    if output.len() != (end - first) as usize {
        return Err("snapshot exact merge cardinality");
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
