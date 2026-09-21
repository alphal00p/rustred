//! Shared structural admission before native imports; no filesystem authority.

use rustred::persistence::{ProgramEnvelope, SectionTag};

use crate::application::AppError;

use super::super::{CandidateBundleLimits, codec, model::ProgramRecord};

pub(super) struct IngressBudget {
    limits: CandidateBundleLimits,
    collections: codec::CollectionBudget,
    coefficient_entries: usize,
    coefficient_bytes: usize,
}

impl IngressBudget {
    pub(super) fn new(limits: CandidateBundleLimits, sectors: usize) -> Result<Self, AppError> {
        Ok(Self {
            limits,
            collections: codec::CollectionBudget::new(limits, sectors)?,
            coefficient_entries: 0,
            coefficient_bytes: 0,
        })
    }

    pub(super) fn admit(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        let (envelope, record, _) = codec::read_structure(bytes, self.limits)?;
        self.admit_structure(&envelope, &record)
    }

    pub(super) fn admit_structure(
        &mut self,
        envelope: &ProgramEnvelope<'_>,
        record: &ProgramRecord,
    ) -> Result<(), AppError> {
        for sector in &record.sectors {
            self.collections.admit_sector(sector)?;
        }
        let table = envelope
            .section(SectionTag::COEFFICIENTS)
            .expect("checked section");
        let count_bytes = table
            .get(..8)
            .ok_or_else(|| AppError::schema("truncated coefficient count"))?;
        let count = usize::try_from(u64::from_le_bytes(count_bytes.try_into().unwrap()))
            .map_err(|_| AppError::limit("coefficient count exceeds host width"))?;
        // Preserve the complete-checkpoint diagnostics while sharing precisely
        // the same conservative accounting with selected immutable records.
        charge(
            &mut self.coefficient_entries,
            count,
            self.limits.max_collection_entries,
            "checkpoint aggregate coefficient-entry budget exceeded",
        )?;
        charge(
            &mut self.coefficient_bytes,
            table.len(),
            self.limits.max_total_coefficient_bytes,
            "checkpoint aggregate coefficient-table byte budget exceeded",
        )
    }
}

pub(super) fn charge(
    total: &mut usize,
    amount: usize,
    limit: usize,
    message: &'static str,
) -> Result<(), AppError> {
    let next = total
        .checked_add(amount)
        .ok_or_else(|| AppError::limit(message))?;
    if next > limit {
        return Err(AppError::limit(message));
    }
    *total = next;
    Ok(())
}
