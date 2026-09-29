//! ledger6: one `u64` per ID (W2.0 protocol §4). Bits 63..61 hold the tag;
//! the payload depends on the tag. `Ledger6::apply` is the runtime mutator: it
//! checks the current tag against the exhaustive transition table and refuses
//! every other pair without mutation (a release check, never a debug assert).
//! Restore appends validated raw words only to a provisional private ledger.
//!
//! Preconditions that need more than the ledger (containment tokens, buckets,
//! the protected prefix, the watermark `W_k`, `in_flight` sequence numbers)
//! are checked by the caller (`state.rs`, `merge.rs`) before `apply`.

pub(super) const TAG_SHIFT: u32 = 61;
pub(super) const PAYLOAD_MASK: u64 = (1 << TAG_SHIFT) - 1;
/// Merge epochs are stored in 48 bits (F13 preflight: k + 1 < 2^48).
pub(super) const EPOCH_LIMIT: u64 = 1 << 48;
const EPOCH_MASK: u64 = EPOCH_LIMIT - 1;
const RESIDUAL_BIT: u64 = 1 << 48;
const DBAND_BIT: u64 = 1 << 49;

/// Attempts at which T7 turns into T8 (§8.4: attempts >= 8 or guard >= 3).
pub(super) const MAX_ATTEMPTS: u8 = 8;
pub(super) const MAX_GUARD: u8 = 3;

/// The NativeError `err` class codes (bits 48-55; §4.1, §9.1). T11 (designed
/// only) is keyed on `ALLOWANCE`. A native `ResourceLimit` failure is
/// `NATIVE_FAILURE` (its record's error text names the resource).
pub(super) mod err_class {
    pub const NATIVE_FAILURE: u8 = 1;
    pub const CONVERSION: u8 = 2;
    pub const RESOLVER_RANGE: u8 = 3;
    pub const RESOLVER_SUMMARY: u8 = 4;
    pub const RESOLVER_DIAGNOSTIC: u8 = 5;
    /// The per-inspection allowance crossed (break reason `allowance`).
    pub const ALLOWANCE: u8 = 6;
    /// A recurring C3 panic (parity-exempt, no successors, no edges).
    pub const RECURRING_PANIC: u8 = 7;
    /// A recurring C3 of an unclassified kind.
    pub const RECURRING_UNKNOWN: u8 = 8;
    pub fn name(code: u8) -> &'static str {
        match code {
            NATIVE_FAILURE => "native_failure",
            CONVERSION => "conversion",
            RESOLVER_RANGE => "resolver_range",
            RESOLVER_SUMMARY => "resolver_summary",
            RESOLVER_DIAGNOSTIC => "resolver_diagnostic",
            ALLOWANCE => "allowance",
            RECURRING_PANIC => "recurring_panic",
            RECURRING_UNKNOWN => "recurring_unknown",
            _ => "invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum Tag {
    Pending = 0,
    Reserved = 1,
    Native = 2,
    NativeFrontier = 3,
    NativeError = 4,
    Alias = 5,
    Exhausted = 6,
    Abandoned = 7,
}

impl Tag {
    pub const ALL: [Tag; 8] = [
        Tag::Pending,
        Tag::Reserved,
        Tag::Native,
        Tag::NativeFrontier,
        Tag::NativeError,
        Tag::Alias,
        Tag::Exhausted,
        Tag::Abandoned,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Tag::Pending => "pending",
            Tag::Reserved => "reserved",
            Tag::Native => "native",
            Tag::NativeFrontier => "native_frontier",
            Tag::NativeError => "native_error",
            Tag::Alias => "alias",
            Tag::Exhausted => "exhausted",
            Tag::Abandoned => "abandoned",
        }
    }
    fn from_bits(bits: u64) -> Option<Self> {
        Some(match bits {
            0 => Tag::Pending,
            1 => Tag::Reserved,
            2 => Tag::Native,
            3 => Tag::NativeFrontier,
            4 => Tag::NativeError,
            5 => Tag::Alias,
            6 => Tag::Exhausted,
            7 => Tag::Abandoned,
            _ => return None,
        })
    }
}

/// Liveness counters of an unmerged ID (Pending, Reserved, Exhausted). They
/// survive requeue (T7) and are reset only by T9.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Counters {
    /// bits 0-7
    pub attempts: u8,
    /// bits 8-11 (u4)
    pub guard: u8,
    /// bits 12-19: the class code of the last discarded result (0 = none).
    pub last_err: u8,
    /// bits 20-27
    pub dispatch_class: u8,
}

impl Counters {
    fn encode(self) -> u64 {
        u64::from(self.attempts)
            | (u64::from(self.guard & 0xf) << 8)
            | (u64::from(self.last_err) << 12)
            | (u64::from(self.dispatch_class) << 20)
    }
    fn decode(payload: u64) -> Result<Self, LedgerError> {
        if payload >> 28 != 0 {
            return Err(LedgerError::Malformed);
        }
        Ok(Self {
            attempts: payload as u8,
            guard: (payload >> 8) as u8 & 0xf,
            last_err: (payload >> 12) as u8,
            dispatch_class: (payload >> 20) as u8,
        })
    }
}

/// One decoded ledger entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Entry6 {
    Pending(Counters),
    Reserved(Counters),
    Native {
        epoch: u64,
        residual: bool,
        dband: bool,
    },
    NativeFrontier {
        epoch: u64,
    },
    NativeError {
        epoch: u64,
        err: u8,
    },
    Alias {
        to: u32,
    },
    Exhausted(Counters),
    Abandoned {
        epoch: u64,
    },
}

impl Entry6 {
    pub fn tag(self) -> Tag {
        match self {
            Entry6::Pending(_) => Tag::Pending,
            Entry6::Reserved(_) => Tag::Reserved,
            Entry6::Native { .. } => Tag::Native,
            Entry6::NativeFrontier { .. } => Tag::NativeFrontier,
            Entry6::NativeError { .. } => Tag::NativeError,
            Entry6::Alias { .. } => Tag::Alias,
            Entry6::Exhausted(_) => Tag::Exhausted,
            Entry6::Abandoned { .. } => Tag::Abandoned,
        }
    }
    pub fn encode(self) -> u64 {
        let (tag, payload) = match self {
            Entry6::Pending(c) => (Tag::Pending, c.encode()),
            Entry6::Reserved(c) => (Tag::Reserved, c.encode()),
            Entry6::Native {
                epoch,
                residual,
                dband,
            } => (
                Tag::Native,
                (epoch & EPOCH_MASK)
                    | if residual { RESIDUAL_BIT } else { 0 }
                    | if dband { DBAND_BIT } else { 0 },
            ),
            Entry6::NativeFrontier { epoch } => (Tag::NativeFrontier, epoch & EPOCH_MASK),
            Entry6::NativeError { epoch, err } => (
                Tag::NativeError,
                (epoch & EPOCH_MASK) | (u64::from(err) << 48),
            ),
            Entry6::Alias { to } => (Tag::Alias, u64::from(to)),
            Entry6::Exhausted(c) => (Tag::Exhausted, c.encode()),
            Entry6::Abandoned { epoch } => (Tag::Abandoned, epoch & EPOCH_MASK),
        };
        ((tag as u64) << TAG_SHIFT) | payload
    }
    /// Refuses any payload bit outside the tag's layout.
    pub fn decode(word: u64) -> Result<Self, LedgerError> {
        let tag = Tag::from_bits(word >> TAG_SHIFT).ok_or(LedgerError::InvalidTag)?;
        let payload = word & PAYLOAD_MASK;
        Ok(match tag {
            Tag::Pending => Entry6::Pending(Counters::decode(payload)?),
            Tag::Reserved => Entry6::Reserved(Counters::decode(payload)?),
            Tag::Exhausted => Entry6::Exhausted(Counters::decode(payload)?),
            Tag::Native => {
                if payload >> 50 != 0 {
                    return Err(LedgerError::Malformed);
                }
                Entry6::Native {
                    epoch: payload & EPOCH_MASK,
                    residual: payload & RESIDUAL_BIT != 0,
                    dband: payload & DBAND_BIT != 0,
                }
            }
            Tag::NativeFrontier => {
                if payload >> 48 != 0 {
                    return Err(LedgerError::Malformed);
                }
                Entry6::NativeFrontier { epoch: payload }
            }
            Tag::Abandoned => {
                if payload >> 48 != 0 {
                    return Err(LedgerError::Malformed);
                }
                Entry6::Abandoned { epoch: payload }
            }
            Tag::NativeError => {
                if payload >> 56 != 0 {
                    return Err(LedgerError::Malformed);
                }
                Entry6::NativeError {
                    epoch: payload & EPOCH_MASK,
                    err: (payload >> 48) as u8,
                }
            }
            Tag::Alias => {
                if payload >> 32 != 0 {
                    return Err(LedgerError::Malformed);
                }
                Entry6::Alias { to: payload as u32 }
            }
        })
    }
    /// The merge epoch of a merged native (Native, NativeFrontier, NativeError).
    #[cfg(test)]
    pub fn merge_epoch(self) -> Option<u64> {
        match self {
            Entry6::Native { epoch, .. }
            | Entry6::NativeFrontier { epoch }
            | Entry6::NativeError { epoch, .. } => Some(epoch),
            _ => None,
        }
    }
    pub fn counters(self) -> Option<Counters> {
        match self {
            Entry6::Pending(c) | Entry6::Reserved(c) | Entry6::Exhausted(c) => Some(c),
            _ => None,
        }
    }
}

/// The ledger transitions of §4.3. T11 is designed, not built (W2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Transition {
    /// (new) -> Pending: P3 step 1 or initial admission; `id == len`.
    T1New { dispatch_class: u8 },
    /// Pending -> Reserved: refill.
    T2Reserve,
    /// Pending -> Alias{to}: P3 retirement with a verified token (caller).
    T3Alias { to: u32 },
    /// Reserved -> Native (C0, 0 frontiers).
    T4Native {
        epoch: u64,
        residual: bool,
        dband: bool,
    },
    /// Reserved -> NativeFrontier (C0 with >= 1 frontier).
    T5Frontier { epoch: u64 },
    /// Reserved -> NativeError (C2, or a recurring C3).
    T6Error { epoch: u64, err: u8 },
    /// Reserved -> Reserved: requeue of a discarded C1/C3 result.
    T7Requeue {
        d_attempts: u8,
        d_guard: u8,
        last_err: u8,
    },
    /// Reserved -> Exhausted: T7 that would reach the liveness limits
    /// (checked here: the bumped counters must reach attempts >= 8 or
    /// guard >= 3; at restore the journal-derived guard increment is passed
    /// as `d_guard`).
    T8Exhaust {
        d_attempts: u8,
        d_guard: u8,
        last_err: u8,
    },
    /// Exhausted -> Pending (restore with --retry-exhausted; counters reset).
    #[allow(dead_code)] // restore-time operator flag (S3)
    T9Retry,
    /// Exhausted -> Alias{to}: P3 retirement (nothing is in flight).
    T10ExhaustedAlias { to: u32 },
    /// Pending or Reserved -> Exhausted (restore with --exhaust-id).
    #[allow(dead_code)] // restore-time operator flag (S3)
    T12ExhaustId,
    /// Reserved -> Abandoned: explicit resume-rescue retirement, never inspected.
    T13Abandon { epoch: u64 },
}

impl Transition {
    /// Every transition kind once (payloads are placeholders), for the
    /// exhaustive table test.
    #[cfg(test)]
    pub const KINDS: [Transition; 12] = [
        Transition::T1New { dispatch_class: 0 },
        Transition::T2Reserve,
        Transition::T3Alias { to: 1 },
        Transition::T4Native {
            epoch: 1,
            residual: false,
            dband: false,
        },
        Transition::T5Frontier { epoch: 1 },
        Transition::T6Error { epoch: 1, err: 1 },
        Transition::T7Requeue {
            d_attempts: 1,
            d_guard: 0,
            last_err: 1,
        },
        Transition::T8Exhaust {
            d_attempts: MAX_ATTEMPTS,
            d_guard: 0,
            last_err: 1,
        },
        Transition::T9Retry,
        Transition::T10ExhaustedAlias { to: 1 },
        Transition::T12ExhaustId,
        Transition::T13Abandon { epoch: 1 },
    ];

    /// The tag an existing entry must have (None: T1, which creates it).
    pub fn allowed_from(self, from: Tag) -> bool {
        matches!(
            (self, from),
            (Transition::T2Reserve, Tag::Pending)
                | (Transition::T3Alias { .. }, Tag::Pending)
                | (Transition::T4Native { .. }, Tag::Reserved)
                | (Transition::T5Frontier { .. }, Tag::Reserved)
                | (Transition::T6Error { .. }, Tag::Reserved)
                | (Transition::T7Requeue { .. }, Tag::Reserved)
                | (Transition::T8Exhaust { .. }, Tag::Reserved)
                | (Transition::T9Retry, Tag::Exhausted)
                | (Transition::T10ExhaustedAlias { .. }, Tag::Exhausted)
                | (Transition::T12ExhaustId, Tag::Pending | Tag::Reserved)
                | (Transition::T13Abandon { .. }, Tag::Reserved)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LedgerError {
    /// Tag 7 or an unknown tag (restore refuses).
    InvalidTag,
    /// Payload bits outside the tag's layout.
    Malformed,
    /// `id` beyond the ledger (or T1 not at the end).
    InvalidId,
    /// The transition is not in the table for the current tag.
    Refused { from: Tag },
    /// A counter or epoch outside its field.
    Overflow,
    /// T3/T10 to a target that is not strictly greater.
    BackwardAlias,
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::InvalidTag => f.write_str("ledger6: invalid tag"),
            LedgerError::Malformed => f.write_str("ledger6: malformed payload"),
            LedgerError::InvalidId => f.write_str("ledger6: invalid id"),
            LedgerError::Refused { from } => {
                write!(f, "ledger6: transition refused from {}", from.name())
            }
            LedgerError::Overflow => f.write_str("ledger6: counter or epoch overflow"),
            LedgerError::BackwardAlias => f.write_str("ledger6: alias target not greater"),
        }
    }
}

/// Per-tag counts, kept current by `apply`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TagCounts(pub [u64; 8]);

impl TagCounts {
    pub fn get(&self, tag: Tag) -> u64 {
        self.0[tag as usize]
    }
    pub fn json(&self) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        for tag in Tag::ALL {
            out.insert(tag.name().into(), self.get(tag).into());
        }
        serde_json::Value::Object(out)
    }
}

#[derive(Default)]
pub(super) struct Ledger6 {
    entries: Vec<u64>,
    counts: TagCounts,
}

impl Ledger6 {
    pub fn counts(&self) -> TagCounts {
        self.counts
    }
    pub fn get(&self, id: u32) -> Result<Entry6, LedgerError> {
        let word = *self
            .entries
            .get(id as usize)
            .ok_or(LedgerError::InvalidId)?;
        Entry6::decode(word)
    }
    pub fn tag(&self, id: u32) -> Option<Tag> {
        self.get(id).ok().map(Entry6::tag)
    }
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), &'static str> {
        self.entries
            .try_reserve(additional)
            .map_err(|_| "ledger6 allocation")
    }
    /// The raw words (export and identity comparison).
    pub fn words(&self) -> &[u64] {
        &self.entries
    }

    /// Restore-only append to a provisional ledger. Decode before mutation,
    /// preserve every attempts/guard bit, and rebuild tag counts. The section
    /// digest and cross-state validators still precede any dispatch.
    pub fn restore_word(&mut self, word: u64) -> Result<(), LedgerError> {
        let entry = Entry6::decode(word)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| LedgerError::Overflow)?;
        self.entries.push(word);
        self.counts.0[entry.tag() as usize] += 1;
        Ok(())
    }

    /// The only mutator. T1 appends `id == len`; every other transition
    /// checks the current tag against the table and refuses, without
    /// mutation, any pair it does not list.
    pub fn apply(&mut self, id: u32, t: Transition) -> Result<(), LedgerError> {
        if let Transition::T1New { dispatch_class } = t {
            if id as usize != self.entries.len() {
                return Err(LedgerError::InvalidId);
            }
            if self.entries.len() == self.entries.capacity() {
                self.entries
                    .try_reserve(1)
                    .map_err(|_| LedgerError::Overflow)?;
            }
            let entry = Entry6::Pending(Counters {
                dispatch_class,
                ..Counters::default()
            });
            self.entries.push(entry.encode());
            self.counts.0[Tag::Pending as usize] += 1;
            return Ok(());
        }
        let current = self.get(id)?;
        let from = current.tag();
        if !t.allowed_from(from) {
            return Err(LedgerError::Refused { from });
        }
        let next = match (t, current) {
            (Transition::T13Abandon { epoch }, _) => {
                check_epoch(epoch)?;
                Entry6::Abandoned { epoch }
            }
            (Transition::T2Reserve, Entry6::Pending(c)) => Entry6::Reserved(c),
            (Transition::T3Alias { to }, _) | (Transition::T10ExhaustedAlias { to }, _) => {
                if to <= id {
                    return Err(LedgerError::BackwardAlias);
                }
                Entry6::Alias { to }
            }
            (
                Transition::T4Native {
                    epoch,
                    residual,
                    dband,
                },
                _,
            ) => {
                check_epoch(epoch)?;
                Entry6::Native {
                    epoch,
                    residual,
                    dband,
                }
            }
            (Transition::T5Frontier { epoch }, _) => {
                check_epoch(epoch)?;
                Entry6::NativeFrontier { epoch }
            }
            (Transition::T6Error { epoch, err }, _) => {
                check_epoch(epoch)?;
                Entry6::NativeError { epoch, err }
            }
            (
                Transition::T7Requeue {
                    d_attempts,
                    d_guard,
                    last_err,
                },
                Entry6::Reserved(c),
            ) => {
                let c = bump(c, d_attempts, d_guard, last_err)?;
                if c.attempts >= MAX_ATTEMPTS || c.guard >= MAX_GUARD {
                    // T7 that reaches the limits must be T8 (caller decides).
                    return Err(LedgerError::Refused { from });
                }
                Entry6::Reserved(c)
            }
            (
                Transition::T8Exhaust {
                    d_attempts,
                    d_guard,
                    last_err,
                },
                Entry6::Reserved(c),
            ) => {
                let c = bump(c, d_attempts, d_guard, last_err)?;
                if c.attempts < MAX_ATTEMPTS && c.guard < MAX_GUARD {
                    // Below the limits T8 is not allowed (it would be T7).
                    return Err(LedgerError::Refused { from });
                }
                Entry6::Exhausted(c)
            }
            (Transition::T9Retry, Entry6::Exhausted(c)) => Entry6::Pending(Counters {
                dispatch_class: c.dispatch_class,
                ..Counters::default()
            }),
            (Transition::T12ExhaustId, Entry6::Pending(c) | Entry6::Reserved(c)) => {
                Entry6::Exhausted(c)
            }
            _ => return Err(LedgerError::Refused { from }),
        };
        self.entries[id as usize] = next.encode();
        self.counts.0[from as usize] -= 1;
        self.counts.0[next.tag() as usize] += 1;
        Ok(())
    }

    /// Recount from the words (restore/audit cross-check of the counts).
    #[cfg(test)]
    pub fn recount(&self) -> Result<TagCounts, LedgerError> {
        let mut counts = TagCounts::default();
        for &word in &self.entries {
            counts.0[Entry6::decode(word)?.tag() as usize] += 1;
        }
        Ok(counts)
    }
}

fn check_epoch(epoch: u64) -> Result<(), LedgerError> {
    // Epoch 0 is the CP5-import stamp (§11.6); merges stamp k + 1 >= 1.
    if epoch >= EPOCH_LIMIT {
        Err(LedgerError::Overflow)
    } else {
        Ok(())
    }
}

fn bump(c: Counters, d_attempts: u8, d_guard: u8, last_err: u8) -> Result<Counters, LedgerError> {
    let attempts = c
        .attempts
        .checked_add(d_attempts)
        .ok_or(LedgerError::Overflow)?;
    let guard = c.guard.checked_add(d_guard).ok_or(LedgerError::Overflow)?;
    if guard > 0xf {
        return Err(LedgerError::Overflow);
    }
    Ok(Counters {
        attempts,
        guard,
        last_err,
        dispatch_class: c.dispatch_class,
    })
}
