//! One resume-boundary append transaction, before workers or lookup replicas.
//! A preparation/admission failure leaves the previously durable generation
//! authoritative. No partial amendment cursor is ever published.
use super::super::super::metadata::{Admission, Identity};
use super::{Restored, controller};
use crate::application::routed_campaign::walking::{epoch, rescue::AmendmentRef};
use rustred::solver::RoutedCandidateReducer;
use std::{cell::Cell, collections::BTreeSet, io};

fn refused(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::Interrupted,
        "rescue preparation interrupted; no amendment was published and the previous durable generation remains reusable",
    )
}

fn check_stop(stop: &mut impl FnMut() -> bool) -> io::Result<()> {
    if stop() { Err(interrupted()) } else { Ok(()) }
}

fn resize_bits(bits: &mut Vec<u64>, words: usize) -> io::Result<()> {
    bits.try_reserve(words.saturating_sub(bits.len()))
        .map_err(|_| io::Error::other("rescue bitset allocation"))?;
    bits.resize(words, 0);
    Ok(())
}

pub(super) fn apply<const N: usize>(
    restored: &mut Restored<N>,
    identity: &Identity<'_>,
    reducer: &RoutedCandidateReducer<N>,
    b: usize,
    mut requested: impl FnMut() -> bool,
    saved: &mut impl FnMut(&Restored<N>, &super::super::super::publication::Receipt),
) -> io::Result<()> {
    if identity.amendments().is_empty() {
        return Ok(());
    }
    let cancelled = Cell::new(false);
    let mut stop = || {
        let value = cancelled.get() || requested();
        cancelled.set(value);
        value
    };
    let scan_failed = || {
        if cancelled.get() {
            interrupted()
        } else {
            io::Error::other("rescue dependency graph unavailable or scratch allocation failed")
        }
    };
    check_stop(&mut stop)?;
    if !matches!(restored.admission, Admission::Complete) {
        return Err(refused("rescue requires complete original input admission"));
    }
    let recorded = restored
        .state
        .rescue
        .as_ref()
        .map_or(0, |r| r.amendments.len());
    let p0 = restored.state.p0;
    let superseded: BTreeSet<&str> = identity
        .amendments()
        .iter()
        .flat_map(|a| a.supersede.iter().map(String::as_str))
        .collect();
    // Existing abandoned obligations remain irreversible, even if a later
    // amendment revives geometrically equal coverage under a new ID.
    check_stop(&mut stop)?;
    let words = restored.state.store.len().div_ceil(64);
    let mut seeds = Vec::new();
    seeds
        .try_reserve_exact(words)
        .map_err(|_| io::Error::other("rescue seed allocation"))?;
    seeds.resize(words, 0);
    if let Some(rescue) = &restored.state.rescue {
        seeds.copy_from_slice(&rescue.abandoned);
    }
    for row in &restored.roots.rows {
        check_stop(&mut stop)?;
        if row["id"].as_str().is_some_and(|id| superseded.contains(id)) {
            if let Some(id) = row["domain"].as_u64() {
                seeds[id as usize / 64] |= 1 << (id % 64);
            }
        }
    }
    let quarantine = restored
        .state
        .tracker
        .tainted_with_cancellable(&seeds, &mut stop)
        .ok_or_else(&scan_failed)?;
    restored
        .state
        .store
        .enable_rescue_duplicates()
        .map_err(refused)?;
    restored
        .state
        .store
        .install_quarantine(quarantine)
        .map_err(refused)?;
    if restored.state.rescue.is_none() {
        check_stop(&mut stop)?;
        let mut abandoned = Vec::new();
        abandoned
            .try_reserve_exact(words)
            .map_err(|_| io::Error::other("rescue abandonment allocation"))?;
        abandoned.resize(words, 0);
        restored.state.rescue = Some(epoch::rescue::State {
            amendments: Vec::new(),
            abandoned,
        });
    }
    for amendment in identity.amendments().iter().skip(recorded) {
        check_stop(&mut stop)?;
        resize_bits(&mut seeds, restored.state.store.len().div_ceil(64))?;
        for row in &restored.roots.rows {
            check_stop(&mut stop)?;
            if row["id"].as_str().is_some_and(|id| superseded.contains(id)) {
                if let Some(id) = row["domain"].as_u64() {
                    seeds[id as usize / 64] |= 1 << (id % 64);
                }
            }
        }
        let quarantine = restored
            .state
            .tracker
            .tainted_with_cancellable(&seeds, &mut stop)
            .ok_or_else(&scan_failed)?;
        restored
            .state
            .store
            .install_quarantine(quarantine)
            .map_err(refused)?;
        let first_input = restored.roots.rows.len() as u64;
        let first_domain = u64::from(restored.state.watermark());
        let quarantined = epoch::rescue::count(&restored.state.store.quarantine);
        restored
            .state
            .rescue
            .as_mut()
            .unwrap()
            .amendments
            .try_reserve(1)
            .map_err(|_| refused("rescue amendment allocation"))?;
        for query in &amendment.queries {
            check_stop(&mut stop)?;
            let domain = crate::application::routed_campaign::walking::rescue::domain::<N>(
                query,
                reducer.programs().owner_sectors().any(|owner| {
                    owner.as_slice()
                        == rustred::storage_array::<_, N>(&query.owner, false)
                            .as_ref()
                            .map(|owner| owner.as_slice())
                            .unwrap_or(&[])
                }),
                identity.route_domain_overcover(),
                reducer.domain_routing_requires_source_conditions(),
            )
            .map_err(refused)?;
            let phase = Some(domain.phase);
            epoch::admission::one(
                &mut restored.state,
                query,
                phase,
                &mut restored.roots.rows,
                &mut restored.roots.frontiers,
            )
            .map_err(|error| refused(format!("rescue admission refused: {}", error.stop().0)))?;
            restored.roots.rows.last_mut().unwrap()["amendment"] = amendment.sequence.into();
        }
        restored
            .state
            .tracker
            .discovered(restored.state.store.len());
        restored
            .state
            .rescue
            .as_mut()
            .unwrap()
            .amendments
            .push(AmendmentRef {
                sequence: amendment.sequence,
                digest: amendment.digest.clone(),
                parent: amendment.parent.clone(),
                queries: amendment.queries.len() as u64,
                first_input,
                first_domain,
                quarantined,
                resumed_generation: restored
                    .publisher
                    .current_generation()
                    .ok_or_else(|| refused("rescue needs a durable resumed generation"))?,
            });
    }
    if restored.state.p0 != p0 {
        return Err(refused("rescue changed the protected original prefix"));
    }
    let live_roots = restored
        .roots
        .rows
        .iter()
        .filter(|row| !row["id"].as_str().is_some_and(|id| superseded.contains(id)))
        .filter_map(|row| row["domain"].as_u64())
        .map(|id| id as u32)
        .filter(|&id| !restored.state.store.is_quarantined(id));
    let reachable = restored
        .state
        .tracker
        .reachable_from_cancellable(live_roots.map(|id| id as usize), &mut stop)
        .ok_or_else(&scan_failed)?;
    let total = restored.state.store.len();
    check_stop(&mut stop)?;
    resize_bits(&mut seeds, total.div_ceil(64))?;
    let abandoned = &mut restored.state.rescue.as_mut().unwrap().abandoned;
    resize_bits(abandoned, total.div_ceil(64))?;
    for id in 0..total as u32 {
        if id % 1024 == 0 {
            check_stop(&mut stop)?;
        }
        if matches!(
            restored.state.ledger.tag(id),
            Some(
                epoch::ledger6::Tag::Pending
                    | epoch::ledger6::Tag::Reserved
                    | epoch::ledger6::Tag::Exhausted
            )
        ) && !epoch::rescue::contains(&reachable, id)
        {
            abandoned[id as usize / 64] |= 1 << (id % 64);
            if restored.state.ledger.tag(id) == Some(epoch::ledger6::Tag::Exhausted) {
                restored
                    .dispatch
                    .rescue_retry(id, &mut restored.state.ledger)
                    .map_err(refused)?;
            }
        }
    }
    for (seed, dead) in seeds.iter_mut().zip(abandoned.iter()) {
        *seed |= dead;
    }
    let quarantine = restored
        .state
        .tracker
        .tainted_with_cancellable(&seeds, &mut stop)
        .ok_or_else(&scan_failed)?;
    restored
        .state
        .store
        .install_quarantine(quarantine)
        .map_err(refused)?;
    let published_len = restored.state.watermark();
    for job in &mut restored.replay {
        job.flags = epoch::rescue::job_flags(&restored.state, job.parent);
        restored
            .state
            .in_flight
            .get_mut(&job.parent)
            .ok_or_else(|| refused("rescue replay reservation absent"))?
            .published_len = published_len;
    }
    if restored.state.g2_store.is_some() {
        check_stop(&mut stop)?;
        epoch::g2::enable(&mut restored.state);
    }
    super::super::roots::validate(
        &restored.roots,
        identity,
        reducer,
        &restored.state.store,
        p0,
    )?;
    check_stop(&mut stop)?;
    let receipt = controller::save_observed(restored, identity, b, None, None, &mut |_, _, _| {})
        .map_err(|failure| match failure {
        controller::Failure::Save(error) => error,
        controller::Failure::Engine(message) => refused(message),
    })?;
    saved(restored, &receipt);
    Ok(())
}
