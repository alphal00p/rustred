//! Optional operational stop-file context; never mathematical authority.
//! Cancellation stays authoritative even when this bounded metadata is absent,
//! malformed or unreadable. Tree attribution is not individual job blame.
use super::super::merge::StopReason;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Metadata {
    Parsed,
    NoContext,
    Unavailable,
    Malformed,
    Oversized,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Stop {
    pub metadata: Metadata,
    pub reason: Option<String>,
    /// Explicit only; unknown must not be inferred as false host pressure.
    pub host_wide: Option<bool>,
    pub own_memory_signal: Option<bool>,
    pub tree_attributed: bool,
}

#[derive(Deserialize)]
struct Document {
    reason: String,
    ram_guard: Option<RamGuard>,
}

#[derive(Deserialize)]
struct RamGuard {
    host_wide: Option<bool>,
    own_memory_signal: Option<bool>,
}

impl Stop {
    fn unknown(metadata: Metadata) -> Self {
        Self {
            metadata,
            reason: None,
            host_wide: None,
            own_memory_signal: None,
            tree_attributed: false,
        }
    }

    fn parse(bytes: &[u8]) -> Self {
        if bytes.len() > MAX_BYTES {
            return Self::unknown(Metadata::Oversized);
        }
        let Ok(document) = serde_json::from_slice::<Document>(bytes) else {
            return Self::unknown(Metadata::Malformed);
        };
        let own_reason = matches!(
            document.reason.as_str(),
            "aggregate_rss_soft_limit" | "own_swap_growth_sustained"
        );
        let ram = own_reason || document.reason == "host_memory_reserve";
        let host_wide = document
            .ram_guard
            .as_ref()
            .and_then(|guard| guard.host_wide);
        let own_memory_signal = document.ram_guard.and_then(|guard| guard.own_memory_signal);
        Self {
            metadata: Metadata::Parsed,
            reason: Some(document.reason),
            host_wide,
            own_memory_signal,
            tree_attributed: ram && (own_reason || own_memory_signal == Some(true)),
        }
    }

    pub fn kind(&self) -> StopReason {
        if self.metadata == Metadata::Parsed
            && self.reason.as_deref().is_some_and(|reason| {
                matches!(
                    reason,
                    "aggregate_rss_soft_limit"
                        | "own_swap_growth_sustained"
                        | "host_memory_reserve"
                )
            })
        {
            StopReason::RamGuard
        } else {
            StopReason::Paused
        }
    }

    /// Saved diagnostics cannot silently change unknown context into memory
    /// blame. This is still only tree-level context, never a guard increment.
    pub fn valid(&self) -> bool {
        if self.metadata != Metadata::Parsed {
            return self.reason.is_none()
                && self.host_wide.is_none()
                && self.own_memory_signal.is_none()
                && !self.tree_attributed;
        }
        let Some(reason) = &self.reason else {
            return false;
        };
        let own = matches!(
            reason.as_str(),
            "aggregate_rss_soft_limit" | "own_swap_growth_sustained"
        );
        reason.len() <= MAX_BYTES
            && self.tree_attributed
                == (self.kind() == StopReason::RamGuard
                    && (own || self.own_memory_signal == Some(true)))
    }
}

/// Read only after the existing cancellation flag is set. No path or a
/// missing/malformed file cannot distinguish signal/operator/unknown causes.
/// The caller must report that lack of context rather than invent RAM blame.
pub(super) fn requested(cancel: &AtomicBool, path: Option<&Path>) -> Option<Stop> {
    if !cancel.load(Ordering::Acquire) {
        return None;
    }
    let Some(path) = path else {
        return Some(Stop::unknown(Metadata::NoContext));
    };
    let read = || -> std::io::Result<Stop> {
        // Refuse nonregular operational inputs (in particular FIFOs) before
        // opening. The stop file is supervisor-owned, not checkpoint authority.
        if !std::fs::symlink_metadata(path)?.is_file() {
            return Ok(Stop::unknown(Metadata::Unavailable));
        }
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Ok(Stop::unknown(Metadata::Unavailable));
        }
        if metadata.len() > MAX_BYTES as u64 {
            return Ok(Stop::unknown(Metadata::Oversized));
        }
        let mut bytes = [0; MAX_BYTES + 1];
        let mut input = file.take(bytes.len() as u64);
        let mut count = 0;
        loop {
            let read = input.read(&mut bytes[count..])?;
            if read == 0 {
                break;
            }
            count += read;
        }
        Ok(Stop::parse(&bytes[..count]))
    };
    Some(read().unwrap_or_else(|_| Stop::unknown(Metadata::Unavailable)))
}

#[cfg(test)]
mod tests {
    use super::super::tests::Directory;
    use super::*;

    #[test]
    fn manual_empty_signal_missing_and_malformed_always_preserve_cancellation() {
        let directory = Directory::new();
        let path = directory.0.join("stop.json");
        let flag = AtomicBool::new(false);
        assert!(requested(&flag, Some(&path)).is_none());
        flag.store(true, Ordering::Release); // Same application flag for SIGINT and API cancellation.
        assert_eq!(
            requested(&flag, None).unwrap().metadata,
            Metadata::NoContext
        );
        assert_eq!(
            requested(&flag, Some(&path)).unwrap().metadata,
            Metadata::Unavailable
        );
        for bytes in [
            b"".as_slice(),
            b"{",
            b"{\"reason\":7}",
            b"{\"reason\":\"host_memory_reserve\",\"ram_guard\":{\"own_memory_signal\":\"yes\"}}",
        ] {
            std::fs::write(&path, bytes).unwrap();
            let stop = requested(&flag, Some(&path)).unwrap();
            assert_eq!(stop.metadata, Metadata::Malformed);
            assert_eq!(stop.kind(), StopReason::Paused);
            assert!(!stop.tree_attributed);
        }
        std::fs::write(&path, vec![b' '; MAX_BYTES + 1]).unwrap();
        let stop = requested(&flag, Some(&path)).unwrap();
        assert_eq!(stop.metadata, Metadata::Oversized);
        assert_eq!(stop.kind(), StopReason::Paused);
    }

    #[test]
    fn ram_guard_stop_reason_parsed_without_promoting_tree_to_job_blame() {
        for (reason, extra, kind, attributed) in [
            ("aggregate_rss_soft_limit", "", StopReason::RamGuard, true),
            ("own_swap_growth_sustained", "", StopReason::RamGuard, true),
            ("host_memory_reserve", "", StopReason::RamGuard, false),
            (
                "host_memory_reserve",
                ",\"ram_guard\":{\"host_wide\":true,\"own_memory_signal\":false}",
                StopReason::RamGuard,
                false,
            ),
            (
                "host_memory_reserve",
                ",\"ram_guard\":{\"host_wide\":true,\"own_memory_signal\":true}",
                StopReason::RamGuard,
                true,
            ),
            (
                "operator",
                ",\"ram_guard\":{\"own_memory_signal\":true}",
                StopReason::Paused,
                false,
            ),
            ("unknown_reason", "", StopReason::Paused, false),
        ] {
            let bytes = format!(
                "{{\"reason\":\"{reason}\",\"unix_time\":1,\"family_closure_claim\":false{extra}}}"
            );
            let stop = Stop::parse(bytes.as_bytes());
            assert_eq!(stop.metadata, Metadata::Parsed);
            assert_eq!(stop.reason.as_deref(), Some(reason));
            assert_eq!(stop.kind(), kind);
            assert_eq!(stop.tree_attributed, attributed);
            assert!(stop.valid());
        }
    }

    #[test]
    fn operational_path_does_not_change_mathematical_request_binding() {
        use crate::application::routed_campaign::walking::checkpoint::epoch_request_binding;
        use crate::{OwnerDomainMatchRequest, OwnerDomainWalkRequest};
        let mut request =
            OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("{}".into(), "{}".into()));
        let before = epoch_request_binding(&request);
        request.epoch_stop_file = Some("operational-stop.json".into());
        assert_eq!(epoch_request_binding(&request), before);
    }

    #[test]
    fn saved_unknown_context_cannot_claim_tree_attribution() {
        let mut unknown = Stop::unknown(Metadata::NoContext);
        assert!(unknown.valid());
        unknown.tree_attributed = true;
        assert!(!unknown.valid());
        let mut host = Stop::parse(br#"{"reason":"host_memory_reserve"}"#);
        host.tree_attributed = true;
        assert!(!host.valid());
    }
}
