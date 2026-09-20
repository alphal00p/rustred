//! Bounded sharing of full native images between scalar reconstruction calls.
//!
//! The limits bound retained images/slots, not cumulative probes. Deterministic
//! least-recently-used eviction may recompute an image through the unchanged
//! native oracle; it cannot alter a value, the frozen chronology, or authority.
//! An individual image too large for the cache still fails explicitly. Native
//! per-coefficient reconstruction budgets and transient probe memory are separate.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};

use super::super::{CacheKey, Fp};
use super::{
    Limits, MaterializationError, ProbeFrame, invalid,
    probe::{WeightImage, probe_prefix},
};

pub(super) enum Slot {
    Target(u32),
    Weight(usize),
}

struct CachedImage {
    image: Option<WeightImage>,
    charged_slots: usize,
    last_access: usize,
}

pub(super) struct ImageCache<'a> {
    frame: &'a ProbeFrame,
    target: usize,
    limits: Limits,
    images: HashMap<Arc<CacheKey>, CachedImage>,
    // Both indexes share the same immutable key allocation. Tree order depends
    // only on request order, never randomized HashMap iteration order.
    recency: BTreeMap<usize, Arc<CacheKey>>,
    next_access: usize,
    scalar_slots: usize,
    chronology: Option<Vec<Option<u32>>>,
    failure: Option<MaterializationError>,
}

impl<'a> ImageCache<'a> {
    pub(super) fn new(frame: &'a ProbeFrame, target: usize, limits: Limits) -> Self {
        Self {
            frame,
            target,
            limits,
            images: HashMap::new(),
            recency: BTreeMap::new(),
            next_access: 0,
            scalar_slots: 0,
            chronology: None,
            failure: None,
        }
    }

    pub(super) fn freeze(&mut self, chronology: Vec<Option<u32>>) {
        self.chronology = Some(chronology);
    }

    pub(super) fn image(
        &mut self,
        field: &Zp64,
        point: &[Fp],
    ) -> Result<Option<&WeightImage>, MaterializationError> {
        if self.limits.max_cached_images == 0 || self.limits.max_cached_values == 0 {
            return Err(invalid("cache budgets must be positive"));
        }
        // On theoretical counter exhaustion drop memoized data, not the frozen
        // mathematical branch. This avoids a cumulative-call ceiling as well as
        // wrapping two access IDs onto the same LRU position.
        if self.next_access == usize::MAX {
            self.images.clear();
            self.recency.clear();
            self.scalar_slots = 0;
            self.next_access = 0;
        }
        let access = self.next_access;
        self.next_access += 1;
        let key = (
            field.get_prime(),
            point.iter().map(|value| *value.inner()).collect::<Vec<_>>(),
        );
        if let Some((shared_key, entry)) = self.images.get_key_value(&key) {
            let shared_key = Arc::clone(shared_key);
            let old_access = entry.last_access;
            self.recency.remove(&old_access);
            self.recency.insert(access, Arc::clone(&shared_key));
            self.images
                .get_mut(&shared_key)
                .expect("cached entry exists")
                .last_access = access;
        } else {
            // Charge key coordinates even for unusable samples. Check the
            // cheap single-entry lower bound before another modular reduction.
            // Include both stored recency IDs, in addition to prime+point.
            let key_slots = point
                .len()
                .checked_add(3)
                .ok_or_else(|| invalid("cache key accounting overflow"))?;
            if key_slots > self.limits.max_cached_values {
                return Err(invalid("single probe key exceeds cached-value capacity"));
            }
            let source_limit = self
                .chronology
                .as_ref()
                .map_or(self.frame.rows().len(), Vec::len);
            let image = probe_prefix(
                self.frame,
                self.target,
                self.limits.max_weight_slots,
                source_limit,
                field,
                point,
            )?;
            // In addition to field entries, charge stored index/chronology
            // slots conservatively. Invalid samples still occupy an image slot.
            let slots = if let Some(image) = &image {
                image
                    .row
                    .len()
                    .checked_mul(2)
                    .and_then(|size| size.checked_add(image.weights.len()))
                    .and_then(|size| size.checked_add(image.pivots.len()))
                    .and_then(|size| size.checked_add(image.accepted_sources.len()))
                    .and_then(|size| size.checked_add(image.accepted_l_rows.len()))
                    .ok_or_else(|| invalid("cached image accounting overflow"))?
            } else {
                0
            };
            let charged_slots = key_slots
                .checked_add(slots)
                .filter(|sum| *sum <= self.limits.max_cached_values)
                .ok_or_else(|| invalid("single native image exceeds cached-value capacity"))?;
            // Subtraction is safe after the single-image admission above. Check
            // capacity without overflowing a prospective aggregate sum.
            while self.images.len() >= self.limits.max_cached_images
                || self.scalar_slots > self.limits.max_cached_values - charged_slots
            {
                let (_, evicted_key) = self
                    .recency
                    .pop_first()
                    .ok_or_else(|| invalid("cache recency/accounting invariant mismatch"))?;
                let evicted = self
                    .images
                    .remove(&evicted_key)
                    .ok_or_else(|| invalid("cache recency names an absent image"))?;
                self.scalar_slots = self
                    .scalar_slots
                    .checked_sub(evicted.charged_slots)
                    .ok_or_else(|| invalid("cache retained-slot accounting underflow"))?;
            }
            self.scalar_slots += charged_slots;
            let shared_key = Arc::new(key.clone());
            self.recency.insert(access, Arc::clone(&shared_key));
            self.images.insert(
                shared_key,
                CachedImage {
                    image,
                    charged_slots,
                    last_access: access,
                },
            );
        }
        let image = self.images.get(&key).and_then(|entry| entry.image.as_ref());
        Ok(image.filter(|image| {
            self.chronology
                .as_ref()
                .is_none_or(|expected| expected == &image.pivots)
        }))
    }

    pub(super) fn coefficient(&mut self, field: &Zp64, point: &[Fp], slot: Slot) -> Option<Fp> {
        if self.failure.is_some() {
            return None;
        }
        match self.image(field, point) {
            Ok(Some(image)) => match slot {
                Slot::Target(column) => Some(
                    image
                        .row
                        .iter()
                        .find(|(id, _)| *id == column)
                        .map_or_else(|| field.to_element(0), |(_, value)| *value),
                ),
                Slot::Weight(source) => image.weights.get(source).copied(),
            },
            Ok(None) => None,
            Err(error) => {
                self.failure = Some(error);
                None
            }
        }
    }

    pub(super) fn check_failure(&mut self) -> Result<(), MaterializationError> {
        self.failure.take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod adversarial_tests;
