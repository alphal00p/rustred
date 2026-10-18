use super::*;
use std::{
    collections::{BTreeSet, hash_map::RandomState},
    mem::size_of,
    sync::Arc,
};

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
    result.payload_bytes = directory_slots_payload(runs)?;
    // Metadata order is still the deterministic run/page order. This index
    // answers only exact digest membership; its randomized iteration is never
    // observed. Tight scratch grants retain the old tree implementation.
    let page_count = runs
        .iter()
        .fold(0usize, |sum, run| sum.saturating_add(run.len()));
    // This sizing prepass is only a hint. A failure must defer to the normal
    // page-ordered validator, not change which malformed record it reports.
    let measured_payload = directory_payload_bytes(runs).ok();
    let logical_index = page_count.checked_mul(size_of::<GeneratedWorkDigest>());
    let scratch_grant = logical_index
        .and_then(|index| measured_payload?.checked_sub(index))
        .map(|data| limits.max_directory_payload_bytes.saturating_sub(data))
        .unwrap_or(0);
    let generous_hash_bound = page_count.checked_mul(64).and_then(|n| n.checked_add(128));
    let mut hash_digests = if page_count <= limits.max_pages
        && generous_hash_bound.is_some_and(|n| n <= scratch_grant)
    {
        let mut index = hashbrown::HashSet::with_hasher(RandomState::new());
        // Public hashbrown allocation accounting guards the estimate too.
        // A failed optional contiguous allocation retains the old tree path.
        if index.try_reserve(page_count).is_ok()
            && index.allocation_size() <= scratch_grant.min(logical_index.unwrap_or(0))
        {
            Some(index)
        } else {
            None
        }
    } else {
        None
    };
    let mut tree_digests = BTreeSet::new();
    for run in runs {
        if run.is_empty() {
            return Err(corrupt("empty finite run"));
        }
        add(&mut result.payload_bytes, run_slots_payload(run)?)?;
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
            let new_digest = if let Some(digests) = &mut hash_digests {
                // Complete pre-reservation above covers every possible page;
                // equality still compares all 32 bytes, including collisions.
                digests.insert(&page.object.digest)
            } else {
                tree_digests.insert(page.object.digest)
            };
            if !new_digest {
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
            add_page_payload(&mut result.payload_bytes, page)?;
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

/// Measure the same actual vector/key capacities charged by directory
/// validation, without building its temporary uniqueness index. This is only
/// allocation accounting: it returns no validated directory or membership
/// authority, and deliberately does not authenticate ranges, records or IDs.
pub fn directory_payload_bytes(
    runs: &Vec<Vec<GeneratedWorkPageRef>>,
) -> GeneratedWorkResult<usize> {
    let mut bytes = directory_slots_payload(runs)?;
    for run in runs {
        add(&mut bytes, run_slots_payload(run)?)?;
        for page in run {
            add_page_payload(&mut bytes, page)?;
        }
    }
    Ok(bytes)
}

fn directory_slots_payload(runs: &Vec<Vec<GeneratedWorkPageRef>>) -> GeneratedWorkResult<usize> {
    runs.capacity()
        .checked_mul(size_of::<Vec<GeneratedWorkPageRef>>())
        .ok_or_else(|| resource("run directory overflow"))
}
fn run_slots_payload(run: &Vec<GeneratedWorkPageRef>) -> GeneratedWorkResult<usize> {
    run.capacity()
        .checked_mul(size_of::<GeneratedWorkPageRef>())
        .ok_or_else(|| resource("page directory overflow"))
}
fn add_page_payload(bytes: &mut usize, page: &GeneratedWorkPageRef) -> GeneratedWorkResult<()> {
    add(bytes, page.first.heap_bytes()?)?;
    add(bytes, page.last.heap_bytes()?)?;
    add(bytes, size_of::<GeneratedWorkDigest>())
}
fn add(total: &mut usize, amount: usize) -> GeneratedWorkResult<()> {
    *total = total
        .checked_add(amount)
        .ok_or_else(|| resource("finite directory payload overflow"))?;
    Ok(())
}

#[cfg(test)]
mod digest_index_tests {
    use super::*;

    fn fixture() -> (GeneratedExecutionBinding, Vec<Vec<GeneratedWorkPageRef>>) {
        let execution = GeneratedExecutionBinding {
            physical_arity: 2,
            program_identity: [1; 32],
        };
        let runs = (0..128u64)
            .map(|n| {
                // All first24 bytes match: only full digest equality is valid.
                let mut digest = [0; 32];
                digest[24..].copy_from_slice(&n.to_le_bytes());
                let key = GeneratedWorkKey::Route(vec![n as i64, 1]);
                vec![GeneratedWorkPageRef {
                    object: GeneratedObjectRef { digest, bytes: 128 },
                    records: 1,
                    first: key.clone(),
                    last: key,
                }]
            })
            .collect();
        (execution, runs)
    }
    fn limits(bytes: usize) -> GeneratedDirectoryLimits {
        GeneratedDirectoryLimits {
            max_arity: 2,
            max_runs: 1000,
            max_pages: 1000,
            max_directory_payload_bytes: bytes,
            max_page_records: 1000,
            max_page_bytes: 1000,
            max_referenced_bytes: 1_000_000,
            max_lookup_pages: 1000,
            max_lookup_bytes: 1_000_000,
        }
    }

    #[test]
    fn borrowed_hash_and_tight_grant_tree_have_identical_exact_validation() {
        let (execution, mut runs) = fixture();
        let payload = directory_payload_bytes(&runs).unwrap();
        let exact = validate_directory(&execution, &runs, limits(payload)).unwrap();
        let roomy = validate_directory(&execution, &runs, limits(payload + 1_000_000)).unwrap();
        assert_eq!(exact, roomy);
        assert_eq!(exact.records, 128);
        assert!(validate_directory(&execution, &runs, limits(payload - 1)).is_err());
        // Same invalid later page is reported independent of hash seeding.
        runs[100][0].object.digest = runs[0][0].object.digest;
        let tree_error = validate_directory(&execution, &runs, limits(payload)).unwrap_err();
        assert!(
            tree_error
                .to_string()
                .contains("duplicate referenced finite page")
        );
        for _ in 0..8 {
            assert_eq!(
                validate_directory(&execution, &runs, limits(payload + 1_000_000)).unwrap_err(),
                tree_error
            );
        }
        // Preserve run/page traversal order when several errors are present.
        runs[5][0].records = 0;
        let tree_error = validate_directory(&execution, &runs, limits(payload)).unwrap_err();
        assert!(
            tree_error
                .to_string()
                .contains("invalid finite page range/count/frame")
        );
        assert_eq!(
            validate_directory(&execution, &runs, limits(payload + 1_000_000)).unwrap_err(),
            tree_error
        );
    }
}
