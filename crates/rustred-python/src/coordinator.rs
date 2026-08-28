use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, OnceLock, mpsc};

type Task = Box<dyn FnOnce() + Send + 'static>;

static COORDINATOR: OnceLock<Result<Coordinator, String>> = OnceLock::new();

/// The one process-wide entrance to RustRed/Symbolica from Python.
///
/// A zero-capacity channel applies backpressure before a caller can enqueue a
/// second potentially large request. The receiver is owned by exactly one
/// stable OS thread, so all top-level application calls enter Symbolica from
/// that thread even when many Python threads call concurrently.
pub(crate) struct Coordinator {
    tasks: SyncSender<Task>,
    poisoned: Arc<AtomicBool>,
    creator_pid: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoordinatorError {
    Poisoned,
    Forked { creator_pid: u32, current_pid: u32 },
    Panicked(String),
    Unavailable(String),
}

impl Coordinator {
    fn start() -> Result<Self, String> {
        let (tasks, receiver) = mpsc::sync_channel::<Task>(0);
        let poisoned = Arc::new(AtomicBool::new(false));
        std::thread::Builder::new()
            .name("rustred-python-coordinator".to_owned())
            .spawn(move || {
                while let Ok(task) = receiver.recv() {
                    task();
                }
            })
            .map_err(|error| format!("cannot start the RustRed Python coordinator: {error}"))?;
        Ok(Self {
            tasks,
            poisoned,
            creator_pid: std::process::id(),
        })
    }

    pub(crate) fn execute<T, F>(&self, operation: F) -> Result<T, CoordinatorError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let current_pid = std::process::id();
        if current_pid != self.creator_pid {
            return Err(CoordinatorError::Forked {
                creator_pid: self.creator_pid,
                current_pid,
            });
        }
        if self.poisoned.load(Ordering::Acquire) {
            return Err(CoordinatorError::Poisoned);
        }

        let (response, result) = mpsc::sync_channel(1);
        let poisoned = Arc::clone(&self.poisoned);
        let task = Box::new(move || {
            if poisoned.load(Ordering::Acquire) {
                let _ = response.send(Err(CoordinatorError::Poisoned));
                return;
            }
            match catch_unwind(AssertUnwindSafe(operation)) {
                Ok(value) => {
                    let _ = response.send(Ok(value));
                }
                Err(payload) => {
                    // Publish poison before waking the current caller. Any
                    // request already waiting at the rendezvous channel will
                    // observe it before executing application work.
                    poisoned.store(true, Ordering::Release);
                    let _ = response.send(Err(CoordinatorError::Panicked(panic_message(
                        payload.as_ref(),
                    ))));
                }
            }
        });
        if let Err(error) = self.tasks.send(task) {
            self.poisoned.store(true, Ordering::Release);
            return Err(CoordinatorError::Unavailable(format!(
                "the RustRed Python coordinator stopped accepting work: {error}"
            )));
        }
        result.recv().map_err(|error| {
            self.poisoned.store(true, Ordering::Release);
            CoordinatorError::Unavailable(format!(
                "the RustRed Python coordinator stopped before replying: {error}"
            ))
        })?
    }
}

pub(crate) fn process_coordinator() -> Result<&'static Coordinator, String> {
    COORDINATOR
        .get_or_init(Coordinator::start)
        .as_ref()
        .map_err(Clone::clone)
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else {
        "non-string Rust panic payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;

    use super::*;

    #[test]
    fn one_thread_executes_work_from_concurrent_callers() {
        let coordinator = Arc::new(Coordinator::start().expect("start coordinator"));
        let barrier = Arc::new(Barrier::new(9));
        let callers: Vec<_> = (0..8)
            .map(|_| {
                let coordinator = Arc::clone(&coordinator);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    coordinator
                        .execute(|| std::thread::current().id())
                        .expect("coordinator result")
                })
            })
            .collect();
        barrier.wait();
        let worker_threads: HashSet<_> = callers
            .into_iter()
            .map(|caller| caller.join().expect("caller thread"))
            .collect();
        assert_eq!(worker_threads.len(), 1);
    }

    #[test]
    fn concurrent_tasks_never_overlap() {
        let coordinator = Arc::new(Coordinator::start().expect("start coordinator"));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let callers: Vec<_> = (0..8)
            .map(|_| {
                let coordinator = Arc::clone(&coordinator);
                let active = Arc::clone(&active);
                let maximum = Arc::clone(&maximum);
                std::thread::spawn(move || {
                    coordinator
                        .execute(move || {
                            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                            maximum.fetch_max(now, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(2));
                            active.fetch_sub(1, Ordering::SeqCst);
                        })
                        .expect("serialized task");
                })
            })
            .collect();
        for caller in callers {
            caller.join().expect("caller thread");
        }
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_panic_poison_is_permanent() {
        let coordinator = Coordinator::start().expect("start coordinator");
        assert_eq!(coordinator.execute(|| 7), Ok(7));
        let failure = coordinator.execute(|| panic!("deliberate coordinator test panic"));
        assert_eq!(
            failure,
            Err(CoordinatorError::Panicked(
                "deliberate coordinator test panic".to_owned()
            ))
        );
        assert_eq!(coordinator.execute(|| 11), Err(CoordinatorError::Poisoned));
    }

    #[test]
    fn a_request_waiting_behind_a_panic_observes_poison() {
        let coordinator = Arc::new(Coordinator::start().expect("start coordinator"));
        let (entered, entered_receiver) = mpsc::sync_channel(0);
        let (release, release_receiver) = mpsc::sync_channel(0);

        let panic_coordinator = Arc::clone(&coordinator);
        let panicking = std::thread::spawn(move || {
            panic_coordinator.execute(move || {
                entered.send(()).expect("announce active panic task");
                release_receiver.recv().expect("release panic task");
                panic!("queued poison test");
            })
        });
        entered_receiver.recv().expect("panic task entered");

        let queued_coordinator = Arc::clone(&coordinator);
        let queued = std::thread::spawn(move || queued_coordinator.execute(|| 17));
        release.send(()).expect("release panic task");

        assert!(matches!(
            panicking.join().expect("panic caller"),
            Err(CoordinatorError::Panicked(message)) if message == "queued poison test"
        ));
        assert_eq!(
            queued.join().expect("queued caller"),
            Err(CoordinatorError::Poisoned)
        );
        assert_eq!(coordinator.execute(|| 19), Err(CoordinatorError::Poisoned));
    }

    #[test]
    fn a_post_fork_pid_mismatch_fails_before_channel_use() {
        let mut coordinator = Coordinator::start().expect("start coordinator");
        coordinator.creator_pid = coordinator.creator_pid.wrapping_add(1);
        assert!(matches!(
            coordinator.execute(|| 23),
            Err(CoordinatorError::Forked { .. })
        ));
    }

    #[test]
    fn coordinator_and_application_payloads_are_thread_safe() {
        fn assert_send<T: Send>() {}
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<Coordinator>();
        assert_send::<rustred_app::DeriveRequest>();
        assert_send::<rustred_app::DeriveResult>();
        assert_send::<rustred_app::CampaignPlanRequest>();
        assert_send::<rustred_app::CampaignPlanResult>();
        assert_send::<rustred_app::CampaignPreflightRequest>();
        assert_send::<rustred_app::CampaignPreflightResult>();
        assert_send::<rustred_app::AppError>();
    }
}
