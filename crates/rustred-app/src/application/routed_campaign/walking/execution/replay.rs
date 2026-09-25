//! Verified replay of only an unfinished physical stream's committed prefix.
//!
//! Runs of identical logical callbacks are hashed canonically, independently
//! of producer chunking. No native coefficient or algebra operation is added.
use super::super::inspection::{Effect, Event};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};

#[derive(Clone, Copy)]
pub(super) struct Token([u8; 32]);

struct HashWriter<'a>(&'a mut blake3::Hasher);
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Fixed-size digest of the complete logical callback descriptor. `count` is
/// deliberately separate: producer-side run compaction may change on replay.
pub(super) fn token<const N: usize>(event: &Event<N>) -> Result<Token, &'static str> {
    let mut hash = blake3::Hasher::new();
    hash.update(b"rustred-walk-event-v1\0");
    let powers = |p: rustred::solver::DomainPowerBounds| {
        (
            p.max_positive_power,
            p.min_power_difference,
            p.max_power_difference,
        )
    };
    let result = match &event.effect {
        Effect::Count => serde_json::to_writer(HashWriter(&mut hash), &(0u8,)),
        Effect::KnownReuse {
            successor,
            conditional,
        } => serde_json::to_writer(HashWriter(&mut hash), &(1u8, successor, conditional)),
        Effect::PreAdmittedOrthantReuse {
            target,
            successor,
            conditional,
        } => serde_json::to_writer(
            HashWriter(&mut hash),
            &(2u8, target, successor, conditional),
        ),
        Effect::Admit {
            domain,
            successor,
            conditional,
        } => serde_json::to_writer(
            HashWriter(&mut hash),
            &(
                3u8,
                matches!(domain.phase, super::super::queue::Phase::Route),
                &domain.owner[..],
                &domain.lower,
                &domain.upper,
                domain.rank,
                powers(domain.powers),
                successor,
                conditional,
            ),
        ),
        Effect::Frontier {
            value,
            successor,
            conditional,
        } => serde_json::to_writer(HashWriter(&mut hash), &(4u8, value, successor, conditional)),
        Effect::Optional(d) => serde_json::to_writer(
            HashWriter(&mut hash),
            &(
                5u8,
                format!("{:?}", d.disposition),
                d.rank,
                powers(d.powers),
                &d.lower,
                &d.upper,
                &d.shift,
                d.ordinal,
                d.resource,
                d.requested,
                d.limit,
            ),
        ),
    };
    result.map_err(|_| "checkpoint callback fingerprint encoding")?;
    Ok(Token(*hash.finalize().as_bytes()))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Progress {
    events: usize,
    digest: [u8; 32],
}

/// The serialized state describes canonical accepted work, never an in-flight
/// attempt. A second interruption during replay retains the original target.
pub(super) struct Replay {
    accepted: usize,
    replayed: usize,
    target: Option<Progress>,
    hash: blake3::Hasher,
    run: Option<([u8; 32], u64)>,
}

impl Serialize for Replay {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.snapshot().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Replay {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::restore(Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl Default for Replay {
    fn default() -> Self {
        let mut hash = blake3::Hasher::new();
        hash.update(b"rustred-walk-prefix-v1\0");
        Self {
            accepted: 0,
            replayed: 0,
            target: None,
            hash,
            run: None,
        }
    }
}

impl Replay {
    pub(super) fn restore(value: Value) -> Result<Self, String> {
        let target: Progress = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let mut replay = Self::default();
        if target.events == 0 {
            if target.digest != replay.digest() {
                return Err("checkpoint empty replay prefix digest mismatch".into());
            }
        } else {
            replay.accepted = target.events;
            replay.target = Some(target);
        }
        Ok(replay)
    }

    pub(super) fn snapshot(&self) -> Value {
        let progress = self.target.unwrap_or(Progress {
            events: self.accepted,
            digest: self.digest(),
        });
        serde_json::to_value(progress).expect("fixed integer checkpoint progress")
    }

    pub(super) fn accepted_events(&self) -> usize {
        self.accepted
    }

    /// Remove only an already committed prefix. A changed prefix fails before
    /// any suffix can reach queue admission or canonical accounting.
    pub(super) fn filter<const N: usize>(
        &mut self,
        event: &mut Event<N>,
    ) -> Result<bool, &'static str> {
        let Some(target) = self.target else {
            return Ok(event.count != 0);
        };
        if event.count == 0 {
            return Err("zero-length checkpoint replay callback");
        }
        let skipped = event.count.min(target.events - self.replayed);
        self.append(token(event)?, skipped)?;
        self.replayed += skipped;
        event.count -= skipped;
        if self.replayed == target.events {
            if self.digest() != target.digest {
                return Err("checkpoint replay prefix differs from committed callbacks");
            }
            self.target = None;
        }
        Ok(event.count != 0)
    }

    /// Call with the exact accepted count after canonical publication. The
    /// token must be captured before an Event is moved into prepared admission.
    pub(super) fn record_accepted(
        &mut self,
        token: Token,
        count: usize,
    ) -> Result<(), &'static str> {
        if self.target.is_some() {
            return Err("checkpoint suffix admitted before prefix verification");
        }
        let accepted = self
            .accepted
            .checked_add(count)
            .ok_or("checkpoint accepted callback count overflow")?;
        self.append(token, count)?;
        self.accepted = accepted;
        Ok(())
    }

    pub(super) fn finish(&self) -> Result<(), &'static str> {
        if self.target.is_some() {
            Err("checkpoint replay ended before committed prefix")
        } else {
            Ok(())
        }
    }

    fn append(&mut self, token: Token, count: usize) -> Result<(), &'static str> {
        if count == 0 {
            return Ok(());
        }
        let count = u64::try_from(count).map_err(|_| "checkpoint callback count overflow")?;
        if let Some((key, previous)) = &mut self.run {
            if *key == token.0 {
                *previous = previous
                    .checked_add(count)
                    .ok_or("checkpoint callback run overflow")?;
                return Ok(());
            }
        }
        if let Some((key, count)) = self.run.take() {
            self.hash.update(&key);
            self.hash.update(&count.to_le_bytes());
        }
        self.run = Some((token.0, count));
        Ok(())
    }

    fn digest(&self) -> [u8; 32] {
        let mut hash = self.hash.clone();
        if let Some((key, count)) = self.run {
            hash.update(&key);
            hash.update(&count.to_le_bytes());
        }
        *hash.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(n: usize) -> Event<1> {
        Event {
            count: n,
            effect: Effect::Count,
        }
    }
    fn reuse(n: usize, conditional: bool) -> Event<1> {
        Event {
            count: n,
            effect: Effect::KnownReuse {
                successor: true,
                conditional,
            },
        }
    }
    fn accept(replay: &mut Replay, event: &Event<1>) {
        replay
            .record_accepted(token(event).unwrap(), event.count)
            .unwrap();
    }

    #[test]
    fn prefix_is_independent_of_compaction_and_keeps_suffix_exactly_once() {
        let mut first = Replay::default();
        accept(&mut first, &count(2));
        accept(&mut first, &count(3));
        accept(&mut first, &reuse(4, false));
        let mut replay = Replay::restore(first.snapshot()).unwrap();
        assert!(!replay.filter(&mut count(5)).unwrap());
        assert!(!replay.filter(&mut reuse(1, false)).unwrap());
        let mut tail = reuse(7, false);
        assert!(replay.filter(&mut tail).unwrap());
        assert_eq!(tail.count, 4);
        accept(&mut replay, &tail);
        accept(&mut first, &reuse(4, false));
        assert_eq!(replay.snapshot(), first.snapshot());
        assert_eq!(replay.accepted_events(), 13);
        replay.finish().unwrap();
    }

    #[test]
    fn altered_prefix_and_early_finish_cannot_publish_suffix() {
        let mut first = Replay::default();
        accept(&mut first, &reuse(4, false));
        let saved = first.snapshot();
        let mut replay = Replay::restore(saved.clone()).unwrap();
        assert!(replay.filter(&mut reuse(5, true)).is_err());
        assert!(
            replay
                .record_accepted(token(&count(1)).unwrap(), 1)
                .is_err()
        );
        let mut short = Replay::restore(saved).unwrap();
        assert!(!short.filter(&mut reuse(3, false)).unwrap());
        assert!(short.finish().is_err());
    }

    #[test]
    fn interruption_during_replay_retains_original_checkpoint_authority() {
        let mut first = Replay::default();
        accept(&mut first, &count(1_000_000_000));
        let saved = first.snapshot();
        let mut replay = Replay::restore(saved.clone()).unwrap();
        assert!(!replay.filter(&mut count(7)).unwrap());
        assert_eq!(replay.snapshot(), saved);
        let mut restarted = Replay::restore(replay.snapshot()).unwrap();
        assert!(!restarted.filter(&mut count(1_000_000_000)).unwrap());
        restarted.finish().unwrap();
        assert_eq!(restarted.snapshot(), saved);
    }
}
