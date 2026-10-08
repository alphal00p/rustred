//! Saved execution order is provisional data, not a runnable Dispatch.
//! No new sequence can be issued until durable cross-session reservation and
//! exact unfinished-batch replay land. This decoder preserves every old ID.
use super::super::super::dispatch::DispatchSnapshot;
use super::super::super::ledger6::{EPOCH_LIMIT, Ledger6, Tag};
use super::super::super::state::JobMeta;
use super::super::{
    Reservations, SEQUENCE_COUNTER_LIMIT, SESSION_LIMIT, Section, SectionReceipt, invalid,
};
use super::{CheckedRead, open_section_with_arity};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::path::Path;

pub(super) struct Binding<'a> {
    pub ledger: &'a Ledger6,
    pub nodes: &'a [u8],
    pub k: u64,
    pub p0: u32,
    pub lockstep_b: usize,
    pub adaptive: Option<&'a super::super::super::dispatch::adaptive::Saved>,
}

pub(super) struct SavedDispatch {
    pub session: u64,
    pub counter: u64,
    pub cursor: u32,
    pub requeue: VecDeque<u32>,
    pub deferred: VecDeque<u32>,
    pub in_flight: BTreeMap<u32, JobMeta>,
    pub adaptive: Option<super::super::super::dispatch::adaptive::Saved>,
}

impl SavedDispatch {
    pub fn snapshot(&self) -> DispatchSnapshot<'_> {
        DispatchSnapshot {
            session: self.session,
            counter: self.counter,
            cursor: self.cursor,
            requeue: &self.requeue,
            deferred: &self.deferred,
            adaptive: self.adaptive.as_ref(),
        }
    }
}

fn queue(reader: &mut CheckedRead, remaining: &mut u64) -> io::Result<VecDeque<u32>> {
    let count = reader.u64()?;
    if count > *remaining
        || count
            .checked_mul(4)
            .is_none_or(|bytes| bytes > reader.remaining())
    {
        return Err(invalid(
            "epoch dispatch queue count exceeds inventory or bytes",
        ));
    }
    let count = usize::try_from(count).map_err(|_| invalid("epoch dispatch queue count range"))?;
    let mut queue = VecDeque::new();
    queue
        .try_reserve_exact(count)
        .map_err(|_| io::Error::other("epoch restored queue allocation"))?;
    for _ in 0..count {
        queue.push_back(reader.u32()?);
    }
    *remaining -= count as u64;
    Ok(queue)
}

pub(super) fn read<const N: usize>(
    directory: &Path,
    receipt: &SectionReceipt,
    count: u64,
    binding: Binding<'_>,
) -> io::Result<SavedDispatch> {
    read_with_arity::<N>(directory, receipt, count, binding, N)
}

pub(super) fn read_with_arity<const N: usize>(
    directory: &Path,
    receipt: &SectionReceipt,
    count: u64,
    binding: Binding<'_>,
    wire_arity: usize,
) -> io::Result<SavedDispatch> {
    if receipt.section != Section::Dispatch
        || binding.nodes.len() >= u32::MAX as usize
        || binding.nodes.len() != binding.ledger.words().len()
        || binding.k >= EPOCH_LIMIT
        || binding.p0 as usize > binding.nodes.len()
        || !(1..=4096).contains(&binding.lockstep_b)
        || count != binding.ledger.counts().get(Tag::Reserved)
        || count > binding.nodes.len() as u64
    {
        return Err(invalid("epoch dispatch/ledger binding differs"));
    }
    let (mut reader, _) = open_section_with_arity::<N>(directory, receipt, count, wire_arity)?;
    // Six scalars and three counts occupy72 bytes. Each Reserved descriptor
    // costs4 bytes, with16 more per in-flight item. Infer F from actual bytes
    // before allocating queues; huge forged counts cannot cause a reserve.
    let base = count
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(72))
        .ok_or_else(|| invalid("epoch dispatch byte count overflow"))?;
    let extra = reader
        .remaining()
        .checked_sub(base)
        .ok_or_else(|| invalid("epoch dispatch section shorter than inventory"))?;
    let flight_count = extra / 16;
    if extra % 16 != 0 || flight_count > count || flight_count > binding.lockstep_b as u64 {
        return Err(invalid("epoch dispatch byte shape or in-flight bound"));
    }
    if reader.u64()? != binding.k
        || reader.u64()? != u64::from(binding.p0)
        || reader.u64()? != binding.lockstep_b as u64
    {
        return Err(invalid(
            "epoch dispatch merge version, initial prefix or B differs",
        ));
    }
    let session = reader.u64()?;
    let counter = reader.u64()?;
    let cursor =
        u32::try_from(reader.u64()?).map_err(|_| invalid("epoch dispatch cursor range"))?;
    if session == 0
        || session >= SESSION_LIMIT
        || counter >= SEQUENCE_COUNTER_LIMIT
        || cursor as usize > binding.nodes.len()
    {
        return Err(invalid("epoch saved dispatch sequence or cursor range"));
    }
    let mut queued = count - flight_count;
    let requeue = queue(&mut reader, &mut queued)?;
    let deferred = queue(&mut reader, &mut queued)?;
    if queued != 0 || reader.u64()? != flight_count {
        return Err(invalid("epoch dispatch queue/in-flight inventory differs"));
    }
    let mut in_flight = BTreeMap::new();
    let mut previous = None;
    for _ in 0..flight_count {
        let id = reader.u32()?;
        if previous.is_some_and(|previous| previous >= id) {
            return Err(invalid("epoch in-flight IDs not strictly increasing"));
        }
        previous = Some(id);
        in_flight.insert(
            id,
            JobMeta {
                published_len: 0, // Refreshed with v0 when the saved job is reissued.
                seq: reader.u64()?,
                v0: reader.u64()?,
            },
        );
    }
    reader.finish()?;
    let saved = SavedDispatch {
        session,
        counter,
        cursor,
        requeue,
        deferred,
        in_flight,
        adaptive: binding.adaptive.cloned(),
    };
    Reservations {
        ledger: binding.ledger,
        nodes: binding.nodes,
        k: binding.k,
        in_flight: &saved.in_flight,
        dispatch: saved.snapshot(),
        lockstep_b: binding.lockstep_b,
    }
    .validate()?;
    Ok(saved)
}

#[cfg(test)]
mod tests;
