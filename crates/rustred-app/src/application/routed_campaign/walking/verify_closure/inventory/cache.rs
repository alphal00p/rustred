//! A derived raw census, not a checkpoint or an independent certificate.
//!
//! Every current record is still authenticated, decoded, and checked. Reuse
//! requires an identical immutable CP6 record-segment prefix and identical
//! classifier inputs. A cache is emitted only when that prefix published every
//! domain ID exactly once. Therefore publication order need not be ID order:
//! its immutable frames nevertheless contain exactly IDs [0, domains).
use super::super::{Kind, Loaded, checkpoint};
use super::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::Ordering;

// This versions both the codec AND classification implementation. Bump it if
// matcher/census semantics change without a walk-semantics version bump.
pub(super) const SCHEMA: &str = "rustred.walk-inventory-census.v1";
pub(super) const FINITE_SCHEMA: &str = "rustred.walk-inventory-census.finite.v1";
const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Segment {
    generation: u64,
    first: u64,
    count: u64,
    bytes: u64,
    file: String,
    blake3: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Identity {
    schema: String,
    // This is deliberately stricter than a hand-picked list of matcher knobs.
    classifier: String,
    family: String,
    physical_arity: usize,
    owners: Vec<String>,
    scope_extensions: Vec<String>,
    generation: u64,
    domains: usize,
    segments: Vec<Segment>,
    pub counts: Counts,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Counts {
    pub apply: usize,
    pub route: usize,
    pub full_cover: usize,
}

#[derive(Serialize, Deserialize)]
struct Payload {
    identity: Identity,
    rules: Vec<(RuleIdentity, u64)>,
    terminals: Vec<Vec<i64>>,
    selected_rule_events: u64,
    terminal_events: u64,
    zero_events: u64,
}

/// A faithful completed finite census is retained for provenance, not accepted
/// by the older symbolic-prefix reuse reader. No coefficient strings occur.
#[derive(Serialize, Deserialize)]
struct FinitePayload {
    identity: Identity,
    symbolic_rules: Vec<(RuleIdentity, u64)>,
    terminals: Vec<Vec<i64>>,
    symbolic_selected_rule_events: u64,
    symbolic_terminal_events: u64,
    symbolic_zero_events: u64,
    finite: finite::State,
}

/// Conservative bincode variable-integer/string bound, checked BEFORE cloning
/// the retained census into its transport shape. Every integer needs at most
/// ten bytes; no coefficient/algebra object is serialized by this codec.
fn finite_payload_bound(
    identity: &Identity,
    rules: &BTreeMap<RuleIdentity, u64>,
    terminals: &BTreeSet<IntegralKey>,
    finite: &finite::State,
) -> Result<usize, AppError> {
    let mut bytes = 256usize; // fixed fields, lengths and scalar counters
    let mut add = |value: usize| {
        bytes = bytes
            .checked_add(value)
            .ok_or_else(|| AppError::limit("finite census binary bound overflow"))?;
        if bytes > MAX_BYTES {
            return Err(AppError::limit(
                "finite census exceeds native binary size limit",
            ));
        }
        Ok(())
    };
    for value in [&identity.schema, &identity.classifier, &identity.family] {
        add(value.len())?;
    }
    for value in identity.owners.iter().chain(&identity.scope_extensions) {
        add(value
            .len()
            .checked_add(10)
            .ok_or_else(|| AppError::limit("finite census string bound overflow"))?)?;
    }
    for segment in &identity.segments {
        add(segment
            .file
            .len()
            .checked_add(segment.blake3.len())
            .and_then(|n| n.checked_add(60))
            .ok_or_else(|| AppError::limit("finite census segment bound overflow"))?)?;
    }
    for (rule, count) in rules {
        if *count != 0 {
            add(rule
                .owner
                .len()
                .checked_add(40)
                .ok_or_else(|| AppError::limit("finite census rule bound overflow"))?)?;
        }
    }
    for key in terminals {
        add(key
            .powers()
            .len()
            .checked_mul(10)
            .and_then(|n| n.checked_add(10))
            .ok_or_else(|| AppError::limit("finite census terminal bound overflow"))?)?;
    }
    for selection in finite.selections.keys() {
        add(selection
            .owner
            .len()
            .checked_add(50)
            .ok_or_else(|| AppError::limit("finite census selection bound overflow"))?)?;
    }
    Ok(bytes)
}

pub(super) struct Reuse {
    pub domains: usize,
    pub counts: Counts,
    pub status: String,
    pub output: Option<Identity>,
}

fn classifier_binding(request: &OwnerDomainWalkRequest) -> String {
    // Query bounds/roles are freshly checked by scope::check. They select new
    // obligations, but cannot change classification of an identical domain.
    // Only authenticated append-only required-scope extensions are supported.
    let mut classifier = request.clone();
    classifier.matching.queries_json.clear();
    checkpoint::epoch_request_binding(&classifier)
}

fn census_binding(request: &OwnerDomainWalkRequest, schema: &str) -> String {
    if schema == FINITE_SCHEMA {
        // A whole finite recipe depends on its precise source region and roles,
        // unlike a reusable ordinary classification prefix. Bind the full request.
        checkpoint::epoch_request_binding(request)
    } else {
        classifier_binding(request)
    }
}

fn identity<const N: usize>(
    request: &OwnerDomainWalkRequest,
    loaded: &Loaded<N>,
    collector: &Collector,
) -> Option<Identity> {
    if !loaded.finite_replay.is_empty() {
        return None;
    }
    bound_identity(request, loaded, collector, SCHEMA)
}

fn bound_identity<const N: usize>(
    request: &OwnerDomainWalkRequest,
    loaded: &Loaded<N>,
    collector: &Collector,
    schema: &str,
) -> Option<Identity> {
    let manifest = &loaded.epoch.as_ref()?.manifest;
    if manifest["format"] != "RUSTRED-WALK-CP6"
        || manifest["schema"] != 3
        || loaded.raw.walk_semantics_version != 4
    {
        return None;
    }
    if request.amendments.len() != loaded.raw.amendments.len()
        || request.amendments.iter().any(|amendment| {
            serde_json::from_str::<Value>(&amendment.text)
                .ok()
                .is_none_or(|document| {
                    document["schema"] != super::super::super::rescue::SCOPE_EXTENSION_SCHEMA
                })
        })
    {
        return None;
    }
    let segments: Vec<Segment> =
        serde_json::from_value(manifest["record_segments"].clone()).ok()?;
    let state = collector.state.lock().expect("inventory census binding");
    let family = state.family.as_ref()?;
    Some(Identity {
        schema: schema.into(),
        classifier: census_binding(request, schema),
        family: family.fingerprint().into(),
        physical_arity: family.denominator_count(),
        owners: loaded.raw.owners.clone(),
        scope_extensions: loaded
            .raw
            .amendments
            .iter()
            .map(|a| a.digest.clone())
            .collect(),
        generation: loaded.raw.generation,
        domains: loaded.domains.len(),
        segments,
        counts: Counts::default(),
    })
}

impl Collector {
    pub(in crate::application::routed_campaign::walking::verify_closure) fn bind_finite_identity<
        const N: usize,
    >(
        &self,
        request: &OwnerDomainWalkRequest,
        loaded: &Loaded<N>,
    ) {
        if !loaded.finite_replay.is_empty() {
            *self.census_identity.lock().expect("finite census identity") =
                bound_identity(request, loaded, self, FINITE_SCHEMA);
        }
    }
}

fn fully_published<const N: usize>(loaded: &Loaded<N>, cancellation: &AtomicBool) -> bool {
    for (id, node) in loaded.nodes.iter().enumerate() {
        if id % 4096 == 0 && cancellation.load(Ordering::Relaxed) {
            return false;
        }
        if node.kind == Kind::Missing || node.abandoned || node.error {
            return false;
        }
        // Aliases and G2 full covers emit no Apply classifications. A failed
        // or interrupted actual inspection is never cached in this version.
        let full_cover =
            node.kind == Kind::G2 && loaded.g2.get(&id).is_some_and(|g| g.residual.is_none());
        if node.native() && !full_cover && !node.finished {
            return false;
        }
    }
    loaded
        .raw
        .records
        .iter()
        .try_fold(0usize, |sum, (_, count)| sum.checked_add(*count))
        == Some(loaded.domains.len())
}

pub(super) fn prepare<const N: usize>(
    request: &OwnerDomainWalkRequest,
    loaded: &Loaded<N>,
    collector: &Collector,
    cancellation: &AtomicBool,
) -> Reuse {
    let current = identity(request, loaded, collector);
    let output = current
        .clone()
        .filter(|_| fully_published(loaded, cancellation));
    let mut reuse = Reuse {
        domains: 0,
        counts: Counts::default(),
        status: "not_requested".into(),
        output,
    };
    let Some(source) = &collector.previous_census else {
        return reuse;
    };
    let result = (|| {
        let current = current.as_ref().ok_or("unsupported_current_scope")?;
        let payload = read(source)?;
        compatible(&payload.identity, current)?;
        let state = state(payload, collector)?;
        if cancellation.load(Ordering::Relaxed) {
            return Err("cancelled");
        }
        // All file, prefix, classifier, shape and resource checks precede this
        // single seed. A cache miss cannot leave a partly seeded collector.
        let mut target = collector.state.lock().expect("inventory census seed");
        target.rules = state.0.rules;
        target.terminals = state.0.terminals;
        target.selected_rule_events = state.0.selected_rule_events;
        target.terminal_events = state.0.terminal_events;
        target.zero_events = state.0.zero_events;
        reuse.domains = state.1.domains;
        reuse.counts = state.1.counts;
        Ok(())
    })();
    reuse.status = match result {
        Ok(()) => "reused_authenticated_record_prefix".into(),
        Err(reason) => format!("full_census:{reason}"),
    };
    reuse
}

fn compatible(old: &Identity, current: &Identity) -> Result<(), &'static str> {
    if old.schema != SCHEMA
        || old.classifier != current.classifier
        || old.family != current.family
        || old.physical_arity != current.physical_arity
        || old.owners != current.owners
        || !current.scope_extensions.starts_with(&old.scope_extensions)
    {
        return Err("classifier_binding_differs");
    }
    if old.generation == 0
        || old.generation > current.generation
        || old.domains > current.domains
        || !current.segments.starts_with(&old.segments)
        || old
            .segments
            .iter()
            .try_fold(0u64, |sum, part| sum.checked_add(part.count))
            != Some(old.domains as u64)
    {
        return Err("immutable_record_prefix_differs");
    }
    let classified = old
        .counts
        .apply
        .checked_add(old.counts.route)
        .and_then(|sum| sum.checked_add(old.counts.full_cover));
    if classified.is_none_or(|count| count > old.domains) {
        return Err("invalid_cached_counts");
    }
    Ok(())
}

fn read(source: &OwnerDomainWalkCensusSource) -> Result<Payload, &'static str> {
    if source.bytes > MAX_BYTES as u64 {
        return Err("cache_size_limit");
    }
    let expected =
        blake3::Hash::from_hex(&source.expected_blake3).map_err(|_| "invalid_cache_digest")?;
    let meta = std::fs::symlink_metadata(&source.path).map_err(|_| "cache_unavailable")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != source.bytes {
        return Err("cache_file_shape");
    }
    let mut file = File::open(&source.path).map_err(|_| "cache_unavailable")?;
    if file.metadata().map_err(|_| "cache_unavailable")?.len() != source.bytes {
        return Err("cache_file_shape");
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(source.bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cache_unreadable")?;
    if bytes.len() as u64 != source.bytes || blake3::hash(&bytes) != expected {
        return Err("cache_digest_differs");
    }
    let (payload, used) = bincode::serde::decode_from_slice(
        &bytes,
        bincode::config::standard().with_limit::<MAX_BYTES>(),
    )
    .map_err(|_| "cache_decode")?;
    if used != bytes.len() {
        return Err("cache_trailing_bytes");
    }
    Ok(payload)
}

fn state(payload: Payload, collector: &Collector) -> Result<(State, Identity), &'static str> {
    if payload.rules.len() > collector.max_unique_rules
        || payload.terminals.len() > collector.max_unique_terminals
    {
        return Err("cache_resource_limit");
    }
    let arity = payload.identity.physical_arity;
    let mut state = State::default();
    for (rule, count) in payload.rules {
        if rule.owner.len() != arity || count == 0 || state.rules.contains_key(&rule) {
            return Err("cache_rule_shape");
        }
        state.selected_rule_events = state
            .selected_rule_events
            .checked_add(count)
            .ok_or("cache_event_overflow")?;
        state.rules.insert(rule, count);
    }
    for powers in payload.terminals {
        if powers.len() != arity {
            return Err("cache_terminal_shape");
        }
        let key = IntegralKey::try_new(powers).map_err(|_| "cache_terminal_shape")?;
        if !state.terminals.insert(key) {
            return Err("cache_terminal_duplicate");
        }
    }
    if state.selected_rule_events != payload.selected_rule_events
        || payload.terminal_events < state.terminals.len() as u64
    {
        return Err("cache_event_counts");
    }
    state.terminal_events = payload.terminal_events;
    state.zero_events = payload.zero_events;
    Ok((state, payload.identity))
}

impl OwnerDomainWalkInventory {
    pub(crate) fn requires_finite_census_persistence(&self) -> bool {
        self.finite.is_some()
    }
    /// Persist a compact derived raw census only for a completed, safe CP6
    /// fast census. The receipt must be retained by the publishing artifact;
    /// the file is never accepted on the strength of a self-checksum alone.
    pub fn write_census(&self, path: impl AsRef<Path>) -> Result<Option<Value>, AppError> {
        if self.creator_pid != std::process::id()
            || !self.complete()
            || (self.finite.is_none() && self.verification["verdict"] != "TRUSTED_SAVED_SCOPE")
        {
            return Ok(None);
        }
        let Some(identity) = &self.census_identity else {
            return Ok(None);
        };
        if let Some(finite) = &self.finite {
            if identity.schema != FINITE_SCHEMA || finite.invocations == 0 {
                return Err(AppError::internal_invariant(
                    "finite census persistence lacks authenticated native replay identity",
                ));
            }
            finite_payload_bound(identity, &self.rules, &self.terminals, finite)?;
            let payload = FinitePayload {
                identity: identity.clone(),
                symbolic_rules: self
                    .rules
                    .iter()
                    .filter(|(_, count)| **count != 0)
                    .map(|(key, count)| (key.clone(), *count))
                    .collect(),
                terminals: self
                    .terminals
                    .iter()
                    .map(|key| key.powers().to_vec())
                    .collect(),
                symbolic_selected_rule_events: self.selected_rule_events,
                symbolic_terminal_events: self.terminal_events,
                symbolic_zero_events: self.zero_events,
                finite: finite.clone(),
            };
            let bytes = bincode::serde::encode_to_vec(&payload, bincode::config::standard())
                .map_err(|error| {
                    AppError::execution(format!("encode finite inventory census: {error}"))
                })?;
            if bytes.len() > MAX_BYTES {
                return Err(AppError::limit(
                    "finite census exceeds native binary size limit",
                ));
            }
            let digest = blake3::hash(&bytes).to_hex().to_string();
            crate::application::atomic_file::write_file_atomically(path.as_ref(), &bytes, true)
                .map_err(AppError::execution)?;
            return Ok(Some(
                json!({"schema":FINITE_SCHEMA,"blake3":digest,"bytes":bytes.len(),
                "required_for_finite_inventory":true,"prefix_reuse":false}),
            ));
        }
        let payload = Payload {
            identity: identity.clone(),
            rules: self
                .rules
                .iter()
                .map(|(key, count)| (key.clone(), *count))
                .collect(),
            terminals: self
                .terminals
                .iter()
                .map(|key| key.powers().to_vec())
                .collect(),
            selected_rule_events: self.selected_rule_events,
            terminal_events: self.terminal_events,
            zero_events: self.zero_events,
        };
        let bytes = bincode::serde::encode_to_vec(&payload, bincode::config::standard())
            .map_err(|e| AppError::execution(format!("encode inventory census: {e}")))?;
        if bytes.len() > MAX_BYTES {
            return Ok(None);
        }
        let digest = blake3::hash(&bytes).to_hex().to_string();
        crate::application::atomic_file::write_file_atomically(path.as_ref(), &bytes, true)
            .map_err(AppError::execution)?;
        Ok(Some(
            json!({"schema":SCHEMA,"blake3":digest,"bytes":bytes.len()}),
        ))
    }
}

#[cfg(test)]
mod tests;
