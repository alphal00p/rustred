//! Owner-class keys and ids (types: M2-I0 rr-iface; classify and route: M2-T1 rr-class).
//!
//! The class of a sector is the canonical projector graph of its support
//! `raw ∪ forced` (`sector::symmetry::discovery`): a marked-matroid
//! invariant of the glued graph with the external line marked and every line
//! coloured by its power shift, so 2-isomorphic (Whitney-twisted) sectors share
//! a class, and an eps line never shares one with an integer line.
//!
//! [`OwnerClassKey::canonical`] is written here, field by field:
//!
//! ```text
//! loops u8, lines u8, eps_lines u8, vertices u16, edges u16
//! vertices in canonical order, each a tag and its colour:
//!   0 line centre    mass, projector diagonal, shift a, shift b (shift = a + b d)
//!   1 external       index u16, projector diagonal
//!   2 orientation
//! edges sorted by (min, max, colour), each min u16, max u16, then
//!   0 incidence | 1 projector, signed value
//! ```
//!
//! Integers are little endian, a rational is `u16` length + ASCII
//! `numerator/denominator` in lowest terms. Never bincode or Symbolica's native
//! encoding: they embed the global symbol table.
//!
//! # Versioning
//!
//! Bump [`P_CLASS_KEY_VERSION`] and refresh the golden id list of
//! `tests/p_library/class.rs` whenever one sector's bytes could change: this
//! format, the graph of `discovery` (rows, colours, their order), or the
//! canonical form of the vendored Symbolica (`graph.canonize`). A bump renames
//! every class and so changes the cache identity of each library program. The
//! golden list is what catches a change nobody meant to make.
use std::sync::Arc;

use symbolica::domains::SelfRing;
use symbolica::prelude::Rational;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::sector::Mask;
use crate::sector::symmetry::discovery::{
    self, DiscoveryCache, DiscoveryError, EdgeColor, PreparedSupport, ShiftAdmission, VertexColor,
};
use crate::sector::symmetry::{
    self, CoefficientMatrix, DenominatorAction, Jacobian, MomentumMap, VerifiedMap,
};

use super::census::SignedAffineLoopMap;
use super::error::PError;
use super::presentation::Presentation;
use super::{OwnerClassId, P_CLASS_KEY_VERSION, PresentationId};

/// Marked-matroid class of a (presentation, support) pair: own deterministic
/// bytes, never Symbolica's native encoding (plan M2 section 3.1).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OwnerClassKey {
    pub loops: u8,
    pub lines: u8,
    pub eps_lines: u8,
    pub canonical: Arc<[u8]>,
}

impl OwnerClassKey {
    /// The low 64 bits (first eight digest bytes, little endian) of
    /// `blake3(P_CLASS_KEY_VERSION LE || canonical)`. A name, not a proof:
    /// distinct bytes under one id are `PError::MalformedLibrary`.
    pub fn id(&self) -> OwnerClassId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&P_CLASS_KEY_VERSION.to_le_bytes());
        hasher.update(&self.canonical);
        let digest = hasher.finalize();
        let (low, _) = digest
            .as_bytes()
            .split_first_chunk::<8>()
            .expect("a blake3 digest has 32 bytes");
        OwnerClassId(u64::from_le_bytes(*low))
    }

    /// Display form `L3T08-<16 hex>`.
    pub fn label(&self) -> String {
        format!("L{}T{:02}-{:016x}", self.loops, self.lines, self.id().0)
    }
}

/// Resource policy of classification and route verification.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClassLimits {
    pub symmetry: symmetry::Limits,
}

/// One labelled sector with its class key. `support` = `raw` plus the forced
/// eps axes of the presentation.
#[derive(Clone, Debug)]
pub struct ClassedSector {
    pub presentation: PresentationId,
    pub raw: u64,
    pub support: u64,
    pub key: OwnerClassKey,
    prepared: Arc<PreparedSupport>,
}

/// The class of `raw` in `p`; `None` when the sector has no class geometry
/// (its lines do not span the loop momenta). `raw` is a mask of line slots: a
/// positive power on an ISP slot is no sector of `p`.
pub fn classify(
    p: &Presentation,
    raw: u64,
    limits: &ClassLimits,
) -> Result<Option<ClassedSector>, PError> {
    let line_slots = p.lines().fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
    if raw & !line_slots != 0 {
        return Err(PError::Contract(format!(
            "raw mask {raw:#x} of presentation {} leaves its line slots {line_slots:#x}",
            p.id.0
        )));
    }
    let mask = Mask::try_new((0..p.slots.len()).map(|slot| raw >> slot & 1 == 1))
        .map_err(|error| PError::Contract(format!("presentation {}: {error}", p.id.0)))?;
    let mut cache = DiscoveryCache::with_admission(Arc::clone(&p.family), ShiftAdmission::Forced);
    let prepared = cache
        .prepare_support(&mask, limits.symmetry, &never)
        .map_err(discovery_error)?;
    let Some(prepared) = prepared else {
        return Ok(None);
    };
    let support = bits(prepared.root());
    if support != raw | p.forced_axes() {
        return Err(PError::Contract(format!(
            "presentation {}: the power shifts of the family force {support:#x}, not {:#x}",
            p.id.0,
            raw | p.forced_axes()
        )));
    }
    let key = class_key(p, support, &prepared)?;
    Ok(Some(ClassedSector {
        presentation: p.id,
        raw,
        support,
        key,
        prepared,
    }))
}

/// The verified map from `source` to `owner`: unit Jacobian, `q -> +-q`,
/// forced axes onto equal-shift axes. `None` when no such map exists; equal
/// keys are necessary for one, not sufficient.
pub fn route(
    source: &ClassedSector,
    owner: &ClassedSector,
    limits: &ClassLimits,
) -> Result<Option<SignedAffineLoopMap>, PError> {
    check_contexts(source, owner)?;
    if source.key != owner.key {
        return Ok(None);
    }
    let proposal =
        discovery::match_supports(&source.prepared, &owner.prepared, limits.symmetry, &never)
            .map_err(discovery_error)?;
    let Some(verified) = proposal else {
        return Ok(None);
    };
    admit(source, owner, &verified)
}

/// [`route`] for a map that did not come from discovery, such as a stored
/// census route: the same verification and admission, so the result is
/// `candidate` itself or `None`.
pub fn verify_route(
    source: &ClassedSector,
    owner: &ClassedSector,
    candidate: &SignedAffineLoopMap,
    limits: &ClassLimits,
) -> Result<Option<SignedAffineLoopMap>, PError> {
    check_contexts(source, owner)?;
    let (from, to) = (source.prepared.family(), owner.prepared.family());
    if usize::from(candidate.loops_in) != from.loop_count()
        || usize::from(candidate.loops_out) != to.loop_count()
        || !candidate.is_unimodular()
    {
        return Ok(None);
    }
    let momentum = momentum_map(candidate, from.coefficient_context())?;
    let verified = symmetry::verify(from, to, momentum, limits.symmetry)
        .map_err(|error| PError::Unsupported(format!("route verification: {error}")))?;
    admit(source, owner, &verified)
}

/// The admission of one verified map as a route, the only way to one. Discovery
/// colours already keep shifts on their lines; `verify` knows nothing of
/// shifts, so they are compared here, explicitly and exactly, on every line of
/// the support (critique-eng S6): an integer line cannot land on a forced axis
/// either.
fn admit(
    source: &ClassedSector,
    owner: &ClassedSector,
    verified: &VerifiedMap,
) -> Result<Option<SignedAffineLoopMap>, PError> {
    let resolved = verified
        .nonzero_conditions()
        .iter()
        .all(|condition| condition.polynomial().is_constant() && !condition.polynomial().is_zero());
    if !matches!(verified.jacobian(), Jacobian::Unit { .. }) || !resolved {
        return Ok(None);
    }
    let source_shifts = source.prepared.family().power_shifts();
    let owner_shifts = owner.prepared.family().power_shifts();
    let mut images = 0_u64;
    for slot in (0..u64::BITS as usize).filter(|slot| source.support >> slot & 1 == 1) {
        let DenominatorAction::Monomial { target, scale } = &verified.row_actions()[slot] else {
            return Ok(None);
        };
        let image = 1_u64 << target;
        if !scale.is_one()
            || owner.support & image == 0
            || images & image != 0
            || source_shifts[slot] != owner_shifts[*target]
        {
            return Ok(None);
        }
        images |= image;
    }
    if images != owner.support {
        return Ok(None);
    }
    Ok(integer_map(verified.momentum()).filter(SignedAffineLoopMap::is_unimodular))
}

fn never() -> bool {
    false
}

fn discovery_error(error: DiscoveryError) -> PError {
    match error {
        DiscoveryError::ForeignCoefficient => PError::ContextMismatch,
        other => PError::Unsupported(other.to_string()),
    }
}

fn check_contexts(source: &ClassedSector, owner: &ClassedSector) -> Result<(), PError> {
    let context = source.prepared.family().coefficient_context();
    if context.has_same_variable_map(owner.prepared.family().coefficient_context()) {
        Ok(())
    } else {
        Err(PError::ContextMismatch)
    }
}

fn bits(mask: &Mask) -> u64 {
    mask.active_bits()
        .iter()
        .enumerate()
        .fold(0, |bits, (slot, &active)| bits | u64::from(active) << slot)
}

fn momentum_map(
    map: &SignedAffineLoopMap,
    context: &CoefficientContext,
) -> Result<MomentumMap, PError> {
    let matrix = |rows: usize, columns: usize, entries: &[i64]| {
        CoefficientMatrix::try_new(
            rows,
            columns,
            entries.iter().map(|&entry| context.integer(entry)),
        )
        .map_err(|error| PError::Unsupported(format!("route verification: {error}")))
    };
    let loops = usize::from(map.loops_in);
    Ok(MomentumMap::new(
        matrix(loops, usize::from(map.loops_out), &map.loop_linear)?,
        matrix(loops, 1, &map.loop_external)?,
        matrix(1, 1, &[i64::from(map.external_sign)])?,
    ))
}

/// A map with integer entries and one external momentum, as a loop map.
fn integer_map(momentum: &MomentumMap) -> Option<SignedAffineLoopMap> {
    let integers = |matrix: &CoefficientMatrix| {
        matrix
            .entries()
            .iter()
            .map(integer_entry)
            .collect::<Option<Box<[i64]>>>()
    };
    let linear = momentum.loop_linear();
    let external_sign = integers(momentum.external_linear())?;
    let &[external_sign] = external_sign.as_ref() else {
        return None;
    };
    Some(SignedAffineLoopMap {
        loops_in: u8::try_from(linear.rows()).ok()?,
        loops_out: u8::try_from(linear.columns()).ok()?,
        loop_linear: integers(linear)?,
        loop_external: integers(momentum.loop_external())?,
        external_sign: i8::try_from(external_sign).ok()?,
    })
}

fn integer_entry(value: &Coefficient) -> Option<i64> {
    if !value.denominator.is_one() || !value.numerator.is_constant() {
        return None;
    }
    value.numerator.get_constant().to_i64()
}

fn class_key(
    p: &Presentation,
    support: u64,
    prepared: &PreparedSupport,
) -> Result<OwnerClassKey, PError> {
    let width = |count: u32, what: &str| {
        u8::try_from(count).map_err(|_| {
            PError::Unsupported(format!(
                "presentation {}: {count} {what} in one class key",
                p.id.0
            ))
        })
    };
    let lines = width(support.count_ones(), "lines")?;
    let eps_lines = width((support & p.forced_axes()).count_ones(), "eps lines")?;
    let mut bytes = vec![p.loops, lines, eps_lines];
    write_graph(prepared, &mut bytes)?;
    Ok(OwnerClassKey {
        loops: p.loops,
        lines,
        eps_lines,
        canonical: bytes.into(),
    })
}

/// The canonical graph in the format of the module docs.
fn write_graph(prepared: &PreparedSupport, bytes: &mut Vec<u8>) -> Result<(), PError> {
    let graph = prepared.canonical_graph();
    write_count(graph.nodes().len(), bytes)?;
    write_count(graph.edges().len(), bytes)?;
    for node in graph.nodes() {
        match &node.data {
            VertexColor::Center {
                mass,
                diagonal,
                shift,
            } => {
                let (constant, slope) = match shift {
                    Some(shift) => affine_in_d(&shift.0)?,
                    None => (Rational::zero(), Rational::zero()),
                };
                bytes.push(0);
                write_rational(&constant_rational(&mass.0)?, bytes)?;
                write_rational(diagonal, bytes)?;
                write_rational(&constant, bytes)?;
                write_rational(&slope, bytes)?;
            }
            VertexColor::External { index, diagonal } => {
                bytes.push(1);
                write_count(*index, bytes)?;
                write_rational(diagonal, bytes)?;
            }
            VertexColor::Orientation => bytes.push(2),
        }
    }
    let mut edges = graph
        .edges()
        .iter()
        .map(|edge| {
            let (a, b) = edge.vertices;
            let colour = match &edge.data {
                EdgeColor::Incidence => None,
                EdgeColor::Projector(value) => Some(rational_text(value)),
            };
            (a.min(b), a.max(b), colour)
        })
        .collect::<Vec<_>>();
    edges.sort_unstable();
    for (low, high, colour) in edges {
        write_count(low, bytes)?;
        write_count(high, bytes)?;
        match colour {
            None => bytes.push(0),
            Some(text) => {
                bytes.push(1);
                write_text(&text, bytes)?;
            }
        }
    }
    Ok(())
}

fn write_count(value: usize, bytes: &mut Vec<u8>) -> Result<(), PError> {
    let value = u16::try_from(value)
        .map_err(|_| PError::Unsupported(format!("class key entry {value} exceeds 16 bits")))?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_text(text: &str, bytes: &mut Vec<u8>) -> Result<(), PError> {
    write_count(text.len(), bytes)?;
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

fn write_rational(value: &Rational, bytes: &mut Vec<u8>) -> Result<(), PError> {
    write_text(&rational_text(value), bytes)
}

fn rational_text(value: &Rational) -> String {
    format!("{}/{}", value.numerator_ref(), value.denominator_ref())
}

fn constant_rational(value: &Coefficient) -> Result<Rational, PError> {
    if !value.numerator.is_constant() || !value.denominator.is_constant() {
        return Err(PError::Unsupported(format!(
            "line mass {value} depends on d"
        )));
    }
    Ok(Rational::from((
        value.numerator.get_constant(),
        value.denominator.get_constant(),
    )))
}

/// `(a, b)` of `value = a + b d`; the one parameter of a p family is `d`.
fn affine_in_d(value: &Coefficient) -> Result<(Rational, Rational), PError> {
    let unsupported = || PError::Unsupported(format!("power shift {value} is not affine in d"));
    if !value.denominator.is_constant() {
        return Err(unsupported());
    }
    let denominator = value.denominator.get_constant();
    let mut parts = [Rational::zero(), Rational::zero()];
    for term in 0..value.numerator.nterms() {
        let degree = value.numerator.exponents(term).iter().sum::<u16>();
        let part = parts.get_mut(usize::from(degree)).ok_or_else(unsupported)?;
        *part = Rational::from((
            value.numerator.coefficients[term].clone(),
            denominator.clone(),
        ));
    }
    let [constant, slope] = parts;
    Ok((constant, slope))
}
