//! Independent pre-index policy model: full linear semantic forward/reverse
//! scans, including exact/orthant priority and immutable admission IDs.
use super::*;

#[derive(Default)]
struct Bucket {
    ids: Vec<usize>,
    orthant: Option<usize>,
}

pub(super) struct LinearSemantic<const N: usize> {
    pub domains: Vec<Arc<Domain<N>>>,
    summaries: Vec<DomainPowerSummary<N>>,
    exact: HashMap<Arc<Domain<N>>, usize>,
    buckets: HashMap<(Phase, [bool; N]), Bucket>,
    max_domains: usize,
    pub comparisons: usize,
    pub maintenance: usize,
}

impl<const N: usize> LinearSemantic<N> {
    pub fn new(max_domains: usize) -> Self {
        Self {
            domains: Vec::new(),
            summaries: Vec::new(),
            exact: HashMap::new(),
            buckets: HashMap::new(),
            max_domains,
            comparisons: 0,
            maintenance: 0,
        }
    }

    pub fn admit(&mut self, domain: Domain<N>) -> Result<(usize, bool), &'static str> {
        if let Some(&id) = self.exact.get(&domain) {
            return Ok((id, false));
        }
        let summary = DomainPowerSummary::try_new(
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        )
        .map_err(summary_error)?;
        let key = (domain.phase, domain.owner);
        if let Some(bucket) = self.buckets.get(&key) {
            if let Some(id) = bucket.orthant
                && rank_contains(self.domains[id].rank, domain.rank)
            {
                return Ok((id, false));
            }
            for &id in &bucket.ids {
                self.comparisons += 1;
                if self.summaries[id].contains(&summary) {
                    return Ok((id, false));
                }
            }
        }
        if self.domains.len() == self.max_domains {
            return Err("scheduled domain allowance");
        }
        let id = self.domains.len();
        let bucket = self.buckets.entry(key).or_default();
        self.comparisons += bucket.ids.len();
        self.maintenance += bucket.ids.len();
        bucket
            .ids
            .retain(|&old| !summary.contains(&self.summaries[old]));
        bucket.ids.push(id);
        if domain.is_full_orthant()
            && bucket
                .orthant
                .is_none_or(|old| rank_contains(domain.rank, self.domains[old].rank))
        {
            bucket.orthant = Some(id);
        }
        let domain = Arc::new(domain);
        self.exact.insert(Arc::clone(&domain), id);
        self.domains.push(domain);
        self.summaries.push(summary);
        Ok((id, true))
    }

    pub fn assert_same_state(&self, queue: &Queue<N>) {
        assert_eq!(self.domains, queue.domains);
        assert_eq!(self.summaries, queue.summaries);
        assert_eq!(self.exact, queue.exact);
        assert_eq!(self.buckets.len(), queue.by_owner.len());
        for (key, bucket) in &self.buckets {
            let indexed = &queue.by_owner[key];
            assert_eq!(bucket.ids, indexed.candidate_ids());
            assert_eq!(bucket.orthant, indexed.orthant);
        }
        assert_eq!(
            queue.next, 0,
            "lookup retirement must not discharge pending jobs"
        );
    }
}
