//! Only the collection mechanism differs: one shared lockstep boundary loop.
//! Inline W1 never spawns, authorizes an off-thread CAS, or promises a poll
//! while CAS is running. Each poll returns after one caller-thread inspection.
use crate::application::routed_campaign::walking::epoch::inspector::{
    Poll, Pool, RunError, Status, SubmitError, Work, with_authorized_pool,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(super) trait Execution {
    fn submit(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError>;
    fn poll(&mut self, timeout: Duration) -> Result<Poll, String>;
    fn cancel(&mut self) -> Result<(), String>;
    fn take_cancelled_status(&mut self) -> Result<Vec<Status>, String>;
}

impl Execution for Pool<'_> {
    fn submit(&mut self, jobs: Vec<Work>) -> Result<(), SubmitError> {
        Pool::submit(self, jobs)
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
    jobs: std::vec::IntoIter<Work>,
    status: Vec<Status>,
    next: usize,
    stop: AtomicBool,
}

impl Execution for Inline<'_> {
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
        self.jobs = jobs.into_iter();
        self.next = 0;
        Ok(())
    }

    fn poll(&mut self, _: Duration) -> Result<Poll, String> {
        if self.stop.load(Ordering::Acquire) {
            return Err("inline poll after cancellation".into());
        }
        let Some(job) = self.jobs.next() else {
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
            bytes,
        })
    }

    fn cancel(&mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        self.jobs = Vec::new().into_iter();
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
            jobs: Vec::new().into_iter(),
            status: Vec::new(),
            next: 0,
            stop: AtomicBool::new(false),
        };
        Ok(body(&mut inline))
    } else {
        with_authorized_pool(budget - 1, authorize, inspect, |pool| body(pool))
    }
}
