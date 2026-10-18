//! Compact exact membership. Hash slots own no heap allocation; coordinate
//! storage is released in chunks, not one or two frees for every Work record.
use super::*;
use hashbrown::HashTable;
use std::{collections::hash_map::RandomState, hash::BuildHasher};

const CHUNK_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy)]
pub(super) struct Slot {
    hash: u64,
    /// Low bit is Route/Apply; the remaining bits identify a full powers row.
    tagged_record: usize,
}

pub(super) struct PackedWork {
    table: HashTable<Slot>,
    arena: PowersArena,
    hasher: RandomState,
}

struct PowersArena {
    arity: usize,
    records_per_chunk: usize,
    words_per_chunk: usize,
    chunks: Vec<Vec<i64>>,
    words_capacity: usize,
    records: usize,
}

impl PackedWork {
    pub fn new(arity: usize) -> GeneratedWorkResult<Self> {
        if arity == 0 {
            return Err(corrupt("resident Work zero arity"));
        }
        let records_per_chunk = (CHUNK_BYTES / size_of::<i64>() / arity).max(1);
        let words_per_chunk = records_per_chunk
            .checked_mul(arity)
            .ok_or_else(|| resource("resident arena chunk overflow"))?;
        Ok(Self {
            table: HashTable::new(),
            arena: PowersArena {
                arity,
                records_per_chunk,
                words_per_chunk,
                chunks: Vec::new(),
                words_capacity: 0,
                records: 0,
            },
            hasher: RandomState::new(),
        })
    }
    pub fn len(&self) -> usize {
        self.table.len()
    }
    pub fn capacity(&self) -> usize {
        self.table.capacity()
    }

    pub fn contains(&self, key: &GeneratedWorkKey) -> bool {
        self.contains_hashed(key, self.hash(key))
    }
    /// Validated native keys borrow their coordinates and require no wire-key
    /// Vec allocation. The caller must validate physical arity and Apply owner.
    pub fn contains_parts(&self, apply: bool, powers: &[i64]) -> bool {
        let phase = usize::from(apply);
        let hash = self.hasher.hash_one((phase, powers));
        self.table
            .find(hash, |slot| {
                slot.tagged_record & 1 == phase && self.arena.get(slot.tagged_record >> 1) == powers
            })
            .is_some()
    }
    fn hash(&self, key: &GeneratedWorkKey) -> u64 {
        self.hasher.hash_one((phase(key), key.powers()))
    }
    fn contains_hashed(&self, key: &GeneratedWorkKey, hash: u64) -> bool {
        self.table
            .find(hash, |slot| {
                slot.tagged_record & 1 == phase(key)
                    && self.arena.get(slot.tagged_record >> 1) == key.powers()
            })
            .is_some()
    }

    /// Conservative new allocation during growth, on top of all retained
    /// buffers: hash reallocation can coexist with the original hash table.
    pub fn growth_payload(&self, additional: usize) -> GeneratedWorkResult<usize> {
        let required = self
            .len()
            .checked_add(additional)
            .ok_or_else(|| resource("resident delta count overflow"))?;
        let table = if required > self.capacity() {
            table_estimate(required)?
        } else {
            0
        };
        sum(&[table, self.arena.growth_payload(additional)?])
    }
    pub fn retained_payload(&self) -> GeneratedWorkResult<usize> {
        let table = if self.table.capacity() == 0 {
            0
        } else {
            bucket_estimate(self.table.num_buckets())?
        };
        sum(&[table, self.arena.retained_payload()?])
    }

    /// Allocation failure declines this optional index. Integrity/overflow and
    /// cancellation are explicit errors. No data are inserted into membership.
    pub fn reserve(&mut self, additional: usize, cancel: &AtomicBool) -> GeneratedWorkResult<bool> {
        check(cancel)?;
        if self
            .table
            .try_reserve(additional, |slot| slot.hash)
            .is_err()
        {
            check(cancel)?;
            return Ok(false);
        }
        self.arena.reserve(additional, cancel)
    }

    /// Append a privately staged key into already reserved arena capacity.
    /// The caller validated owner == positive support, so (phase,powers) is an
    /// injective representation of the full original key, not an approximation.
    pub fn stage(&mut self, key: &GeneratedWorkKey) -> Slot {
        let record = self.arena.append_reserved(key.powers());
        Slot {
            hash: self.hash(key),
            tagged_record: (record << 1) | phase(key),
        }
    }

    pub fn hydrate_key(&mut self, key: &GeneratedWorkKey) -> bool {
        let slot = self.stage(key);
        if self.contains_hashed(key, slot.hash) {
            return false;
        }
        self.insert_reserved(slot);
        true
    }

    /// All table capacity was reserved before the durable commit. The sealed
    /// delta is unique and disjoint from the preceding completed Work set.
    pub fn insert_reserved(&mut self, slot: Slot) {
        debug_assert!(self.table.len() < self.table.capacity());
        self.table
            .insert_unique(slot.hash, slot, |stored| stored.hash);
    }
}

fn phase(key: &GeneratedWorkKey) -> usize {
    usize::from(matches!(key, GeneratedWorkKey::Apply { .. }))
}

impl PowersArena {
    fn needed_chunks(&self, additional: usize) -> GeneratedWorkResult<usize> {
        let required = self
            .records
            .checked_add(additional)
            .ok_or_else(|| resource("resident arena record count overflow"))?;
        if required > (usize::MAX >> 1) {
            return Err(resource("resident arena tagged offset overflow"));
        }
        Ok(required / self.records_per_chunk + usize::from(required % self.records_per_chunk != 0))
    }
    fn growth_payload(&self, additional: usize) -> GeneratedWorkResult<usize> {
        let chunks = self.needed_chunks(additional)?;
        let new_chunks = chunks.saturating_sub(self.chunks.len());
        let words = new_chunks
            .checked_mul(self.words_per_chunk)
            .and_then(|n| n.checked_mul(size_of::<i64>()))
            .ok_or_else(|| resource("resident arena growth payload overflow"))?;
        let descriptors = if chunks > self.chunks.capacity() {
            chunks
                .checked_mul(size_of::<Vec<i64>>())
                .ok_or_else(|| resource("resident chunk directory growth overflow"))?
        } else {
            0
        };
        sum(&[words, descriptors])
    }
    fn retained_payload(&self) -> GeneratedWorkResult<usize> {
        sum(&[
            self.words_capacity
                .checked_mul(size_of::<i64>())
                .ok_or_else(|| resource("resident arena payload overflow"))?,
            self.chunks
                .capacity()
                .checked_mul(size_of::<Vec<i64>>())
                .ok_or_else(|| resource("resident chunk directory payload overflow"))?,
        ])
    }
    fn reserve(&mut self, additional: usize, cancel: &AtomicBool) -> GeneratedWorkResult<bool> {
        let required = self.needed_chunks(additional)?;
        if self
            .chunks
            .try_reserve_exact(required.saturating_sub(self.chunks.len()))
            .is_err()
        {
            check(cancel)?;
            return Ok(false);
        }
        while self.chunks.len() < required {
            check(cancel)?;
            let mut chunk = Vec::new();
            if chunk.try_reserve_exact(self.words_per_chunk).is_err() {
                check(cancel)?;
                return Ok(false);
            }
            self.words_capacity = sum(&[self.words_capacity, chunk.capacity()])?;
            self.chunks.push(chunk);
        }
        check(cancel)?;
        Ok(true)
    }
    fn get(&self, record: usize) -> &[i64] {
        let chunk = record / self.records_per_chunk;
        let start = (record % self.records_per_chunk) * self.arity;
        &self.chunks[chunk][start..start + self.arity]
    }
    fn append_reserved(&mut self, powers: &[i64]) -> usize {
        debug_assert_eq!(powers.len(), self.arity);
        debug_assert!(self.records <= (usize::MAX >> 1));
        let record = self.records;
        let chunk = &mut self.chunks[record / self.records_per_chunk];
        debug_assert!(chunk.capacity() - chunk.len() >= powers.len());
        chunk.extend_from_slice(powers);
        self.records += 1;
        record
    }
}

// Conservative HashTable slot/control estimate. Using num_buckets for the
// retained allocation avoids guessing the load factor from capacity.
fn bucket_estimate(buckets: usize) -> GeneratedWorkResult<usize> {
    if buckets == 0 {
        return Ok(0);
    }
    sum(&[
        buckets
            .checked_mul(size_of::<Slot>() + 1)
            .ok_or_else(|| resource("resident Work slot estimate overflow"))?,
        64,
    ])
}
fn table_estimate(records: usize) -> GeneratedWorkResult<usize> {
    if records == 0 {
        return Ok(0);
    }
    let buckets = records
        .checked_mul(2)
        .and_then(usize::checked_next_power_of_two)
        .ok_or_else(|| resource("resident Work table estimate overflow"))?
        .max(4);
    bucket_estimate(buckets)
}

#[cfg(test)]
mod tests;
