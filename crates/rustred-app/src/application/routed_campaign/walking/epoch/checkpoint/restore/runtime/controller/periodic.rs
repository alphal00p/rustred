//! Private fixed-interval adapter. No adaptive interval, closure refresh or
//! persistence schema: only successful publication restarts the interval.
use std::cell::Cell;
use std::io;
use std::time::{Duration, Instant};

pub(super) struct Schedule {
    interval: Option<Duration>,
    last: Cell<Instant>,
}

impl Schedule {
    pub fn new(interval: Option<Duration>) -> io::Result<Self> {
        Self::at(interval, Instant::now())
    }

    fn at(interval: Option<Duration>, now: Instant) -> io::Result<Self> {
        if interval == Some(Duration::ZERO) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "epoch periodic interval must be positive",
            ));
        }
        Ok(Self {
            interval,
            last: Cell::new(now),
        })
    }

    pub fn due(&self) -> bool {
        self.due_at(Instant::now())
    }

    fn due_at(&self, now: Instant) -> bool {
        self.interval
            .is_some_and(|interval| now.saturating_duration_since(self.last.get()) >= interval)
    }

    pub fn saved(&self) {
        self.saved_at(Instant::now());
    }

    fn saved_at(&self, now: Instant) {
        self.last.set(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_interval_is_monotonic_and_only_publication_resets_it() {
        let now = Instant::now();
        let schedule = Schedule::at(Some(Duration::from_secs(10)), now).unwrap();
        assert!(!schedule.due_at(now + Duration::from_secs(9)));
        assert!(schedule.due_at(now + Duration::from_secs(10)));
        // A due query does not acknowledge a failed or still-running save.
        assert_eq!(schedule.last.get(), now);
        assert!(schedule.due_at(now + Duration::from_secs(11)));
        schedule.saved_at(now + Duration::from_secs(12));
        assert!(!schedule.due_at(now + Duration::from_secs(21)));
        assert!(schedule.due_at(now + Duration::from_secs(22)));
        assert!(!schedule.due_at(now));
        assert!(
            !Schedule::at(None, now)
                .unwrap()
                .due_at(now + Duration::from_secs(100))
        );
        assert!(
            matches!(Schedule::at(Some(Duration::ZERO), now), Err(error) if error.kind() == io::ErrorKind::InvalidInput)
        );
    }
}
