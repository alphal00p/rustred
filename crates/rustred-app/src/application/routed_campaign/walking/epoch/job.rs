//! Byte-serialized job and result types (W2.0 protocol §2.4). Jobs and
//! results cross the inspector/coordinator boundary as little-endian bytes
//! with no pointers. Shared lookup leases never travel in a result, and no
//! serialized summary is trusted. Explicit writer/reader; no new dependency.
//!
//! S2 layout (versioned by `JOB_MAGIC` / `RESULT_MAGIC`): the job header is
//! 32 bytes plus the canonical image; the result header is fixed and is
//! followed by length-prefixed variable parts (stats, error, frontiers,
//! refusal provenance, the initial-overlap scope, the G2' part and the
//! obligations). ERS4 adds private lockstep snapshot targets and work counters;
//! all target images are independently verified, not sampled canaries. Local,
//! MRU and rolling refresh are absent. This ephemeral byte version is not a
//! checkpoint compatibility importer or a change to persisted record bodies.
use super::super::queue::{CompactDomain, Phase};
use rustred::solver::DomainPowerBounds;

pub(super) const JOB_MAGIC: u32 = u32::from_le_bytes(*b"EJB2");
pub(super) const JOB_RESCUE_ABANDONED: u16 = 1;
/// ERS4: checked snapshot identity, inspector lookup work and optional stored
/// target per obligation. No serialized summary is mathematical authority.
pub(super) const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"ERS4");

// ---- little-endian writer and reader --------------------------------------

#[derive(Default)]
pub(super) struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    /// Length-prefixed bytes (u32 length).
    /// Results are encoded inside the inspector's unwind frame: a part of
    /// 4 GiB or more is a C3 result there, never a truncated length.
    pub fn bytes(&mut self, v: &[u8]) {
        self.count(v.len());
        self.0.extend_from_slice(v);
    }
    /// A u32 element count (checked; see `bytes`).
    pub fn count(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("a result part below 2^32 elements"));
    }
    pub fn opt_str(&mut self, v: Option<&str>) {
        match v {
            None => self.u32(u32::MAX),
            Some(s) => self.bytes(s.as_bytes()),
        }
    }
    pub fn opt_u64(&mut self, v: Option<u64>) {
        self.u8(u8::from(v.is_some()));
        self.u64(v.unwrap_or(0));
    }
    pub fn opt_i64(&mut self, v: Option<i64>) {
        self.u8(u8::from(v.is_some()));
        self.i64(v.unwrap_or(0));
    }
    pub fn powers(&mut self, p: DomainPowerBounds) {
        self.opt_u64(p.max_positive_power);
        self.opt_i64(p.min_power_difference);
        self.opt_i64(p.max_power_difference);
    }
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

type Decoded<T> = Result<T, &'static str>;

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, n: usize) -> Decoded<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or("byte length overflow")?;
        let slice = self.bytes.get(self.at..end).ok_or("truncated bytes")?;
        self.at = end;
        Ok(slice)
    }
    pub fn u8(&mut self) -> Decoded<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Decoded<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("2")))
    }
    pub fn u32(&mut self) -> Decoded<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }
    pub fn u64(&mut self) -> Decoded<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8")))
    }
    pub fn i64(&mut self) -> Decoded<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().expect("8")))
    }
    pub fn f64(&mut self) -> Decoded<f64> {
        Ok(f64::from_bits(self.u64()?))
    }
    pub fn bytes(&mut self) -> Decoded<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }
    pub fn opt_str(&mut self) -> Decoded<Option<String>> {
        let len = self.u32()?;
        if len == u32::MAX {
            return Ok(None);
        }
        let bytes = self.take(len as usize)?;
        String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|_| "non-UTF-8 string")
    }
    fn flag(&mut self) -> Decoded<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("invalid flag byte"),
        }
    }
    pub fn opt_u64(&mut self) -> Decoded<Option<u64>> {
        let present = self.flag()?;
        let value = self.u64()?;
        if !present && value != 0 {
            return Err("non-canonical absent value");
        }
        Ok(present.then_some(value))
    }
    pub fn opt_i64(&mut self) -> Decoded<Option<i64>> {
        let present = self.flag()?;
        let value = self.i64()?;
        if !present && value != 0 {
            return Err("non-canonical absent value");
        }
        Ok(present.then_some(value))
    }
    pub fn powers(&mut self) -> Decoded<DomainPowerBounds> {
        Ok(DomainPowerBounds {
            max_positive_power: self.opt_u64()?,
            min_power_difference: self.opt_i64()?,
            max_power_difference: self.opt_i64()?,
        })
    }
    pub fn finish(&self) -> Decoded<()> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err("trailing bytes")
        }
    }
}

// ---- canonical image --------------------------------------------------------

const INFINITE: u16 = u16::MAX;

/// The canonical field bytes of one compact image (37 + 4N bytes): phase,
/// owner bits, rank, lower and upper coordinates (u16, +infinity = 0xFFFF)
/// and the three power bounds.
pub(super) fn write_image<const N: usize>(w: &mut Writer, image: &CompactDomain<N>) {
    w.u8(match image.phase() {
        Phase::Apply => 0,
        Phase::Route => 1,
    });
    let owner = image
        .owner()
        .iter()
        .enumerate()
        .fold(0u32, |bits, (axis, &on)| bits | (u32::from(on) << axis));
    w.u32(owner);
    w.u8(u8::from(image.rank().is_some()));
    w.u32(image.rank().unwrap_or(0));
    for axis in 0..N {
        w.u16(image.lower(axis) as u16);
    }
    for axis in 0..N {
        w.u16(image.upper(axis).map_or(INFINITE, |v| v as u16));
    }
    w.powers(image.powers());
}

/// Decode, then rebuild through the same checked compact encoding as transport
/// domains (F1), without allocating temporary coordinate vectors. Canonical
/// wire checks and their error order remain independent of domain validation.
pub(super) fn read_image<const N: usize>(r: &mut Reader<'_>) -> Decoded<CompactDomain<N>> {
    read_image_with_arity(r, N)
}

pub(super) fn read_image_with_arity<const N: usize>(
    r: &mut Reader<'_>,
    wire_arity: usize,
) -> Decoded<CompactDomain<N>> {
    if !crate::application::routed_campaign::storage::compatible_width(wire_arity, N) {
        return Err("image wire arity");
    }
    let phase = match r.u8()? {
        0 => Phase::Apply,
        1 => Phase::Route,
        _ => return Err("invalid image phase"),
    };
    let owner_bits = r.u32()?;
    if N < 32 && owner_bits >> N != 0 {
        return Err("image owner beyond arity");
    }
    let rank_present = r.flag()?;
    let rank_value = r.u32()?;
    if !rank_present && rank_value != 0 {
        return Err("non-canonical absent rank");
    }
    let mut lower = Vec::with_capacity(wire_arity);
    for _ in 0..wire_arity {
        lower.push(u64::from(r.u16()?));
    }
    let mut upper = Vec::with_capacity(wire_arity);
    for _ in 0..wire_arity {
        let v = r.u16()?;
        upper.push((v != INFINITE).then_some(u64::from(v)));
    }
    let powers = r.powers()?;
    CompactDomain::try_from_parts(
        phase,
        std::array::from_fn(|axis| owner_bits >> axis & 1 == 1),
        &lower,
        &upper,
        rank_present.then_some(rank_value),
        powers,
    )
}

// ---- jobs --------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Job<const N: usize> {
    /// session << 40 | counter; unique across the run.
    pub seq: u64,
    pub parent: u32,
    /// The merge counter at dispatch (S_v0 is the snapshot the job saw).
    pub v0: u64,
    pub attempts: u8,
    pub flags: u16,
    pub image: CompactDomain<N>,
}

impl<const N: usize> Job<N> {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(Vec::with_capacity(32 + 37 + 4 * N));
        w.u32(JOB_MAGIC);
        w.u64(self.seq);
        w.u32(self.parent);
        w.u64(self.v0);
        w.u8(match self.image.phase() {
            Phase::Apply => 0,
            Phase::Route => 1,
        });
        w.u8(self.attempts);
        w.u16(self.flags);
        w.u32(0); // reserved (Route micro-batch count, S4)
        write_image(&mut w, &self.image);
        w.0
    }
    pub fn decode(bytes: &[u8]) -> Decoded<Self> {
        let mut r = Reader::new(bytes);
        if r.u32()? != JOB_MAGIC {
            return Err("job magic");
        }
        let seq = r.u64()?;
        let parent = r.u32()?;
        let v0 = r.u64()?;
        let kind = r.u8()?;
        let attempts = r.u8()?;
        let flags = r.u16()?;
        if r.u32()? != 0 {
            return Err("job reserved bytes");
        }
        let image = read_image::<N>(&mut r)?;
        r.finish()?;
        if kind != image.phase() as u8 {
            return Err("job kind differs from its image phase");
        }
        Ok(Self {
            seq,
            parent,
            v0,
            attempts,
            flags,
            image,
        })
    }
}

// ---- results -----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeKind {
    Apply = 0,
    /// An InitialDBand residual inspection (carries `Scope`).
    ApplyPartial = 1,
    Route = 2,
    /// A G2' residual inspection (carries `G2Part`; W4 planner, refused
    /// without a bound G2' flag).
    G2Residual = 3,
    Abandoned = 4,
    /// Whole initial finite domain, discharged only by bound cold exact replay.
    FiniteReplay = 5,
}

/// The native `error_kind` values (`inspection.rs`), plus `Other` for any
/// kind this build does not know (classified C3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ErrorKind {
    None = 0,
    Cancelled = 1,
    ConsumerStop = 2,
    NativeFailure = 3,
    Conversion = 4,
    Other = 5,
}

impl ErrorKind {
    pub fn of(kind: &str) -> Self {
        match kind {
            "none" => ErrorKind::None,
            "cancelled" => ErrorKind::Cancelled,
            "consumer_stop" => ErrorKind::ConsumerStop,
            "native_failure" => ErrorKind::NativeFailure,
            "conversion" => ErrorKind::Conversion,
            _ => ErrorKind::Other,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            ErrorKind::None => "none",
            ErrorKind::Cancelled => "cancelled",
            ErrorKind::ConsumerStop => "consumer_stop",
            ErrorKind::NativeFailure => "native_failure",
            ErrorKind::Conversion => "conversion",
            ErrorKind::Other => "other",
        }
    }
}

/// Why the resolver returned `Break` (§2.4); `Protocol` is an event the
/// resolver must never receive (a pre-admitted orthant with empty orthants).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BreakReason {
    None = 0,
    Allowance = 1,
    ResolverRange = 2,
    ResolverSummary = 3,
    ResolverDiagnostic = 4,
    SpillIo = 5,
    Alloc = 6,
    Cancel = 7,
    Protocol = 8,
}

impl BreakReason {
    fn of(code: u8) -> Decoded<Self> {
        Ok(match code {
            0 => BreakReason::None,
            1 => BreakReason::Allowance,
            2 => BreakReason::ResolverRange,
            3 => BreakReason::ResolverSummary,
            4 => BreakReason::ResolverDiagnostic,
            5 => BreakReason::SpillIo,
            6 => BreakReason::Alloc,
            7 => BreakReason::Cancel,
            8 => BreakReason::Protocol,
            _ => return Err("invalid break reason"),
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            BreakReason::None => "none",
            BreakReason::Allowance => "allowance",
            BreakReason::ResolverRange => "resolver_range",
            BreakReason::ResolverSummary => "resolver_summary",
            BreakReason::ResolverDiagnostic => "resolver_diagnostic",
            BreakReason::SpillIo => "spill_io",
            BreakReason::Alloc => "alloc",
            BreakReason::Cancel => "cancel",
            BreakReason::Protocol => "protocol",
        }
    }
}

/// An initial D-band scope (InitialDBand anchor, §7 R2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Scope {
    pub anchor: u32,
    pub cut: i64,
    pub residual: DomainPowerBounds,
}

/// The G2' part of a result (§7 R3, union form): the record kind code (1
/// G2Native, 2 G2Residual), the anchors `(anchor, stamp, lent scope code)`
/// planned from `MergedView` of S_{v0}, and the inspected residual pieces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct G2Part {
    pub kind: u8,
    pub anchors: Vec<(u32, u64, u8)>,
    pub pieces: Vec<super::anchors::Piece>,
}

/// One distinct Admit obligation. The historical type/field name `Miss` stays
/// internal; a private S4 stored proposal is explicitly identified by `target`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Miss<const N: usize> {
    pub ordinal: u32,
    pub digest: u64,
    pub image: CompactDomain<N>,
    /// A snapshot lookup proposal, independently verified in P2. None retains
    /// the ordinary all-miss path. The image is always shipped in full.
    pub target: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct LookupReport {
    pub version: u64,
    pub published_len: u32,
    pub lookup: super::store::LookupCounters,
    pub verify: super::verify::VerifyCounters,
    /// Admit-path elapsed time: preparation, deduplication, lookup and buffering.
    /// Worker elapsed time, not CPU attribution or a whole-command estimate.
    pub seconds: f64,
}

impl LookupReport {
    fn encode(&self, w: &mut Writer) {
        w.u64(self.version);
        w.u32(self.published_len);
        for n in [
            self.lookup.exact_hits,
            self.lookup.orthant_hits,
            self.lookup.contained_hits,
            self.lookup.misses,
            self.lookup.forward_candidates,
            self.lookup.forward_tests,
            self.lookup.reverse_candidates,
            self.lookup.reverse_tests,
            self.verify.calls,
            self.verify.accepted,
            self.verify.raw_inclusions,
            self.verify.recomputes,
            self.verify.refused_range,
            self.verify.refused_bucket,
            self.verify.union_covers,
        ] {
            w.u64(n);
        }
        w.f64(self.seconds);
    }

    fn decode(r: &mut Reader<'_>) -> Decoded<Self> {
        let version = r.u64()?;
        let published_len = r.u32()?;
        let lookup = super::store::LookupCounters {
            exact_hits: r.u64()?,
            orthant_hits: r.u64()?,
            contained_hits: r.u64()?,
            misses: r.u64()?,
            forward_candidates: r.u64()?,
            forward_tests: r.u64()?,
            reverse_candidates: r.u64()?,
            reverse_tests: r.u64()?,
        };
        let verify = super::verify::VerifyCounters {
            calls: r.u64()?,
            accepted: r.u64()?,
            raw_inclusions: r.u64()?,
            recomputes: r.u64()?,
            refused_range: r.u64()?,
            refused_bucket: r.u64()?,
            union_covers: r.u64()?,
        };
        let seconds = r.f64()?;
        if !seconds.is_finite() || seconds < 0.0 {
            return Err("invalid inspector lookup duration");
        }
        Ok(Self {
            version,
            published_len,
            lookup,
            verify,
            seconds,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct JobResult<const N: usize> {
    pub seq: u64,
    pub parent: u32,
    pub v0: u64,
    pub kind: NativeKind,
    pub error_kind: ErrorKind,
    pub break_reason: BreakReason,
    pub panic: bool,
    pub emitted: u64,
    pub accepted: u64,
    /// The native stats' event count; `u64::MAX` when unavailable (panic).
    pub stats_events: u64,
    pub successors: u64,
    pub conditional: u64,
    pub known_reuse: u64,
    pub job_duplicates: u64,
    /// optional coefficient refusals: total, original, coalesced
    pub optional: [u64; 3],
    pub route_masks: u64,
    pub route_joint_pruned: u64,
    pub seconds: f64,
    pub stats_json: Vec<u8>,
    pub error: Option<String>,
    pub frontiers: Vec<Vec<u8>>,
    pub refusals: Vec<Vec<u8>>,
    pub refusals_truncated: bool,
    pub scope: Option<Scope>,
    pub g2: Option<G2Part>,
    pub finite_replay: Option<super::super::finite_replay::Recipe>,
    pub lookup: Option<LookupReport>,
    pub misses: Vec<Miss<N>>,
}

impl<const N: usize> JobResult<N> {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(Vec::with_capacity(256 + self.misses.len() * (49 + 4 * N)));
        w.u32(RESULT_MAGIC);
        w.u64(self.seq);
        w.u32(self.parent);
        w.u64(self.v0);
        w.u8(self.kind as u8);
        w.u8(self.error_kind as u8);
        w.u8(self.break_reason as u8);
        w.u8(u8::from(self.panic));
        for v in [
            self.emitted,
            self.accepted,
            self.stats_events,
            self.successors,
            self.conditional,
            self.known_reuse,
            self.job_duplicates,
            self.optional[0],
            self.optional[1],
            self.optional[2],
            self.route_masks,
            self.route_joint_pruned,
        ] {
            w.u64(v);
        }
        w.f64(self.seconds);
        w.bytes(&self.stats_json);
        w.opt_str(self.error.as_deref());
        w.count(self.frontiers.len());
        for frontier in &self.frontiers {
            w.bytes(frontier);
        }
        w.count(self.refusals.len());
        for refusal in &self.refusals {
            w.bytes(refusal);
        }
        w.u8(u8::from(self.refusals_truncated));
        match &self.scope {
            None => w.u8(0),
            Some(scope) => {
                w.u8(1);
                w.u32(scope.anchor);
                w.i64(scope.cut);
                w.powers(scope.residual);
            }
        }
        match &self.g2 {
            None => w.u8(0),
            Some(g2) => {
                w.u8(1);
                w.u8(g2.kind);
                w.count(g2.anchors.len());
                for &(anchor, stamp, lent) in &g2.anchors {
                    w.u32(anchor);
                    w.u64(stamp);
                    w.u8(lent);
                }
                w.count(g2.pieces.len());
                for piece in &g2.pieces {
                    assert!(
                        piece.lower.len() == N && piece.upper.len() == N,
                        "G2' piece arity"
                    );
                    w.opt_i64(piece.d_lo);
                    w.opt_i64(piece.d_hi);
                    for &v in piece.lower.iter().chain(&piece.upper) {
                        w.u16(v);
                    }
                }
            }
        }
        w.u8(u8::from(self.lookup.is_some()));
        if let Some(lookup) = &self.lookup {
            lookup.encode(&mut w);
        }
        w.count(self.misses.len());
        for miss in &self.misses {
            w.u32(miss.ordinal);
            w.u64(miss.digest);
            write_image(&mut w, &miss.image);
            w.u8(u8::from(miss.target.is_some()));
            if let Some(target) = miss.target {
                w.u32(target);
            }
        }
        if self.kind == NativeKind::FiniteReplay {
            let recipe = self.finite_replay.expect("finite replay kind needs recipe");
            w.u32(recipe.version);
            let limits = recipe.limits;
            for limit in [
                limits.max_nodes,
                limits.max_rule_applications,
                limits.max_transport_calls,
                limits.max_transport_operations,
                limits.max_transport_endpoints,
                limits.max_coalescing_additions,
                limits.max_positive_layers,
                limits.max_seed_points,
                limits.max_seed_bytes,
            ] {
                w.u64(limit as u64);
            }
        } else {
            assert!(
                self.finite_replay.is_none(),
                "recipe without finite replay kind"
            );
        }
        w.0
    }

    pub fn decode(bytes: &[u8]) -> Decoded<Self> {
        let mut r = Reader::new(bytes);
        if r.u32()? != RESULT_MAGIC {
            return Err("result magic");
        }
        let seq = r.u64()?;
        let parent = r.u32()?;
        let v0 = r.u64()?;
        let kind = match r.u8()? {
            0 => NativeKind::Apply,
            1 => NativeKind::ApplyPartial,
            2 => NativeKind::Route,
            3 => NativeKind::G2Residual,
            4 => NativeKind::Abandoned,
            5 => NativeKind::FiniteReplay,
            _ => return Err("invalid native kind"),
        };
        let error_kind = match r.u8()? {
            0 => ErrorKind::None,
            1 => ErrorKind::Cancelled,
            2 => ErrorKind::ConsumerStop,
            3 => ErrorKind::NativeFailure,
            4 => ErrorKind::Conversion,
            5 => ErrorKind::Other,
            _ => return Err("invalid error kind"),
        };
        let break_reason = BreakReason::of(r.u8()?)?;
        let panic = r.flag()?;
        let mut numbers = [0u64; 12];
        for slot in &mut numbers {
            *slot = r.u64()?;
        }
        let seconds = r.f64()?;
        let stats_json = r.bytes()?.to_vec();
        let error = r.opt_str()?;
        let frontiers = (0..r.u32()?)
            .map(|_| r.bytes().map(<[u8]>::to_vec))
            .collect::<Decoded<Vec<_>>>()?;
        let refusals = (0..r.u32()?)
            .map(|_| r.bytes().map(<[u8]>::to_vec))
            .collect::<Decoded<Vec<_>>>()?;
        let refusals_truncated = r.flag()?;
        let scope = if r.flag()? {
            Some(Scope {
                anchor: r.u32()?,
                cut: r.i64()?,
                residual: r.powers()?,
            })
        } else {
            None
        };
        let g2 = if r.flag()? {
            let kind = r.u8()?;
            let n = r.u32()? as usize;
            let mut anchors = Vec::new();
            anchors
                .try_reserve(n.min(1 << 20))
                .map_err(|_| "anchor allocation")?;
            for _ in 0..n {
                anchors.push((r.u32()?, r.u64()?, r.u8()?));
            }
            let n = r.u32()? as usize;
            let mut pieces = Vec::new();
            pieces
                .try_reserve(n.min(1 << 20))
                .map_err(|_| "piece allocation")?;
            for _ in 0..n {
                let d_lo = r.opt_i64()?;
                let d_hi = r.opt_i64()?;
                let lower = (0..N).map(|_| r.u16()).collect::<Decoded<Vec<_>>>()?;
                let upper = (0..N).map(|_| r.u16()).collect::<Decoded<Vec<_>>>()?;
                pieces.push(super::anchors::Piece {
                    d_lo,
                    d_hi,
                    lower,
                    upper,
                });
            }
            Some(G2Part {
                kind,
                anchors,
                pieces,
            })
        } else {
            None
        };
        let lookup = if r.flag()? {
            Some(LookupReport::decode(&mut r)?)
        } else {
            None
        };
        let count = r.u32()?;
        let mut misses = Vec::new();
        misses
            .try_reserve(count as usize)
            .map_err(|_| "miss allocation")?;
        for _ in 0..count {
            misses.push(Miss {
                ordinal: r.u32()?,
                digest: r.u64()?,
                image: read_image::<N>(&mut r)?,
                target: if r.flag()? { Some(r.u32()?) } else { None },
            });
        }
        let finite_replay = if kind == NativeKind::FiniteReplay {
            let version = r.u32()?;
            let mut limit =
                || usize::try_from(r.u64()?).map_err(|_| "finite replay allowance range");
            let recipe = super::super::finite_replay::Recipe {
                version,
                limits: super::super::OwnerDomainWalkFiniteReplayLimits {
                    max_nodes: limit()?,
                    max_rule_applications: limit()?,
                    max_transport_calls: limit()?,
                    max_transport_operations: limit()?,
                    max_transport_endpoints: limit()?,
                    max_coalescing_additions: limit()?,
                    max_positive_layers: limit()?,
                    max_seed_points: limit()?,
                    max_seed_bytes: limit()?,
                },
            };
            recipe.validate()?;
            Some(recipe)
        } else {
            None
        };
        r.finish()?;
        let [
            emitted,
            accepted,
            stats_events,
            successors,
            conditional,
            known_reuse,
            job_duplicates,
            optional_total,
            optional_original,
            optional_coalesced,
            route_masks,
            route_joint_pruned,
        ] = numbers;
        Ok(Self {
            seq,
            parent,
            v0,
            kind,
            error_kind,
            break_reason,
            panic,
            emitted,
            accepted,
            stats_events,
            successors,
            conditional,
            known_reuse,
            job_duplicates,
            optional: [optional_total, optional_original, optional_coalesced],
            route_masks,
            route_joint_pruned,
            seconds,
            stats_json,
            error,
            frontiers,
            refusals,
            refusals_truncated,
            scope,
            g2,
            finite_replay,
            lookup,
            misses,
        })
    }
}
