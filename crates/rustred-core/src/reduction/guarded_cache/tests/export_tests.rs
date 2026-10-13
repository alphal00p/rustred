use super::*;

struct Sink<'a> {
    inner: Backing,
    cache: &'a GuardedRootCache,
    fail_after: Option<usize>,
    order: Vec<ApplicationCacheKey>,
}
impl GuardedCacheBacking for Sink<'_> {
    fn binding(&self) -> &ApplicationCacheBinding {
        self.inner.binding()
    }
    fn lookup(
        &mut self,
        _: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        panic!("export cannot perform backing lookups")
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        // This re-enters the public read-only API; it would deadlock if the
        // exporter retained its state mutex across arbitrary callbacks.
        assert!(self.cache.statistics().cached_integrals > 0);
        if self.fail_after == Some(self.inner.stores) {
            return Err(GuardedCacheBackingError {
                kind: GuardedCacheBackingErrorKind::Limit,
                message: "export sink full".into(),
            });
        }
        self.inner.store_completed(record, context)?;
        self.order.push(record.key.clone());
        Ok(())
    }
}

#[test]
fn export_is_unlocked_ordered_payload_only_and_does_not_touch_policy_usage() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [41; 32]);
    for policy in [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let mut cache = GuardedRootCache::with_policy(&program, Default::default(), policy);
        for n in [3, 1, 4] {
            program
                .reduce_guarded_root(&key(n), &mut cache, Default::default(), None)
                .unwrap();
        }
        program
            .reduce_guarded_root(&key(3), &mut cache, Default::default(), None)
            .unwrap();
        let before = cache.statistics();
        let mut sink = Sink {
            inner: Backing::new(&program),
            cache: &cache,
            fail_after: None,
            order: Vec::new(),
        };
        let exported = program.export_completed(&cache, &mut sink).unwrap();
        assert_eq!(exported.exported_records, 3);
        assert_eq!(
            exported.exported_coefficient_bytes,
            before.cached_coefficient_bytes
        );
        assert_eq!(
            exported.exported_coefficient_terms,
            before.cached_coefficient_terms
        );
        assert_eq!(
            sink.order,
            [1, 3, 4].map(|n| ApplicationCacheKey::guarded_root(&key(n)))
        );
        assert_eq!(cache.statistics(), before);
        for n in [1, 3, 4] {
            assert_eq!(
                sink.inner.records[&ApplicationCacheKey::guarded_root(&key(n))],
                baseline(&program, n)
            );
        }
        let mut restored = GuardedRootCache::with_policy(&program, Default::default(), policy);
        let hit = program
            .reduce_guarded_root(
                &key(4),
                &mut restored,
                Default::default(),
                Some(&mut sink.inner),
            )
            .unwrap();
        assert_eq!(hit.source, GuardedRootSource::Backing);
        assert_eq!(restored.statistics().work.rule_applications(), 0);
        assert_eq!(restored.statistics().ram_hits, 0, "usage is not serialized");
        assert_eq!(restored.statistics().backing_hits, 1);
    }
}

#[test]
fn export_reports_accepted_prefix_and_binding_failure_without_mutating_cache() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [42; 32]);
    let mut cache = GuardedRootCache::new(&program, Default::default());
    for n in 1..=4 {
        program
            .reduce_guarded_root(&key(n), &mut cache, Default::default(), None)
            .unwrap();
    }
    let before = cache.statistics();
    let mut sink = Sink {
        inner: Backing::new(&program),
        cache: &cache,
        fail_after: Some(2),
        order: Vec::new(),
    };
    let error = program.export_completed(&cache, &mut sink).unwrap_err();
    assert_eq!(error.accepted_prefix.exported_records, 2);
    assert!(matches!(
        error.source,
        GuardedCacheError::Backing(GuardedCacheBackingError {
            kind: GuardedCacheBackingErrorKind::Limit,
            ..
        })
    ));
    assert_eq!(sink.inner.records.len(), 2);
    assert_eq!(cache.statistics(), before);
    sink.inner.binding.artifact_digest[0] ^= 1;
    let error = program.export_completed(&cache, &mut sink).unwrap_err();
    assert_eq!(error.accepted_prefix.exported_records, 0);
    assert_eq!(error.source, GuardedCacheError::BindingMismatch);
    assert_eq!(sink.inner.records.len(), 2);
}
