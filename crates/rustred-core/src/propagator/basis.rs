//! The basis layer: terminals onto the Forcer masters (owner: M2-T9 rr-basis).
//!
//! A library reduces onto its terminals: `Unit` (the trivial node, id 0) and
//! the residuals of its Custom owners. [`PBasisLayer`] is the library's
//! [`PBasisMap`]: one row per terminal, each a sum of `Unit` and Forcer master
//! terms with Gamma tags and coefficients exact in `d`. With it,
//! `PLibrary::reduce(.., onto_masters = true)` returns convention C3 of the M2
//! plan: `(eps G(1,1))^-L Sum c I_M(x)` at `Q^2 = 1` in Forcer's G-scheme, which
//! is vakint's `reduced = (-1)^(Sum a + r) C_G`.
//!
//! Reference masters ([`PMasterDefinition`], plan M2 section 3.3): a master is a
//! Forcer topology with every line at power 1 (plus its eps units) and every
//! ISP at 0. Under convention C1 at `Q^2 = 1`,
//! `I_M(master) = sign (eps G(1,1))^L Master(name)`:
//! - `Master(t105) = Zt1star5(1,1,1,1,1) Q^2` (forcer-rules.h:15, :242;
//!   forcer.h:277), so `I_M(t1star05; 1,1,1,1,1+eps) = -(eps G(1,1))^2 Master(t105)`;
//! - `Master(no) = Zno(1^8,0) Q^4` (forcer-rules.h:16; forcer.h:276), so
//!   `I_M(NO; 1^8, 0) = +(eps G(1,1))^3 Master(no)`.
//!
//! [`P_BASIS_MASTERS_1_TO_3`] holds those two; [`P_BASIS_MASTERS_1_TO_4`] all
//! of Forcer's 16 (plan M3 D-M3-5), the scope-4 choice of
//! [`p_basis_masters`]. Every 4L definition has the same form,
//! `Z_topo(1,..,1, 0,..,0) = Master(name) / Q^(2w)`, read at `Q^2 = 1`; t155's
//! `Z(0,0,0,0,2,1,1,1,1,1)` (forcer-rules.h:281) is no dot: the first five
//! arguments of Forcer's t1 family are the eps units of its lines
//! (forcer-rules.h:279), so it is t1star55 at unit powers with two eps units on
//! line 2-4, the convention above. The signs and normalisations are pinned by
//! Forcer anchors (M3-T17), never only read off the source.
//!
//! Derivation ([`derive_basis_layer`], `rustred p-basis`, AD section 6): each
//! reference master whose topology is a node of the library is reduced in that
//! node's presentation onto the terminals, `X_i = Sum_j A_ij T_j + S_i` (`S_i`
//! the `Unit` part); for a square invertible `A` the rows are
//! `T = A^-1 (X - S)`, provenance `rustred`. The rows are checked at fresh `d`
//! modulo `2^61 - 1` against an independent re-reduction in `Z_p(d)`
//! arithmetic. More terminals than masters, a tag on a residual or a singular
//! `A` fail closed.
//!
//! Format `layer/p-layer.json` (schema [`P_BASIS_LAYER_SCHEMA`]): the base is
//! bound to `P_OWNER_LIBRARY_SEMANTICS`, `P_CENSUS_VERSION`,
//! `P_CLASS_KEY_VERSION` and the terminal inventory (one row per terminal, in
//! id order: class label, DAG node, key); see [`PBasisLayer::to_json`].
//! Coefficients are rational functions of `d` in text; the rows also evaluate
//! modulo a prime ([`PBasisLayer::images`]), so the map serves the `Z_p(d)`
//! replay path as well as the exact one.
//!
//! [`g_scheme_constant`] and [`forcer_g_scheme_terms`] translate Forcer's
//! `GschemeConstants(x1, x2)` and its G-scheme output into tags and masters
//! (the anchors of the tests).
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use symbolica::prelude::*;
use symbolica::try_parse;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionError, ReductionLimits,
};
use crate::solver::litered::{DEFAULT_PRIME, PrimeField, ZpSample};

use super::error::PError;
use super::kernel::bubble::{LinePower, relative_tag};
use super::kernel::carpet::carpet_tag;
use super::kernel::owner_edges;
use super::lane::LaneRoles;
use super::library::{
    PBasis, PBasisMap, PLibrary, PReductionRequest, PTerm, PTerminal, PUncoveredPolicy,
};
use super::presentation::{Presentation, PresentationKind, Slot};
use super::tag::GammaTag;
use super::topology::{Action, Fate, GraphKey, Normalised, PTopo, canonical, normalise};
use super::{
    NodeIndex, P_BASIS_LAYER_SCHEMA, P_CENSUS_VERSION, P_CLASS_KEY_VERSION,
    P_OWNER_LIBRARY_SEMANTICS, PTerminalId, PresentationId,
};

/// Forcer's 16 masters (Masters.prc names), sorted: the only names a row may
/// carry (vakint `masters::p_master_names`).
pub const FORCER_MASTER_NAMES: [&str; 16] = [
    "bebe", "cross", "fastar2", "haha", "lala", "no", "no1", "no2", "no6", "nono", "nostar5",
    "nostar6", "t105", "t124", "t145", "t155",
];

/// `Unit` in the `master` field of a row term.
const UNIT_TEXT: &str = "1";

fn malformed(detail: impl Into<String>) -> PError {
    PError::MalformedLibrary(detail.into())
}

fn algebra(error: ExactAlgebraError) -> PError {
    PError::Engine(ReductionError::ExactAlgebra(error))
}

// ---------------------------------------------------------------------------
// Reference masters
// ---------------------------------------------------------------------------

/// One reference master: a Forcer topology at unit line powers (eps units on
/// its eps lines) and ISPs at 0, and its value in Forcer's masters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PMasterDefinition {
    /// The Masters.prc name.
    pub name: &'static str,
    /// The Forcer topology of the integral (pathforcer.h).
    pub topology: &'static str,
    /// That topology in Forcer notation (`a-b` one line, each `*` one eps unit;
    /// the lines at vertex 0 and at the highest vertex are the external legs,
    /// their stars are not part of the topology), as in
    /// `rustred-topology/tests/fixtures/forcer/forcer_dag_68a30bf.tsv`.
    pub notation: &'static str,
    /// `s` of `I_M(x) = s (eps G(1,1))^L Master(name)` at `Q^2 = 1`, with `L`
    /// the loops of the topology: `(-1)^(lines)` by convention C1.
    pub sign: i8,
    /// Where the definition is read.
    pub source: &'static str,
}

const MASTER_NO: PMasterDefinition = PMasterDefinition {
    name: "no",
    topology: "nostar0",
    notation: "0-1* 3-4 2-3 1-2 1-6 5-6 4-5 3-6 2-5 4-9",
    sign: 1,
    source: "Zno(1,1,1,1,1,1,1,1,0) = BasicNOIntegral / (Q^2)^2 (forcer-rules.h:16); \
             BasicNOIntegral = Master(no) (forcer.h:276)",
};

const MASTER_T105: PMasterDefinition = PMasterDefinition {
    name: "t105",
    topology: "t1star05",
    notation: "0-1* 2-3 1-2 1-4 3-4 2-4* 3-9",
    sign: -1,
    source: "Zt1star5(1,1,1,1,1) = BasicT1Integral ep^2 / Q^2 (forcer-rules.h:15, :242); \
             BasicT1Integral = Master(t105) / ep^2 (forcer.h:277)",
};

/// The masters of the 1L-3L library (plan M2 section 3.3, pinned).
pub const P_BASIS_MASTERS_1_TO_3: [PMasterDefinition; 2] = [MASTER_NO, MASTER_T105];

/// Forcer's 16 masters, the masters of a library of scope 1-4 (plan M3
/// D-M3-5), by name as [`FORCER_MASTER_NAMES`]. Paths are relative to vakint's
/// `form_src/forcer/` (Forcer 68a30bf); the notations are those of
/// `forcer_dag_68a30bf.tsv`. `no` and `t105` are the entries of
/// [`P_BASIS_MASTERS_1_TO_3`].
pub const P_BASIS_MASTERS_1_TO_4: [PMasterDefinition; 16] = [
    PMasterDefinition {
        name: "bebe",
        topology: "bebe",
        notation: "0-1 4-5 1-4 3-5 1-3 1-2 2-4 2-3 3-4 5-9",
        sign: 1,
        source: "Z(1,1,1,1,1,1,1,1,0,0,0,0,0,0) = Master(bebe) (forcer/forcer-manual.h:9066)",
    },
    PMasterDefinition {
        name: "cross",
        topology: "cross",
        notation: "0-1 3-5 1-3 4-5 1-4 2-5 1-2 2-3 2-4 5-9",
        sign: 1,
        source: "Z(1,1,1,1,1,1,1,1,0,0,0,0,0,0) = Master(cross) (forcer/forcer-manual.h:7383)",
    },
    PMasterDefinition {
        name: "fastar2",
        topology: "fastar2",
        notation: "0-1 3-4 2-3* 1-2 1-5 4-5 3-5 2-5 4-9",
        sign: -1,
        source: "Zfastar2(1,1,1,1,1,1,1,0,0) = Master(fastar2) / Q2 (forcer/forcer-rules.h:182, \
                 FASTAR2SCHEME 1, the default; scheme 0: forcer/forcer-manual.h:665)",
    },
    PMasterDefinition {
        name: "haha",
        topology: "haha",
        notation: "0-3 5-8 4-5 3-4 2-3 1-2 1-8 5-6 1-6 4-7 2-7 6-7 8-9",
        sign: -1,
        source: "Z(1,1,1,1,1,1,1,1,1,1,1,0,0,0) = Master(haha) (forcer/forcer-manual.h:1886)",
    },
    PMasterDefinition {
        name: "lala",
        topology: "lala",
        notation: "0-3 5-8 4-5 3-4 2-3 1-2 1-8 2-4 1-4 1-5 8-9",
        sign: -1,
        source: "Z(1,1,1,1,1,1,1,1,1,0,0,0,0,0) = Master(lala) (forcer/forcer-manual.h:5008)",
    },
    MASTER_NO,
    PMasterDefinition {
        name: "no1",
        topology: "no1",
        notation: "0-1 4-8 3-4 2-3 1-2 1-5 5-6 6-7 7-8 3-7 2-6 4-5 8-9",
        sign: -1,
        source: "Z(1,1,1,1,1,1,1,1,1,1,1,0,0,0) = Master(no1) (forcer/forcer-manual.h:2594, :2687)",
    },
    PMasterDefinition {
        name: "no2",
        topology: "no2",
        notation: "0-1 5-8 4-5 2-4 1-2 1-6 3-6 3-7 7-8 5-6 3-4 2-7 8-9",
        sign: -1,
        source: "Zno2(1,1,1,1,1,1,1,1,1,1,1,0,0,0) = Master(no2) / Q2^3 (forcer/forcer-rules.h:57)",
    },
    PMasterDefinition {
        name: "no6",
        topology: "no6",
        notation: "0-1 7-8 6-7 2-6 1-2 1-7 3-7 3-4 4-8 4-6 2-3 8-9",
        sign: 1,
        source: "Z(1,1,1,1,1,1,1,1,1,1,0,0,0,0) = Master(no6) (forcer/forcer-manual.h:3851)",
    },
    PMasterDefinition {
        name: "nono",
        topology: "nono",
        notation: "0-1 4-6 2-4 1-2 1-3 3-5 5-6 4-5 3-4 2-5 6-9",
        sign: -1,
        source: "Z(1,1,1,1,1,1,1,1,1,0,0,0,0,0) = Master(nono) (forcer/forcer-manual.h:6421)",
    },
    PMasterDefinition {
        name: "nostar5",
        topology: "nostar5",
        notation: "0-1 3-4 2-3 1-2 1-6 5-6* 4-5 3-6 2-5 4-9",
        sign: 1,
        source: "Znostar5(1,1,1,1,1,1,1,1,0) = BasicNOIntegral5 / Q2^2 (forcer/forcer-rules.h:201, \
                 NOSTAR5SCHEME 1, the default; scheme 0: :196, XXX: normalization?); \
                 BasicNOIntegral5 = Master(nostar5) (forcer.h:278)",
    },
    PMasterDefinition {
        name: "nostar6",
        topology: "nostar6",
        notation: "0-1 3-4 2-3 1-2 1-6 5-6 4-5* 3-6 2-5 4-9",
        sign: 1,
        source: "Znostar6(1,1,1,1,1,1,1,1,0) = BasicNOIntegral6 / Q2^2 (forcer/forcer-rules.h:221, \
                 NOSTAR6SCHEME 1, the default; scheme 0: :216, XXX: normalization?); \
                 BasicNOIntegral6 = Master(nostar6) (forcer.h:279)",
    },
    MASTER_T105,
    PMasterDefinition {
        name: "t124",
        topology: "t1star24",
        notation: "0-1 2-3 1-2* 1-4 3-4* 2-4 3-9",
        sign: -1,
        source: "Z(0,1,0,1,0,1,1,1,1,1) = BasicT1Integral24 / Q2 * Q2^5 (forcer/forcer-rules.h:252, \
                 XXX: normalization?), i.e. Zt1star24(1,1,1,1,1) = BasicT1Integral24 / Q2 (:250); \
                 BasicT1Integral24 = Master(t124) (forcer.h:282)",
    },
    PMasterDefinition {
        name: "t145",
        topology: "t1star45",
        notation: "0-1 2-3 1-2 1-4 3-4* 2-4* 3-9",
        sign: -1,
        source: "Z(0,0,0,1,1,1,1,1,1,1) = BasicT1Integral45 / Q2 * Q2^5 (forcer/forcer-rules.h:271, \
                 XXX: normalization?), i.e. Zt1star45(1,1,1,1,1) = BasicT1Integral45 / Q2 (:269); \
                 BasicT1Integral45 = Master(t145) (forcer.h:281)",
    },
    PMasterDefinition {
        name: "t155",
        topology: "t1star55",
        notation: "0-1 2-3 1-2 1-4 3-4 2-4** 3-9",
        sign: -1,
        source: "Z(0,0,0,0,2,1,1,1,1,1) = BasicT1Integral55 / Q2 * Q2^5 (forcer/forcer-rules.h:281, \
                 XXX: normalization?), i.e. Zt1star55(1,1,1,1,1) = BasicT1Integral55 / Q2 (:279; \
                 the leading 0,0,0,0,2 are eps units: line 2-4 at 1+2eps, every line at power 1, \
                 no dot); BasicT1Integral55 = Master(t155) (forcer.h:280)",
    },
];

/// The reference masters of a library whose loops go up to `max_loops`:
/// [`P_BASIS_MASTERS_1_TO_3`] for 1-3 (unchanged since M2),
/// [`P_BASIS_MASTERS_1_TO_4`] for 4. Forcer has no masters above four loops.
pub fn p_basis_masters(max_loops: u8) -> Result<&'static [PMasterDefinition], PError> {
    match max_loops {
        1..=3 => Ok(&P_BASIS_MASTERS_1_TO_3),
        4 => Ok(&P_BASIS_MASTERS_1_TO_4),
        _ => Err(PError::Unsupported(format!(
            "no Forcer reference masters for libraries of {max_loops} loops (1-4 are known)"
        ))),
    }
}

/// A topology in Forcer notation (see [`PMasterDefinition::notation`]): the
/// internal lines, vertices numbered in order of first appearance from the two
/// leg ends `s` and `t`.
pub fn forcer_topology(notation: &str) -> Result<PTopo, PError> {
    let bad = |detail: &str| PError::Unsupported(format!("Forcer topology {notation:?}: {detail}"));
    let mut lines = Vec::new();
    for token in notation.split_whitespace() {
        let stars = token.chars().rev().take_while(|&c| c == '*').count();
        let (a, b) = token[..token.len() - stars]
            .split_once('-')
            .ok_or_else(|| bad("a line is not a-b"))?;
        let vertex = |text: &str| {
            text.parse::<u32>()
                .map_err(|_| bad("a vertex is not a number"))
        };
        let stars = u8::try_from(stars).map_err(|_| bad("too many eps units"))?;
        lines.push((vertex(a)?, vertex(b)?, stars));
    }
    let outer = lines
        .iter()
        .map(|&(a, b, _)| a.max(b))
        .max()
        .ok_or_else(|| bad("no line"))?;
    let is_leg = |&(a, b, _): &(u32, u32, u8)| a == 0 || b == 0 || a == outer || b == outer;
    let leg_end = |end: u32| -> Result<u32, PError> {
        let mut legs = lines.iter().filter(|&&(a, b, _)| a == end || b == end);
        match (legs.next(), legs.next()) {
            (Some(&(a, b, _)), None) if a != b => Ok(if a == end { b } else { a }),
            _ => Err(bad("each external vertex needs exactly one leg")),
        }
    };
    let (s, t) = (leg_end(0)?, leg_end(outer)?);
    if s == t || [s, t].iter().any(|&v| v == 0 || v == outer) {
        return Err(bad("the legs must end on two distinct internal vertices"));
    }
    let mut ids = BTreeMap::<u32, u8>::new();
    let mut id = |vertex: u32| -> Result<u8, PError> {
        let next = u8::try_from(ids.len()).map_err(|_| bad("too many vertices"))?;
        Ok(*ids.entry(vertex).or_insert(next))
    };
    let (s, t) = (id(s)?, id(t)?);
    let mut internal = Vec::new();
    for line in lines.iter().filter(|line| !is_leg(line)) {
        internal.push((id(line.0)?, id(line.1)?, line.2));
    }
    if internal.is_empty() {
        return Err(bad("no internal line"));
    }
    let n = u8::try_from(ids.len()).map_err(|_| bad("too many vertices"))?;
    Ok(PTopo::new(n, s, t, internal))
}

impl PMasterDefinition {
    /// The topology of the master, which must be in Forcer's normal form.
    pub fn graph(&self) -> Result<PTopo, PError> {
        let topo = forcer_topology(self.notation)?;
        match normalise(&topo) {
            Normalised::Topo(_, fates)
                if (fates.iter().enumerate()).all(|(i, fate)| *fate == Fate::Kept(i as u8)) =>
            {
                Ok(topo)
            }
            other => Err(PError::Unsupported(format!(
                "master {}: topology {} is not a normalised graph ({other:?})",
                self.name, self.topology
            ))),
        }
    }

    /// The `GraphKey` of the master's topology: the DAG node it lives on.
    pub fn graph_key(&self) -> Result<GraphKey, PError> {
        Ok(canonical(&self.graph()?).0)
    }

    /// The loop count `L` of `(eps G(1,1))^L`.
    pub fn loops(&self) -> Result<u8, PError> {
        Ok(self.graph()?.loops())
    }

    /// The tag `(eps G(1,1))^L` without its `(d - 3)^-L`.
    pub fn scheme_tag(&self) -> Result<GammaTag, PError> {
        scheme_tag(i16::from(self.loops()?))
    }

    /// `X = sign (eps G(1,1))^L Master(name)` in the context of a library.
    pub fn value(
        &self,
        context: &CoefficientContext,
        exact: ExactAlgebraLimits,
    ) -> Result<PTerm, PError> {
        if !FORCER_MASTER_NAMES.contains(&self.name) {
            return Err(PError::Unsupported(format!(
                "{} is not one of Forcer's 16 masters",
                self.name
            )));
        }
        let loops = self.loops()?;
        let d = context.parameter("d").ok_or(PError::ContextMismatch)?;
        let d_minus_3 = context
            .try_sub(&d, &context.integer(3), exact)
            .map_err(algebra)?;
        let mut coefficient = context.integer(i64::from(self.sign));
        for _ in 0..loops {
            coefficient = context
                .try_div(&coefficient, &d_minus_3, exact)
                .map_err(algebra)?;
        }
        Ok(PTerm {
            basis: PBasis::Master(self.name.to_owned()),
            tag: self.scheme_tag()?,
            coefficient,
        })
    }
}

/// `(eps G(1,1))^loops` without `(d - 3)^-loops`: `{(+1,+1),(-1,+2),(-2,-1)}`
/// per loop (convention C3).
fn scheme_tag(loops: i16) -> Result<GammaTag, PError> {
    GammaTag::from_pairs([(1, loops), (-1, 2 * loops), (-2, -loops)])
}

// ---------------------------------------------------------------------------
// Rows and the layer
// ---------------------------------------------------------------------------

/// Where a row comes from (the provenances of `SavedApplicationRuleProvenance`
/// that apply to a p library).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PBasisProvenance {
    /// Derived by RustRed (`T = A^-1 (X - S)`).
    Rustred,
    /// The trivial row `Unit = 1 Unit`; no relation is derived.
    Identity,
}

impl PBasisProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rustred => "rustred",
            Self::Identity => "identity",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Self::Rustred, Self::Identity]
            .into_iter()
            .find(|provenance| provenance.as_str() == text)
    }
}

/// The terminal a row maps, as the library names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PBasisTerminal {
    pub id: PTerminalId,
    /// The owner class label of a residual; `None` for `Unit`.
    pub class: Option<String>,
    /// The DAG node: the trivial node for `Unit`, the class's node for a
    /// residual; `None` in a library without a topology.
    pub node: Option<NodeIndex>,
    /// The residual key on its owner presentation; empty for `Unit`.
    pub key: Vec<i64>,
}

/// One row: `terminal = Sum terms`, each term a `Unit` or master with a
/// relative tag and a coefficient in `Q(d)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PBasisRow {
    pub terminal: PBasisTerminal,
    pub terms: Vec<PTerm>,
    pub provenance: PBasisProvenance,
}

/// `Unit` before masters, masters by name: the order of the terms of a row.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BasisKey {
    Unit,
    Master(String),
}

impl BasisKey {
    fn of(basis: &PBasis) -> Result<Self, PError> {
        match basis {
            PBasis::Unit => Ok(Self::Unit),
            PBasis::Master(name) if FORCER_MASTER_NAMES.contains(&name.as_str()) => {
                Ok(Self::Master(name.clone()))
            }
            PBasis::Master(name) => Err(malformed(format!(
                "the basis layer names {name}, which is not one of Forcer's 16 masters"
            ))),
            PBasis::Residual(id) => Err(malformed(format!(
                "a basis layer row maps onto the residual terminal {}",
                id.0
            ))),
        }
    }

    fn basis(&self) -> PBasis {
        match self {
            Self::Unit => PBasis::Unit,
            Self::Master(name) => PBasis::Master(name.clone()),
        }
    }
}

/// Terms collected by basis and tag, exact zeros dropped.
type TermSum = BTreeMap<(BasisKey, GammaTag), Coefficient>;

fn add_term(
    context: &CoefficientContext,
    exact: ExactAlgebraLimits,
    sum: &mut TermSum,
    key: (BasisKey, GammaTag),
    value: &Coefficient,
) -> Result<(), PError> {
    let next = match sum.get(&key) {
        Some(old) => context.try_add(old, value, exact).map_err(algebra)?,
        None => value.clone(),
    };
    if next.is_zero() {
        sum.remove(&key);
    } else {
        sum.insert(key, next);
    }
    Ok(())
}

/// One row modulo a prime: `(basis, tag, value)` per term.
pub type PRowImage = Vec<(PBasis, GammaTag, u64)>;

/// The basis layer of one library: row `i` maps terminal `i`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PBasisLayer {
    rows: Vec<PBasisRow>,
}

impl PBasisLayer {
    /// A layer of `rows` in `context`: row `i` names terminal `i`; row 0 is the
    /// identity row of `Unit`, every other row a residual's, provenance
    /// `rustred`; a row has at least one term, no residual term, masters only
    /// from Forcer's 16, tags within the key range, coefficients in `context`,
    /// and its terms sorted by (`Unit` then masters by name, tag) without
    /// repeats. A terminal that vanishes has one term with a zero coefficient.
    pub fn new(context: &CoefficientContext, rows: Vec<PBasisRow>) -> Result<Self, PError> {
        if rows.is_empty() {
            return Err(malformed("a basis layer without the Unit row"));
        }
        for (index, row) in rows.iter().enumerate() {
            let terminal = &row.terminal;
            if terminal.id.0 as usize != index {
                return Err(malformed(format!(
                    "basis layer row {index} names terminal {}",
                    terminal.id.0
                )));
            }
            let unit = index == 0;
            if unit != terminal.class.is_none()
                || (unit && !terminal.key.is_empty())
                || unit != (row.provenance == PBasisProvenance::Identity)
            {
                return Err(malformed(format!(
                    "basis layer row {index}: only row 0 is Unit (no class, no key, provenance identity)"
                )));
            }
            if unit
                && (row.terms.len() != 1
                    || row.terms[0].basis != PBasis::Unit
                    || !row.terms[0].tag.is_one()
                    || row.terms[0].coefficient != context.one())
            {
                return Err(malformed(
                    "the Unit row of a basis layer must be Unit = 1 Unit",
                ));
            }
            if row.terms.is_empty() {
                return Err(malformed(format!(
                    "basis layer row {index} has no term; a vanishing terminal has a zero coefficient"
                )));
            }
            let mut previous: Option<(BasisKey, &GammaTag)> = None;
            for term in &row.terms {
                let key = BasisKey::of(&term.basis)?;
                term.tag.encode_dense(&mut Vec::new())?;
                if !context.contains(&term.coefficient) {
                    return Err(PError::ContextMismatch);
                }
                if let Some(last) = &previous
                    && (&last.0, last.1) >= (&key, &term.tag)
                {
                    return Err(malformed(format!(
                        "basis layer row {index}: terms out of order or repeated"
                    )));
                }
                previous = Some((key, &term.tag));
            }
        }
        Ok(Self { rows })
    }

    pub fn rows(&self) -> &[PBasisRow] {
        &self.rows
    }

    /// The layer behind `library`'s basis map, read back through
    /// [`PBasisMap::map`] (tag one) for every terminal, with the library's
    /// terminal inventory: what a writer serialises (`layer/p-layer.json`)
    /// without knowing the map's type. `None` for a library without a map. The
    /// result must be a valid layer ([`Self::new`]): `Unit = 1 Unit`, every
    /// residual a `rustred` row.
    pub fn from_library(library: &PLibrary) -> Result<Option<Self>, PError> {
        let Some(map) = library.basis_map() else {
            return Ok(None);
        };
        let mut rows = Vec::with_capacity(library.terminals().len());
        for (index, terminal) in library.terminals().iter().enumerate() {
            let id = PTerminalId(index as u32);
            rows.push(PBasisRow {
                terminal: library_terminal(library, id, terminal)?,
                terms: map.map(id, &GammaTag::one())?,
                provenance: if index == 0 {
                    PBasisProvenance::Identity
                } else {
                    PBasisProvenance::Rustred
                },
            });
        }
        Self::new(library.coefficient_context(), rows).map(Some)
    }

    /// The distinct master names of the rows, sorted.
    pub fn masters(&self) -> Vec<String> {
        let mut names: Vec<String> = (self.rows.iter())
            .flat_map(|row| &row.terms)
            .filter_map(|term| match &term.basis {
                PBasis::Master(name) => Some(name.clone()),
                _ => None,
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// The inventory check: one row per terminal of `library`, each naming that
    /// terminal (class label, DAG node, key) as the library does.
    pub fn check_library(&self, library: &PLibrary) -> Result<(), PError> {
        let terminals = library.terminals();
        if terminals.len() != self.rows.len() {
            return Err(malformed(format!(
                "the basis layer has {} rows for {} terminals",
                self.rows.len(),
                terminals.len()
            )));
        }
        for (index, (terminal, row)) in terminals.iter().zip(&self.rows).enumerate() {
            let expected = library_terminal(library, PTerminalId(index as u32), terminal)?;
            if row.terminal != expected {
                return Err(malformed(format!(
                    "basis layer row {index} names {:?}, the library has {expected:?}",
                    row.terminal
                )));
            }
        }
        Ok(())
    }

    /// Every row modulo the sample's prime at its `d`: `(basis, tag, value)`
    /// per term. A coefficient with a pole at the sample is refused.
    pub fn images(&self, sample: &ZpSample) -> Result<Vec<PRowImage>, PError> {
        (self.rows.iter())
            .map(|row| {
                (row.terms.iter())
                    .map(|term| {
                        Ok((
                            term.basis.clone(),
                            term.tag.clone(),
                            image(sample, &term.coefficient)?,
                        ))
                    })
                    .collect()
            })
            .collect()
    }
}

/// `value` modulo the sample's prime at its `d`.
fn image(sample: &ZpSample, value: &Coefficient) -> Result<u64, PError> {
    sample
        .coefficient(value)
        .map_err(|error| PError::Unsupported(format!("modular image: {error}")))?
        .ok_or_else(|| {
            PError::Unsupported(format!(
                "the coefficient {value} has a pole at d = {} modulo {}",
                sample.base.first().copied().unwrap_or_default(),
                sample.field.prime()
            ))
        })
}

/// How `library` names `terminal` (id `id`).
fn library_terminal(
    library: &PLibrary,
    id: PTerminalId,
    terminal: &PTerminal,
) -> Result<PBasisTerminal, PError> {
    match terminal {
        PTerminal::Unit => Ok(PBasisTerminal {
            id,
            class: None,
            node: trivial_node(library),
            key: Vec::new(),
        }),
        PTerminal::Residual { owner, key } => {
            let record = library.census().classes.get(owner).ok_or_else(|| {
                malformed(format!(
                    "terminal {} is a residual of the unknown class {:016x}",
                    id.0, owner.0
                ))
            })?;
            Ok(PBasisTerminal {
                id,
                class: Some(record.key.label()),
                node: record.node,
                key: key.powers().to_vec(),
            })
        }
    }
}

/// The trivial node of the library's DAG, if it has one.
fn trivial_node(library: &PLibrary) -> Option<NodeIndex> {
    library.topology().and_then(|dag| {
        (dag.nodes.iter())
            .find(|node| node.action == Action::Trivial)
            .map(|node| NodeIndex(node.index))
    })
}

impl PBasisMap for PBasisLayer {
    /// The row of `terminal`, each term's tag added to `tag` (tags are absolute
    /// on both sides).
    fn map(&self, terminal: PTerminalId, tag: &GammaTag) -> Result<Vec<PTerm>, PError> {
        let row = self.rows.get(terminal.0 as usize).ok_or_else(|| {
            malformed(format!(
                "the basis layer has no row for terminal {}",
                terminal.0
            ))
        })?;
        (row.terms.iter())
            .map(|term| {
                let tag = tag.add(&term.tag);
                tag.encode_dense(&mut Vec::new())?;
                Ok(PTerm {
                    basis: term.basis.clone(),
                    tag,
                    coefficient: term.coefficient.clone(),
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// The p-layer.json format
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerWire {
    schema: String,
    base: BaseWire,
    parameters: Vec<String>,
    masters: Vec<String>,
    rows: Vec<RowWire>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseWire {
    #[serde(rename = "P_OWNER_LIBRARY_SEMANTICS")]
    semantics: String,
    #[serde(rename = "P_CENSUS_VERSION")]
    census_version: String,
    #[serde(rename = "P_CLASS_KEY_VERSION")]
    class_key_version: String,
    /// The number of terminals (rows).
    terminals: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowWire {
    terminal: TerminalWire,
    terms: Vec<TermWire>,
    provenance: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalWire {
    id: u32,
    class: Option<String>,
    node: Option<u32>,
    key: Vec<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TermWire {
    /// A master name, or `"1"` for `Unit`.
    master: String,
    /// `[[m, e], ..]`: `Prod Gamma(1 + m eps)^e`.
    tag: Vec<(i8, i16)>,
    coefficient: String,
}

fn base_versions() -> [(&'static str, String); 3] {
    [
        (
            "P_OWNER_LIBRARY_SEMANTICS",
            P_OWNER_LIBRARY_SEMANTICS.to_owned(),
        ),
        ("P_CENSUS_VERSION", P_CENSUS_VERSION.to_string()),
        ("P_CLASS_KEY_VERSION", P_CLASS_KEY_VERSION.to_string()),
    ]
}

/// The text form of a coefficient (Symbolica's, in `d`).
pub fn coefficient_text(value: &Coefficient) -> String {
    value.to_string()
}

/// Parse a coefficient of the layer grammar (integers, `d`, `+ - * / ^ ( )`)
/// into `context`. Anything else is refused.
pub fn parse_coefficient(
    context: &CoefficientContext,
    text: &str,
    limits: ExactAlgebraLimits,
) -> Result<Coefficient, PError> {
    let bad = |detail: String| PError::Unsupported(format!("coefficient {text:?}: {detail}"));
    if text.trim().is_empty() {
        return Err(bad("empty".to_owned()));
    }
    if !text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || " +-*/^()_".contains(c))
    {
        return Err(bad("characters outside the parameter grammar".to_owned()));
    }
    let atom = try_parse!(text, default_namespace = "rustred").map_err(|e| bad(e.to_string()))?;
    let coefficient = atom
        .as_view()
        .try_to_rational_polynomial(&Q, &Z, Some(context.variables().clone()))
        .map_err(|e| bad(format!("not a rational function: {e}")))?;
    if !context.contains(&coefficient) {
        return Err(bad(format!(
            "symbols outside the parameters {:?}",
            context.parameter_names()
        )));
    }
    context
        .validate_with_limits(&coefficient, limits)
        .map_err(algebra)?;
    Ok(coefficient)
}

impl PBasisLayer {
    /// `layer/p-layer.json`: pretty JSON with a final newline, deterministic.
    /// Fields: `schema`, `base` (the bound versions and the terminal count),
    /// `parameters` (`["d"]`), `masters` (the master names used), `rows`
    /// (`terminal {id, class, node, key}`, `terms [{master | "1", tag [[m,e]..],
    /// coefficient}]`, `provenance`).
    pub fn to_json(&self) -> String {
        let [semantics, census, class] = base_versions();
        let wire = LayerWire {
            schema: P_BASIS_LAYER_SCHEMA.to_owned(),
            base: BaseWire {
                semantics: semantics.1,
                census_version: census.1,
                class_key_version: class.1,
                terminals: self.rows.len(),
            },
            parameters: vec!["d".to_owned()],
            masters: self.masters(),
            rows: (self.rows.iter())
                .map(|row| RowWire {
                    terminal: TerminalWire {
                        id: row.terminal.id.0,
                        class: row.terminal.class.clone(),
                        node: row.terminal.node.map(|node| node.0),
                        key: row.terminal.key.clone(),
                    },
                    terms: (row.terms.iter())
                        .map(|term| TermWire {
                            master: match &term.basis {
                                PBasis::Master(name) => name.clone(),
                                _ => UNIT_TEXT.to_owned(),
                            },
                            tag: term.tag.exponents().to_vec(),
                            coefficient: coefficient_text(&term.coefficient),
                        })
                        .collect(),
                    provenance: row.provenance.as_str().to_owned(),
                })
                .collect(),
        };
        serde_json::to_string_pretty(&wire).expect("the basis layer serialises") + "\n"
    }

    /// Read `p-layer.json` into `context`, failing closed: the schema and every
    /// bound version must be this RustRed's (`PError::VersionMismatch`), the
    /// parameters `["d"]`, the master list the rows' own, every row valid
    /// ([`Self::new`]). Bind it to a library with [`Self::check_library`].
    pub fn from_json(text: &str, context: &CoefficientContext) -> Result<Self, PError> {
        let wire: LayerWire = serde_json::from_str(text)
            .map_err(|error| malformed(format!("p-layer.json: {error}")))?;
        let mismatch = |name: &str, found: &str, expected: &str| PError::VersionMismatch {
            name: name.to_owned(),
            found: found.to_owned(),
            expected: expected.to_owned(),
        };
        if wire.schema != P_BASIS_LAYER_SCHEMA {
            return Err(mismatch(
                "P_BASIS_LAYER_SCHEMA",
                &wire.schema,
                P_BASIS_LAYER_SCHEMA,
            ));
        }
        let [semantics, census, class] = base_versions();
        for ((name, expected), found) in [semantics, census, class].iter().zip([
            &wire.base.semantics,
            &wire.base.census_version,
            &wire.base.class_key_version,
        ]) {
            if found != expected {
                return Err(mismatch(name, found, expected));
            }
        }
        if wire.parameters != context.parameter_names() {
            return Err(PError::ContextMismatch);
        }
        if wire.base.terminals != wire.rows.len() {
            return Err(malformed(format!(
                "p-layer.json: base names {} terminals, {} rows",
                wire.base.terminals,
                wire.rows.len()
            )));
        }
        let limits = ExactAlgebraLimits::default();
        let mut rows = Vec::with_capacity(wire.rows.len());
        for row in wire.rows {
            let provenance = PBasisProvenance::parse(&row.provenance).ok_or_else(|| {
                malformed(format!(
                    "p-layer.json: unknown provenance {:?}",
                    row.provenance
                ))
            })?;
            let terms = (row.terms.into_iter())
                .map(|term| {
                    Ok(PTerm {
                        basis: if term.master == UNIT_TEXT {
                            PBasis::Unit
                        } else {
                            PBasis::Master(term.master)
                        },
                        tag: GammaTag::from_pairs(term.tag.iter().copied())?,
                        coefficient: parse_coefficient(context, &term.coefficient, limits)?,
                    })
                })
                .collect::<Result<_, PError>>()?;
            rows.push(PBasisRow {
                terminal: PBasisTerminal {
                    id: PTerminalId(row.terminal.id),
                    class: row.terminal.class,
                    node: row.terminal.node.map(NodeIndex),
                    key: row.terminal.key,
                },
                terms,
                provenance,
            });
        }
        let layer = Self::new(context, rows)?;
        if layer.masters() != wire.masters {
            return Err(malformed(format!(
                "p-layer.json: master list {:?}, the rows use {:?}",
                wire.masters,
                layer.masters()
            )));
        }
        Ok(layer)
    }
}

// ---------------------------------------------------------------------------
// Derivation (`rustred p-basis`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PBasisOptions {
    /// The reference masters; those whose topology is not a node of the
    /// library are skipped (reported in [`PBasisDerivation::absent`]).
    pub masters: Vec<PMasterDefinition>,
    pub limits: ReductionLimits,
    /// Fresh `d` values modulo `2^61 - 1` of the re-reduction check.
    pub check_points: usize,
    /// Seed of the check points.
    pub seed: u64,
}

/// The 1L-3L masters ([`P_BASIS_MASTERS_1_TO_3`]); a scope 1-4 library needs
/// [`PBasisOptions::for_library`] (with the default it fails closed: two
/// masters for its sixteen residuals).
impl Default for PBasisOptions {
    fn default() -> Self {
        Self {
            masters: P_BASIS_MASTERS_1_TO_3.to_vec(),
            limits: ReductionLimits::default(),
            check_points: 2,
            seed: 0x7105_0000_0000_b0a5,
        }
    }
}

impl PBasisOptions {
    /// The defaults with the masters of [`p_basis_masters`]`(max_loops)`.
    pub fn for_max_loops(max_loops: u8) -> Result<Self, PError> {
        Ok(Self {
            masters: p_basis_masters(max_loops)?.to_vec(),
            ..Self::default()
        })
    }

    /// [`Self::for_max_loops`] at the top loop count of `library`'s scope.
    pub fn for_library(library: &PLibrary) -> Result<Self, PError> {
        Self::for_max_loops(*library.scope().end())
    }
}

/// Where a reference master sits in a library, and what it reduces to.
#[derive(Clone, Debug)]
pub struct PMasterSite {
    pub master: PMasterDefinition,
    pub node: NodeIndex,
    /// The node's presentation.
    pub presentation: PresentationId,
    /// [`node_corner`] of the presentation.
    pub key: Vec<i64>,
    /// `sign (eps G(1,1))^L Master(name)`.
    pub value: PTerm,
    /// The exact reduction of the key onto the terminals.
    pub reduction: Vec<PTerm>,
}

/// One fresh-`d` check: every master's `Sum_j A_ij T_j + S_i = X_i`, with
/// `A`, `S` from a `Z_p(d)` re-reduction, evaluated modulo `prime` at `d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PFreshCheck {
    pub prime: u64,
    pub d: u64,
    /// The `(master site, basis, tag)` components compared.
    pub components: usize,
}

#[derive(Clone, Debug)]
pub struct PBasisDerivation {
    pub layer: PBasisLayer,
    pub sites: Vec<PMasterSite>,
    /// Reference masters whose topology is not a node of the library.
    pub absent: Vec<&'static str>,
    pub checks: Vec<PFreshCheck>,
}

/// SplitMix64: the deterministic draw of the fresh check points.
fn split_mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A master on a node of the library, before its reduction.
struct Found {
    master: PMasterDefinition,
    node: NodeIndex,
    presentation: PresentationId,
    key: Vec<i64>,
}

/// A node's integral at unit powers on a presentation of that node: 1 on each
/// line of the node's graph (on an induced presentation, the lines of its
/// sector; elsewhere every line), 0 on every other slot. A master's site key.
pub fn node_corner(presentation: &Presentation) -> Vec<i64> {
    let sector = match presentation.kind {
        PresentationKind::Induced { mask, .. } => mask,
        _ => u64::MAX,
    };
    (presentation.slots.iter().enumerate())
        .map(|(slot, kind)| match kind {
            Slot::Line(_) if sector >> slot & 1 == 1 => 1,
            _ => 0,
        })
        .collect()
}

/// The master sites of `library`: each master of `options` whose topology is a
/// node of the library's DAG, on the first presentation of that node; and the
/// names of the others.
fn master_sites(
    library: &PLibrary,
    options: &PBasisOptions,
) -> Result<(Vec<Found>, Vec<&'static str>), PError> {
    let mut sites = Vec::new();
    let mut absent = Vec::new();
    let dag = library.topology();
    for master in &options.masters {
        let key = master.graph_key()?;
        let Some(node) = dag.and_then(|dag| dag.find(&key)) else {
            absent.push(master.name);
            continue;
        };
        let presentation = (library.presentations().iter())
            .find(|p| p.node.as_ref().is_some_and(|r| r.index.0 == node.index))
            .ok_or_else(|| {
                malformed(format!(
                    "the node {} of master {} has no presentation",
                    node.index, master.name
                ))
            })?;
        // The eps units line by line, as a multiset (t155: one line with two).
        let key = node_corner(presentation);
        let mut slot_units: Vec<u8> = (presentation.slots.iter().zip(&key))
            .filter_map(|(slot, &power)| match slot {
                Slot::Line(line) if power == 1 => Some(line.eps_units),
                _ => None,
            })
            .collect();
        let graph = master.graph()?;
        let mut graph_units: Vec<u8> = graph.lines.iter().map(|line| line.2).collect();
        slot_units.sort_unstable();
        graph_units.sort_unstable();
        if slot_units != graph_units || presentation.loops != graph.loops() {
            return Err(malformed(format!(
                "presentation {} of master {} does not have the master's loops and eps units \
                 ({slot_units:?} on its lines, {graph_units:?} on the master's)",
                presentation.id.0, master.name
            )));
        }
        sites.push(Found {
            master: *master,
            node: NodeIndex(node.index),
            presentation: presentation.id,
            key,
        });
    }
    Ok((sites, absent))
}

fn reduce_site(
    library: &PLibrary,
    presentation: PresentationId,
    key: &[i64],
    arithmetic: OrderedWeightArithmetic,
    limits: &ReductionLimits,
) -> Result<Vec<PTerm>, PError> {
    let key = IntegralKey::try_new(key.iter().copied())
        .map_err(|error| PError::Unsupported(format!("master key {key:?}: {error:?}")))?;
    let reduction = library.reduce(PReductionRequest {
        inputs: vec![(presentation, key, library.coefficient_context().one())],
        arithmetic,
        scheduler: OrderedSchedulerOptions::default(),
        step_cache: None,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters: false,
        limits: *limits,
        inspector: None,
    })?;
    Ok(reduction.terms)
}

/// `X - S` and the `A` row of one site: residual terms must carry tag one.
fn split_reduction(
    site: &PMasterSite,
    reduction: &[PTerm],
    residuals: &BTreeMap<PTerminalId, usize>,
) -> Result<(Vec<Option<Coefficient>>, Vec<PTerm>), PError> {
    let mut row = vec![None; residuals.len()];
    let mut unit = Vec::new();
    for term in reduction {
        match &term.basis {
            PBasis::Unit => unit.push(term.clone()),
            PBasis::Residual(id) => {
                if !term.tag.is_one() {
                    return Err(PError::Unsupported(format!(
                        "master {} reduces onto terminal {} with the Gamma tag {:?}: \
                         tagged residual systems are not supported",
                        site.master.name,
                        id.0,
                        term.tag.exponents()
                    )));
                }
                let column = residuals.get(id).ok_or_else(|| {
                    malformed(format!(
                        "master {} reduces onto terminal {}, which is not a residual",
                        site.master.name, id.0
                    ))
                })?;
                row[*column] = Some(term.coefficient.clone());
            }
            PBasis::Master(name) => {
                return Err(malformed(format!(
                    "a reduction onto terminals returned the master {name}"
                )));
            }
        }
    }
    Ok((row, unit))
}

/// `A^-1` of a square matrix over `Q(d)` (Gauss-Jordan); `None` entries are 0.
fn invert(
    context: &CoefficientContext,
    exact: ExactAlgebraLimits,
    matrix: Vec<Vec<Option<Coefficient>>>,
    names: &[String],
) -> Result<Vec<Vec<Coefficient>>, PError> {
    let n = matrix.len();
    let mut a: Vec<Vec<Coefficient>> = (matrix.into_iter())
        .map(|row| {
            row.into_iter()
                .map(|c| c.unwrap_or_else(|| context.zero()))
                .collect()
        })
        .collect();
    let mut inverse: Vec<Vec<Coefficient>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    if i == j {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect();
    for column in 0..n {
        let pivot = (column..n)
            .find(|&row| !a[row][column].is_zero())
            .ok_or_else(|| {
                PError::Unsupported(format!(
                    "the reference masters do not determine the residual terminal {}: \
                 the master reductions are singular in it",
                    names[column]
                ))
            })?;
        a.swap(column, pivot);
        inverse.swap(column, pivot);
        let scale = a[column][column].clone();
        for j in 0..n {
            a[column][j] = context
                .try_div(&a[column][j], &scale, exact)
                .map_err(algebra)?;
            inverse[column][j] = context
                .try_div(&inverse[column][j], &scale, exact)
                .map_err(algebra)?;
        }
        for row in 0..n {
            if row == column || a[row][column].is_zero() {
                continue;
            }
            let factor = a[row][column].clone();
            for j in 0..n {
                let product = context
                    .try_mul(&factor, &a[column][j], exact)
                    .map_err(algebra)?;
                a[row][j] = context
                    .try_sub(&a[row][j], &product, exact)
                    .map_err(algebra)?;
                let product = context
                    .try_mul(&factor, &inverse[column][j], exact)
                    .map_err(algebra)?;
                inverse[row][j] = context
                    .try_sub(&inverse[row][j], &product, exact)
                    .map_err(algebra)?;
            }
        }
    }
    Ok(inverse)
}

/// The basis layer of `library` (plan M2-T9; AD section 6): reduce every
/// reference master that sits on a node of the library onto the terminals,
/// solve `T = A^-1 (X - S)` exactly, and check the rows at
/// `options.check_points` fresh `d` modulo `2^61 - 1` against a `Z_p(d)`
/// re-reduction. Fails closed when the masters do not match the residual
/// terminals one to one (`A` not square or singular), when a residual is
/// reached with a Gamma tag, or when a check fails.
pub fn derive_basis_layer(
    library: &PLibrary,
    options: &PBasisOptions,
) -> Result<PBasisDerivation, PError> {
    let context = library.coefficient_context();
    let exact = options.limits.exact_algebra;
    let (found, absent) = master_sites(library, options)?;

    let mut residuals = BTreeMap::new();
    let mut names = Vec::new();
    for (index, terminal) in library.terminals().iter().enumerate() {
        if let PTerminal::Residual { owner, key } = terminal {
            residuals.insert(PTerminalId(index as u32), residuals.len());
            let label = (library.census().classes.get(owner))
                .map(|record| record.key.label())
                .unwrap_or_else(|| format!("{:016x}", owner.0));
            names.push(format!("{index} ({label} {:?})", key.powers()));
        }
    }
    if found.len() != residuals.len() {
        let sites: Vec<_> = found.iter().map(|site| site.master.name).collect();
        return Err(PError::Unsupported(format!(
            "{} reference masters {sites:?} for {} residual terminals {names:?}: \
             p-basis needs a square system (relations between residuals are M3, AD section 6 step 3)",
            found.len(),
            residuals.len()
        )));
    }

    let mut sites = Vec::with_capacity(found.len());
    let mut matrix = Vec::with_capacity(found.len());
    let mut units = Vec::with_capacity(found.len());
    for Found {
        master,
        node,
        presentation,
        key,
    } in found
    {
        let reduction = reduce_site(
            library,
            presentation,
            &key,
            OrderedWeightArithmetic::Exact,
            &options.limits,
        )?;
        let site = PMasterSite {
            master,
            node,
            presentation,
            key,
            value: master.value(context, exact)?,
            reduction,
        };
        let (row, unit) = split_reduction(&site, &site.reduction, &residuals)?;
        matrix.push(row);
        units.push(unit);
        sites.push(site);
    }
    let inverse = invert(context, exact, matrix, &names)?;

    // Row j: T_j = Sum_i inverse[j][i] (X_i - S_i).
    let mut rows = vec![PBasisRow {
        terminal: library_terminal(library, PTerminalId(0), &PTerminal::Unit)?,
        terms: vec![PTerm {
            basis: PBasis::Unit,
            tag: GammaTag::one(),
            coefficient: context.one(),
        }],
        provenance: PBasisProvenance::Identity,
    }];
    for (index, terminal) in library.terminals().iter().enumerate().skip(1) {
        let id = PTerminalId(index as u32);
        let column = *residuals
            .get(&id)
            .ok_or_else(|| malformed(format!("terminal {index} is a second Unit")))?;
        let mut sum = TermSum::new();
        for (i, site) in sites.iter().enumerate() {
            let weight = &inverse[column][i];
            if weight.is_zero() {
                continue;
            }
            let value = context
                .try_mul(weight, &site.value.coefficient, exact)
                .map_err(algebra)?;
            add_term(
                context,
                exact,
                &mut sum,
                (BasisKey::of(&site.value.basis)?, site.value.tag.clone()),
                &value,
            )?;
            for term in &units[i] {
                let value = context
                    .try_mul(weight, &term.coefficient, exact)
                    .map_err(algebra)?;
                let value = context.try_neg(&value, exact).map_err(algebra)?;
                add_term(
                    context,
                    exact,
                    &mut sum,
                    (BasisKey::Unit, term.tag.clone()),
                    &value,
                )?;
            }
        }
        if sum.is_empty() {
            return Err(malformed(format!(
                "residual terminal {} maps to zero",
                names[column]
            )));
        }
        rows.push(PBasisRow {
            terminal: library_terminal(library, id, terminal)?,
            terms: (sum.into_iter())
                .map(|((basis, tag), coefficient)| PTerm {
                    basis: basis.basis(),
                    tag,
                    coefficient,
                })
                .collect(),
            provenance: PBasisProvenance::Rustred,
        });
    }
    let layer = PBasisLayer::new(context, rows)?;
    let checks = fresh_checks(library, options, &layer, &sites, &residuals)?;
    Ok(PBasisDerivation {
        layer,
        sites,
        absent,
        checks,
    })
}

/// The check of [`derive_basis_layer`]: each site re-reduced in `Z_p(d)`
/// arithmetic (one thread) must give the exact reduction again, and at
/// `options.check_points` fresh `d` modulo `2^61 - 1` its
/// `Sum_j A_ij row_j + S_i` must equal `X_i` component by component.
fn fresh_checks(
    library: &PLibrary,
    options: &PBasisOptions,
    layer: &PBasisLayer,
    sites: &[PMasterSite],
    residuals: &BTreeMap<PTerminalId, usize>,
) -> Result<Vec<PFreshCheck>, PError> {
    let failed = |detail: String| PError::Generation(format!("p-basis fresh-d check: {detail}"));
    let modular = OrderedWeightArithmetic::ModularPolynomial(
        ModularCommon {
            threads: 1,
            ..Default::default()
        },
        PolynomialOptions::default(),
    );
    let mut replays = Vec::with_capacity(sites.len());
    for site in sites {
        let replay = reduce_site(
            library,
            site.presentation,
            &site.key,
            modular,
            &options.limits,
        )?;
        if replay != site.reduction {
            return Err(failed(format!(
                "master {}: the Z_p(d) re-reduction differs from the exact one",
                site.master.name
            )));
        }
        replays.push(split_reduction(site, &replay, residuals)?);
    }
    let field = PrimeField::new(DEFAULT_PRIME)
        .map_err(|error| PError::Unsupported(format!("prime field: {error}")))?;
    let mut state = options.seed;
    let mut checks = Vec::with_capacity(options.check_points);
    let mut attempts = 0;
    while checks.len() < options.check_points {
        attempts += 1;
        if attempts > 64 * options.check_points.max(1) {
            return Err(failed("no sample point without a pole".to_owned()));
        }
        let d = split_mix(&mut state) % DEFAULT_PRIME;
        let sample = ZpSample::new(field.clone(), vec![d]);
        let Ok(rows) = layer.images(&sample) else {
            continue;
        };
        let mut components = 0;
        let mut pole = false;
        for (site, (row, unit)) in sites.iter().zip(&replays) {
            let mut lhs = BTreeMap::<(BasisKey, GammaTag), u64>::new();
            let mut add = |key: (BasisKey, GammaTag), value: u64| {
                let entry = lhs.entry(key).or_insert(0);
                *entry = field.add(*entry, value);
            };
            for (id, column) in residuals {
                let Some(weight) = &row[*column] else {
                    continue;
                };
                let Ok(weight) = image(&sample, weight) else {
                    pole = true;
                    continue;
                };
                for (basis, tag, value) in &rows[id.0 as usize] {
                    add(
                        (BasisKey::of(basis)?, tag.clone()),
                        field.mul(weight, *value),
                    );
                }
            }
            for term in unit {
                let Ok(value) = image(&sample, &term.coefficient) else {
                    pole = true;
                    continue;
                };
                add((BasisKey::Unit, term.tag.clone()), value);
            }
            lhs.retain(|_, value| *value != 0);
            let Ok(value) = image(&sample, &site.value.coefficient) else {
                pole = true;
                continue;
            };
            let rhs = BTreeMap::from([(
                (BasisKey::of(&site.value.basis)?, site.value.tag.clone()),
                value,
            )]);
            components += lhs.len().max(rhs.len());
            if !pole && lhs != rhs {
                return Err(failed(format!(
                    "master {} at d = {d} modulo {DEFAULT_PRIME}: {lhs:?} != {rhs:?}",
                    site.master.name
                )));
            }
        }
        if pole {
            continue;
        }
        checks.push(PFreshCheck {
            prime: DEFAULT_PRIME,
            d,
            components,
        });
    }
    Ok(checks)
}

/// `library` again with `layer` as its basis map: the same presentations,
/// topology, census, edge maps, terminals and owner programs (each program is
/// built once on `library` and shared, so a certified assembly stays the
/// certified one), checked against the layer's terminal inventory first. The
/// identity, and so the step cache, is unchanged: the map acts after the
/// engine. A loader with the layer in hand sets it on its builder instead.
pub fn with_basis_layer(library: &PLibrary, layer: Arc<PBasisLayer>) -> Result<PLibrary, PError> {
    layer.check_library(library)?;
    let mut builder = PLibrary::builder(library.coefficient_context().clone(), library.scope());
    for presentation in library.presentations() {
        builder.add_presentation(presentation.as_ref().clone())?;
    }
    if let Some(dag) = library.topology() {
        builder.set_topology(Arc::clone(dag))?;
    }
    builder.set_census(library.census().clone())?;
    for (index, map) in library.edge_maps().iter().enumerate() {
        if builder.add_edge_map(map.clone())? != index {
            return Err(malformed(
                "edge maps renumbered while attaching the basis layer",
            ));
        }
    }
    for (index, terminal) in library.terminals().iter().enumerate().skip(1) {
        if builder.add_terminal(terminal.clone())?.0 as usize != index {
            return Err(malformed(
                "terminals renumbered while attaching the basis layer",
            ));
        }
    }
    for owner in library.owners() {
        let program = owner.program(library)?.clone();
        builder.add_owner_with_program(owner.spec.clone(), program)?;
    }
    builder.set_basis_map(layer)?;
    builder.build()
}

/// [`derive_basis_layer`] then [`with_basis_layer`]: `library` with its own
/// derived layer as basis map, and the derivation (sites, checks, rows to
/// write as `layer/p-layer.json`). What assembly does after building a
/// program.
pub fn with_derived_basis_layer(
    library: &PLibrary,
    options: &PBasisOptions,
) -> Result<(PLibrary, PBasisDerivation), PError> {
    let derivation = derive_basis_layer(library, options)?;
    let mapped = with_basis_layer(library, Arc::new(derivation.layer.clone()))?;
    Ok((mapped, derivation))
}

// ---------------------------------------------------------------------------
// The Gamma tag range (plan M3-T17 item 6)
// ---------------------------------------------------------------------------

/// The largest `|m|` of a `Gamma(1 + m eps)` that a step of `library`
/// multiplies in: the tag `T_ins(j1, j2)` of every owner with Insertion roles
/// (the Insertion and SeriesParallel lanes; `j` of its two bubble lines) and
/// with Carpet roles (`j_inner = j_third + j_blob + L_blob` and `j_r`, as
/// `CarpetStep::new` computes them); the other lanes multiply no Gamma
/// function, and 0 if no owner does. A reduction's tags are products of these
/// (exponents add, `m` stays) and the layer's `(eps G(1,1))^L` has `|m| <= 2`,
/// so every tag of a reduction onto masters has `|m|` at most the larger of
/// this count and 2. A tag beyond `P_GAMMA_TAG_MAX_M` is
/// `PError::TagOutOfRange` here, as in the step itself.
pub fn largest_gamma_m(library: &PLibrary) -> Result<i8, PError> {
    let units = |presentation: &Presentation, slot: u16| -> Result<i64, PError> {
        match presentation.slots.get(usize::from(slot)) {
            Some(Slot::Line(line)) => Ok(i64::from(line.eps_units)),
            _ => Err(malformed(format!(
                "role slot {slot} of presentation {} is not a line",
                presentation.id.0
            ))),
        }
    };
    let mut largest = 0_i8;
    for owner in library.owners() {
        let tag = match owner.spec.roles {
            LaneRoles::Insertion(roles) => {
                let parent = library.presentation(owner.spec.presentation);
                relative_tag(
                    units(parent, roles.lines[0])?,
                    units(parent, roles.lines[1])?,
                )?
            }
            LaneRoles::Carpet(roles) => {
                let (parent, edges) = owner_edges(library, &owner.spec)?;
                let [(edge, child)] = edges.as_slice() else {
                    return Err(malformed(format!(
                        "the carpet owner on presentation {} has {} edge maps",
                        parent.id.0,
                        edges.len()
                    )));
                };
                let mut blob = 0;
                for (slot, target) in parent.slots.iter().zip(&edge.line_slots) {
                    if let (Slot::Line(line), Some(_)) = (slot, target) {
                        blob += i64::from(line.eps_units);
                    }
                }
                let third = match roles.third {
                    Some(third) => units(&parent, third)?,
                    None => 0,
                };
                let inner = third + blob + i64::from(child.loops);
                carpet_tag(
                    LinePower::new(1, inner),
                    LinePower::new(1, units(&parent, roles.r)?),
                )?
            }
            LaneRoles::None
            | LaneRoles::Triangle(_)
            | LaneRoles::Diamond(_)
            | LaneRoles::Scheme(_) => continue,
        };
        for &(m, _) in tag.exponents() {
            largest = largest.max(m.abs());
        }
    }
    Ok(largest)
}

// ---------------------------------------------------------------------------
// Forcer's G-scheme output
// ---------------------------------------------------------------------------

/// `GschemeConstants(x1, x2) = (1 + x1 + x2) G(1 + x1 eps, 1 + x2 eps) / (eps G(1,1))`
/// (Masters.prc; vakint `finalize::pg_value`) as a tag and a rational factor:
/// `Gamma(1+(1+s)eps) Gamma(1-(1+x1)eps) Gamma(1-(1+x2)eps) Gamma(1-2eps)
/// / (Gamma(1+x1 eps) Gamma(1+x2 eps) Gamma(1-(2+s)eps) Gamma(1+eps) Gamma(1-eps)^2)`
/// times `(1 - 2 eps) / (eps (1 - (2+s) eps))`, `s = x1 + x2`, `eps = (4-d)/2`.
pub fn g_scheme_constant(
    context: &CoefficientContext,
    x1: i8,
    x2: i8,
    exact: ExactAlgebraLimits,
) -> Result<(GammaTag, Coefficient), PError> {
    let s = x1.checked_add(x2).ok_or(PError::TagOutOfRange)?;
    let m = |value: i16| i8::try_from(value).map_err(|_| PError::TagOutOfRange);
    let (x1w, x2w, sw) = (i16::from(x1), i16::from(x2), i16::from(s));
    let tag = GammaTag::from_pairs([
        (m(1 + sw)?, 1),
        (m(-1 - x1w)?, 1),
        (m(-1 - x2w)?, 1),
        (-2, 1),
        (x1, -1),
        (x2, -1),
        (m(-2 - sw)?, -1),
        (1, -1),
        (-1, -2),
    ])?;
    let d = context.parameter("d").ok_or(PError::ContextMismatch)?;
    let eps = context
        .try_div(
            &context
                .try_sub(&context.integer(4), &d, exact)
                .map_err(algebra)?,
            &context.integer(2),
            exact,
        )
        .map_err(algebra)?;
    let linear = |c: i64| -> Result<Coefficient, PError> {
        let term = context
            .try_mul(&context.integer(c), &eps, exact)
            .map_err(algebra)?;
        context
            .try_add(&context.one(), &term, exact)
            .map_err(algebra)
    };
    let numerator = linear(-2)?;
    let denominator = context
        .try_mul(&eps, &linear(-2 - i64::from(s))?, exact)
        .map_err(algebra)?;
    let factor = context
        .try_div(&numerator, &denominator, exact)
        .map_err(algebra)?;
    Ok((tag, factor))
}

/// A Forcer G-scheme output (`rat(P, Q)` with `P`, `Q` polynomials in `ep`,
/// `GschemeConstants(x1,x2)` and `Master(name)` factors with integer powers,
/// summed) as terms in the (`Unit` or master, tag) basis with coefficients in
/// `d`, sorted, exact zeros dropped. Every other factor is refused.
pub fn forcer_g_scheme_terms(
    context: &CoefficientContext,
    output: &str,
    exact: ExactAlgebraLimits,
) -> Result<Vec<PTerm>, PError> {
    let bad = |detail: String| PError::Unsupported(format!("Forcer output {output:?}: {detail}"));
    let text: String = output.chars().filter(|c| !c.is_whitespace()).collect();
    let mut sum = TermSum::new();
    for (negative, term) in split_top_level(&text, &['+', '-']).map_err(&bad)? {
        if term.is_empty() {
            return Err(bad("an empty term".to_owned()));
        }
        let mut coefficient = context.integer(if negative { -1 } else { 1 });
        let mut tag = GammaTag::one();
        let mut master: Option<String> = None;
        for (_, factor) in split_top_level(term, &['*']).map_err(&bad)? {
            let (base, power) = match factor.rsplit_once('^') {
                Some((base, power)) if !power.contains(')') => (
                    base,
                    power
                        .parse::<i16>()
                        .map_err(|_| bad(format!("power {power:?}")))?,
                ),
                _ => (factor, 1),
            };
            if let Some(arguments) = call(base, "GschemeConstants") {
                let (x1, x2) = arguments
                    .split_once(',')
                    .ok_or_else(|| bad(format!("factor {factor:?}")))?;
                let index = |x: &str| {
                    x.parse::<i8>()
                        .map_err(|_| bad(format!("factor {factor:?}")))
                };
                let (g, rational) = g_scheme_constant(context, index(x1)?, index(x2)?, exact)?;
                for _ in 0..power.unsigned_abs() {
                    if power > 0 {
                        tag = tag.add(&g);
                        coefficient = context
                            .try_mul(&coefficient, &rational, exact)
                            .map_err(algebra)?;
                    } else {
                        tag = tag.add(&g.neg());
                        coefficient = context
                            .try_div(&coefficient, &rational, exact)
                            .map_err(algebra)?;
                    }
                }
            } else if let Some(name) = call(base, "Master") {
                if power != 1 || master.is_some() {
                    return Err(bad("a term with more than one master".to_owned()));
                }
                master = Some(name.to_owned());
            } else if let Some(arguments) = call(base, "rat") {
                let parts = split_top_level(arguments, &[',']).map_err(&bad)?;
                let [(_, numerator), (_, denominator)] = parts.as_slice() else {
                    return Err(bad(format!("factor {factor:?}")));
                };
                let in_d = |polynomial: &str| {
                    parse_coefficient(context, &polynomial.replace("ep", "((4-d)/2)"), exact)
                };
                let rational = context
                    .try_div(&in_d(numerator)?, &in_d(denominator)?, exact)
                    .map_err(algebra)?;
                for _ in 0..power.unsigned_abs() {
                    coefficient = if power > 0 {
                        context.try_mul(&coefficient, &rational, exact)
                    } else {
                        context.try_div(&coefficient, &rational, exact)
                    }
                    .map_err(algebra)?;
                }
            } else if let Ok(value) = base.parse::<i64>() {
                for _ in 0..power.unsigned_abs() {
                    coefficient = if power > 0 {
                        context.try_mul(&coefficient, &context.integer(value), exact)
                    } else {
                        context.try_div(&coefficient, &context.integer(value), exact)
                    }
                    .map_err(algebra)?;
                }
            } else {
                return Err(bad(format!("unknown factor {factor:?}")));
            }
        }
        let basis = match master {
            None => BasisKey::Unit,
            Some(name) => BasisKey::of(&PBasis::Master(name))?,
        };
        add_term(context, exact, &mut sum, (basis, tag), &coefficient)?;
    }
    Ok((sum.into_iter())
        .map(|((basis, tag), coefficient)| PTerm {
            basis: basis.basis(),
            tag,
            coefficient,
        })
        .collect())
}

/// `text` split at the separators outside parentheses; each part with whether
/// a `-` preceded it.
fn split_top_level<'a>(text: &'a str, separators: &[char]) -> Result<Vec<(bool, &'a str)>, String> {
    let mut parts = Vec::new();
    let (mut depth, mut start, mut negative) = (0_i32, 0, false);
    let mut previous = None;
    for (index, c) in text.char_indices() {
        let exponent_sign = c == '-' && previous == Some('^');
        previous = Some(c);
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err("unbalanced parentheses".to_owned());
                }
            }
            c if depth == 0 && separators.contains(&c) && !exponent_sign => {
                if index > start || !parts.is_empty() || c != '-' {
                    parts.push((negative, &text[start..index]));
                }
                negative = c == '-';
                start = index + c.len_utf8();
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unbalanced parentheses".to_owned());
    }
    parts.push((negative, &text[start..]));
    Ok(parts)
}

/// The argument text of `name(...)`.
fn call<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
}

impl fmt::Display for PBasisProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Forcer fixture cases (vakint `tests/fixtures/forcer/*.json`, schema of
/// `ForcerCase`) as library inputs and expected `onto_masters` terms: the
/// anchors of the M2-T9 tests, shared by the core and app test binaries.
#[doc(hidden)]
pub mod anchors {
    use std::collections::BTreeMap;

    use serde::Deserialize;

    use super::{BasisKey, TermSum, add_term, forcer_g_scheme_terms, malformed, parse_coefficient};
    use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
    use crate::family::IntegralKey;
    use crate::propagator::error::PError;
    use crate::propagator::library::PTerm;
    use crate::propagator::presentation::{Presentation, Slot};
    use crate::propagator::{PresentationId, tag::GammaTag};

    /// `a.b` with `a`, `b` over `(k_1..k_L, q)`.
    pub type ScalarProduct = (Vec<i64>, Vec<i64>);

    /// One weighted input of a reduction request.
    pub type PInput = (PresentationId, IntegralKey, Coefficient);

    /// One fixture file.
    #[derive(Clone, Debug, Deserialize)]
    pub struct ForcerFixture {
        pub forcer_commit: String,
        pub form_version: String,
        #[serde(default)]
        pub provenance: Option<String>,
        pub cases: Vec<ForcerCase>,
    }

    /// One case: a vakint family, its line powers (ISPs at 0), a numerator in
    /// `dot(k(i),p(1))`, `dot(k(i),k(j))` monomials, the canonical `p(1)` in
    /// the user's head (`p(1)` or `-p(1)`) and Forcer's raw outputs `C_G`.
    #[derive(Clone, Debug, Deserialize)]
    pub struct ForcerCase {
        pub name: String,
        pub family: String,
        pub powers: Vec<i64>,
        pub numerator: String,
        pub external: String,
        pub outputs: Vec<String>,
    }

    pub fn parse_fixture(json: &str) -> Result<ForcerFixture, PError> {
        serde_json::from_str(json).map_err(|error| malformed(format!("Forcer fixture: {error}")))
    }

    impl ForcerCase {
        fn bad(&self, detail: impl std::fmt::Display) -> PError {
            PError::Unsupported(format!("Forcer case {}: {detail}", self.name))
        }

        /// The numerator's scalar products over `(k_1..k_L, q)`, with the
        /// user's `p(1) = sign q`.
        pub fn scalar_products(&self, loops: u8) -> Result<Vec<ScalarProduct>, PError> {
            let sign = match self.external.as_str() {
                "p(1)" => 1,
                "-p(1)" => -1,
                other => return Err(self.bad(format!("external {other}"))),
            };
            let width = usize::from(loops) + 1;
            let vector = |text: &str| -> Result<Vec<i64>, PError> {
                let mut row = vec![0_i64; width];
                if text == "p(1)" {
                    row[width - 1] = sign;
                    return Ok(row);
                }
                let index = (text.strip_prefix("k("))
                    .and_then(|t| t.strip_suffix(')'))
                    .and_then(|t| t.parse::<usize>().ok())
                    .filter(|&i| (1..width).contains(&i))
                    .ok_or_else(|| self.bad(format!("momentum {text}")))?;
                row[index - 1] = 1;
                Ok(row)
            };
            let mut products = Vec::new();
            let numerator: String = self
                .numerator
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if numerator == "1" {
                return Ok(products);
            }
            for factor in numerator.split('*') {
                let (base, power) = match factor.rsplit_once(")^") {
                    Some((base, power)) => (
                        format!("{base})"),
                        power
                            .parse::<usize>()
                            .map_err(|_| self.bad(format!("factor {factor}")))?,
                    ),
                    None => (factor.to_owned(), 1),
                };
                let (a, b) = (base.strip_prefix("dot("))
                    .and_then(|t| t.strip_suffix(')'))
                    .and_then(|t| t.split_once("),"))
                    .ok_or_else(|| self.bad(format!("factor {factor}")))?;
                let (a, b) = (vector(&format!("{a})"))?, vector(b)?);
                if a[..width - 1]
                    .iter()
                    .chain(&b[..width - 1])
                    .all(|&c| c == 0)
                {
                    return Err(self.bad(format!("factor {factor} has no loop momentum")));
                }
                for _ in 0..power {
                    products.push((a.clone(), b.clone()));
                }
            }
            Ok(products)
        }

        /// The case on presentation `p` (its family's top): the powers times the
        /// scalar products, lowered onto the slots by `express_scalar_product`
        /// and expanded.
        pub fn inputs(
            &self,
            context: &CoefficientContext,
            p: &Presentation,
        ) -> Result<Vec<PInput>, PError> {
            let exact = ExactAlgebraLimits::default();
            let algebra = super::algebra;
            let lines = (p.slots.iter())
                .filter(|s| matches!(s, Slot::Line(_)))
                .count();
            if self.powers.len() != lines {
                return Err(self.bad(format!("{} powers for {lines} lines", self.powers.len())));
            }
            let mut base = self.powers.clone();
            base.resize(p.slots.len(), 0);
            let mut polynomial: BTreeMap<Vec<i64>, Coefficient> =
                BTreeMap::from([(vec![0_i64; p.slots.len()], context.one())]);
            for (a, b) in self.scalar_products(p.loops)? {
                let form = p.express_scalar_product(&a, &b)?;
                let mut next: BTreeMap<Vec<i64>, Coefficient> = BTreeMap::new();
                for (exponents, weight) in &polynomial {
                    let mut terms = Vec::new();
                    if !form.constant.is_zero() {
                        terms.push((exponents.clone(), form.constant.to_string()));
                    }
                    for (slot, factor) in &form.terms {
                        let mut raised = exponents.clone();
                        raised[usize::from(*slot)] += 1;
                        terms.push((raised, factor.to_string()));
                    }
                    for (exponents, factor) in terms {
                        let factor = parse_coefficient(context, &factor, exact)?;
                        let value = context.try_mul(weight, &factor, exact).map_err(algebra)?;
                        let entry = next.entry(exponents).or_insert_with(|| context.zero());
                        *entry = context.try_add(entry, &value, exact).map_err(algebra)?;
                    }
                }
                next.retain(|_, value| !value.is_zero());
                polynomial = next;
            }
            (polynomial.into_iter())
                .map(|(exponents, weight)| {
                    let powers = base.iter().zip(&exponents).map(|(b, e)| b - e);
                    let key = IntegralKey::try_new(powers)
                        .map_err(|error| self.bad(format!("key: {error:?}")))?;
                    Ok((p.id, key, weight))
                })
                .collect()
        }

        /// vakint's `reduced = (-1)^(Sum a + r) C_G` in the (`Unit` or master,
        /// tag) basis: `Sum a` over the positive line powers, `r` the number of
        /// scalar products (each has a loop momentum), the outputs summed.
        pub fn expected(
            &self,
            context: &CoefficientContext,
            loops: u8,
        ) -> Result<Vec<PTerm>, PError> {
            let exact = ExactAlgebraLimits::default();
            let positive: i64 = self.powers.iter().filter(|&&a| a > 0).sum();
            let rank = self.scalar_products(loops)?.len() as i64;
            let sign = context.integer(if (positive + rank) % 2 == 0 { 1 } else { -1 });
            let mut sum = TermSum::new();
            for output in &self.outputs {
                for term in forcer_g_scheme_terms(context, output, exact)? {
                    let value = context
                        .try_mul(&term.coefficient, &sign, exact)
                        .map_err(super::algebra)?;
                    add_term(
                        context,
                        exact,
                        &mut sum,
                        (BasisKey::of(&term.basis)?, term.tag),
                        &value,
                    )?;
                }
            }
            Ok((sum.into_iter())
                .map(
                    |((basis, tag), coefficient): ((BasisKey, GammaTag), Coefficient)| PTerm {
                        basis: basis.basis(),
                        tag,
                        coefficient,
                    },
                )
                .collect())
        }
    }
}
