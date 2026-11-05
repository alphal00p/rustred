//! Eps axes: p-only shifted admission, eps transport, LP with shifts (owner: M2-T7a rr-eps).
//!
//! An eps line has the power `n + j eps` at `d = 4 - 2 eps`, stored as the
//! power shift `j (4-d)/2` on a line slot of a massless one-external family.
//!
//! - F4 (M1 missing test 10): a unit-scale propagator family must be
//!   massless; a massive line is refused instead of being tied to `q^2`.
//! - The p-only admission `shifted_unit_scale_kinematics` admits exactly
//!   `j (4-d)/2` on line slots; every unit-scale consumer of the core keeps
//!   refusing shifted families.
//! - Eps transport: forced axes map onto forced axes with the same shift and
//!   are carried as lines at every integer power; checked against a direct
//!   momentum substitution computed here.
//! - The zero analyzer on the 2L eps shapes of the 3L Forcer DAG.
//! - Lee-Pomeransky counts with shifts (`MasterCounter::try_new_p_shifted`)
//!   equal `rules_study/targets.tsv` (`masters_lp_nosym`).
//! - MT9 (M1 missing test 9): reductions with `gram(q,q,-1)` equal the
//!   Euclidean G-function evaluator under plan convention C1,
//!   `I_M(x) = (-1)^(sum x) I_E(x)`.
//!
//! The four-loop counts are slow and ignored by default:
//!
//! ```text
//! cargo test --release -p rustred --test p_library -- eps:: --ignored --test-threads 1
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustred::family::{IntegralFamily, IntegralKey, ShiftedUnitScale, UnitScaleKinematics};
use rustred::foundry::artifact::{ArtifactError, SourcePortAudit, validate_shifted_p_family};
use rustred::reduction::ReductionLimits;
use rustred::scalar_numerator::{
    FamilyScalarNumeratorService, ScalarNumeratorError, ScalarNumeratorLimits,
};
use rustred::sector::masters::{MasterCount, MasterCountError, MasterCountOptions, MasterCounter};
use rustred::sector::symmetry::integral_transport::{Error as TransportError, compile};
use rustred::sector::symmetry::{
    CoefficientMatrix, DenominatorAction, Limits, MomentumMap, verify,
};
use rustred::sector::zero::{Analyzer, Decision, Error as ZeroError};
use rustred::sector::{CutConstraint, Mask, OrderingPolicy, Pattern, Restrictions};
use rustred::solver::litered::{DynamicLiteRedSession, LiteRedOptions};
use rustred::solver::{
    CandidateFamilyAdmission, CandidateReducer, CandidateReductionError, SectorSolution,
};
use symbolica::atom::{Atom, NamespacedSymbol, Symbol, SymbolAttribute, SymbolBuilder};
use symbolica::domains::rational::Rational;

use crate::support::p_integrals::{
    BUBBLE, GValue, KITE, PFamily, Power, collect, compile as compile_source, g, rational,
};

// ---------------------------------------------------------------------------------------------
// Families built in-test
// ---------------------------------------------------------------------------------------------

/// A p family over `(k1..kL, q)`: slots in order, each a squared momentum
/// with its eps units `j` (power shift `j (4-d)/2`); `lines` slots may carry
/// positive powers, the rest are ISPs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct EpsFamily {
    name: String,
    loops: usize,
    slots: Vec<(Vec<i64>, i64)>,
    lines: usize,
}

impl EpsFamily {
    fn new(name: &str, loops: usize, slots: Vec<(Vec<i64>, i64)>, lines: usize) -> Self {
        assert!(slots.iter().all(|(row, _)| row.len() == loops + 1));
        Self {
            name: name.to_owned(),
            loops,
            slots,
            lines,
        }
    }

    /// The compact input with `q.q = gram`.
    fn source(&self, gram: i64) -> String {
        let mut source = format!(
            "I(name({}),loops({}),externals(q),dimension(d)",
            self.name,
            (1..=self.loops)
                .map(|l| format!("k{l}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        for (slot, (row, _)) in self.slots.iter().enumerate() {
            source += &format!(
                ",prop(D{},({})^2,{})",
                slot + 1,
                momentum(row, self.loops),
                i64::from(slot < self.lines)
            );
        }
        source += &format!(",gram(q,q,{gram})");
        for (slot, (_, units)) in self.slots.iter().enumerate() {
            if *units != 0 {
                source += &format!(",power_shift(D{},{units}*(2-d/2))", slot + 1);
            }
        }
        source + ")"
    }

    fn family(&self, gram: i64) -> Arc<IntegralFamily> {
        Arc::new(compile_source(&self.source(gram)))
    }

    fn forced(&self) -> Vec<(usize, i64)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, (_, units))| *units != 0)
            .map(|(slot, (_, units))| (slot, *units))
            .collect()
    }

    /// The sector of all lines (ISPs inactive).
    fn top(&self) -> Mask {
        Mask::try_new((0..self.slots.len()).map(|slot| slot < self.lines)).unwrap()
    }

    /// A family from a graph: `edges` such as `"0-1* 0-2 1-2"` (one `*` per
    /// eps unit), `q` entering at vertex `s` and leaving at vertex `t`. Chords
    /// of a breadth-first spanning tree from `s` carry the loop momenta; ISPs
    /// complete the basis.
    fn from_graph(name: &str, edges: &str, s: u32, t: u32) -> Self {
        let edges: Vec<(u32, u32, i64)> = edges.split_whitespace().map(parse_edge).collect();
        let lines = route(&edges, s, t);
        let loops = lines[0].0.len() - 1;
        let isps = completion(
            &lines.iter().map(|(row, _)| row.clone()).collect::<Vec<_>>(),
            loops,
        );
        let count = lines.len();
        let mut slots = lines;
        slots.extend(isps.into_iter().map(|row| (row, 0)));
        Self::new(name, loops, slots, count)
    }

    /// A family from Forcer notation (`forcer_dag.tsv`): legs `0-x` (in) and
    /// `y-vmax` (out), eps marks on the legs ignored (eps on q is no key).
    fn from_forcer(name: &str, topology: &str) -> Self {
        let edges: Vec<(u32, u32, i64)> = topology.split_whitespace().map(parse_edge).collect();
        let vmax = edges.iter().map(|&(u, v, _)| u.max(v)).max().unwrap();
        let leg = |vertex: u32| {
            edges
                .iter()
                .find_map(|&(u, v, _)| (u == vertex).then_some(v).or((v == vertex).then_some(u)))
                .unwrap()
        };
        let internal: Vec<String> = edges
            .iter()
            .filter(|&&(u, v, _)| u != 0 && v != 0 && u != vmax && v != vmax)
            .map(|&(u, v, units)| format!("{u}-{v}{}", "*".repeat(units as usize)))
            .collect();
        Self::from_graph(name, &internal.join(" "), leg(0), leg(vmax))
    }
}

fn momentum(row: &[i64], loops: usize) -> String {
    let mut text = String::new();
    for (column, &c) in row.iter().enumerate() {
        if c != 0 {
            let name = if column < loops {
                format!("k{}", column + 1)
            } else {
                "q".to_owned()
            };
            text += &format!("{c:+}*{name}");
        }
    }
    text
}

fn parse_edge(edge: &str) -> (u32, u32, i64) {
    let units = edge.matches('*').count() as i64;
    let (u, v) = edge.trim_end_matches('*').split_once('-').unwrap();
    (u.parse().unwrap(), v.parse().unwrap(), units)
}

/// Momentum rows over `(k1..kL, q)` of every edge, with its eps units.
fn route(edges: &[(u32, u32, i64)], s: u32, t: u32) -> Vec<(Vec<i64>, i64)> {
    let vertices: BTreeSet<u32> = edges.iter().flat_map(|&(u, v, _)| [u, v]).collect();
    // Breadth-first tree from s: parent edge of every other vertex.
    let mut parent: BTreeMap<u32, usize> = BTreeMap::new();
    let mut seen = BTreeSet::from([s]);
    let mut queue = std::collections::VecDeque::from([s]);
    while let Some(x) = queue.pop_front() {
        for (index, &(u, v, _)) in edges.iter().enumerate() {
            let y = if u == x {
                v
            } else if v == x {
                u
            } else {
                continue;
            };
            if seen.insert(y) {
                parent.insert(y, index);
                queue.push_back(y);
            }
        }
    }
    assert_eq!(seen, vertices, "disconnected graph");
    let chords: Vec<usize> = (0..edges.len())
        .filter(|index| !parent.values().any(|tree| tree == index))
        .collect();
    let loops = chords.len();
    let mut rows = vec![vec![0_i64; loops + 1]; edges.len()];
    // Push one unit of flow along the tree path from `from` to `to` into
    // column `column`: edge (u, v) traversed u -> v gets +1.
    let push = |rows: &mut Vec<Vec<i64>>, from: u32, to: u32, column: usize| {
        let ancestors = |mut x: u32| {
            let mut path = vec![x];
            while let Some(&edge) = parent.get(&x) {
                let (u, v, _) = edges[edge];
                x = if u == x { v } else { u };
                path.push(x);
            }
            path
        };
        let (up, down) = (ancestors(from), ancestors(to));
        let meet = *up.iter().find(|x| down.contains(x)).unwrap();
        for (path, sign) in [(&up, 1), (&down, -1)] {
            for x in path.iter().take_while(|&&x| x != meet) {
                let edge = parent[x];
                let (u, _, _) = edges[edge];
                // Walking from x towards the root: forward if x == u.
                let forward = if u == *x { 1 } else { -1 };
                rows[edge][column] += sign * forward;
            }
        }
    };
    for (column, &chord) in chords.iter().enumerate() {
        let (u, v, _) = edges[chord];
        rows[chord][column] += 1;
        // Close the loop: from v back to u through the tree.
        push(&mut rows, v, u, column);
    }
    push(&mut rows, s, t, loops);
    rows.into_iter()
        .zip(edges)
        .map(|(row, &(_, _, units))| (row, units))
        .collect()
}

/// `r^2` over the scalar products `k_a.k_b` (`a <= b`) and `k_a.q`.
fn quadratic(row: &[i64], loops: usize) -> Vec<Rational> {
    let mut form = Vec::new();
    for a in 0..loops {
        for b in a..loops {
            let factor = if a == b { 1 } else { 2 };
            form.push(Rational::from(factor * row[a] * row[b]));
        }
    }
    for a in 0..loops {
        form.push(Rational::from(2 * row[a] * row[loops]));
    }
    form
}

/// Clear `column` in every row but `pivot` with multiples of the pivot row.
fn eliminate(rows: &mut [Vec<Rational>], pivot: usize, column: usize) {
    let pivot_row = rows[pivot].clone();
    for (index, row) in rows.iter_mut().enumerate() {
        if index != pivot && !row[column].is_zero() {
            let factor = &row[column] / &pivot_row[column];
            for (entry, value) in row.iter_mut().zip(&pivot_row) {
                *entry = &*entry - &(&factor * value);
            }
        }
    }
}

/// Gaussian elimination: the rank of `rows`.
fn rank(rows: &[Vec<Rational>]) -> usize {
    let mut rows = rows.to_vec();
    let columns = rows.first().map_or(0, Vec::len);
    let mut rank = 0;
    for column in 0..columns {
        let Some(pivot) = (rank..rows.len()).find(|&r| !rows[r][column].is_zero()) else {
            continue;
        };
        rows.swap(rank, pivot);
        eliminate(&mut rows, rank, column);
        rank += 1;
    }
    rank
}

/// Solve the square system `matrix x = rhs` exactly, `None` when singular.
fn solve(matrix: &[Vec<Rational>], rhs: &[Rational]) -> Option<Vec<Rational>> {
    let n = matrix.len();
    let mut rows: Vec<Vec<Rational>> = matrix
        .iter()
        .zip(rhs)
        .map(|(row, value)| row.iter().cloned().chain([value.clone()]).collect())
        .collect();
    for column in 0..n {
        let pivot = (column..n).find(|&r| !rows[r][column].is_zero())?;
        rows.swap(column, pivot);
        eliminate(&mut rows, column, column);
    }
    Some((0..n).map(|r| &rows[r][n] / &rows[r][r]).collect())
}

/// ISP rows completing `lines` to a basis of the scalar products: squares of
/// `k_a`, `k_a - k_b`, `k_a + q`, `k_a + k_b`, taken greedily.
fn completion(lines: &[Vec<i64>], loops: usize) -> Vec<Vec<i64>> {
    let size = loops * (loops + 1) / 2 + loops;
    let unit = |a: usize| (0..=loops).map(|c| i64::from(c == a)).collect::<Vec<_>>();
    let add = |x: &[i64], y: &[i64], s: i64| -> Vec<i64> {
        x.iter().zip(y).map(|(a, b)| a + s * b).collect()
    };
    let mut candidates: Vec<Vec<i64>> = (0..loops).map(unit).collect();
    for a in 0..loops {
        for b in a + 1..loops {
            candidates.push(add(&unit(a), &unit(b), -1));
        }
    }
    candidates.extend((0..loops).map(|a| add(&unit(a), &unit(loops), 1)));
    for a in 0..loops {
        for b in a + 1..loops {
            candidates.push(add(&unit(a), &unit(b), 1));
        }
    }
    let mut forms: Vec<Vec<Rational>> = lines.iter().map(|row| quadratic(row, loops)).collect();
    assert_eq!(rank(&forms), lines.len(), "dependent lines");
    let mut isps = Vec::new();
    for candidate in candidates {
        if forms.len() == size {
            break;
        }
        forms.push(quadratic(&candidate, loops));
        if rank(&forms) == forms.len() {
            isps.push(candidate);
        } else {
            forms.pop();
        }
    }
    assert_eq!(forms.len(), size, "no complete basis");
    isps
}

fn t001c() -> EpsFamily {
    // rules_study/knockout/families/T001c.sym: eps on D1 and D5.
    EpsFamily::new(
        "T001c",
        2,
        vec![
            (vec![1, 0, 0], 1),
            (vec![-1, -1, 0], 0),
            (vec![0, 1, 0], 0),
            (vec![1, 1, -1], 0),
            (vec![0, -1, 1], 1),
        ],
        5,
    )
}

fn t013c() -> EpsFamily {
    // rules_study/knockout/families/T013c.sym: eps on D1, D3 and D4.
    let mut family = t001c();
    family.name = "T013c".into();
    for (slot, units) in [1, 0, 1, 1, 0].into_iter().enumerate() {
        family.slots[slot].1 = units;
    }
    family
}

fn compile_text(source: &str) -> IntegralFamily {
    compile_source(source)
}

// ---------------------------------------------------------------------------------------------
// F4 and the p-only admission
// ---------------------------------------------------------------------------------------------

fn propagator(props: &str, gram: i64) -> IntegralFamily {
    compile_text(&format!(
        "I(loops(k1),externals(q),dimension(d),{props},gram(q,q,{gram}))"
    ))
}

fn two_loop(props: &str, gram: i64) -> IntegralFamily {
    compile_text(&format!(
        "I(loops(k1,k2),externals(q),dimension(d),{props},gram(q,q,{gram}))"
    ))
}

#[test]
fn f4_massless_lines_and_isps_are_admitted() {
    for gram in [1, -1] {
        let expected = Some(UnitScaleKinematics::UnitScalePropagator {
            external_square: gram as i8,
        });
        // (k-q)^2, an ISP k.q, (k1-k2)^2 and an ISP k1.k2.
        for family in [
            propagator("prop(D1,k1^2,1),prop(D2,(k1-q)^2,1)", gram),
            propagator("prop(D1,k1^2,1),prop(D2,sp(k1,q),0)", gram),
            propagator("prop(D1,(k1+q)^2,1),prop(D2,-2*sp(k1,q),0)", gram),
            two_loop(
                "prop(D1,k1^2,1),prop(D2,k2^2,1),prop(D3,(k1-k2)^2,1),\
                 prop(D4,sp(k1,q),0),prop(D5,(k2-q)^2,1)",
                gram,
            ),
            two_loop(
                "prop(D1,k1^2,1),prop(D2,k2^2,1),prop(D3,sp(k1,k2),0),\
                 prop(D4,(k1-q)^2,1),prop(D5,(k2-q)^2,1)",
                gram,
            ),
        ] {
            assert_eq!(family.unit_scale_kinematics().unwrap(), expected);
            assert_eq!(
                family.shifted_unit_scale_kinematics().unwrap(),
                Some(ShiftedUnitScale {
                    external_square: gram as i8,
                    forced: vec![],
                })
            );
        }
    }
    // The P0 families stay admitted at both signs of q.q.
    for family in [&BUBBLE, &KITE] {
        for gram in [1, -1] {
            assert!(
                compile_text(&family.source(gram))
                    .unit_scale_kinematics()
                    .unwrap()
                    .is_some()
            );
        }
    }
}

#[test]
fn f4_massive_lines_are_refused() {
    // k^2 - 1 at q.q = -1 is k^2 + q.q: a mass tied to Q^2, refused (M1 F4).
    for family in [
        propagator("prop(D1,k1^2-1,1),prop(D2,(k1-q)^2,1)", -1),
        propagator("prop(D1,k1^2+1,1),prop(D2,(k1-q)^2,1)", 1),
        propagator("prop(D1,k1^2,1),prop(D2,(k1-q)^2-1,1)", 1),
        // k^2 + k.q = (k + q/2)^2 - q.q/4: a line with mass^2 = q.q/4.
        propagator("prop(D1,k1^2+sp(k1,q),1),prop(D2,(k1-q)^2,1)", 1),
        // An ISP with a constant: k.q + q.q.
        propagator("prop(D1,k1^2,1),prop(D2,sp(k1,q)+1,0)", 1),
        two_loop(
            "prop(D1,k1^2,1),prop(D2,k2^2,1),prop(D3,sp(k1,k2)-1,0),\
             prop(D4,(k1-q)^2,1),prop(D5,(k2-q)^2,1)",
            -1,
        ),
    ] {
        assert_eq!(family.unit_scale_kinematics().unwrap(), None);
        assert_eq!(family.shifted_unit_scale_kinematics().unwrap(), None);
        assert_eq!(
            validate_shifted_p_family(&family),
            Err(ArtifactError::UnsupportedClosureShape)
        );
    }
    // The vacuum lane is unchanged: one common unit mass.
    let sunset = compile_text(
        "I(loops(k1,k2),externals(),dimension(d),prop(D1,k1^2-1,1),\
         prop(D2,k2^2-1,1),prop(D3,(k1+k2)^2-1,1))",
    );
    assert_eq!(
        sunset.unit_scale_kinematics().unwrap(),
        Some(UnitScaleKinematics::UnitMassVacuum)
    );
    assert_eq!(sunset.shifted_unit_scale_kinematics().unwrap(), None);
}

#[test]
fn shifted_admission_takes_exactly_eps_shifts_on_lines() {
    for gram in [1, -1] {
        // T001c, and T005-shaped units 1 and 2.
        let family = t001c().family(gram);
        assert_eq!(family.unit_scale_kinematics().unwrap(), None);
        let admitted = family.shifted_unit_scale_kinematics().unwrap().unwrap();
        assert_eq!(admitted.external_square, gram as i8);
        assert_eq!(admitted.forced, [(0, 1), (4, 1)]);
        assert!(admitted.is_forced(4) && !admitted.is_forced(1));
        assert_eq!(validate_shifted_p_family(&family), Ok(admitted));
        let t005 = EpsFamily::from_graph("T005c", "0-1 0-2 0-3 1-2* 1-3**", 2, 3);
        let admitted = validate_shifted_p_family(&t005.family(gram)).unwrap();
        assert_eq!(admitted.forced, t005.forced());
        assert_eq!(
            admitted.forced.iter().map(|(_, j)| *j).collect::<Vec<_>>(),
            [1, 2]
        );
    }
    let bubble = |shift: &str| {
        propagator(
            &format!("prop(D1,k1^2,1),prop(D2,(k1-q)^2,1),power_shift(D2,{shift})"),
            1,
        )
    };
    assert_eq!(
        bubble("-3*(2-d/2)")
            .shifted_unit_scale_kinematics()
            .unwrap()
            .unwrap()
            .forced,
        [(1, -3)]
    );
    // Not j (4-d)/2 with integer j != 0.
    for shift in ["1", "1/2", "d/2", "3-d/2", "(4-d)/3", "1/2*(2-d/2)"] {
        let family = bubble(shift);
        assert_eq!(
            family.shifted_unit_scale_kinematics().unwrap(),
            None,
            "{shift}"
        );
        assert_eq!(family.unit_scale_kinematics().unwrap(), None, "{shift}");
    }
    // A shifted ISP slot.
    let isp = propagator(
        "prop(D1,k1^2,1),prop(D2,sp(k1,q),0),power_shift(D2,2-d/2)",
        1,
    );
    assert_eq!(isp.shifted_unit_scale_kinematics().unwrap(), None);
    // A shifted vacuum family, and a massive eps family.
    let vacuum = compile_text(
        "I(loops(k1),externals(),dimension(d),prop(D1,k1^2-1,1),power_shift(D1,2-d/2))",
    );
    assert_eq!(vacuum.shifted_unit_scale_kinematics().unwrap(), None);
    let massive = propagator(
        "prop(D1,k1^2-1,1),prop(D2,(k1-q)^2,1),power_shift(D2,2-d/2)",
        -1,
    );
    assert_eq!(massive.shifted_unit_scale_kinematics().unwrap(), None);
}

// ---------------------------------------------------------------------------------------------
// Still refused: every unit-scale consumer of the core
// ---------------------------------------------------------------------------------------------

fn dot_head() -> Symbol {
    SymbolBuilder::new(NamespacedSymbol::try_parse("rustred_p_library_eps::dot").unwrap())
        .with_attributes(vec![SymbolAttribute::Symmetric, SymbolAttribute::Linear])
        .build()
        .unwrap()
}

fn momentum_symbol(name: &str) -> Atom {
    SymbolBuilder::new(
        NamespacedSymbol::try_parse(format!("rustred_p_library_eps::{name}")).unwrap(),
    )
    .build()
    .unwrap()
    .to_atom()
}

#[test]
fn shifted_families_stay_refused_by_the_core_unit_scale_consumers() {
    for gram in [1, -1] {
        let family = t001c().family(gram);
        assert_eq!(family.unit_scale_kinematics().unwrap(), None);
        let unsupported = |result: Result<CandidateReducer<5>, CandidateReductionError>| {
            matches!(result, Err(CandidateReductionError::UnsupportedFamily(_)))
        };
        // CandidateReducer preparation through CandidateFamilyAdmission::unit_scale_for.
        assert_eq!(
            CandidateFamilyAdmission::unit_scale_for(&family),
            CandidateFamilyAdmission::UnitScalePropagator
        );
        assert!(unsupported(CandidateReducer::<5>::try_new(
            &family,
            [true; 5],
            OrderingPolicy::default(),
            std::iter::empty::<([bool; 5], SectorSolution<5>)>(),
            Vec::new(),
            ReductionLimits::default(),
        )));
        // Every explicit admission refuses it as well.
        for admission in [
            CandidateFamilyAdmission::unit_scale_for(&family),
            CandidateFamilyAdmission::UnitScalePropagator,
            CandidateFamilyAdmission::UnitMassVacuum,
            CandidateFamilyAdmission::ParametricVacuum,
        ] {
            assert!(
                unsupported(CandidateReducer::<5>::try_new_with_admission(
                    &family,
                    [true; 5],
                    OrderingPolicy::default(),
                    std::iter::empty::<([bool; 5], SectorSolution<5>)>(),
                    Vec::new(),
                    ReductionLimits::default(),
                    None,
                    admission,
                )),
                "{admission:?}"
            );
        }
        // Install/closing: validate_unit_scale_family (the homogeneity proof
        // CommonMassHomogeneityProof::for_kinematics needs its kinematics).
        assert!(SourcePortAudit::<5>::validate_install_family(&family).is_err());
        assert!(SourcePortAudit::<5>::try_new(&family, Arc::from(Vec::new())).is_err());
        // Scalar numerator lowering.
        let error = FamilyScalarNumeratorService::try_new_with_external_momenta(
            &family,
            dot_head(),
            vec![momentum_symbol("k1"), momentum_symbol("k2")],
            vec![momentum_symbol("q")],
            ScalarNumeratorLimits::default(),
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            ScalarNumeratorError::FamilyEligibility(ArtifactError::UnsupportedClosureShape)
        ));
        // The integer master counter.
        assert_eq!(
            MasterCounter::try_new(&family, MasterCountOptions::default()).unwrap_err(),
            MasterCountError::UnsupportedNonzeroPowerShift { denominator: 0 }
        );
        // The unshifted kite is admitted by the same consumers' predicate.
        assert!(SourcePortAudit::<5>::validate_install_family(&KITE.family()).is_ok());
    }
}

// ---------------------------------------------------------------------------------------------
// Eps transport
// ---------------------------------------------------------------------------------------------

/// `k = A k' + B q'`, `q = C q'` for two loops.
#[derive(Clone, Copy, Debug)]
struct LoopMap {
    a: [[i64; 2]; 2],
    b: [i64; 2],
    c: i64,
}

impl LoopMap {
    /// The image over `(k1', k2', q')` of a source momentum row.
    fn image(&self, row: &[i64]) -> Vec<i64> {
        let mut image: Vec<i64> = (0..2)
            .map(|m| (0..2).map(|l| row[l] * self.a[l][m]).sum())
            .collect();
        image.push((0..2).map(|l| row[l] * self.b[l]).sum::<i64>() + row[2] * self.c);
        image
    }

    fn momentum_map(&self, family: &IntegralFamily) -> MomentumMap {
        let context = family.coefficient_context();
        let matrix = |values: &[i64], rows, columns| {
            CoefficientMatrix::try_new(rows, columns, values.iter().map(|v| context.integer(*v)))
                .unwrap()
        };
        MomentumMap::new(
            matrix(
                &[self.a[0][0], self.a[0][1], self.a[1][0], self.a[1][1]],
                2,
                2,
            ),
            matrix(&self.b, 2, 1),
            matrix(&[self.c], 1, 1),
        )
    }
}

const MAPS: [LoopMap; 4] = [
    // q -> -q with k -> -k: every line onto itself.
    LoopMap {
        a: [[-1, 0], [0, -1]],
        b: [0, 0],
        c: -1,
    },
    // k1 <-> k2.
    LoopMap {
        a: [[0, 1], [1, 0]],
        b: [0, 0],
        c: 1,
    },
    // k -> q - k.
    LoopMap {
        a: [[-1, 0], [0, -1]],
        b: [1, 1],
        c: 1,
    },
    // k1 = -k2' + q', k2 = -k1', q = -q'.
    LoopMap {
        a: [[0, -1], [-1, 0]],
        b: [1, 0],
        c: -1,
    },
];

/// The target family of `source` under `map`: slot `permutation[i]` holds
/// the image of source slot `i` (sign flipped on odd slots) with its eps
/// units, except that the image of source slot `numerator` (if any) is
/// replaced by another row completing the basis, so that this slot maps
/// affinely.
fn image_family(
    source: &EpsFamily,
    map: LoopMap,
    permutation: &[usize],
    numerator: Option<usize>,
) -> EpsFamily {
    let mut slots = vec![(vec![], 0); source.slots.len()];
    for (slot, (row, units)) in source.slots.iter().enumerate() {
        let image = map.image(row);
        let sign = if slot % 2 == 1 { -1 } else { 1 };
        slots[permutation[slot]] = (image.iter().map(|c| sign * c).collect(), *units);
    }
    if let Some(slot) = numerator {
        let image = map.image(&source.slots[slot].0);
        let others: Vec<Vec<i64>> = slots
            .iter()
            .enumerate()
            .filter(|(target, _)| *target != permutation[slot])
            .map(|(_, (row, _))| row.clone())
            .collect();
        let replacement = [
            [1, 0, -1],
            [0, 1, -1],
            [1, -1, 0],
            [1, 0, 1],
            [0, 1, 1],
            [1, 1, 0],
        ]
        .into_iter()
        .map(|row| row.to_vec())
        .find(|row| {
            let negated: Vec<i64> = row.iter().map(|c| -c).collect();
            let mut forms: Vec<Vec<Rational>> =
                others.iter().map(|other| quadratic(other, 2)).collect();
            forms.push(quadratic(row, 2));
            row != &image && negated != image && rank(&forms) == forms.len()
        })
        .unwrap();
        slots[permutation[slot]] = (replacement, 0);
    }
    EpsFamily::new(&format!("{}_image", source.name), 2, slots, source.lines)
}

/// `D_source = c + sum_t L_t D_target` for a source row's image, solved
/// directly from the momenta (independent of the verified map).
fn affine_image(image: &[i64], target: &EpsFamily, gram: i64) -> (Rational, Vec<Rational>) {
    let loops = target.loops;
    let forms: Vec<Vec<Rational>> = target
        .slots
        .iter()
        .map(|(row, _)| quadratic(row, loops))
        .collect();
    let transposed: Vec<Vec<Rational>> = (0..forms.len())
        .map(|coordinate| forms.iter().map(|form| form[coordinate].clone()).collect())
        .collect();
    let linear = solve(&transposed, &quadratic(image, loops)).unwrap();
    let constant_of = |row: &[i64]| Rational::from(row[loops] * row[loops] * gram);
    let mut constant = constant_of(image);
    for (coefficient, (row, _)) in linear.iter().zip(&target.slots) {
        constant = &constant - &(coefficient * &constant_of(row));
    }
    (constant, linear)
}

/// The direct substitution of `map` into the integrand of `key`: a forced or
/// root slot whose image is (up to sign) a target slot moves there with its
/// power, checked to carry the same eps units; every other slot is a
/// numerator `(c + sum L_t D_t)^(-power)` expanded here.
fn substitute(
    source: &EpsFamily,
    target: &EpsFamily,
    map: LoopMap,
    root: &[bool],
    key: &[i64],
    gram: i64,
) -> BTreeMap<Vec<i64>, Rational> {
    let mut base = vec![0_i64; target.slots.len()];
    let mut polynomial: BTreeMap<Vec<i64>, Rational> =
        BTreeMap::from([(vec![0; target.slots.len()], Rational::one())]);
    for (slot, (row, units)) in source.slots.iter().enumerate() {
        let image = map.image(row);
        let negated: Vec<i64> = image.iter().map(|c| -c).collect();
        if root[slot] {
            let column = target
                .slots
                .iter()
                .position(|(target_row, _)| target_row == &image || target_row == &negated)
                .expect("a root line maps onto a target line");
            assert_eq!(target.slots[column].1, *units, "shift preserved");
            base[column] += key[slot];
            continue;
        }
        assert_eq!(*units, 0, "forced axes lie in the root");
        assert!(key[slot] <= 0);
        let (constant, linear) = affine_image(&image, target, gram);
        for _ in 0..-key[slot] {
            let mut next: BTreeMap<Vec<i64>, Rational> = BTreeMap::new();
            for (exponents, value) in &polynomial {
                let mut push = |exponents: Vec<i64>, factor: &Rational| {
                    let entry = next.entry(exponents).or_insert_with(Rational::zero);
                    *entry = &*entry + &(value * factor);
                };
                push(exponents.clone(), &constant);
                for (column, coefficient) in linear.iter().enumerate() {
                    let mut raised = exponents.clone();
                    raised[column] += 1;
                    push(raised, coefficient);
                }
            }
            polynomial = next;
        }
    }
    polynomial
        .into_iter()
        .filter(|(_, value)| !value.is_zero())
        .map(|(exponents, value)| {
            let key = base.iter().zip(&exponents).map(|(b, e)| b - e).collect();
            (key, value)
        })
        .collect()
}

fn to_rational(value: &rustred::algebra::Coefficient) -> Rational {
    assert!(value.numerator.is_constant() && value.denominator.is_constant());
    Rational::from((
        value.numerator.get_constant(),
        value.denominator.get_constant(),
    ))
}

/// Every key with the forced axes in `-2..=3`, the other root slots in
/// `-1..=2` and the numerator slot in `-2..=0`.
fn transport_keys(source: &EpsFamily, root: &[bool]) -> Vec<Vec<i64>> {
    let ranges: Vec<Vec<i64>> = source
        .slots
        .iter()
        .zip(root)
        .map(|((_, units), &active)| match (*units != 0, active) {
            (true, _) => (-2..=3).collect(),
            (false, true) => (-1..=2).collect(),
            (false, false) => (-2..=0).collect(),
        })
        .collect();
    let mut keys = vec![vec![]];
    for range in ranges {
        keys = keys
            .into_iter()
            .flat_map(|key: Vec<i64>| {
                range.iter().map(move |power| {
                    let mut next = key.clone();
                    next.push(*power);
                    next
                })
            })
            .collect();
    }
    keys
}

/// Compile the transport `source -> target` under `map` and check every key
/// against the direct substitution. Returns the number of keys checked.
fn check_eps_transport(
    source: &EpsFamily,
    target: &EpsFamily,
    map: LoopMap,
    root: &[bool],
    gram: i64,
) -> usize {
    let source_family = source.family(gram);
    let target_family = target.family(gram);
    let verified = verify(
        &source_family,
        &target_family,
        map.momentum_map(&source_family),
        Limits::default(),
    )
    .unwrap();
    let mut target_root = vec![false; target.slots.len()];
    for (slot, action) in verified.row_actions().iter().enumerate() {
        if root[slot] {
            let DenominatorAction::Monomial { target, .. } = action else {
                panic!("root slot {slot} is not a line image");
            };
            target_root[*target] = true;
        }
    }
    let prepared = compile(
        &source_family,
        Arc::clone(&target_family),
        Arc::new(verified),
        Mask::try_new(root.iter().copied()).unwrap(),
        Mask::try_new(target_root).unwrap(),
        Default::default(),
    )
    .unwrap();
    let keys = transport_keys(source, root);
    for key in &keys {
        let transported = prepared
            .transport(
                &IntegralKey::try_new(key.clone()).unwrap(),
                Default::default(),
            )
            .unwrap();
        let terms: BTreeMap<Vec<i64>, Rational> = transported
            .terms()
            .iter()
            .map(|term| {
                (
                    term.key().powers().to_vec(),
                    to_rational(term.coefficient()),
                )
            })
            .collect();
        assert_eq!(
            terms,
            substitute(source, target, map, root, key, gram),
            "{} -> {} under {map:?}: key {key:?}",
            source.name,
            target.name
        );
    }
    keys.len()
}

#[test]
fn eps_transport_carries_forced_axes_exactly() {
    let mut checked = 0;
    for (source, numerator) in [(t001c(), 1), (t013c(), 1), (t013c(), 4)] {
        for (index, map) in MAPS.into_iter().enumerate() {
            let permutation: Vec<usize> = (0..5).map(|slot| (slot + index) % 5).collect();
            let gram = if index % 2 == 0 { -1 } else { 1 };
            // All lines in the root: the forced axes and the integer lines
            // map by the unit bijection.
            let target = image_family(&source, map, &permutation, None);
            checked += check_eps_transport(&source, &target, map, &[true; 5], gram);
            // One integer line outside the root maps affinely (a numerator).
            let target = image_family(&source, map, &permutation, Some(numerator));
            let root: Vec<bool> = (0..5).map(|slot| slot != numerator).collect();
            checked += check_eps_transport(&source, &target, map, &root, gram);
        }
    }
    assert!(checked > 10_000, "{checked}");
}

/// The first compile error of `source -> target` under `map` and `root`.
fn transport_error(
    source: &IntegralFamily,
    target: Arc<IntegralFamily>,
    map: LoopMap,
    source_root: &[bool],
    target_root: &[bool],
) -> TransportError {
    let verified = verify(source, &target, map.momentum_map(source), Limits::default()).unwrap();
    compile(
        source,
        target,
        Arc::new(verified),
        Mask::try_new(source_root.iter().copied()).unwrap(),
        Mask::try_new(target_root.iter().copied()).unwrap(),
        Default::default(),
    )
    .unwrap_err()
}

#[test]
fn eps_transport_refuses_changed_shifts() {
    let source = t001c();
    let map = MAPS[1];
    let permutation = [2, 0, 4, 1, 3];
    let target = image_family(&source, map, &permutation, None);
    let all = [true; 5];
    let refused = |target: &EpsFamily, source_root: &[bool], target_root: &[bool]| {
        transport_error(
            &source.family(1),
            target.family(1),
            map,
            source_root,
            target_root,
        )
    };
    // The image of D1 with two eps units instead of one.
    let mut doubled = target.clone();
    doubled.slots[permutation[0]].1 = 2;
    assert!(matches!(
        refused(&doubled, &all, &all),
        TransportError::AnalyticPowerShift { .. }
    ));
    // The eps unit moved from the image of D5 to the image of D3.
    let mut moved = target.clone();
    moved.slots[permutation[4]].1 = 0;
    moved.slots[permutation[2]].1 = 1;
    assert!(matches!(
        refused(&moved, &all, &all),
        TransportError::AnalyticPowerShift { .. }
    ));
    // The same lines without eps units.
    let mut unshifted = target.clone();
    for slot in &mut unshifted.slots {
        slot.1 = 0;
    }
    assert!(matches!(
        refused(&unshifted, &all, &all),
        TransportError::AnalyticPowerShift { .. }
    ));
    // A forced axis outside the root would turn the eps line into a
    // numerator (and drop it at power 0): refused on both sides.
    let mut source_root = all;
    source_root[0] = false;
    let mut target_root = all;
    target_root[permutation[0]] = false;
    assert_eq!(
        refused(&target, &source_root, &target_root),
        TransportError::AnalyticPowerShift {
            side: "source",
            axis: 0
        }
    );
    // A shift other than j (4-d)/2 keeps the old refusal.
    let third = |name: &str| {
        Arc::new(compile_text(&format!(
            "I(name({name}),loops(k1),externals(q),dimension(d),prop(D1,k1^2,1),\
             prop(D2,(k1-q)^2,1),gram(q,q,1),power_shift(D1,(4-d)/3))"
        )))
    };
    let (left, right) = (third("a"), third("b"));
    let context = left.coefficient_context();
    let identity = MomentumMap::new(
        CoefficientMatrix::try_new(1, 1, [context.integer(1)]).unwrap(),
        CoefficientMatrix::try_new(1, 1, [context.integer(0)]).unwrap(),
        CoefficientMatrix::try_new(1, 1, [context.integer(1)]).unwrap(),
    );
    let verified = verify(&left, &right, identity, Limits::default()).unwrap();
    let error = compile(
        &left,
        right,
        Arc::new(verified),
        Mask::try_new([true, true]).unwrap(),
        Mask::try_new([true, true]).unwrap(),
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        TransportError::AnalyticPowerShift {
            side: "source",
            axis: 0
        }
    );
}

// ---------------------------------------------------------------------------------------------
// The zero analyzer on eps shapes
// ---------------------------------------------------------------------------------------------

/// The 2L/1L eps nodes of the 3L Forcer DAG (`forcer_dag.tsv`) and the
/// rules-study kites.
fn eps_shapes() -> Vec<EpsFamily> {
    let mut shapes: Vec<EpsFamily> = [
        ("d4", "0-1** 1-2 1-2* 2-9"),
        ("d6", "0-1* 1-2 1-2** 2-9"),
        ("d7", "0-1* 1-2* 1-2* 2-9"),
        ("d11", "0-1* 1-2 1-2 1-2* 2-9"),
        ("d13", "0-2* 1-5 1-5 1-2* 1-2 5-9"),
        ("d14", "0-2* 1-3 1-3 1-2* 2-3 1-9"),
        ("d15", "0-1* 1-2 1-4 2-4 2-4* 2-9"),
        ("d17", "0-2* 1-3 1-3 1-2* 2-3 3-9"),
        ("d30", "0-2* 3-5 1-5 1-3 1-2* 2-3 5-9"),
        ("t1star05", "0-1* 2-3 1-2 1-4 3-4 2-4* 3-9"),
    ]
    .into_iter()
    .map(|(name, topology)| EpsFamily::from_forcer(name, topology))
    .collect();
    shapes.push(t001c());
    shapes.push(t013c());
    shapes.push(EpsFamily::from_graph("T007c", "0-1* 0-2 0-3 1-2 1-3", 2, 3));
    shapes
}

fn canonical(row: &[i64]) -> Vec<i64> {
    match row.iter().find(|c| **c != 0) {
        Some(c) if *c < 0 => row.iter().map(|c| -c).collect(),
        _ => row.to_vec(),
    }
}

/// One loop at a time through two lines (a one-loop insertion), the lines
/// carrying powers `alpha + m d/2` (lines of equal momentum merged); zero for
/// a scaleless integral (a loop momentum in no line or in one line only),
/// `None` when no insertion is left before all loops are gone.
fn integrate(input: Vec<(Vec<i64>, Power)>, loops: usize, d: &Rational) -> Option<GValue> {
    let mut lines: BTreeMap<Vec<i64>, Power> = BTreeMap::new();
    let merge = |lines: &mut BTreeMap<Vec<i64>, Power>, row: &[i64], power: Power| {
        let entry = lines.entry(canonical(row)).or_insert((0, 0));
        entry.0 += power.0;
        entry.1 += power.1;
    };
    for (row, power) in input {
        merge(&mut lines, &row, power);
    }
    let mut value = GValue::rational(Rational::one());
    let mut integrated = vec![false; loops];
    loop {
        lines.retain(|_, power| *power != (0, 0));
        let hits = |l: usize| lines.keys().filter(|row| row[l] != 0).count();
        if (0..loops).any(|l| !integrated[l] && hits(l) <= 1) {
            return Some(GValue::zero());
        }
        let Some(l) = (0..loops)
            .find(|&l| !integrated[l] && hits(l) == 2 && lines.keys().all(|row| row[l].abs() <= 1))
        else {
            return (0..loops).all(|l| integrated[l]).then_some(value);
        };
        let hit: Vec<Vec<i64>> = lines.keys().filter(|row| row[l] != 0).cloned().collect();
        let a = lines.remove(&hit[0]).unwrap();
        let b = lines.remove(&hit[1]).unwrap();
        let oriented: Vec<Vec<i64>> = hit
            .iter()
            .map(|row| row.iter().map(|c| c * row[l]).collect())
            .collect();
        let bubble: Vec<i64> = oriented[1]
            .iter()
            .zip(&oriented[0])
            .map(|(x, y)| x - y)
            .collect();
        if bubble.iter().all(|c| *c == 0) {
            return Some(GValue::zero());
        }
        value = value.mul(&g(a, b, d));
        if value.is_zero() {
            return Some(value);
        }
        merge(&mut lines, &bubble, (a.0 + b.0, a.1 + b.1 - 1));
        integrated[l] = true;
    }
}

/// The value of the sector `raw` of an eps family at unit powers: raw lines
/// at `n = 1`, forced lines outside `raw` at `n = 0`, i.e. power `n + j eps
/// = (n + 2j) - j d/2`.
fn sector_value(family: &EpsFamily, raw: &[bool], d: &Rational) -> Option<GValue> {
    let lines = family
        .slots
        .iter()
        .zip(raw)
        .filter(|((_, units), active)| **active || *units != 0)
        .map(|((row, units), active)| (row.clone(), (i64::from(*active) + 2 * units, -units)))
        .collect();
    integrate(lines, family.loops, d)
}

#[test]
fn zero_analyzer_decides_eps_sectors_by_scalelessness() {
    let d = rational(37, 10);
    let mut evaluated = 0;
    for shape in eps_shapes() {
        let family = shape.family(1);
        let forced = validate_shifted_p_family(&family).unwrap().forced;
        assert_eq!(forced, shape.forced(), "{}", shape.name);
        let analyzer = Analyzer::try_unrestricted(&family).unwrap();
        for bits in 0_u32..1 << shape.lines {
            let raw: Vec<bool> = (0..shape.slots.len())
                .map(|slot| slot < shape.lines && bits >> slot & 1 == 1)
                .collect();
            let zero = matches!(
                analyzer
                    .analyze(&Mask::try_new(raw.iter().copied()).unwrap())
                    .unwrap(),
                Decision::ProvedZero(_)
            );
            if let Some(value) = sector_value(&shape, &raw, &d) {
                assert_eq!(zero, value.is_zero(), "{} sector {raw:?}", shape.name);
                evaluated += 1;
            } else {
                // Not recursively one-loop: a kite with all lines present.
                assert!(!zero, "{} sector {raw:?}", shape.name);
            }
        }
    }
    eprintln!("evaluated {evaluated} eps sectors");
    assert!(evaluated >= 190, "{evaluated}");
}

#[test]
fn zero_analyzer_refuses_only_integer_separated_shifts_and_shifted_cuts() {
    // Equal and unequal eps units are fine (d7, T005c).
    for edges in ["0-1* 0-1*", "0-1* 0-1**"] {
        let bubble = EpsFamily::from_graph("bubble", edges, 0, 1).family(1);
        assert!(Analyzer::try_unrestricted(&bubble).is_ok());
    }
    let separated = compile_text(
        "I(loops(k1),externals(q),dimension(d),prop(D1,k1^2,1),prop(D2,(k1-q)^2,1),\
         gram(q,q,1),power_shift(D1,2-d/2),power_shift(D2,3-d/2))",
    );
    assert!(matches!(
        Analyzer::try_unrestricted(&separated),
        Err(ZeroError::UnsupportedIntegerSeparatedPowerShifts { left: 0, right: 1 })
    ));
    let family = t001c().family(1);
    let cut = Restrictions::try_new(
        CutConstraint::try_from_positions(5, [0]).unwrap(),
        Pattern::any(5).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        Analyzer::try_new(&family, cut),
        Err(ZeroError::UnsupportedShiftedCut { denominator: 0 })
    ));
    let cut = Restrictions::try_new(
        CutConstraint::try_from_positions(5, [1]).unwrap(),
        Pattern::any(5).unwrap(),
    )
    .unwrap();
    assert!(Analyzer::try_new(&family, cut).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Lee-Pomeransky counts with shifts
// ---------------------------------------------------------------------------------------------

/// `rules_study/targets.tsv`: id, edges (`*` = eps unit), S, T,
/// `masters_lp_nosym`.
const LP_TARGETS_2L: [(&str, &str, u32, u32, usize); 5] = [
    ("T001", "0-1* 0-2 0-3 1-2 1-3*", 2, 3, 1),
    ("T003", "0-1 0-2 0-3 1-2* 1-3*", 2, 3, 0),
    ("T005", "0-1 0-2 0-3 1-2* 1-3**", 2, 3, 0),
    ("T007", "0-1* 0-2 0-3 1-2 1-3", 2, 3, 1),
    ("T013", "0-1* 0-2 0-3* 1-2* 1-3", 2, 3, 2),
];

const LP_TARGETS_4L: [(&str, &str, u32, u32, usize); 7] = [
    ("T070", "0-1 0-2 0-5 1-3 1-4 2-3 2-4 2-5 3-5*", 4, 5, 0),
    ("T074", "0-1 0-2 0-4 1-3 1-5 2-3 2-5 3-4 3-5*", 4, 5, 0),
    ("T075", "0-1 0-2 0-4 1-3 1-5 2-3* 2-5 3-4 3-5", 4, 5, 0),
    ("T083", "0-1 0-2 0-5 1-3 1-4 1-5 2-3* 2-4 3-5", 4, 5, 1),
    ("T090", "0-1 0-2 0-5 1-3 1-4 2-3 2-4 3-4 3-5*", 4, 5, 0),
    ("T093", "0-2 0-3 0-4 1-2 1-3 1-4 2-4 2-5 3-5*", 4, 5, 1),
    ("T148", "0-1 0-2 0-4 1-3 1-5 2-3 2-5 3-4 3-5", 4, 5, 0),
];

/// The shifted Lee-Pomeransky count of the top sector of each target.
fn check_lp(targets: &[(&str, &str, u32, u32, usize)]) {
    for &(id, edges, s, t, expected) in targets {
        let shape = EpsFamily::from_graph(id, edges, s, t);
        let family = shape.family(1);
        let start = std::time::Instant::now();
        let mut counter =
            MasterCounter::try_new_p_shifted(&family, MasterCountOptions::default()).unwrap();
        let count = counter.count(&shape.top()).unwrap();
        eprintln!(
            "{id}: {count:?} ({:.2} s, forced {:?})",
            start.elapsed().as_secs_f64(),
            shape.forced()
        );
        let expected = if expected == 0 {
            [MasterCount::Zero, MasterCount::Counted(0)]
        } else {
            [
                MasterCount::Counted(expected),
                MasterCount::Counted(expected),
            ]
        };
        assert!(expected.contains(&count), "{id}: {count:?}");
        if shape.forced().is_empty() {
            // Without eps lines the count is the integer one.
            let mut integer =
                MasterCounter::try_new(&family, MasterCountOptions::default()).unwrap();
            assert_eq!(integer.count(&shape.top()).unwrap(), count, "{id}");
        }
    }
}

#[test]
fn shifted_lp_counts_equal_the_two_loop_targets() {
    check_lp(&LP_TARGETS_2L);
}

#[test]
#[ignore = "slow: four-loop Lee-Pomeransky Euler faces (43 s release, 1 thread; run with --ignored)"]
fn shifted_lp_counts_equal_the_four_loop_targets() {
    check_lp(&LP_TARGETS_4L);
}

#[test]
fn shifted_lp_counter_admits_only_p_families_and_forced_sectors() {
    let family = t001c().family(-1);
    let mut counter =
        MasterCounter::try_new_p_shifted(&family, MasterCountOptions::default()).unwrap();
    // A sector without the eps line D5 is not a sector of the family.
    assert_eq!(
        counter
            .count(&Mask::try_new([true, true, true, true, false]).unwrap())
            .unwrap_err(),
        MasterCountError::ForcedAxisInactive { denominator: 4 }
    );
    // Every sector containing both eps lines is a sector of the family.
    for bits in 0_u32..8 {
        let sector = [true, bits & 1 == 1, bits & 2 == 2, bits & 4 == 4, true];
        assert!(counter.count(&Mask::try_new(sector).unwrap()).is_ok());
    }
    let shift = |text: &str| {
        compile_text(&format!(
            "I(loops(k1),externals(q),dimension(d),prop(D1,k1^2,1),prop(D2,(k1-q)^2,1),\
             gram(q,q,1),power_shift(D2,{text}))"
        ))
    };
    assert_eq!(
        MasterCounter::try_new_p_shifted(&shift("1/3"), MasterCountOptions::default()).unwrap_err(),
        MasterCountError::UnsupportedNonzeroPowerShift { denominator: 1 }
    );
    let vacuum = compile_text(
        "I(loops(k1,k2),externals(),dimension(d),prop(D1,k1^2-1,1),\
         prop(D2,k2^2-1,1),prop(D3,(k1+k2)^2-1,1))",
    );
    assert!(matches!(
        MasterCounter::try_new_p_shifted(&vacuum, MasterCountOptions::default()),
        Err(MasterCountError::ShiftedFamilyAdmission { .. })
    ));
    // The bubble with one eps line: G(1, 1 + eps), one master.
    let mut bubble =
        MasterCounter::try_new_p_shifted(&shift("2-d/2"), MasterCountOptions::default()).unwrap();
    assert_eq!(
        bubble.count(&Mask::try_new([true, true]).unwrap()).unwrap(),
        MasterCount::Counted(1)
    );
    assert_eq!(
        bubble
            .count(&Mask::try_new([false, true]).unwrap())
            .unwrap(),
        MasterCount::Zero
    );
}

// ---------------------------------------------------------------------------------------------
// MT9: reductions with gram(q,q,-1) under plan C1
// ---------------------------------------------------------------------------------------------

/// `(-1)^(sum of powers)`: the Minkowski sign of plan C1.
fn minkowski_sign(key: &IntegralKey) -> Rational {
    if key.powers().iter().sum::<i64>().rem_euclid(2) == 0 {
        Rational::one()
    } else {
        rational(-1, 1)
    }
}

/// Generate the LiteRed lane of `family` at `q.q = -1` and check
/// `I_M(target) = sum_i c_i I_M(master_i)` with `I_M = (-1)^(sum x) I_E` and
/// the Euclidean G-function values. Returns the number of checked identities.
fn check_minkowski_reductions(
    family: &PFamily,
    targets: &[IntegralKey],
    closed_form: impl Fn(&IntegralKey, &Rational) -> Option<Vec<GValue>>,
) -> usize {
    let native = Arc::new(compile_text(&family.source(-1)));
    assert_eq!(
        native.unit_scale_kinematics().unwrap(),
        Some(UnitScaleKinematics::UnitScalePropagator {
            external_square: -1
        })
    );
    let (mut lane, _) = DynamicLiteRedSession::generate(
        native,
        family.root,
        1,
        true,
        LiteRedOptions {
            workers: 1,
            ..LiteRedOptions::default()
        },
        None,
    )
    .unwrap();
    lane.compress(&[]).unwrap();
    let masters = lane.masters();
    let ids = lane.add_targets(targets).unwrap();
    assert_eq!(lane.statistics(&ids).uncovered, 0);
    let minus_one = rational(-1, 1);
    let mut checked = 0;
    for d in [rational(37, 10), rational(43, 10)] {
        let reduction = lane
            .reduce_exact_at_point(std::slice::from_ref(&d), &ids)
            .unwrap();
        for (target, row) in targets.iter().zip(&reduction.coefficients) {
            let expected = closed_form(target, &d)
                .or_else(|| family.value(target, &d).map(|value| vec![value]))
                .expect("every target has a Euclidean value");
            let terms: Vec<GValue> = masters
                .iter()
                .zip(row)
                .filter(|(_, c)| !c.is_zero())
                .map(|(master, c)| {
                    let euclidean = family.value(master, &d).expect("evaluable master");
                    euclidean.scale(&(c * &minkowski_sign(master)))
                })
                .collect();
            let sign = &minkowski_sign(target) * &minus_one;
            let difference = terms
                .into_iter()
                .chain(expected.iter().map(|value| value.scale(&sign)));
            assert!(
                collect(difference).is_empty(),
                "{}: {:?} at d = {d} differs from its Euclidean value under C1",
                family.name,
                target.powers()
            );
            checked += 1;
        }
    }
    checked
}

#[test]
fn bubble_reductions_at_minus_one_gram_match_the_g_function_under_c1() {
    let mut targets = Vec::new();
    for a in -3..=4_i64 {
        for b in -3..=4_i64 {
            let rank = (-a).max(0) + (-b).max(0);
            if (a > 0 || b > 0) && rank <= 3 {
                targets.push(BUBBLE.key(&[a, b]));
            }
        }
    }
    let checked = check_minkowski_reductions(&BUBBLE, &targets, |_, _| None);
    assert_eq!(checked, 2 * targets.len());
}

#[test]
fn sunset_and_kite_reductions_at_minus_one_gram_match_under_c1() {
    let top = KITE.key(&[1, 1, 1, 1, 1]);
    // K = 2/(d-4) G(1,1) [G(2,1) - G(2,3-d/2)] (Euclidean).
    let kite = |key: &IntegralKey, d: &Rational| {
        (key == &top).then(|| {
            let prefactor = &rational(2, 1) / &(d - &rational(4, 1));
            let g11 = g((1, 0), (1, 0), d).scale(&prefactor);
            vec![
                g11.mul(&g((2, 0), (1, 0), d)),
                g11.mul(&g((2, 0), (3, -1), d)).scale(&rational(-1, 1)),
            ]
        })
    };
    // The kite, the sunset {1,4,5} with dots and with each of its two ISPs
    // as a numerator (odd and even rank), and the bubble product. (A
    // numerator on both ISPs, or on the kite, has no recursively one-loop
    // Euclidean value.)
    let targets = [
        [1, 1, 1, 1, 1],
        [1, 0, 0, 1, 1],
        [2, 0, 0, 1, 1],
        [1, 0, 0, 2, 1],
        [1, 0, 0, 1, 2],
        [1, -1, 0, 1, 1],
        [1, 0, -1, 1, 1],
        [2, -2, 0, 1, 1],
        [1, 0, -3, 2, 1],
        [1, 1, 1, 1, 0],
        [2, 1, 1, 2, 0],
    ]
    .map(|powers| KITE.key(&powers));
    let checked = check_minkowski_reductions(&KITE, &targets, kite);
    assert_eq!(checked, 2 * targets.len());
}
