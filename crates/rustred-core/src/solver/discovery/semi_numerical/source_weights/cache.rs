//! Bounded sharing of full native images between scalar reconstruction calls.

use std::collections::HashMap;
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

pub(super) struct ImageCache<'a> {
    frame: &'a ProbeFrame,
    target: usize,
    limits: Limits,
    images: HashMap<CacheKey, Option<WeightImage>>,
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
        let key = (
            field.get_prime(),
            point.iter().map(|value| *value.inner()).collect::<Vec<_>>(),
        );
        if !self.images.contains_key(&key) {
            if self.images.len() >= self.limits.max_cached_images {
                return Err(invalid("cached-image budget exceeded"));
            }
            // Charge key coordinates even for unusable samples. Check the
            // cheap lower bound before running another modular reduction.
            let key_slots = point
                .len()
                .checked_add(1)
                .ok_or_else(|| invalid("cache key accounting overflow"))?;
            let base = self
                .scalar_slots
                .checked_add(key_slots)
                .filter(|sum| *sum <= self.limits.max_cached_values)
                .ok_or_else(|| invalid("cached-value budget exceeded by probe key"))?;
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
            let total = base
                .checked_add(slots)
                .filter(|sum| *sum <= self.limits.max_cached_values)
                .ok_or_else(|| invalid("cached-value budget exceeded by native image"))?;
            self.scalar_slots = total;
            self.images.insert(key.clone(), image);
        }
        let image = self.images.get(&key).and_then(Option::as_ref);
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
