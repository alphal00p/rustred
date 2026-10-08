use super::*;
use symbolica::prelude::{Integer, MultivariatePolynomial, Z};

fn polynomial() -> CoefficientPolynomial {
    let variables = Arc::new(vec![PolyVariable::Temporary(0), PolyVariable::Temporary(1)]);
    let mut value = MultivariatePolynomial::<_, u16>::new(&Z, None, variables);
    value.append_monomial(Integer::from(3), &[1, 2]);
    value.append_monomial(Integer::from(-5), &[0, 1]);
    value
}

fn copy(cache: &mut TemplateCache, value: &CoefficientPolynomial) -> CoefficientPolynomial {
    cache.with_template(value, |template| value.clone_with_context_of(template))
}

fn assert_queue(cache: &TemplateCache) {
    let queued: std::collections::HashSet<_> = cache.sweep.iter().copied().collect();
    assert_eq!(queued.len(), cache.sweep.len());
    assert_eq!(queued.len(), cache.entries.len());
    assert!(queued.iter().all(|key| cache.entries.contains_key(key)));
}

#[test]
fn nested_local_copies_and_zeros_reuse_one_context() {
    let source = polynomial();
    let mut cache = TemplateCache::default();
    let mut local = copy(&mut cache, &source);
    let local_map = Arc::as_ptr(local.variables());
    for _ in 0..10_000 {
        local = copy(&mut cache, &local);
        assert_eq!(Arc::as_ptr(local.variables()), local_map);
        let zero = cache.with_template(&local, |template| template.zero_with_capacity(4));
        assert!(zero.is_zero());
        assert_eq!(Arc::as_ptr(zero.variables()), local_map);
    }
    assert_eq!(local, source);
    assert_eq!(cache.entries.len(), 2);
    assert_queue(&cache);
}

#[test]
fn local_alias_survives_source_template_eviction_without_context_changes() {
    let mut cache = TemplateCache::default();
    let source = polynomial();
    let source_key = Arc::as_ptr(source.variables()) as usize;
    let local = copy(&mut cache, &source);
    let local_key = Arc::as_ptr(local.variables()) as usize;
    drop(source);
    cache.sweep_expired();
    assert!(!cache.entries.contains_key(&source_key));
    assert!(cache.entries.contains_key(&local_key));
    let again = copy(&mut cache, &local);
    assert_eq!(Arc::as_ptr(again.variables()) as usize, local_key);
    assert_eq!(again, local);
    drop(local);
    drop(again);
    cache.sweep_expired();
    assert!(cache.entries.is_empty());
    assert_queue(&cache);
}

#[test]
fn many_live_equal_maps_keep_distinct_stable_local_contexts() {
    let mut cache = TemplateCache::default();
    let sources: Vec<_> = (0..2048).map(|_| polynomial()).collect();
    let local_maps: Vec<_> = sources
        .iter()
        .map(|source| Arc::as_ptr(copy(&mut cache, source).variables()) as usize)
        .collect();
    assert_eq!(
        local_maps
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        sources.len()
    );
    for (source, &key) in sources.iter().zip(&local_maps).rev() {
        let local = copy(&mut cache, source);
        assert_eq!(local, *source);
        assert_eq!(Arc::as_ptr(local.variables()) as usize, key);
        assert!(!Arc::ptr_eq(local.variables(), source.variables()));
    }
    assert_eq!(cache.entries.len(), 2 * sources.len());
    assert_queue(&cache);
    drop(sources);
    let sweep_calls = cache.entries.len().div_ceil(TemplateCache::SWEEP_BUDGET) + 1;
    for _ in 0..sweep_calls {
        cache.sweep_expired();
    }
    assert!(cache.entries.is_empty());
    assert_queue(&cache);
}

#[test]
fn expired_map_churn_has_bounded_cache_and_weak_guards_every_pointer() {
    let mut cache = TemplateCache::default();
    for _ in 0..10_000 {
        let source = polynomial();
        let local = copy(&mut cache, &source);
        let source_key = Arc::as_ptr(source.variables()) as usize;
        let local_key = Arc::as_ptr(local.variables()) as usize;
        assert!(
            matches!(&cache.entries[&source_key], TemplateEntry::Shared { map, .. }
            if map.as_ptr() as usize == source_key)
        );
        assert!(
            matches!(&cache.entries[&local_key], TemplateEntry::Local { map }
            if map.as_ptr() as usize == local_key)
        );
        // The cache holds no extra strong reference to the source map.
        assert_eq!(Arc::strong_count(source.variables()), 1);
        assert!(cache.entries.len() <= 2 * TemplateCache::SWEEP_BUDGET);
        assert_eq!(local, source);
    }
    for _ in 0..2 {
        cache.sweep_expired();
    }
    assert!(cache.entries.is_empty());
    assert_queue(&cache);
}

#[test]
fn local_polynomial_sent_to_another_thread_gets_that_threads_context() {
    let source = polynomial();
    let local = clone_thread_owned(&source);
    let other = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let copied = clone_thread_owned(&local);
                assert_eq!(copied, local);
                assert!(!Arc::ptr_eq(copied.variables(), local.variables()));
                let twice = clone_thread_owned(&copied);
                assert!(Arc::ptr_eq(copied.variables(), twice.variables()));
                copied
            })
            .join()
            .unwrap()
    });
    let returned = clone_thread_owned(&other);
    assert_eq!(returned, source);
    assert!(!Arc::ptr_eq(returned.variables(), other.variables()));
    let twice = clone_thread_owned(&returned);
    assert!(Arc::ptr_eq(returned.variables(), twice.variables()));
}

// Frozen pre-fix algorithm for a matched diagnostic, not a second production path.
#[derive(Default)]
struct LegacyCache(Vec<(Weak<Vec<PolyVariable>>, CoefficientPolynomial)>);
impl LegacyCache {
    fn copy(&mut self, shared: &CoefficientPolynomial) -> CoefficientPolynomial {
        let key = Arc::as_ptr(shared.variables());
        let index = match self
            .0
            .iter()
            .position(|entry| std::ptr::eq(entry.0.as_ptr(), key))
        {
            Some(index) => index,
            None => {
                self.0.retain(|entry| entry.0.strong_count() > 0);
                self.0.push((
                    Arc::downgrade(shared.variables()),
                    shared.zero_with_new_context(),
                ));
                self.0.len() - 1
            }
        };
        shared.clone_with_context_of(&self.0[index].1)
    }
}

#[test]
#[ignore = "focused release-mode old/new context-cache microbenchmark"]
fn benchmark_legacy_and_indexed_context_caches() {
    use std::hint::black_box;
    use std::time::Instant;
    for _ in 0..3 {
        let source = polynomial();
        let mut legacy = LegacyCache::default();
        let mut value = source.clone();
        let started = Instant::now();
        for _ in 0..8192 {
            value = legacy.copy(black_box(&value));
        }
        let old_seconds = started.elapsed().as_secs_f64();
        assert_eq!(value, source);
        let mut cache = TemplateCache::default();
        let mut value = source.clone();
        let started = Instant::now();
        for _ in 0..8192 {
            value = copy(&mut cache, black_box(&value));
        }
        let new_seconds = started.elapsed().as_secs_f64();
        assert_eq!(value, source);
        assert_eq!(cache.entries.len(), 2);
        println!(
            "nested copies=8192 old_seconds={old_seconds:.6} new_seconds={new_seconds:.6} old_contexts={} new_contexts=1",
            legacy.0.len()
        );

        let sources: Vec<_> = (0..4096).map(|_| polynomial()).collect();
        let mut legacy = LegacyCache::default();
        let started = Instant::now();
        for _ in 0..16 {
            for source in sources.iter().rev() {
                black_box(legacy.copy(source));
            }
        }
        let old_seconds = started.elapsed().as_secs_f64();
        let mut cache = TemplateCache::default();
        let started = Instant::now();
        for _ in 0..16 {
            for source in sources.iter().rev() {
                black_box(copy(&mut cache, source));
            }
        }
        let new_seconds = started.elapsed().as_secs_f64();
        assert_eq!(cache.entries.len(), sources.len() * 2);
        println!(
            "shared maps=4096 copies=65536 old_seconds={old_seconds:.6} new_seconds={new_seconds:.6}"
        );
    }
}
