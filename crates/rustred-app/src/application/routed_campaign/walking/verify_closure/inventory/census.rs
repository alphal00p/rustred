//! Matching-only census over exact distinct recorded Apply inspection scopes.
use super::super::*;
use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Summary {
    pub complete: bool,
    pub seconds: f64,
    jobs: usize,
    completed: usize,
    apply_records: usize,
    route_records_skipped: usize,
    full_cover_records_skipped: usize,
    failed_record_replays: usize,
}
impl Summary {
    pub fn json(&self) -> Value {
        json!({"complete":self.complete,"seconds":self.seconds,
            "unique_apply_scopes":self.jobs,"completed":self.completed,
            "apply_records":self.apply_records,"route_records_skipped":self.route_records_skipped,
            "full_cover_records_skipped":self.full_cover_records_skipped,
            "failed_record_replays":self.failed_record_replays,
            "classification_event_counts":"weighted by identical saved inspection scopes; identity sets deduplicated",
            "matcher":"native ordered power-bounded owner matcher under the saved request",
            "rhs_and_successors":"omitted for completed records; replayed only for failed/interrupted records to preserve their classification prefix"})
    }
}

struct Job<const N: usize> {
    domain: CompactDomain<N>,
    multiplicity: u64,
    fallback: Option<usize>,
}

fn enqueue<const N: usize>(
    jobs: &mut Vec<Job<N>>,
    seen: &mut HashMap<u64, Vec<usize>>,
    domain: CompactDomain<N>,
) {
    let matches = seen.entry(domain.digest().0).or_default();
    if let Some(&index) = matches.iter().find(|&&index| jobs[index].domain == domain) {
        jobs[index].multiplicity += 1;
    } else {
        matches.push(jobs.len());
        jobs.push(Job {
            domain,
            multiplicity: 1,
            fallback: None,
        });
    }
}

pub(super) fn run<const N: usize>(
    request: &OwnerDomainWalkRequest,
    options: &OwnerDomainWalkVerifyOptions,
    loaded: &Loaded<N>,
    reducer: &RoutedCandidateReducer<N>,
    collector: &Collector,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Summary, AppError> {
    let started = Instant::now();
    let mut result = Summary::default();
    let mut jobs = Vec::<Job<N>>::new();
    // The digest only selects candidates: every hit compares the entire exact
    // compact image. A collision never merges different integral domains.
    let mut seen = HashMap::<u64, Vec<usize>>::new();
    for (id, node) in loaded.nodes.iter().enumerate() {
        if id % 4096 == 0 && cancellation.load(Ordering::Relaxed) {
            return Ok(result);
        }
        if !node.native() || node.abandoned {
            continue;
        }
        if loaded.domains[id].phase() == Phase::Route {
            result.route_records_skipped += 1;
            continue;
        }
        if node.kind == Kind::G2 && loaded.g2.get(&id).is_some_and(|g| g.residual.is_none()) {
            result.full_cover_records_skipped += 1;
            continue;
        }
        let domain = CompactDomain::try_from_domain(&inspected_domain(loaded, id))
            .map_err(AppError::input)?;
        result.apply_records += 1;
        if node.error || !node.finished {
            // RHS failures can stop the original matching stream early.
            // Do not invent classifications by finishing that stream now.
            result.failed_record_replays += 1;
            jobs.push(Job {
                domain,
                multiplicity: 1,
                fallback: Some(id),
            });
            continue;
        }
        enqueue(&mut jobs, &mut seen, domain);
    }
    drop(seen);
    result.jobs = jobs.len();
    observer(
        json!({"event":"inventory_census_started","total":jobs.len(),
        "apply_records":result.apply_records,"route_records_skipped":result.route_records_skipped,
        "failed_record_replays":result.failed_record_replays}),
    );
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let exited = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let workers = options.threads.max(1).min(jobs.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                let mut state = collector.local();
                loop {
                    if cancellation.load(Ordering::Relaxed) || failed.load(Ordering::Relaxed) { break; }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else { break; };
                    let domain = job.domain.expand();
                    let mut record = |piece: &OwnerDomainMatchPiece<N>| {
                        if let Err(error) = collector.record_weighted(&mut state, piece, job.multiplicity) {
                            state.error = Some(error); return ControlFlow::Break(());
                        }
                        ControlFlow::Continue(())
                    };
                    let outcome = if let Some(id) = job.fallback {
                        let mut events = 0u64;
                        let finished = inspection::inspect_reference_with_classifications(
                            reducer, &domain, request, cancellation, &mut record,
                            &mut |event| { events += event.count as u64; ControlFlow::Continue(()) },
                        );
                        let node = loaded.nodes[id];
                        if cancellation.load(Ordering::Relaxed) { Ok(()) }
                        else if node.error != finished.error.is_some() || node.events != Some(events) {
                            Err(AppError::execution(format!("failed Apply record {id} classification prefix replay differs from its saved event/error boundary")))
                        } else { Ok(()) }
                    } else {
                        reducer.programs().visit_power_bounded_owner_domain_matches(
                            domain.owner, &domain.lower, &domain.upper, domain.rank,
                            domain.powers, request.matching.match_limits, cancellation,
                            |piece| record(&piece),
                        ).map(|_| ()).map_err(|error| AppError::execution(format!("native inventory matching failed: {error:?}")))
                    };
                    if let Err(error) = outcome {
                        if !cancellation.load(Ordering::Relaxed) && state.error.is_none() { state.error = Some(error); }
                    }
                    if state.error.is_some() { failed.store(true, Ordering::Relaxed); break; }
                    if cancellation.load(Ordering::Relaxed) { break; }
                    done.fetch_add(1, Ordering::Relaxed);
                }
                collector.merge(state);
                exited.fetch_add(1, Ordering::Release);
            });
        }
        let mut last = Instant::now();
        while exited.load(Ordering::Acquire) < workers {
            std::thread::sleep(Duration::from_millis(100));
            if last.elapsed() >= Duration::from_secs(10) {
                observer(
                    json!({"event":"inventory_census_progress", "completed":done.load(Ordering::Relaxed),
                    "total":jobs.len(),"seconds":started.elapsed().as_secs_f64()}),
                );
                last = Instant::now();
            }
        }
    });
    result.completed = done.load(Ordering::Relaxed);
    result.complete = result.completed == jobs.len()
        && !cancellation.load(Ordering::Relaxed)
        && !failed.load(Ordering::Relaxed);
    result.seconds = started.elapsed().as_secs_f64();
    observer(
        json!({"event":"inventory_census_finished", "completed":result.completed,
        "total":result.jobs,"seconds":result.seconds,"complete":result.complete}),
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_saved_scopes_share_matching_but_retain_event_multiplicity() {
        let first = CompactDomain::try_from_domain(&Domain {
            phase: Phase::Apply,
            owner: [true, false],
            lower: vec![0, 0],
            upper: vec![Some(2), Some(0)],
            rank: Some(0),
            powers: DomainPowerBounds::default(),
        })
        .unwrap();
        let mut changed = first.expand();
        changed.upper[0] = Some(3);
        let changed = CompactDomain::try_from_domain(&changed).unwrap();
        let mut jobs = Vec::new();
        let mut seen = HashMap::new();
        enqueue(&mut jobs, &mut seen, first);
        enqueue(&mut jobs, &mut seen, first);
        // Simulate a digest collision: an unrelated exact image is never reused.
        seen.insert(changed.digest().0, vec![0]);
        enqueue(&mut jobs, &mut seen, changed);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].multiplicity, 2);
        assert_eq!(jobs[1].multiplicity, 1);
    }
}
