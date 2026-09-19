//! Bounded exact Boolean/box coverage for source-port affine predicates.
//!
//! An owner is `box ∩ target ∩ !exception_1 ∩ ...`. An affine predicate
//! means its fixed coordinate face AND its exact equations; its rectangular
//! prefilter is never treated as that predicate. We expand only the finite
//! Boolean partition, using `BoxCover` for every rectangular subtraction.
//!
//! Polynomial equalities are opaque atoms, compared through Symbolica's
//! canonical polynomial equality. Requiring coverage for *every* Boolean
//! valuation is stronger than requiring it only for realizable integer
//! valuations, so this can reject a valid cover but cannot accept an invalid
//! one. The existing affine row-bound service may additionally certify that
//! a true-equation branch has no integer point in a box. Before further
//! branching, native singleton substitution can also disprove individual
//! Boolean literals throughout a remaining box. Bounded native affine rank
//! checks can also disprove jointly inconsistent equality/disequality choices.
//! No sampling or new CAS implementation is involved. Other feasibility may
//! remain unresolved and cause conservative incompleteness.

use std::{fmt, sync::Arc};

use crate::algebra::CoefficientPolynomial;
use crate::foundry::completion::{BoxCover, CompletionGeometryLimits, LatticeBox};

use super::AffineApplicationDomain;
use super::scope::EntryDegreeBound;

mod consistency;
mod diagnostic;
mod scope;
use diagnostic::PredicateCoverWitness;

struct TraversalWork<'a> {
    nodes: usize,
    consistency: consistency::RestrictionCache<'a>,
    required_domain: Option<&'a LatticeBox>,
    required_degree: Option<EntryDegreeBound>,
    scoped_geometry: scope::GeometryWork,
}

impl<'a> TraversalWork<'a> {
    #[cfg(test)]
    fn new(sector: &'a [bool], atoms: &'a [Atom<'a>]) -> Self {
        Self::with_work_limit(sector, atoms, super::DEFAULT_PREDICATE_CONSISTENCY_WORK)
    }

    fn with_work_limit(sector: &'a [bool], atoms: &'a [Atom<'a>], max_work: usize) -> Self {
        Self {
            nodes: 0,
            consistency: consistency::RestrictionCache::with_work_limit(sector, atoms, max_work),
            required_domain: None,
            required_degree: None,
            scoped_geometry: scope::GeometryWork::default(),
        }
    }
}

/// Borrowed domains of independently replayed and descending rules.
/// Coverage alone never validates the algebraic rule. Callers must supply
/// only checked owners, and repeat this check after cold replay.
pub(in crate::foundry::artifact) struct PredicateCoveragePiece<'a> {
    pub(in crate::foundry::artifact) boxes: &'a [LatticeBox],
    pub(in crate::foundry::artifact) affine_target: Option<&'a AffineApplicationDomain>,
    pub(in crate::foundry::artifact) affine_exclusions: &'a [Arc<AffineApplicationDomain>],
}

#[derive(Clone, Copy, Debug)]
pub(in crate::foundry::artifact) struct PredicateCoverLimits {
    pub(in crate::foundry::artifact) geometry: CompletionGeometryLimits,
    pub(in crate::foundry::artifact) max_predicates: usize,
    pub(in crate::foundry::artifact) max_clauses: usize,
    pub(in crate::foundry::artifact) max_boolean_nodes: usize,
    pub(in crate::foundry::artifact) max_consistency_work: usize,
}

impl Default for PredicateCoverLimits {
    fn default() -> Self {
        Self {
            geometry: CompletionGeometryLimits::default(),
            max_predicates: super::DEFAULT_PREDICATE_ATOMS,
            max_clauses: 65_536,
            max_boolean_nodes: 65_536,
            max_consistency_work: super::DEFAULT_PREDICATE_CONSISTENCY_WORK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::foundry::artifact) struct PredicateCoverCertificate {
    pub(in crate::foundry::artifact) predicates: usize,
    pub(in crate::foundry::artifact) clauses: usize,
    pub(in crate::foundry::artifact) boolean_nodes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::foundry::artifact) enum PredicateCoverError {
    InvalidDomain(&'static str),
    Budget(&'static str),
    Geometry(String),
    /// Some Boolean valuations may be unrealizable integer cases. This
    /// checker nevertheless fails closed when their boxes remain uncovered.
    Uncovered {
        boxes: usize,
        unbounded_boxes: usize,
        witness: Box<PredicateCoverWitness>,
    },
}

impl fmt::Display for PredicateCoverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDomain(detail) => write!(f, "invalid predicate-cover domain: {detail}"),
            Self::Budget(resource) => write!(f, "predicate-cover budget exhausted: {resource}"),
            Self::Geometry(detail) => write!(f, "predicate-cover geometry: {detail}"),
            Self::Uncovered {
                boxes,
                unbounded_boxes,
                witness,
            } => write!(
                f,
                "predicate cover's first failed abstract Boolean branch leaves {boxes} possibly uncovered boxes ({unbounded_boxes} unbounded); not an exhaustive complement census\n{witness}"
            ),
        }
    }
}

impl std::error::Error for PredicateCoverError {}

struct Atom<'a> {
    indices: &'a [usize],
    equation: &'a CoefficientPolynomial,
}

struct EqualityConstraint<'a> {
    domain: &'a AffineApplicationDomain,
    face: LatticeBox,
    atoms: Vec<usize>,
}

struct Clause {
    domain: LatticeBox,
    literals: Vec<(usize, bool)>,
}

impl Clause {
    fn copy_with_domain(&self, domain: LatticeBox) -> Self {
        Self {
            domain,
            literals: self.literals.clone(),
        }
    }

    /// Conjoin a literal; false means the conjunction is contradictory.
    fn require(&mut self, atom: usize, value: bool) -> bool {
        match self.literals.binary_search_by_key(&atom, |&(id, _)| id) {
            Ok(index) => self.literals[index].1 == value,
            Err(index) => {
                self.literals.insert(index, (atom, value));
                true
            }
        }
    }
}

/// Prove whole-sector coverage by a finite, tautological predicate partition.
/// Coordinate faces are intersected/subtracted geometrically. Each coupled
/// equation is split into equality/non-equality branches. Multiple exceptions
/// retain union/conjunction semantics. Affine targets may have exceptions:
/// no owner-reference edge is trusted, and every recursion fixes a previously
/// unassigned atom, so circular owner claims cannot prove coverage.
pub(in crate::foundry::artifact) fn certify_predicate_cover(
    sector: &[bool],
    owners: &[PredicateCoveragePiece<'_>],
    terminals: &[LatticeBox],
    limits: PredicateCoverLimits,
) -> Result<PredicateCoverCertificate, PredicateCoverError> {
    certify_predicate_cover_impl(sector, owners, terminals, None, None, limits)
}

/// Certify only a caller-required union, retaining infinite endpoints exactly.
/// This proves coverage, not rule validity or closure under RHS successors.
/// The caller must separately certify those obligations before publication.
/// All original owners are admitted even when the requested union is empty.
pub(in crate::foundry::artifact) fn certify_predicate_cover_within(
    sector: &[bool],
    owners: &[PredicateCoveragePiece<'_>],
    terminals: &[LatticeBox],
    required: &[LatticeBox],
    limits: PredicateCoverLimits,
) -> Result<PredicateCoverCertificate, PredicateCoverError> {
    certify_predicate_cover_impl(sector, owners, terminals, Some(required), None, limits)
}

/// Exact degree-scoped coverage without enumerating the integer simplex.
/// The rectangle below is only a traversal hull: each uncovered box must
/// intersect the actual sum-bound before it can prevent coverage. Affine
/// predicates remain attached and receive the same fail-closed checks.
/// This proves neither source identity nor successor closure.
pub(in crate::foundry::artifact) fn certify_predicate_cover_up_to_degree(
    sector: &[bool],
    owners: &[PredicateCoveragePiece<'_>],
    terminals: &[LatticeBox],
    degree: EntryDegreeBound,
    limits: PredicateCoverLimits,
) -> Result<PredicateCoverCertificate, PredicateCoverError> {
    if sector.is_empty() || sector.len() > limits.geometry.max_arity {
        return Err(PredicateCoverError::InvalidDomain("sector arity"));
    }
    if limits.geometry.max_requested_boxes == 0
        || sector
            .len()
            .checked_mul(2)
            .is_none_or(|count| count > limits.geometry.max_requested_box_coordinate_cells)
    {
        return Err(PredicateCoverError::Budget("degree coverage hull"));
    }
    let hull = LatticeBox::try_new(
        sector.iter().map(|_| 0),
        sector.iter().map(|&active| match degree {
            EntryDegreeBound::MaxNegativeIndexDegree(_) if active => None,
            _ => Some(degree.limit()),
        }),
    )
    .map_err(geometry)?;
    certify_predicate_cover_impl(
        sector,
        owners,
        terminals,
        Some(&[hull]),
        Some(degree),
        limits,
    )
}

fn certify_predicate_cover_impl(
    sector: &[bool],
    owners: &[PredicateCoveragePiece<'_>],
    terminals: &[LatticeBox],
    required: Option<&[LatticeBox]>,
    required_degree: Option<EntryDegreeBound>,
    limits: PredicateCoverLimits,
) -> Result<PredicateCoverCertificate, PredicateCoverError> {
    if limits.max_predicates > super::SourcePortLimits::MAX_PREDICATE_ATOMS {
        return Err(PredicateCoverError::Budget(
            "supported predicate atom policy",
        ));
    }
    if sector.is_empty() || sector.len() > limits.geometry.max_arity {
        return Err(PredicateCoverError::InvalidDomain("sector arity"));
    }
    let required = required
        .map(|boxes| scope::prepare_required(sector.len(), boxes, limits.geometry))
        .transpose()?;
    let mut atoms = Vec::new();
    let mut constraints = Vec::new();
    let mut clauses = Vec::new();
    for owner in owners {
        let target = owner
            .affine_target
            .map(|domain| prepare_predicate(domain, sector, &mut atoms, &mut constraints, limits))
            .transpose()?;
        let exclusions = owner
            .affine_exclusions
            .iter()
            .map(|domain| prepare_predicate(domain, sector, &mut atoms, &mut constraints, limits))
            .collect::<Result<Vec<_>, _>>()?;
        for domain in owner.boxes {
            if domain.arity() != sector.len() {
                return Err(PredicateCoverError::InvalidDomain("box arity"));
            }
            let base = match &target {
                Some((face, _)) => intersect_box(domain, face)?,
                None => Some(copy_box(domain)?),
            };
            let Some(domain) = base else { continue };
            let mut initial = Clause {
                domain,
                literals: Vec::new(),
            };
            if let Some((_, required)) = &target {
                for &atom in required {
                    initial.require(atom, true);
                }
            }
            let mut current = vec![initial];
            for (face, equations) in &exclusions {
                let mut next = Vec::new();
                for clause in current {
                    subtract_predicate(clause, face, equations, &mut next, limits)?;
                }
                current = next;
            }
            for clause in current {
                push_clause(&mut clauses, clause, limits)?;
            }
        }
    }
    for terminal in terminals {
        if terminal.arity() != sector.len() || terminal.varying_dimension() != 0 {
            return Err(PredicateCoverError::InvalidDomain(
                "terminal is not a singleton in this sector",
            ));
        }
        push_clause(
            &mut clauses,
            Clause {
                domain: copy_box(terminal)?,
                literals: Vec::new(),
            },
            limits,
        )?;
    }
    let mut assignments = vec![None; atoms.len()];
    let mut work = TraversalWork::with_work_limit(sector, &atoms, limits.max_consistency_work);
    work.required_degree = required_degree;
    let result = (|| {
        if let Some(required) = &required {
            for domain in required.boxes() {
                work.required_domain = Some(domain);
                check_valuations(
                    sector,
                    &atoms,
                    &clauses,
                    &constraints,
                    &mut assignments,
                    &mut work,
                    limits,
                )?;
            }
            Ok(())
        } else {
            check_valuations(
                sector,
                &atoms,
                &clauses,
                &constraints,
                &mut assignments,
                &mut work,
                limits,
            )
        }
    })();
    work.consistency.report(match &result {
        Ok(()) => "covered",
        Err(PredicateCoverError::Budget(_)) => "budget",
        Err(PredicateCoverError::Uncovered { .. }) => "uncovered",
        Err(_) => "error",
    });
    result?;
    Ok(PredicateCoverCertificate {
        predicates: atoms.len(),
        clauses: clauses.len(),
        boolean_nodes: work.nodes,
    })
}

fn prepare_predicate<'a>(
    domain: &'a AffineApplicationDomain,
    sector: &[bool],
    atoms: &mut Vec<Atom<'a>>,
    constraints: &mut Vec<EqualityConstraint<'a>>,
    limits: PredicateCoverLimits,
) -> Result<(LatticeBox, Vec<usize>), PredicateCoverError> {
    if !domain.is_authenticated()
        || domain.sector() != sector
        || domain.fixed().len() != sector.len()
        || domain.indices().len() != sector.len()
        || domain.equations().is_empty()
    {
        return Err(PredicateCoverError::InvalidDomain(
            "unauthenticated or incompatible affine predicate",
        ));
    }
    let mut lower = vec![0; sector.len()];
    let mut upper = vec![None; sector.len()];
    for (axis, fixed) in domain.fixed().iter().enumerate() {
        if let Some(fixed) = fixed {
            let power = i64::from(*fixed);
            let local = if sector[axis] { power - 1 } else { -power };
            let local = u64::try_from(local)
                .map_err(|_| PredicateCoverError::InvalidDomain("fixed face outside sector"))?;
            lower[axis] = local;
            upper[axis] = Some(local);
        }
    }
    let face = LatticeBox::try_new(lower, upper).map_err(geometry)?;
    let mut required = Vec::new();
    for equation in domain.equations() {
        let atom = if let Some(index) = atoms
            .iter()
            .position(|atom| atom.indices == domain.indices() && atom.equation == equation)
        {
            index
        } else {
            if atoms.len() >= limits.max_predicates {
                return Err(PredicateCoverError::Budget("predicate atoms"));
            }
            atoms.push(Atom {
                indices: domain.indices(),
                equation,
            });
            atoms.len() - 1
        };
        if !required.contains(&atom) {
            required.push(atom);
        }
    }
    required.sort_unstable();
    constraints.push(EqualityConstraint {
        domain,
        face: copy_box(&face)?,
        atoms: required.clone(),
    });
    Ok((face, required))
}

/// `D \ (face ∩ p1 ∩ ... ∩ pk)` is the disjoint union of `D \ face`
/// and `D ∩ face ∩ (¬p1 ∪ (p1∩¬p2) ∪ ...)`. Known literal conflicts
/// simplify a branch, but unknown affine feasibility never discards one.
fn subtract_predicate(
    clause: Clause,
    face: &LatticeBox,
    equations: &[usize],
    output: &mut Vec<Clause>,
    limits: PredicateCoverLimits,
) -> Result<(), PredicateCoverError> {
    let Some(inside) = intersect_box(&clause.domain, face)? else {
        return push_clause(output, clause, limits);
    };
    let outside = BoxCover::try_new(face.arity(), [copy_box(face)?], limits.geometry)
        .map_err(geometry)?
        .uncovered_within(copy_box(&clause.domain)?)
        .map_err(geometry)?;
    for piece in outside.boxes() {
        push_clause(output, clause.copy_with_domain(copy_box(piece)?), limits)?;
    }
    let mut prefix = clause.copy_with_domain(inside);
    for &atom in equations {
        let mut failed = prefix.copy_with_domain(copy_box(&prefix.domain)?);
        if failed.require(atom, false) {
            push_clause(output, failed, limits)?;
        }
        if !prefix.require(atom, true) {
            break;
        }
    }
    Ok(())
}

fn check_valuations(
    sector: &[bool],
    atoms: &[Atom<'_>],
    clauses: &[Clause],
    constraints: &[EqualityConstraint<'_>],
    assignments: &mut [Option<bool>],
    work: &mut TraversalWork<'_>,
    limits: PredicateCoverLimits,
) -> Result<(), PredicateCoverError> {
    if work.nodes >= limits.max_boolean_nodes {
        return Err(PredicateCoverError::Budget("Boolean partition nodes"));
    }
    work.nodes += 1;
    let mut definite = Vec::new();
    let mut next_atom = None;
    for clause in clauses {
        if clause
            .literals
            .iter()
            .any(|&(atom, expected)| assignments[atom].is_some_and(|value| value != expected))
        {
            continue;
        }
        if let Some(&(atom, _)) = clause
            .literals
            .iter()
            .find(|&&(atom, _)| assignments[atom].is_none())
        {
            next_atom = Some(next_atom.map_or(atom, |old: usize| old.min(atom)));
        } else {
            definite.push(copy_box(&clause.domain)?);
        }
    }
    let geometry_limits = if work.required_domain.is_some() {
        work.scoped_geometry.remaining(limits.geometry)?
    } else {
        limits.geometry
    };
    let cover = BoxCover::try_new(sector.len(), definite, geometry_limits).map_err(geometry)?;
    let complement = match work.required_domain {
        Some(domain) => cover.uncovered_within(copy_box(domain)?),
        None => cover.uncovered_partition(),
    }
    .map_err(geometry)?;
    if work.required_domain.is_some() {
        work.scoped_geometry.record(&complement, sector.len())?;
    }
    let mut possible = Vec::new();
    for piece in complement.boxes() {
        let tightened;
        let feasibility_box = if let Some(degree) = work.required_degree {
            work.scoped_geometry
                .charge_degree_probe(sector.len(), limits.geometry)?;
            let Some(hull) = scope::degree_hull(sector, piece, degree)? else {
                continue;
            };
            tightened = hull;
            &tightened
        } else {
            piece
        };
        if constraints.iter().any(|constraint| {
            // All equalities AND the fixed face must hold before the
            // authenticated domain's emptiness service applies. An equation
            // being false, or merely intersecting the face, proves nothing.
            constraint
                .atoms
                .iter()
                .all(|&atom| assignments[atom] == Some(true))
                && contains_box(&constraint.face, feasibility_box)
                && constraint.domain.is_proved_empty_in_box(feasibility_box)
        }) {
            continue;
        }
        // Assigned literals already constrain the entire current branch.
        // Discharge contradictions before branching over unrelated atoms;
        // waiting for a leaf needlessly repeats identical exact work.
        if !work
            .consistency
            .contradicts(feasibility_box, assignments)
            .map_err(|_| PredicateCoverError::Budget("native affine literal consistency"))?
        {
            possible.push(piece);
        }
    }
    if possible.is_empty() {
        return Ok(());
    }
    let Some(atom) = next_atom else {
        return Err(PredicateCoverError::Uncovered {
            boxes: possible.len(),
            unbounded_boxes: possible
                .iter()
                .filter(|cell| cell.free_dimension() > 0)
                .count(),
            witness: Box::new(
                PredicateCoverWitness::capture(sector, possible[0], atoms, assignments)
                    .with_required_degree(work.required_degree),
            ),
        });
    };
    assignments[atom] = Some(false);
    check_valuations(
        sector,
        atoms,
        clauses,
        constraints,
        assignments,
        work,
        limits,
    )?;
    assignments[atom] = Some(true);
    check_valuations(
        sector,
        atoms,
        clauses,
        constraints,
        assignments,
        work,
        limits,
    )?;
    assignments[atom] = None;
    Ok(())
}

fn push_clause(
    output: &mut Vec<Clause>,
    clause: Clause,
    limits: PredicateCoverLimits,
) -> Result<(), PredicateCoverError> {
    if output.len() >= limits.max_clauses {
        return Err(PredicateCoverError::Budget("owner clauses"));
    }
    output.push(clause);
    Ok(())
}

fn copy_box(cell: &LatticeBox) -> Result<LatticeBox, PredicateCoverError> {
    LatticeBox::try_new(cell.lower().iter().copied(), cell.upper().iter().copied())
        .map_err(geometry)
}

fn intersect_box(
    left: &LatticeBox,
    right: &LatticeBox,
) -> Result<Option<LatticeBox>, PredicateCoverError> {
    if left.arity() != right.arity() {
        return Err(PredicateCoverError::InvalidDomain("intersection arity"));
    }
    let lower = left
        .lower()
        .iter()
        .zip(right.lower())
        .map(|(&a, &b)| a.max(b))
        .collect::<Vec<_>>();
    let upper = left
        .upper()
        .iter()
        .zip(right.upper())
        .map(|(&a, &b)| match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (None, b) => b,
            (a, None) => a,
        })
        .collect::<Vec<_>>();
    if lower
        .iter()
        .zip(&upper)
        .any(|(&lo, &hi)| hi.is_some_and(|hi| lo > hi))
    {
        return Ok(None);
    }
    LatticeBox::try_new(lower, upper)
        .map(Some)
        .map_err(geometry)
}

fn contains_box(outer: &LatticeBox, inner: &LatticeBox) -> bool {
    outer.arity() == inner.arity()
        && outer
            .lower()
            .iter()
            .zip(outer.upper())
            .zip(inner.lower().iter().zip(inner.upper()))
            .all(|((&lo, &hi), (&inner_lo, &inner_hi))| {
                lo <= inner_lo
                    && match (hi, inner_hi) {
                        (None, _) => true,
                        (Some(hi), Some(inner_hi)) => hi >= inner_hi,
                        (Some(_), None) => false,
                    }
            })
}

fn geometry(error: impl fmt::Display) -> PredicateCoverError {
    PredicateCoverError::Geometry(error.to_string())
}

#[cfg(test)]
#[path = "predicate_cover/tests.rs"]
mod tests;
