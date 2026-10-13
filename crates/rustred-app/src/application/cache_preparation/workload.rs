use std::{collections::BTreeMap, sync::Arc};

use rustred::{
    family::IntegralKey,
    persistence::application_cache::{ApplicationCacheBinding, ApplicationCacheIoLimits},
    solver::RootRegionInput,
};
use symbolica::{combinatorics::CombinationIterator, domains::integer::Integer};

use crate::{
    AppError, EntryPowerBudget, FiniteEntryDomain, SavedApplicationWitnessKind,
    saved_application_boundary_witnesses,
};

/// Caller-selected finite primer policy, unrelated to backend coverage or loop
/// order. Relative dots are counted above one power on each surviving line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePreparationPrimerBudget {
    Relative {
        max_extra_dots: u32,
        max_numerator_rank: u32,
    },
    Absolute(EntryPowerBudget),
}
impl Default for CachePreparationPrimerBudget {
    fn default() -> Self {
        Self::Relative {
            max_extra_dots: 2,
            max_numerator_rank: 1,
        }
    }
}
impl CachePreparationPrimerBudget {
    fn for_support(self, active: usize) -> Result<EntryPowerBudget, AppError> {
        match self {
            Self::Absolute(budget) => Ok(budget),
            Self::Relative {
                max_extra_dots,
                max_numerator_rank,
            } => Ok(EntryPowerBudget {
                max_positive_power: u32::try_from(active)
                    .ok()
                    .and_then(|n| n.checked_add(max_extra_dots))
                    .ok_or_else(|| {
                        AppError::limit("cache primer positive-power allowance overflow")
                    })?,
                max_numerator_rank,
                min_power_difference: None,
                max_power_difference: None,
            }),
        }
    }
}

fn validate_bindings(bindings: &[ApplicationCacheBinding]) -> Result<(), AppError> {
    if bindings.is_empty() || bindings.len() > 128 {
        return Err(AppError::input(
            "cache workload requires 1..=128 exact namespaces",
        ));
    }
    for (index, binding) in bindings.iter().enumerate() {
        binding
            .validate(ApplicationCacheIoLimits::default())
            .map_err(|error| AppError::input(error.to_string()))?;
        if bindings[..index].contains(binding) {
            return Err(AppError::input("duplicate cache workload namespace"));
        }
    }
    Ok(())
}

/// A native root proposal. It must still cross the active application's entry
/// admission before execution; a workload is not new coverage authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachePreparationTarget {
    namespace: usize,
    integral: IntegralKey,
    source_occurrences: u64,
}

impl CachePreparationTarget {
    pub fn namespace(&self) -> usize {
        self.namespace
    }
    pub fn integral(&self) -> &IntegralKey {
        &self.integral
    }
    /// Duplicate source requests are recorded, not fabricated cache hits.
    pub fn source_occurrences(&self) -> u64 {
        self.source_occurrences
    }
}

/// Immutable, bounded and deduplicated execution plan, independent of cache
/// capacity and elapsed-time policy. Bindings are retained once per namespace.
#[derive(Clone, Debug)]
pub struct CachePreparationWorkload {
    bindings: Arc<[ApplicationCacheBinding]>,
    targets: Arc<[CachePreparationTarget]>,
    digest: [u8; 32],
}

impl CachePreparationWorkload {
    /// Enumerate all pinches of explicitly supplied physical parent supports,
    /// including the empty support, using native finite A/R domains. A false
    /// parent slot can never become positive; auxiliary axes may occur anywhere.
    /// The caller authenticates which lines are physical, and the backend still
    /// validates every proposal before cache lookup or reduction.
    ///
    /// `max_requests` bounds generated proposals INCLUDING duplicate parents or
    /// overlapping supports, as in [`Self::explicit`]. Exact native combinatorial
    /// preflight rejects an oversized workload before constructing keys. The same
    /// allowance (at least one) bounds positive layers in each native count.
    /// Counts and enumeration are operational preparation, not mathematical
    /// coverage, topology recognition, rule generation or a minimal basis claim.
    pub fn parent_pinches(
        bindings: Vec<ApplicationCacheBinding>,
        parents: &[(usize, Vec<bool>)],
        budget: CachePreparationPrimerBudget,
        max_requests: usize,
    ) -> Result<Self, AppError> {
        validate_bindings(&bindings)?;
        // Check every declaration before potentially expensive preflight work,
        // including mixed namespaces and a malformed later parent.
        for (namespace, support) in parents {
            let binding = bindings
                .get(*namespace)
                .ok_or_else(|| AppError::input("unknown cache primer namespace"))?;
            if support.len() != binding.physical_arity {
                return Err(AppError::input(
                    "cache primer parent differs from physical arity",
                ));
            }
        }
        FiniteEntryDomain::new(
            vec![false; bindings[0].physical_arity],
            budget.for_support(0)?,
        )?;
        let mut count = Integer::from(0);
        let cap = Integer::from(max_requests);
        let mut groups = Vec::new();
        for (namespace, parent) in parents {
            let physical = parent.iter().filter(|&&present| present).count();
            for active in 0..=physical {
                let selected = budget.for_support(active)?;
                // Permutation-independent count: false axes include both pinched
                // physical lines and auxiliaries. No coefficient algebra is used.
                let representative = (0..parent.len()).map(|axis| axis < active).collect();
                let domain = FiniteEntryDomain::new(representative, selected)?;
                let per_support = domain.target_count(max_requests.max(1))?;
                if per_support == Integer::from(0) {
                    continue;
                }
                count += per_support * Integer::binom(physical as i64, active as i64);
                if count > cap {
                    return Err(AppError::limit(
                        "cache primer exceeds its request allowance",
                    ));
                }
                groups
                    .try_reserve(1)
                    .map_err(|_| AppError::limit("cannot allocate cache primer groups"))?;
                groups.push((*namespace, parent, active, selected));
            }
        }
        let mut requests = Vec::new();
        for (namespace, parent, active, selected) in groups {
            let physical = parent
                .iter()
                .enumerate()
                .filter_map(|(axis, &present)| present.then_some(axis))
                .collect::<Vec<_>>();
            let mut append = |indices: &[usize]| -> Result<(), AppError> {
                let mut support = vec![false; parent.len()];
                for &index in indices {
                    support[physical[index]] = true;
                }
                for target in FiniteEntryDomain::new(support, selected)?.targets() {
                    if requests.len() >= max_requests {
                        return Err(AppError::limit(
                            "cache primer exceeds its request allowance",
                        ));
                    }
                    requests
                        .try_reserve(1)
                        .map_err(|_| AppError::limit("cannot allocate cache primer requests"))?;
                    requests.push((namespace, target?));
                }
                Ok(())
            };
            if active == 0 {
                append(&[])?;
            } else {
                let mut combinations = CombinationIterator::new(physical.len(), active);
                while let Some(indices) = combinations.next() {
                    append(indices)?;
                }
            }
        }
        if Integer::from(requests.len()) != count {
            return Err(AppError::input(
                "native cache primer count/enumeration mismatch",
            ));
        }
        Self::explicit(bindings, requests, max_requests)
    }

    /// Enumerate caller-selected finite power budgets with the existing native
    /// A/R composition iterator. These are operational warmup proposals, not a
    /// physics profile or an extension of any backend's admitted root domain.
    pub fn finite_entry_domains(
        bindings: Vec<ApplicationCacheBinding>,
        domains: &[(usize, FiniteEntryDomain)],
        max_requests: usize,
    ) -> Result<Self, AppError> {
        if domains.len() > max_requests {
            return Err(AppError::limit("cache workload domain allowance exceeded"));
        }
        let mut requests = Vec::new();
        for (namespace, domain) in domains {
            for target in domain.targets() {
                if requests.len() >= max_requests {
                    return Err(AppError::limit(
                        "cache workload exceeds its request allowance",
                    ));
                }
                requests
                    .try_reserve(1)
                    .map_err(|_| AppError::limit("cannot allocate cache workload requests"))?;
                requests.push((*namespace, target?));
            }
        }
        Self::explicit(bindings, requests, max_requests)
    }

    pub fn explicit(
        bindings: Vec<ApplicationCacheBinding>,
        requests: impl IntoIterator<Item = (usize, IntegralKey)>,
        max_requests: usize,
    ) -> Result<Self, AppError> {
        validate_bindings(&bindings)?;
        let mut roots = BTreeMap::<(usize, IntegralKey), u64>::new();
        for (ordinal, (namespace, integral)) in requests.into_iter().enumerate() {
            if ordinal >= max_requests {
                return Err(AppError::limit(
                    "cache workload exceeds its request allowance",
                ));
            }
            let binding = bindings
                .get(namespace)
                .ok_or_else(|| AppError::input("unknown cache workload namespace"))?;
            if integral.powers().len() != binding.physical_arity {
                return Err(AppError::input(
                    "cache workload key differs from physical arity",
                ));
            }
            let key = (namespace, integral);
            let count = roots.entry(key).or_default();
            *count = count
                .checked_add(1)
                .ok_or_else(|| AppError::limit("cache workload occurrence count overflow"))?;
        }
        let targets = roots
            .into_iter()
            .map(
                |((namespace, integral), source_occurrences)| CachePreparationTarget {
                    namespace,
                    integral,
                    source_occurrences,
                },
            )
            .collect::<Vec<_>>();
        let mut hash = blake3::Hasher::new();
        hash.update(b"rustred.cache-preparation-workload.v1\0");
        // Only metadata is serialized here; algebra bytes belong to the cache
        // codec. Full native keys, not their hashes, control deduplication.
        let namespaces = bincode::encode_to_vec(&bindings, bincode::config::standard())
            .map_err(|error| AppError::serialization(error.to_string()))?;
        hash.update(&namespaces);
        for target in &targets {
            hash.update(
                &u64::try_from(target.namespace)
                    .map_err(|_| AppError::limit("namespace index overflow"))?
                    .to_le_bytes(),
            );
            hash.update(&target.source_occurrences.to_le_bytes());
            for power in target.integral.powers() {
                hash.update(&power.to_le_bytes());
            }
        }
        Ok(Self {
            bindings: bindings.into(),
            targets: targets.into(),
            digest: *hash.finalize().as_bytes(),
        })
    }

    /// Reuse the native correlated-domain boundary sampler on the ACTIVE
    /// program's entry_regions(), never reread a possibly replaced path.
    pub fn saved_entry_boundaries<const N: usize>(
        binding: ApplicationCacheBinding,
        regions: &[RootRegionInput<N>],
        max_roots: usize,
    ) -> Result<Self, AppError> {
        let mut requests = Vec::new();
        let allowance = regions
            .len()
            .checked_mul(3)
            .ok_or_else(|| AppError::limit("cache boundary request count overflow"))?;
        // The native sampler produces at most three records per region. The
        // caller-selected root cap also bounds provenance-only duplicates.
        if allowance > max_roots.saturating_mul(3) {
            return Err(AppError::limit(
                "cache boundary regions exceed the workload allowance",
            ));
        }
        requests
            .try_reserve_exact(allowance)
            .map_err(|_| AppError::limit("cannot allocate cache boundary requests"))?;
        for region in regions {
            for witness in saved_application_boundary_witnesses(region)? {
                match witness.kind {
                    SavedApplicationWitnessKind::Representative
                    | SavedApplicationWitnessKind::MaximumNumeratorRank
                    | SavedApplicationWitnessKind::MaximumPositivePower => {}
                }
                requests.push((0, witness.integral));
            }
        }
        let result = Self::explicit(vec![binding], requests, allowance)?;
        if result.targets.len() > max_roots {
            return Err(AppError::limit("cache workload exceeds its root allowance"));
        }
        Ok(result)
    }

    pub fn bindings(&self) -> &[ApplicationCacheBinding] {
        &self.bindings
    }
    pub fn targets(&self) -> &[CachePreparationTarget] {
        &self.targets
    }
    /// Deterministic round-robin over namespaces, preserving exact key order
    /// inside each namespace. A time budget cannot privilege the first family
    /// simply because its complete workload sorts before every other family.
    pub fn execution_order(&self) -> impl Iterator<Item = &CachePreparationTarget> {
        let mut ranges = (0..self.bindings.len())
            .map(|namespace| {
                let start = self
                    .targets
                    .partition_point(|target| target.namespace < namespace);
                let end = self
                    .targets
                    .partition_point(|target| target.namespace <= namespace);
                start..end
            })
            .collect::<Vec<_>>();
        let mut namespace = 0;
        let mut remaining = self.targets.len();
        std::iter::from_fn(move || {
            if remaining == 0 {
                return None;
            }
            loop {
                let selected = namespace;
                namespace = (namespace + 1) % ranges.len();
                if let Some(index) = ranges[selected].next() {
                    remaining -= 1;
                    return Some(&self.targets[index]);
                }
            }
        })
    }
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}
