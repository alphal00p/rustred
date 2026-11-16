//! Typed failures of the p library (owner: M2-I0 rr-iface). Every gap is one
//! of these, never a silent zero (plan M2 section 2 rule 6).
use std::fmt;
use std::sync::Arc;

use crate::family::IntegralKey;
use crate::reduction::ReductionError;
use crate::sector::symmetry::integral_transport;

use super::{OwnerClassId, PresentationId};

/// vk5l WP1: the first bound of a boxed node's box (`dots-rank-v1`) that a
/// key violates ([`super::table_import::BoxDefinition::violation`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoxViolation {
    /// The key has another slot count than the box's chart.
    Arity { found: usize, expected: usize },
    /// A line slot below power 1 or an ISP slot above 0: the key is not on
    /// the box's chart (a subsector, or another sector).
    OffChart { slot: usize, power: i64, line: bool },
    /// A line power above the box's cap.
    LineCap { slot: usize, power: i64, cap: i64 },
    /// The dots `D` of no layer of the box.
    Dots { dots: i64, layers: Vec<u32> },
    /// The rank above the layer's `r_max` (`lines` = `E_S`, the chart's lines).
    Rank {
        dots: i64,
        rank: i64,
        r_max: u32,
        lines: usize,
    },
}

impl fmt::Display for BoxViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity { found, expected } => {
                write!(formatter, "the key has {found} slots, the box {expected}")
            }
            Self::OffChart { slot, power, line } => {
                if *line {
                    write!(
                        formatter,
                        "line slot {slot} power {power} < 1 (off the box's chart)"
                    )
                } else {
                    write!(
                        formatter,
                        "ISP slot {slot} power {power} > 0 (off the box's chart)"
                    )
                }
            }
            Self::LineCap { slot, power, cap } => {
                write!(formatter, "line slot {slot} power {power} > cap {cap}")
            }
            Self::Dots { dots, layers } => {
                write!(
                    formatter,
                    "D = {dots} is no layer of the box (D in {layers:?})"
                )
            }
            Self::Rank {
                dots,
                rank,
                r_max,
                lines,
            } => write!(
                formatter,
                "D = {dots}: r = {rank} > r_max = {r_max} (E_S = {lines})"
            ),
        }
    }
}

/// vk5l WP1: one input of the request a typed refusal comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PRequestInput {
    pub presentation: PresentationId,
    /// The presentation's family name.
    pub family: String,
    pub powers: Vec<i64>,
}

impl fmt::Display for PRequestInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let powers: Vec<String> = self.powers.iter().map(i64::to_string).collect();
        write!(
            formatter,
            "{}[p{}] I({})",
            self.family,
            self.presentation.0,
            powers.join(",")
        )
    }
}

/// vk5l WP1: the original request of a typed refusal
/// ([`PError::OutsideBox`], [`PError::OutsideCertifiedSet`],
/// [`PError::NodeNotClosed`]), filled by `PLibrary::reduce`, and the command
/// line that reproduces it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PRequestContext {
    /// The library identity.
    pub library: String,
    pub inputs: Vec<PRequestInput>,
    /// A shell command line that runs the same request (the inputs at weight
    /// one) on the same package (`p5l_library reduce`).
    pub reproduce: String,
}

impl PRequestContext {
    /// The inputs on one line (at most 8, then a count).
    pub fn inputs_text(&self) -> String {
        let shown: Vec<String> = self
            .inputs
            .iter()
            .take(8)
            .map(ToString::to_string)
            .collect();
        let more = self.inputs.len().saturating_sub(8);
        if more > 0 {
            format!("{} (+{more} more)", shown.join(" + "))
        } else {
            shown.join(" + ")
        }
    }
}

fn request_suffix(request: &Option<Arc<PRequestContext>>) -> String {
    match request {
        Some(request) => format!(
            "; request: {}; reproduce: {}",
            request.inputs_text(),
            request.reproduce
        ),
        None => String::new(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PError {
    /// The named M2 task has not landed in this build.
    NotImplemented(&'static str),
    Unsupported(String),
    MissingOwner {
        class: OwnerClassId,
        ledger: u32,
    },
    Uncovered {
        class: OwnerClassId,
        key: IntegralKey,
    },
    /// boxed-node: a key of a boxed node's chart outside its box: the node's
    /// table covers only the box, so the key has no rule there (never a
    /// value). `node` and `definition` describe the box. vk5l WP1: also the
    /// early check of a request input on a boxed node (before any step);
    /// `violated` is the first violated bound, `request` the original request
    /// (filled by `PLibrary::reduce`).
    OutsideBox {
        class: OwnerClassId,
        key: IntegralKey,
        node: String,
        definition: String,
        violated: Option<Box<BoxViolation>>,
        request: Option<Arc<PRequestContext>>,
    },
    /// vk5l WP1 (plan VK5L §3.4, decision Q1): a key of a boxed node's chart
    /// outside its CERTIFIED KEY SET (the forward closure its table is
    /// certified on): a hard error, never a partial reduction. `violated` is
    /// the box bound the key violates (`None`: inside the box, a hand-down the
    /// closure does not contain).
    OutsideCertifiedSet {
        class: OwnerClassId,
        key: IntegralKey,
        node: String,
        /// The user box (name and layers) for the message.
        definition: String,
        violated: Option<Box<BoxViolation>>,
        certified_keys: u64,
        request: Option<Arc<PRequestContext>>,
    },
    /// vk5l WP1 (plan VK5L §3.6, decision Q8): a key reached a node the
    /// package declares NOT CLOSED (no table, e.g. T266).
    NodeNotClosed {
        class: OwnerClassId,
        node: String,
        key: IntegralKey,
        status: String,
        request: Option<Arc<PRequestContext>>,
    },
    /// vk5l WP2 (plan VK5L §3.3): a recipe of a boxed node failed at run
    /// time (no relation in its seeds, outside its own domain, a budget, a
    /// proof or certified-digest mismatch): node, recipe id, the key being
    /// solved, the kind, the verb's failure object, the original request, one
    /// rule chain and a reproduction command. Never a silent fallback.
    RecipeFailed(Box<super::recipes::PRecipeFailure>),
    /// One request mixes loop counts.
    MixedDimension,
    /// A Gamma tag outside `|m| <= P_GAMMA_TAG_MAX_M`, `|e| <= 127`.
    TagOutOfRange,
    /// A coefficient context other than the library's shared `{d}`.
    ContextMismatch,
    VersionMismatch {
        name: String,
        found: String,
        expected: String,
    },
    MalformedLibrary(String),
    /// A presentation or edge map that breaks its lane contract.
    Contract(String),
    Transport(integral_transport::Error),
    Generation(String),
    Engine(ReductionError),
    /// A generation job ran out of its budget (M3-I1, plan M3 D-M3-4):
    /// `resource` is `"time"` or `"rss"`; nothing of the job is kept.
    BudgetExceeded {
        resource: &'static str,
        detail: String,
    },
}

impl fmt::Display for PError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented(task) => {
                write!(formatter, "not implemented in this build ({task})")
            }
            Self::Unsupported(detail) => write!(formatter, "unsupported: {detail}"),
            Self::MissingOwner { class, ledger } => write!(
                formatter,
                "owner class {:016x} has no program (ledger entry {ledger})",
                class.0
            ),
            Self::Uncovered { class, key } => write!(
                formatter,
                "owner class {:016x} does not cover {:?}",
                class.0,
                key.powers()
            ),
            Self::OutsideBox {
                class,
                key,
                node,
                definition,
                violated,
                request,
            } => write!(
                formatter,
                "owner class {:016x} (boxed node {node}) does not cover {:?}: the key lies outside its {definition}{}{}",
                class.0,
                key.powers(),
                violated
                    .as_ref()
                    .map_or_else(String::new, |bound| format!(" ({bound})")),
                request_suffix(request)
            ),
            Self::OutsideCertifiedSet {
                class,
                key,
                node,
                definition,
                violated,
                certified_keys,
                request,
            } => write!(
                formatter,
                "boxed node {node} (owner class {:016x}) cannot reduce {:?}: {}, and the key is not in the \
                 certified set ({certified_keys} keys); live repair: off{}",
                class.0,
                key.powers(),
                match violated {
                    Some(bound) => format!("{bound}, outside its {definition}"),
                    None => format!(
                        "inside its {definition}, but not in the certified closure (a hand-down not certified)"
                    ),
                },
                request_suffix(request)
            ),
            Self::NodeNotClosed {
                class,
                node,
                key,
                status,
                request,
            } => write!(
                formatter,
                "the p library has no closed reduction for node {node} (owner class {:016x}, status: {status}); \
                 key {:?}{}",
                class.0,
                key.powers(),
                request_suffix(request)
            ),
            Self::RecipeFailed(failure) => write!(formatter, "{failure}"),
            Self::MixedDimension => {
                formatter.write_str("one p reduction request mixes loop counts")
            }
            Self::TagOutOfRange => {
                formatter.write_str("Gamma tag out of range (|m| <= 8, |e| <= 127, 16 dense slots)")
            }
            Self::ContextMismatch => {
                formatter.write_str("the p library requires one coefficient context {d}")
            }
            Self::VersionMismatch {
                name,
                found,
                expected,
            } => write!(
                formatter,
                "p library generated with {name}={found}, this RustRed has {expected}; \
                 regenerate with rustred p-package"
            ),
            Self::MalformedLibrary(detail) => write!(formatter, "malformed p library: {detail}"),
            Self::Contract(detail) => write!(formatter, "lane contract violated: {detail}"),
            Self::Transport(error) => write!(formatter, "transport: {error}"),
            Self::Generation(detail) => write!(formatter, "generation: {detail}"),
            Self::Engine(error) => write!(formatter, "engine: {error}"),
            Self::BudgetExceeded { resource, detail } => {
                write!(formatter, "{resource} budget exceeded: {detail}")
            }
        }
    }
}

impl PError {
    /// vk5l WP1: the same error with the original request recorded (only the
    /// typed refusals that carry one; a request already recorded is kept).
    pub fn with_request(mut self, context: &Arc<PRequestContext>) -> Self {
        if let Self::RecipeFailed(failure) = &mut self
            && failure.request.is_none()
        {
            failure.request = Some(Arc::clone(context));
            return self;
        }
        if let Self::OutsideBox { request, .. }
        | Self::OutsideCertifiedSet { request, .. }
        | Self::NodeNotClosed { request, .. } = &mut self
            && request.is_none()
        {
            *request = Some(Arc::clone(context));
        }
        self
    }

    /// vk5l WP1: the request recorded on a typed refusal.
    pub fn request(&self) -> Option<&PRequestContext> {
        match self {
            Self::OutsideBox { request, .. }
            | Self::OutsideCertifiedSet { request, .. }
            | Self::NodeNotClosed { request, .. } => request.as_deref(),
            Self::RecipeFailed(failure) => failure.request.as_deref(),
            _ => None,
        }
    }

    /// vk5l WP1: a key the request may keep as a symbolic residual under
    /// `PUncoveredPolicy::Retain` (the class and key of an uncovered key, a
    /// key outside a box or certified set, a node not closed, a missing
    /// owner has no key).
    pub fn retainable(&self) -> Option<(OwnerClassId, &IntegralKey)> {
        match self {
            Self::Uncovered { class, key }
            | Self::OutsideBox { class, key, .. }
            | Self::OutsideCertifiedSet { class, key, .. }
            | Self::NodeNotClosed { class, key, .. } => Some((*class, key)),
            _ => None,
        }
    }
}

/// The engine and transport failures keep their cause, so a refused resource
/// budget (a cancelled request) stays recognisable through the chain.
impl std::error::Error for PError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ReductionError> for PError {
    fn from(error: ReductionError) -> Self {
        Self::Engine(error)
    }
}

impl From<integral_transport::Error> for PError {
    fn from(error: integral_transport::Error) -> Self {
        Self::Transport(error)
    }
}
