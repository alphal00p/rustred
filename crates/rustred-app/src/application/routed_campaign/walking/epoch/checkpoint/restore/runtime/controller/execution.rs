//! Only the collection mechanism differs: one shared lockstep boundary loop.
//! Inline W1 never spawns, authorizes an off-thread CAS, or promises a poll
//! while CAS is running. Each poll returns after one caller-thread inspection.
use crate::application::routed_campaign::walking::epoch::inspector::{
    Activity, Poll, Pool, ResultMemory, ReturnedBytes, RunError, Status, SubmitError, Work,
    with_authorized_pool,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(super) trait Execution {
    fn enable_result_escrow(&mut self, total: usize) -> Result<(), String>;
    fn result_memory(&self) -> ResultMemory;
    fn recycle_result(&mut self, key: u64) -> Result<(), String>;
    fn activity(&self) -> Option<Activity>;
    fn profiled_activity(&self, key: Option<u64>) -> Option<(Activity, Option<Status>)>;
    fn submit(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError>;
    fn submit_rolling(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError>;
    fn retire(&mut self, keys: &[u64]) -> Result<(), String>;
    fn retire_all_returned(&mut self) -> Result<(), String>;
    fn poll(&mut self, timeout: Duration) -> Result<Poll, String>;
    fn cancel(&mut self) -> Result<(), String>;
    fn take_cancelled_status(&mut self) -> Result<Vec<Status>, String>;
}

impl Execution for Pool<'_> {
    fn enable_result_escrow(&mut self, total: usize) -> Result<(), String> {
        Pool::enable_result_escrow(self, total)
    }
    fn result_memory(&self) -> ResultMemory {
        Pool::result_memory(self)
    }
    fn recycle_result(&mut self, key: u64) -> Result<(), String> {
        Pool::recycle_result(self, key)
    }
    fn profiled_activity(&self, key: Option<u64>) -> Option<(Activity, Option<Status>)> {
        Pool::profiled_activity(self, key)
    }
    fn activity(&self) -> Option<Activity> {
        Pool::activity(self)
    }
    fn submit(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError> {
        Pool::submit(self, jobs)
    }
    fn submit_rolling(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError> {
        Pool::submit_rolling(self, jobs)
    }
    fn retire(&mut self, keys: &[u64]) -> Result<(), String> {
        Pool::retire(self, keys)
    }
    fn retire_all_returned(&mut self) -> Result<(), String> {
        Pool::retire_all_returned(self)
    }
    fn poll(&mut self, timeout: Duration) -> Result<Poll, String> {
        Pool::poll(self, timeout)
    }
    fn cancel(&mut self) -> Result<(), String> {
        Pool::cancel(self)
    }
    fn take_cancelled_status(&mut self) -> Result<Vec<Status>, String> {
        Pool::take_cancelled_status(self)
    }
}

struct Inline<'a> {
    inspect: &'a (dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync),
    jobs: VecDeque<Work>,
    status: Vec<Status>,
    next: usize,
    stop: AtomicBool,
}

impl Execution for Inline<'_> {
    fn enable_result_escrow(&mut self, _: usize) -> Result<(), String> {
        Err("inline execution has no result escrow".into())
    }
    fn result_memory(&self) -> ResultMemory {
        ResultMemory::default()
    }
    fn recycle_result(&mut self, _: u64) -> Result<(), String> {
        Err("inline execution cannot recycle unmerged results".into())
    }
    fn profiled_activity(&self, key: Option<u64>) -> Option<(Activity, Option<Status>)> {
        let activity = self.activity()?;
        Some((
            activity,
            self.status
                .iter()
                .find(|status| Some(status.key) == key)
                .copied(),
        ))
    }
    fn activity(&self) -> Option<Activity> {
        if self.stop.load(Ordering::Acquire) && self.status.is_empty() {
            return None;
        }
        Some(Activity::from_status(
            self.status.iter(),
            self.stop.load(Ordering::Acquire),
            true,
        ))
    }
    fn submit(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError> {
        if self.stop.load(Ordering::Acquire) || self.jobs.len() != 0 || jobs.len() > 4096 {
            return Err(SubmitError::Protocol(
                "inline submit outside empty lockstep slot",
            ));
        }
        for (index, job) in jobs.iter().enumerate() {
            if jobs[..index].iter().any(|prior| prior.key == job.key) {
                return Err(SubmitError::Protocol("inline duplicate sequence"));
            }
        }
        self.status.clear();
        self.status
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("inline status allocation"))?;
        self.status.extend(jobs.iter().map(|job| Status {
            key: job.key,
            started: false,
            returned: false,
        }));
        self.jobs = jobs.into();
        self.next = 0;
        Ok(())
    }

    fn poll(&mut self, _: Duration) -> Result<Poll, String> {
        if self.stop.load(Ordering::Acquire) {
            return Err("inline poll after cancellation".into());
        }
        let Some(job) = self.jobs.pop_front() else {
            return Ok(Poll::Drained);
        };
        let status = self
            .status
            .get_mut(self.next)
            .ok_or("inline receipt missing")?;
        if status.key != job.key || status.started {
            return Err("inline receipt differs".into());
        }
        status.started = true;
        // Native W1's callback uses the caller cancellation flag; no helper
        // thread is needed to propagate cancellation into cooperative CAS.
        let bytes = (self.inspect)(&job.bytes, &self.stop);
        status.returned = true;
        self.next += 1;
        Ok(Poll::Result {
            key: job.key,
            bytes: ReturnedBytes::uncharged(bytes),
        })
    }

    fn submit_rolling(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError> {
        if self.stop.load(Ordering::Acquire) || self.status.len().saturating_add(jobs.len()) > 4096
        {
            return Err(SubmitError::Protocol(
                "inline rolling bound or cancellation",
            ));
        }
        for (at, job) in jobs.iter().enumerate() {
            if self.status.iter().any(|s| s.key == job.key)
                || jobs[..at].iter().any(|j| j.key == job.key)
            {
                return Err(SubmitError::Protocol("inline duplicate rolling sequence"));
            }
        }
        self.status
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("inline status allocation"))?;
        self.jobs
            .try_reserve(jobs.len())
            .map_err(|_| SubmitError::Allocation("inline jobs allocation"))?;
        self.status.extend(jobs.iter().map(|job| Status {
            key: job.key,
            started: false,
            returned: false,
        }));
        self.jobs.extend(jobs);
        Ok(())
    }

    fn retire(&mut self, keys: &[u64]) -> Result<(), String> {
        if keys
            .iter()
            .any(|key| !self.status.iter().any(|s| s.key == *key && s.returned))
        {
            return Err("inline retirement before returned receipt".into());
        }
        self.status.retain(|s| !keys.contains(&s.key));
        self.next = self.next.saturating_sub(keys.len());
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        self.jobs.clear();
        Ok(())
    }

    fn retire_all_returned(&mut self) -> Result<(), String> {
        if !self.jobs.is_empty() || self.status.iter().any(|status| !status.returned) {
            return Err("inline batch retirement before returned receipts".into());
        }
        self.status.clear();
        self.next = 0;
        Ok(())
    }

    fn take_cancelled_status(&mut self) -> Result<Vec<Status>, String> {
        if !self.stop.load(Ordering::Acquire) {
            return Err("inline status before cancellation".into());
        }
        Ok(std::mem::take(&mut self.status))
    }
}

pub(super) fn with<R>(
    budget: usize,
    authorize: &(dyn Fn() -> Result<(), String> + Sync),
    inspect: &(dyn Fn(&[u8], &AtomicBool) -> Vec<u8> + Sync),
    body: impl FnOnce(&mut dyn Execution) -> R,
) -> Result<R, RunError> {
    if budget == 1 {
        let mut inline = Inline {
            inspect,
            jobs: VecDeque::new(),
            status: Vec::new(),
            next: 0,
            stop: AtomicBool::new(false),
        };
        Ok(body(&mut inline))
    } else {
        with_authorized_pool(budget - 1, authorize, inspect, |pool| body(pool))
    }
}
