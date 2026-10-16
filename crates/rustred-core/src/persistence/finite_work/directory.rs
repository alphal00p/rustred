use super::*;
use std::{collections::BTreeSet, mem::size_of, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedDirectoryLimits {
    pub max_arity: usize,
    pub max_runs: usize,
    pub max_pages: usize,
    pub max_directory_payload_bytes: usize,
    pub max_page_records: usize,
    pub max_page_bytes: usize,
    pub max_referenced_bytes: u64,
    pub max_lookup_pages: usize,
    pub max_lookup_bytes: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedDirectoryStatistics {
    pub records: u64,
    pub pages: usize,
    pub payload_bytes: usize,
    pub referenced_bytes: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedPageReadUsage {
    pub disk_bytes: u64,
    pub cache_hit: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedLookup {
    pub present: bool,
    pub candidate_pages: usize,
    pub disk_bytes: u64,
    pub cache_hits: usize,
}

/// A bounded borrowed-page visit avoids retaining W uncharged pages after
/// cache eviction. Core, not the provider, computes membership and absence.
/// On success the provider must visit exactly once; errors never mean absent.
pub trait GeneratedPageProvider: Send + Sync {
    fn visit_validated_page(
        &self,
        expected: &GeneratedWorkPageRef,
        remaining_read_bytes: u64,
        visitor: &mut dyn FnMut(&ValidatedWorkPage),
    ) -> GeneratedWorkResult<GeneratedPageReadUsage>;
}

/// Complete range directory of an explicitly TRUSTED-GENERATED checkpoint.
/// This constructor validates structure, not historical closure or source
/// equations. The routed adapter separately binds its actual immutable reducer.
#[derive(Clone)]
pub struct TrustedGeneratedWorkDirectory {
    execution: GeneratedExecutionBinding,
    generation: GeneratedWorkDigest,
    pub(super) runs: Arc<Vec<Vec<GeneratedWorkPageRef>>>,
    statistics: GeneratedDirectoryStatistics,
    limits: GeneratedDirectoryLimits,
}
impl TrustedGeneratedWorkDirectory {
    pub fn try_from_trusted_generated(
        execution: GeneratedExecutionBinding,
        generation: GeneratedWorkDigest,
        runs: Arc<Vec<Vec<GeneratedWorkPageRef>>>,
        limits: GeneratedDirectoryLimits,
    ) -> GeneratedWorkResult<Self> {
        let statistics = validate_directory(&execution, &runs, limits)?;
        Ok(Self {
            execution,
            generation,
            runs,
            statistics,
            limits,
        })
    }
    pub fn execution(&self) -> &GeneratedExecutionBinding {
        &self.execution
    }
    pub fn generation(&self) -> GeneratedWorkDigest {
        self.generation
    }
    pub fn statistics(&self) -> GeneratedDirectoryStatistics {
        self.statistics
    }
    pub fn lookup(
        &self,
        key: &GeneratedWorkKey,
        provider: &dyn GeneratedPageProvider,
    ) -> GeneratedWorkResult<GeneratedLookup> {
        key.validate(self.execution.physical_arity as usize)?;
        let mut outcome = GeneratedLookup::default();
        for run in self.runs.iter() {
            let index = run.partition_point(|page| &page.last < key);
            let Some(expected) = run.get(index).filter(|page| &page.first <= key) else {
                continue;
            };
            outcome.candidate_pages = outcome
                .candidate_pages
                .checked_add(1)
                .ok_or_else(|| resource("lookup page count overflow"))?;
            if outcome.candidate_pages > self.limits.max_lookup_pages {
                return Err(resource("finite lookup page allowance"));
            }
            let remaining = self
                .limits
                .max_lookup_bytes
                .checked_sub(outcome.disk_bytes)
                .ok_or_else(|| resource("finite lookup byte accounting"))?;
            let mut visited = false;
            let mut found = false;
            let mut violation = None;
            let usage = provider.visit_validated_page(expected, remaining, &mut |page| {
                if visited {
                    violation.get_or_insert_with(|| {
                        corrupt("generated page provider visited more than once")
                    });
                    return;
                }
                visited = true;
                if !page.matches(expected, &self.execution) {
                    violation = Some(corrupt(
                        "generated page provider substituted descriptor/execution",
                    ));
                    return;
                }
                found = page.contains(key);
            });
            // Callback errors cannot be suppressed by a provider returning Ok
            // (or its own unrelated error) after an invalid page/second visit.
            if let Some(error) = violation {
                return Err(error);
            }
            let usage = usage?;
            if !visited {
                return Err(corrupt(
                    "generated page provider omitted its required visit",
                ));
            }
            if usage.disk_bytes > remaining
                || (usage.cache_hit && usage.disk_bytes != 0)
                || (!usage.cache_hit && usage.disk_bytes != expected.object.bytes)
            {
                return Err(corrupt("generated page provider read accounting mismatch"));
            }
            outcome.disk_bytes = outcome
                .disk_bytes
                .checked_add(usage.disk_bytes)
                .ok_or_else(|| resource("lookup byte count overflow"))?;
            outcome.cache_hits = outcome
                .cache_hits
                .checked_add(usize::from(usage.cache_hit))
                .ok_or_else(|| resource("lookup cache count overflow"))?;
            if found {
                outcome.present = true;
                return Ok(outcome);
            }
        }
        // No provider callback can establish this negative. Every applicable
        // run/page was considered and exactly searched by this core method.
        Ok(outcome)
    }
}

pub fn validate_directory(
    execution: &GeneratedExecutionBinding,
    runs: &Vec<Vec<GeneratedWorkPageRef>>,
    limits: GeneratedDirectoryLimits,
) -> GeneratedWorkResult<GeneratedDirectoryStatistics> {
    let arity = execution.physical_arity as usize;
    if arity == 0
        || arity > limits.max_arity
        || limits.max_page_records == 0
        || limits.max_page_bytes == 0
    {
        return Err(resource("finite directory framing/arity policy"));
    }
    if runs.len() > limits.max_runs {
        return Err(resource("finite run directory"));
    }
    let mut result = GeneratedDirectoryStatistics::default();
    result.payload_bytes = runs
        .capacity()
        .checked_mul(size_of::<Vec<GeneratedWorkPageRef>>())
        .ok_or_else(|| resource("run directory overflow"))?;
    let mut digests = BTreeSet::new();
    for run in runs {
        if run.is_empty() {
            return Err(corrupt("empty finite run"));
        }
        add(
            &mut result.payload_bytes,
            run.capacity()
                .checked_mul(size_of::<GeneratedWorkPageRef>())
                .ok_or_else(|| resource("page directory overflow"))?,
        )?;
        let mut previous = None;
        for page in run {
            page.first.validate(arity)?;
            page.last.validate(arity)?;
            if page.records > limits.max_page_records as u64
                || page.object.bytes > limits.max_page_bytes as u64
            {
                return Err(resource("finite page record/byte allowance"));
            }
            if page.records == 0
                || page.object.bytes == 0
                || page.first > page.last
                || previous.is_some_and(|old| old >= &page.first)
            {
                return Err(corrupt("invalid finite page range/count/frame"));
            }
            previous = Some(&page.last);
            result.pages = result
                .pages
                .checked_add(1)
                .ok_or_else(|| resource("finite page count overflow"))?;
            if result.pages > limits.max_pages {
                return Err(resource("finite page count allowance"));
            }
            if !digests.insert(page.object.digest) {
                return Err(corrupt("duplicate referenced finite page"));
            }
            result.records = result
                .records
                .checked_add(page.records)
                .ok_or_else(|| resource("finite Work count overflow"))?;
            result.referenced_bytes = result
                .referenced_bytes
                .checked_add(page.object.bytes)
                .ok_or_else(|| resource("referenced finite disk overflow"))?;
            add(&mut result.payload_bytes, page.first.heap_bytes()?)?;
            add(&mut result.payload_bytes, page.last.heap_bytes()?)?;
            add(&mut result.payload_bytes, size_of::<GeneratedWorkDigest>())?;
            if result.payload_bytes > limits.max_directory_payload_bytes
                || result.referenced_bytes > limits.max_referenced_bytes
            {
                return Err(resource("finite directory/referenced disk allowance"));
            }
        }
    }
    if result.payload_bytes > limits.max_directory_payload_bytes {
        return Err(resource("finite empty directory allowance"));
    }
    Ok(result)
}
fn add(total: &mut usize, amount: usize) -> GeneratedWorkResult<()> {
    *total = total
        .checked_add(amount)
        .ok_or_else(|| resource("finite directory payload overflow"))?;
    Ok(())
}
