//! Bounded, observed-cost dispatch hints. None of these values grants closure,
//! drops an obligation, or changes the retry/deferred queues. Pending candidates
//! remain Pending until actually issued. The whole small state is authenticated
//! scalar checkpoint metadata, including the fairness position and lookahead.
use super::super::super::queue::{CompactDomain, Phase};
use serde::{Deserialize, Serialize};

pub(crate) const WINDOW: usize = 64;
const BUCKETS: usize = 32;
const FAIRNESS: u8 = 8;
// Saturation is scheduling policy, not mathematical truncation. These caps keep
// every score cross-product below 2^73; counts/seconds in native receipts remain
// unmodified. One-hour cost and one-million growth/reuse cover a broad range.
const MAX_COST_US: u64 = 3_600_000_000;
const MAX_COUNT: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bucket {
    samples: u64,
    cost_us: u64,
    growth: u64,
    reuse: u64,
}

impl Default for Bucket {
    fn default() -> Self {
        Self {
            samples: 0,
            cost_us: 1_000,
            growth: 0,
            reuse: 0,
        }
    }
}

impl Bucket {
    fn valid(&self) -> bool {
        (1..=MAX_COST_US).contains(&self.cost_us)
            && self.growth <= MAX_COUNT
            && self.reuse <= MAX_COUNT
            && (self.samples != 0 || *self == Self::default())
    }

    fn observe(&mut self, seconds: f64, growth: u64, reuse: u64) {
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        let cost = (seconds * 1_000_000.0).clamp(1.0, MAX_COST_US as f64) as u64;
        let blend = |previous, sample| {
            if self.samples == 0 {
                sample
            } else {
                (3 * previous + sample) / 4
            }
        };
        self.cost_us = blend(self.cost_us, cost).max(1);
        self.growth = blend(self.growth, growth.min(MAX_COUNT));
        self.reuse = blend(self.reuse, reuse.min(MAX_COUNT));
        self.samples = self.samples.saturating_add(1);
    }

    fn ratio(&self) -> (u128, u128) {
        (
            u128::from(self.cost_us) * (u128::from(self.growth) + 1),
            u128::from(self.reuse) + 1,
        )
    }
}

/// Stable, bounded bucket geometry: phase x owner-cardinality band x rank band.
/// Geometry is read from the admitted immutable image, never a worker claim.
fn bucket<const N: usize>(image: &CompactDomain<N>) -> usize {
    let phase = usize::from(image.phase() == Phase::Route);
    let owners = image.owner().iter().filter(|&&present| present).count();
    let owner_band = owners.saturating_sub(1).checked_div(4).unwrap_or(0).min(3);
    let rank_band = match image.rank() {
        Some(0..=3) => 0,
        Some(4..=7) => 1,
        Some(_) => 2,
        None => 3,
    };
    phase * 16 + owner_band * 4 + rank_band
}

// Reject the 65th element while decoding; a forged length hint never controls
// allocation. This is also bounded when deserializing untrusted checkpoint JSON.
fn candidates<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<u32>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<u32>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "at most {WINDOW} adaptive candidate IDs")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut result = Vec::with_capacity(WINDOW);
            while let Some(id) = seq.next_element::<u32>()? {
                if result.len() == WINDOW {
                    return Err(serde::de::Error::custom(
                        "adaptive candidate window exceeds 64",
                    ));
                }
                result.push(id);
            }
            Ok(result)
        }
    }
    deserializer.deserialize_seq(Bounded)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Saved {
    version: u8,
    #[serde(deserialize_with = "candidates")]
    pub candidates: Vec<u32>,
    buckets: [Bucket; BUCKETS],
    next: u8,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            candidates: Vec::with_capacity(WINDOW),
            buckets: [Bucket::default(); BUCKETS],
            next: 0,
        }
    }
}

impl Saved {
    pub fn valid(&self, cursor: u32) -> bool {
        self.version == 1
            && self.next < FAIRNESS
            && self.candidates.len() <= WINDOW
            && self.candidates.iter().all(|&id| id < cursor)
            && self.candidates.windows(2).all(|pair| pair[0] < pair[1])
            && self.buckets.iter().all(Bucket::valid)
    }

    pub fn is_initial(&self) -> bool {
        self.candidates.is_empty()
            && self.next == 0
            && self
                .buckets
                .iter()
                .all(|bucket| *bucket == Bucket::default())
    }

    pub fn observe<const N: usize>(
        &mut self,
        image: &CompactDomain<N>,
        seconds: f64,
        growth: u64,
        reuse: u64,
    ) {
        self.buckets[bucket(image)].observe(seconds, growth, reuse);
    }

    /// Every eighth Pending selection is the oldest slot. A candidate can be
    /// bypassed at most 8*64 selections; discoveries cannot displace old slots.
    /// Ordinary ties also prefer the lowest immutable ID.
    pub fn choose<const N: usize>(&mut self, images: &[CompactDomain<N>]) -> Option<u32> {
        if self.candidates.is_empty() {
            return None;
        }
        let mut best = 0;
        if self.next != FAIRNESS - 1 {
            for index in 1..self.candidates.len() {
                let id = self.candidates[index];
                let best_id = self.candidates[best];
                let (num, den) = self.buckets[bucket(&images[id as usize])].ratio();
                let (best_num, best_den) = self.buckets[bucket(&images[best_id as usize])].ratio();
                if (num * best_den, id) < (best_num * den, best_id) {
                    best = index;
                }
            }
        }
        self.next = (self.next + 1) % FAIRNESS;
        Some(self.candidates.remove(best))
    }
}

#[cfg(test)]
mod tests;
