//! Exact bounded windows using native counts and composition iteration.
//!
//! A proposed successor cursor is enumeration metadata, not reduction closure.
//! The backend must commit it atomically with a successful closed-window proof.
//! Untrusted saved cursors can be audited by replaying their transition records;
//! no self-hash or count-only position grants authority to skip a window.
use crate::{AppError, AppErrorKind, EntryPowerBudget, FiniteEntryDomain};
use rustred::family::IntegralKey;
use rustred::solver::{DomainPowerSummary, FiniteRootAdmission, RootRegionInput};
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};
use symbolica::domains::integer::Integer;

#[cfg(test)]
mod tests;
mod trusted_restore;

const VERSION: u32 = 1;
const MAX_RECORD_BYTES: usize = 64 * 1024;

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    bincode::Encode,
    bincode::Decode,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(deny_unknown_fields)]
pub(crate) struct WindowLimits {
    pub max_seed_points: usize,
    /// Vec/key/coordinate payload, not allocator overhead or process RSS.
    pub max_seed_bytes: usize,
    pub max_positive_layers: usize,
    /// Native prefix examinations in one proposed transition, not a scope cap.
    pub max_partition_checks: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ErrorKind {
    Input,
    UnsupportedGeometry,
    Resource,
    Cancelled,
    Cursor,
    Native,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Error {
    pub kind: ErrorKind,
    detail: String,
}
impl Error {
    fn new(kind: ErrorKind, detail: impl ToString) -> Self {
        Self {
            kind,
            detail: detail.to_string(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}
impl std::error::Error for Error {}
impl From<AppError> for Error {
    fn from(error: AppError) -> Self {
        Self::new(
            match error.kind() {
                AppErrorKind::Limit | AppErrorKind::OutputLimit => ErrorKind::Resource,
                AppErrorKind::Cancelled => ErrorKind::Cancelled,
                _ => ErrorKind::Native,
            },
            error,
        )
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct Frame {
    lower: u64,
    upper: u64,
    current: u64,
    parent_count: Integer,
    completed_children: Integer,
}

#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
struct CursorRecord {
    version: u32,
    binding: [u8; 32],
    prefix: Vec<u64>,
    frames: Vec<Frame>,
    emitted: Integer,
    windows: u64,
    finished: bool,
}

/// Only a source's initial state or checked native transitions create this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrefixCursor(CursorRecord);
impl PrefixCursor {
    pub fn is_exhausted(&self) -> bool {
        self.0.finished
    }
    pub fn emitted(&self) -> &Integer {
        &self.0.emitted
    }
    pub fn windows(&self) -> u64 {
        self.0.windows
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        encode(&self.0)
    }
}

/// Exact one-window enumeration transition; carries no reduction authority.
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(crate) struct WindowTransition {
    version: u32,
    before: [u8; 32],
    prefix: Vec<u64>,
    count: Integer,
    after: CursorRecord,
}
impl WindowTransition {
    pub fn encode(&self) -> Result<Vec<u8>> {
        encode(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode(bytes)
    }
}

pub(crate) struct FiniteWindow<const N: usize> {
    targets: Vec<IntegralKey>,
    region: RootRegionInput<N>,
    transition: WindowTransition,
    successor: PrefixCursor,
}
impl<const N: usize> FiniteWindow<N> {
    pub fn targets(&self) -> &[IntegralKey] {
        &self.targets
    }
    pub fn region(&self) -> &RootRegionInput<N> {
        &self.region
    }
    pub fn count(&self) -> &Integer {
        &self.transition.count
    }
    pub fn transition(&self) -> &WindowTransition {
        &self.transition
    }
    /// Adopt only after the caller's complete native closure/atomic commit.
    pub fn successor(&self) -> &PrefixCursor {
        &self.successor
    }
}

pub(crate) struct PrefixWindows<const N: usize> {
    region: RootRegionInput<N>,
    arity: usize,
    limits: WindowLimits,
    singleton: bool,
    total: Integer,
    binding: [u8; 32],
    original_admission: Option<FiniteRootAdmission<N>>,
}

struct Node<const N: usize> {
    region: RootRegionInput<N>,
    summary: DomainPowerSummary<N>,
    envelope: Option<FiniteEntryDomain>,
    count: Integer,
}
struct Plan<const N: usize> {
    node: Node<N>,
    transition: WindowTransition,
}

fn encode(value: &impl bincode::Encode) -> Result<Vec<u8>> {
    let bytes = bincode::encode_to_vec(value, bincode::config::standard())
        .map_err(|error| Error::new(ErrorKind::Cursor, error))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::new(ErrorKind::Resource, "cursor record bytes"));
    }
    Ok(bytes)
}
fn decode<T: bincode::Decode<()>>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::new(ErrorKind::Resource, "cursor record bytes"));
    }
    let (value, used) = bincode::decode_from_slice(
        bytes,
        bincode::config::standard().with_limit::<MAX_RECORD_BYTES>(),
    )
    .map_err(|error| Error::new(ErrorKind::Cursor, error))?;
    if used != bytes.len() {
        return Err(Error::new(ErrorKind::Cursor, "trailing cursor bytes"));
    }
    Ok(value)
}
fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(Error::new(
            ErrorKind::Cancelled,
            "prefix enumeration cancelled",
        ))
    } else {
        Ok(())
    }
}
fn summary<const N: usize>(region: &RootRegionInput<N>) -> Result<DomainPowerSummary<N>> {
    DomainPowerSummary::try_new(
        region.support,
        &region.lower,
        &region.upper,
        region.rank,
        region.powers,
    )
    .map_err(|error| Error::new(ErrorKind::Input, error))
}

impl<const N: usize> PrefixWindows<N> {
    pub fn try_new(
        origin: &str,
        region: RootRegionInput<N>,
        physical_arity: usize,
        limits: WindowLimits,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        check(cancel)?;
        if origin.len() > MAX_RECORD_BYTES {
            return Err(Error::new(ErrorKind::Resource, "origin binding bytes"));
        }
        if !rustred::fits_storage(physical_arity, N)
            || physical_arity == 0
            || region.lower.len() != N
            || region.upper.len() != N
            || (physical_arity..N).any(|axis| {
                region.support[axis] || region.lower[axis] != 0 || region.upper[axis] != Some(0)
            })
        {
            return Err(Error::new(ErrorKind::Input, "prefix source/storage arity"));
        }
        let native = summary(&region)?;
        let singleton =
            (0..physical_arity).all(|axis| region.upper[axis] == Some(region.lower[axis]));
        if !native.is_empty() && !singleton {
            if region.lower[..physical_arity]
                .iter()
                .any(|&value| value != 0)
            {
                return Err(Error::new(
                    ErrorKind::UnsupportedGeometry,
                    "nonzero lower bound is not a full envelope",
                ));
            }
            let mut full = region.clone();
            full.upper[..physical_arity].fill(None);
            let full = summary(&full)?;
            if !native.contains(&full) || !full.contains(&native) {
                return Err(Error::new(
                    ErrorKind::UnsupportedGeometry,
                    "coordinate box differs from whole envelope",
                ));
            }
        }
        let binding = Self::source_binding(origin, &region, physical_arity, limits)?;
        let mut source = Self {
            region,
            arity: physical_arity,
            limits,
            singleton,
            total: Integer::from(0),
            binding,
            original_admission: None,
        };
        if !native.is_empty() {
            source.original_admission = Some(
                FiniteRootAdmission::try_new([source.region.clone()], 1)
                    .map_err(|error| Error::new(ErrorKind::Input, error))?,
            );
            let prefix = if singleton {
                source.region.lower[..physical_arity].to_vec()
            } else {
                Vec::new()
            };
            source.total = source.node(&prefix, cancel)?.count;
            if source.total <= 0 {
                return Err(Error::new(
                    ErrorKind::Native,
                    "nonempty source has zero count",
                ));
            }
        }
        check(cancel)?;
        Ok(source)
    }

    /// Metadata commitment only; lets portable readers reject contradictory
    /// source/window metadata before performing any native count operation.
    pub(crate) fn source_binding(
        origin: &str,
        region: &RootRegionInput<N>,
        physical_arity: usize,
        limits: WindowLimits,
    ) -> Result<[u8; 32]> {
        let binding_bytes = bincode::encode_to_vec(
            (
                VERSION,
                origin,
                physical_arity,
                region.support.to_vec(),
                &region.lower,
                &region.upper,
                region.rank,
                (
                    region.powers.max_positive_power,
                    region.powers.min_power_difference,
                    region.powers.max_power_difference,
                ),
                limits,
            ),
            bincode::config::standard(),
        )
        .map_err(|error| Error::new(ErrorKind::Input, error))?;
        Ok(*blake3::hash(&binding_bytes).as_bytes())
    }

    pub fn total_count(&self) -> &Integer {
        &self.total
    }
    pub fn initial_cursor(&self) -> PrefixCursor {
        PrefixCursor(CursorRecord {
            version: VERSION,
            binding: self.binding,
            prefix: if self.singleton && self.total != 0 {
                self.region.lower[..self.arity].to_vec()
            } else {
                Vec::new()
            },
            frames: Vec::new(),
            emitted: Integer::from(0),
            windows: 0,
            finished: self.total == 0,
        })
    }

    fn node(&self, prefix: &[u64], cancel: &AtomicBool) -> Result<Node<N>> {
        check(cancel)?;
        if prefix.len() > self.arity {
            return Err(Error::new(ErrorKind::Cursor, "prefix arity"));
        }
        let mut region = self.region.clone();
        for (axis, &value) in prefix.iter().enumerate() {
            if value < self.region.lower[axis]
                || self.region.upper[axis].is_some_and(|upper| value > upper)
            {
                return Err(Error::new(
                    ErrorKind::Cursor,
                    "prefix outside original coordinate bounds",
                ));
            }
            region.lower[axis] = value;
            region.upper[axis] = Some(value);
        }
        let native = summary(&region)?;
        if native.is_empty() {
            return Ok(Node {
                region,
                summary: native,
                envelope: None,
                count: Integer::from(0),
            });
        }
        if prefix.len() == self.arity {
            return Ok(Node {
                region,
                summary: native,
                envelope: None,
                count: Integer::from(1),
            });
        }
        let mut full = region.clone();
        full.upper[prefix.len()..self.arity].fill(None);
        let full = summary(&full)?;
        if !native.contains(&full) || !full.contains(&native) {
            return Err(Error::new(
                ErrorKind::UnsupportedGeometry,
                "conditioned coordinate box differs from residual envelope",
            ));
        }
        let (mut a, mut r) = (0i128, 0i128);
        for (axis, &value) in prefix.iter().enumerate() {
            let contribution = i128::from(value);
            if self.region.support[axis] {
                a = a
                    .checked_add(contribution + 1)
                    .ok_or_else(|| Error::new(ErrorKind::Resource, "fixed A overflow"))?;
            } else {
                r = r
                    .checked_add(contribution)
                    .ok_or_else(|| Error::new(ErrorKind::Resource, "fixed R overflow"))?;
            }
        }
        let bounds = residual(
            self.region.powers.max_positive_power,
            self.region.rank,
            self.region.powers.min_power_difference,
            self.region.powers.max_power_difference,
            a,
            r,
        )?;
        let Some(bounds) = bounds else {
            return Err(Error::new(
                ErrorKind::Native,
                "negative residual budget disagrees with native nonempty summary",
            ));
        };
        let envelope = FiniteEntryDomain::new(
            self.region.support[prefix.len()..self.arity].to_vec(),
            bounds,
        )?;
        let count = envelope.target_count(self.limits.max_positive_layers)?;
        check(cancel)?;
        if count <= 0 {
            return Err(Error::new(
                ErrorKind::Native,
                "residual count disagrees with native summary",
            ));
        }
        Ok(Node {
            region,
            summary: native,
            envelope: Some(envelope),
            count,
        })
    }

    fn seed_capacity(&self) -> Result<usize> {
        let per_key = std::mem::size_of::<IntegralKey>()
            .checked_add(
                self.arity
                    .checked_mul(std::mem::size_of::<i64>())
                    .ok_or_else(|| Error::new(ErrorKind::Resource, "seed byte overflow"))?,
            )
            .ok_or_else(|| Error::new(ErrorKind::Resource, "seed byte overflow"))?;
        let bytes = self
            .limits
            .max_seed_bytes
            .saturating_sub(std::mem::size_of::<Vec<IntegralKey>>());
        let capacity = self.limits.max_seed_points.min(bytes / per_key);
        if capacity == 0 {
            return Err(Error::new(
                ErrorKind::Resource,
                "no complete nonempty window fits the seed grant",
            ));
        }
        Ok(capacity)
    }

    fn seed_bytes(&self, capacity: usize) -> Result<usize> {
        capacity
            .checked_mul(
                std::mem::size_of::<IntegralKey>()
                    .checked_add(
                        self.arity
                            .checked_mul(std::mem::size_of::<i64>())
                            .ok_or_else(|| {
                                Error::new(ErrorKind::Resource, "seed coordinate byte overflow")
                            })?,
                    )
                    .ok_or_else(|| Error::new(ErrorKind::Resource, "seed key byte overflow"))?,
            )
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<IntegralKey>>()))
            .ok_or_else(|| Error::new(ErrorKind::Resource, "seed buffer byte overflow"))
    }

    fn charge(&self, checks: &mut usize, cancel: &AtomicBool) -> Result<()> {
        check(cancel)?;
        *checks = checks
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Resource, "partition check overflow"))?;
        if *checks > self.limits.max_partition_checks {
            return Err(Error::new(ErrorKind::Resource, "partition check grant"));
        }
        Ok(())
    }

    /// Advance a completely enumerated subtree; native child counts telescope
    /// to the parent's count. Only one sibling counter is retained per depth.
    fn advance(&self, cursor: &mut CursorRecord, mut count: Integer) -> Result<()> {
        loop {
            let Some(frame) = cursor.frames.last_mut() else {
                if count != self.total {
                    return Err(Error::new(ErrorKind::Cursor, "root count conservation"));
                }
                cursor.prefix.clear();
                cursor.finished = true;
                return Ok(());
            };
            frame.completed_children += &count;
            if frame.completed_children > frame.parent_count {
                return Err(Error::new(ErrorKind::Cursor, "child count exceeds parent"));
            }
            if frame.current < frame.upper {
                frame.current = frame
                    .current
                    .checked_add(1)
                    .ok_or_else(|| Error::new(ErrorKind::Cursor, "sibling overflow"))?;
                *cursor
                    .prefix
                    .last_mut()
                    .ok_or_else(|| Error::new(ErrorKind::Cursor, "missing prefix"))? =
                    frame.current;
                return Ok(());
            }
            if frame.completed_children != frame.parent_count {
                return Err(Error::new(
                    ErrorKind::Cursor,
                    "native child counts do not conserve parent",
                ));
            }
            count = frame.parent_count.clone();
            cursor.frames.pop();
            cursor.prefix.pop();
        }
    }

    fn seek(
        &self,
        cursor: &mut CursorRecord,
        checks: &mut usize,
        cancel: &AtomicBool,
    ) -> Result<Option<Node<N>>> {
        let capacity = if cursor.finished {
            0
        } else {
            self.seed_capacity()?
        };
        while !cursor.finished {
            self.charge(checks, cancel)?;
            let node = self.node(&cursor.prefix, cancel)?;
            if node.count == 0 {
                self.advance(cursor, node.count)?;
                continue;
            }
            if node.count <= Integer::from(capacity) {
                return Ok(Some(node));
            }
            let axis = cursor.prefix.len();
            if axis >= self.arity {
                return Err(Error::new(
                    ErrorKind::Resource,
                    "singleton does not fit seed grant",
                ));
            }
            let extrema = node
                .summary
                .extrema()
                .ok_or_else(|| Error::new(ErrorKind::Native, "missing nonempty extrema"))?;
            let lower = extrema.lower()[axis];
            let upper = extrema.upper()[axis].ok_or_else(|| {
                Error::new(
                    ErrorKind::UnsupportedGeometry,
                    "unbounded prefix coordinate",
                )
            })?;
            cursor.frames.push(Frame {
                lower,
                upper,
                current: lower,
                parent_count: node.count,
                completed_children: Integer::from(0),
            });
            cursor.prefix.push(lower);
        }
        Ok(None)
    }

    fn validate_live(&self, cursor: &PrefixCursor) -> Result<()> {
        let record = &cursor.0;
        if record.version != VERSION
            || record.binding != self.binding
            || record.prefix.len() > self.arity
            || record.frames.len() > self.arity
            || record.emitted < 0
            || record.emitted > self.total
        {
            return Err(Error::new(
                ErrorKind::Cursor,
                "cursor version/binding/range",
            ));
        }
        if record.finished
            && (!record.frames.is_empty()
                || !record.prefix.is_empty()
                || record.emitted != self.total)
        {
            return Err(Error::new(
                ErrorKind::Cursor,
                "exhausted cursor shape/count",
            ));
        }
        if !record.finished && !self.singleton && record.frames.len() != record.prefix.len() {
            return Err(Error::new(ErrorKind::Cursor, "cursor prefix/stack depth"));
        }
        Ok(())
    }

    fn plan(&self, before: &PrefixCursor, cancel: &AtomicBool) -> Result<Option<Plan<N>>> {
        self.validate_live(before)?;
        check(cancel)?;
        let mut current = before.0.clone();
        let mut checks = 0;
        let Some(node) = self.seek(&mut current, &mut checks, cancel)? else {
            return Ok(None);
        };
        let prefix = current.prefix.clone();
        let count = node.count.clone();
        current.emitted += &count;
        current.windows = current
            .windows
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Resource, "window ordinal overflow"))?;
        self.advance(&mut current, count.clone())?;
        // Normalize past empty branches and to the next bounded window. This
        // changes only the proposed state; a failed proposal leaves before intact.
        self.seek(&mut current, &mut checks, cancel)?;
        let after = PrefixCursor(current.clone());
        self.validate_live(&after)?;
        Ok(Some(Plan {
            node,
            transition: WindowTransition {
                version: VERSION,
                before: *blake3::hash(&before.encode()?).as_bytes(),
                prefix,
                count,
                after: current,
            },
        }))
    }

    pub fn next_window(
        &self,
        cursor: &PrefixCursor,
        cancel: &AtomicBool,
    ) -> Result<Option<FiniteWindow<N>>> {
        let Some(plan) = self.plan(cursor, cancel)? else {
            return Ok(None);
        };
        let expected = usize::try_from(plan.node.count.clone())
            .map_err(|error| Error::new(ErrorKind::Resource, error))?;
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(expected)
            .map_err(|error| Error::new(ErrorKind::Resource, error))?;
        if self.seed_bytes(targets.capacity())? > self.limits.max_seed_bytes {
            return Err(Error::new(
                ErrorKind::Resource,
                "reserved seed capacity exceeds byte grant",
            ));
        }
        let admission = FiniteRootAdmission::try_new([plan.node.region.clone()], 1)
            .map_err(|error| Error::new(ErrorKind::Native, error))?;
        let mut accept = |target: IntegralKey| -> Result<()> {
            check(cancel)?;
            if targets.len() >= expected {
                return Err(Error::new(
                    ErrorKind::Native,
                    "native iterator exceeded exact count",
                ));
            }
            self.original_admission
                .as_ref()
                .ok_or_else(|| Error::new(ErrorKind::Native, "nonempty window of empty source"))?
                .validate_entry(&target)
                .map_err(|error| Error::new(ErrorKind::Native, error))?;
            admission
                .validate_entry(&target)
                .map_err(|error| Error::new(ErrorKind::Native, error))?;
            targets.push(target);
            Ok(())
        };
        if let Some(envelope) = &plan.node.envelope {
            for key in envelope.targets() {
                check(cancel)?;
                accept(self.scatter(&plan.transition.prefix, key?.powers())?)?;
            }
        } else {
            accept(self.scatter(&plan.transition.prefix, &[])?)?;
        }
        check(cancel)?;
        if targets.len() != expected {
            return Err(Error::new(
                ErrorKind::Native,
                "native iterator did not exhaust exact count",
            ));
        }
        let successor = PrefixCursor(plan.transition.after.clone());
        Ok(Some(FiniteWindow {
            targets,
            region: plan.node.region,
            transition: plan.transition,
            successor,
        }))
    }

    fn scatter(&self, prefix: &[u64], free: &[i64]) -> Result<IntegralKey> {
        if prefix.len() + free.len() != self.arity {
            return Err(Error::new(ErrorKind::Native, "residual scatter arity"));
        }
        let mut powers = Vec::with_capacity(self.arity);
        for (axis, &coordinate) in prefix.iter().enumerate() {
            let value = i64::try_from(coordinate)
                .map_err(|error| Error::new(ErrorKind::Resource, error))?;
            powers.push(if self.region.support[axis] {
                value.checked_add(1).ok_or_else(|| {
                    Error::new(ErrorKind::Resource, "fixed positive power overflow")
                })?
            } else {
                -value
            });
        }
        powers.extend_from_slice(free);
        IntegralKey::try_new(powers).map_err(|error| Error::new(ErrorKind::Native, error))
    }

    /// Recompute the exact next partition transition, without enumerating prior
    /// points. The caller still needs a generation-bound closed-window proof.
    pub fn validate_transition(
        &self,
        before: &PrefixCursor,
        record: &WindowTransition,
        cancel: &AtomicBool,
    ) -> Result<PrefixCursor> {
        if record.version != VERSION {
            return Err(Error::new(ErrorKind::Cursor, "transition version"));
        }
        let expected = self
            .plan(before, cancel)?
            .ok_or_else(|| Error::new(ErrorKind::Cursor, "transition after exhaustion"))?;
        if &expected.transition != record {
            return Err(Error::new(
                ErrorKind::Cursor,
                "skipped, repeated or changed window transition",
            ));
        }
        Ok(PrefixCursor(expected.transition.after))
    }

    /// Optional deep/audit restore from a transition chain bound to the backend's
    /// generation. This replays metadata/count transitions, never `.skip()` over
    /// old seed points. A future fast generation restore must retain that trust
    /// boundary; decoded cursor bytes or their own hash are not sufficient.
    /// This costs O(prior window transitions), not the planned bounded-depth
    /// normal restore from an authenticated completed backend generation.
    pub fn audit_restore_cursor<'a>(
        &self,
        bytes: &[u8],
        records: impl IntoIterator<Item = &'a WindowTransition>,
        cancel: &AtomicBool,
    ) -> Result<PrefixCursor> {
        let saved: CursorRecord = decode(bytes)?;
        if saved.version != VERSION || saved.binding != self.binding {
            return Err(Error::new(
                ErrorKind::Cursor,
                "saved cursor version/binding",
            ));
        }
        let mut cursor = self.initial_cursor();
        for record in records {
            check(cancel)?;
            cursor = self.validate_transition(&cursor, record, cancel)?;
        }
        if cursor.0 != saved {
            return Err(Error::new(
                ErrorKind::Cursor,
                "saved cursor lacks its exact transition chain",
            ));
        }
        Ok(cursor)
    }
}

fn residual(
    a: Option<u64>,
    r: Option<u32>,
    lo: Option<i64>,
    hi: Option<i64>,
    fixed_a: i128,
    fixed_r: i128,
) -> Result<Option<EntryPowerBudget>> {
    let a = i128::from(a.ok_or_else(|| {
        Error::new(
            ErrorKind::UnsupportedGeometry,
            "finite positive-power bound required",
        )
    })?)
    .checked_sub(fixed_a)
    .ok_or_else(|| Error::new(ErrorKind::Resource, "residual A overflow"))?;
    let r =
        i128::from(r.ok_or_else(|| {
            Error::new(ErrorKind::UnsupportedGeometry, "finite rank bound required")
        })?)
        .checked_sub(fixed_r)
        .ok_or_else(|| Error::new(ErrorKind::Resource, "residual R overflow"))?;
    if a < 0 || r < 0 {
        return Ok(None);
    }
    let shift = |bound: Option<i64>| -> Result<Option<i32>> {
        bound
            .map(|value| {
                i128::from(value)
                    .checked_sub(fixed_a)
                    .and_then(|v| v.checked_add(fixed_r))
                    .ok_or_else(|| Error::new(ErrorKind::Resource, "residual D overflow"))
                    .and_then(|value| {
                        i32::try_from(value)
                            .map_err(|error| Error::new(ErrorKind::UnsupportedGeometry, error))
                    })
            })
            .transpose()
    };
    Ok(Some(EntryPowerBudget {
        max_positive_power: u32::try_from(a)
            .map_err(|error| Error::new(ErrorKind::UnsupportedGeometry, error))?,
        max_numerator_rank: u32::try_from(r)
            .map_err(|error| Error::new(ErrorKind::UnsupportedGeometry, error))?,
        min_power_difference: shift(lo)?,
        max_power_difference: shift(hi)?,
    }))
}
