//! The sector-box session: top-down demand propagation, seeding, row
//! generation and elimination into one persistent modular system, and the
//! two-sample master selection, around [`BoxLaportaSession::extend`].

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Instant;

use crate::family::IntegralFamily;
use crate::sector::masters::{MasterCount, MasterCountOptions, MasterCounter};
use crate::sector::{CutConstraint, Mask};

use super::super::discovery::ColumnComparator;
use super::super::{Integral, SolverError, SourceSystem};
use super::modular::GlobalDiscovery;
use super::order::BoxOrder;
use super::router::{SectorRouter, integral_bits};
use super::rows::{SectorState, ZpRowFactory};
use super::seeds::{
    SeedBox, SeedRegion, box_coordinates, region_shell_count, region_shell_seeds, shell_seeds,
};
use super::select::{Echelon, Replay, closure, nonzero_masters};
use super::system::SelectionState;
use super::{
    BoxOptions, BoxStats, LimitKind, MasterCheckMode, ModularReport, PhaseStats, SectorReport,
    Seeding,
};

mod growth;
mod seeding;
mod stability;
#[cfg(test)]
mod tests;

/// Sub-tubes by (sector, halo, extra dots, tube-target fingerprint).
type SubtubeCache<const N: usize> = HashMap<(u64, i16, i16, [u8; 32]), Arc<BTreeSet<[i16; N]>>>;

/// A sink for one human-readable progress line per phase.
type Progress = Box<dyn FnMut(&str) + Send>;

/// One family, one row space, one column order, persisted across `extend`.
pub struct BoxLaportaSession<const N: usize> {
    family: Arc<IntegralFamily>,
    router: Arc<dyn SectorRouter<N>>,
    options: BoxOptions,
    arity: usize,
    sources: SourceSystem<N>,
    order: Arc<BoxOrder<N>>,
    preferred: Vec<[i16; N]>,
    factory: ZpRowFactory<N>,
    replay: ZpRowFactory<N>,
    modular: GlobalDiscovery<N>,
    seeds: Vec<[i16; N]>,
    /// Seeded region of every seeded sector, by Kira id.
    boxes: BTreeMap<u64, SeedRegion>,
    /// Tube seeds generated outside the seeded regions, by Kira id.
    tube_seeds: BTreeMap<u64, BTreeSet<[i16; N]>>,
    /// Ordinals (into `seeds`) of tube seeds: they carry no automorphism
    /// rows.
    tube_ids: std::collections::HashSet<u32>,
    /// Sub-tubes by (sector, halo, extra dots, tube-target fingerprint).
    subtubes: std::sync::Mutex<SubtubeCache<N>>,
    /// Growth steps per sector: `(dots, s)`.
    growth: BTreeMap<u64, (u8, u8)>,
    demand: BTreeSet<[i16; N]>,
    zero_demand: BTreeSet<[i16; N]>,
    /// Preferred integrals in representative sectors (routed when given in
    /// another sector of their class), and preferred integrals of zero
    /// sectors.
    preferred_zero: BTreeSet<[i16; N]>,
    /// Representative sectors of the down-closure of the demand and the
    /// seeded sectors, through routes (recomputed per seeding round).
    visited: BTreeSet<u64>,
    /// Truncation parameter `l` in force (staircase growth may lower it).
    truncate: Option<i16>,
    /// Times growth lowered `l`.
    truncate_steps: u8,
    /// Terms of every row generated into the current system.
    generated_terms: usize,
    masters: Vec<[i16; N]>,
    counter: Option<MasterCounter>,
    lp: HashMap<u64, MasterCount>,
    stats: BoxStats,
    pool: rayon::ThreadPool,
    started: Instant,
    progress: Option<Progress>,
    /// The last modular selection (materialisation input).
    selection: Option<SelectionState<N>>,
}

/// Second-sample seed: an independent stream derived from the first.
fn replay_seed(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn integral<const N: usize>(powers: [i16; N]) -> Result<Integral<N>, SolverError> {
    Integral::numeric(powers).map_err(SolverError::from)
}

fn powers<const N: usize>(integral: &Integral<N>) -> [i16; N] {
    std::array::from_fn(|axis| integral[axis].value())
}

fn limit(kind: LimitKind, estimate: usize, cap: usize) -> Result<(), SolverError> {
    if estimate > cap {
        return Err(SolverError::Limit {
            kind,
            estimate: estimate as u64,
            cap: cap as u64,
        });
    }
    Ok(())
}

/// All integrals with at most `max_dots` dots and `max_numerators` total
/// numerator power in every nonzero representative subsector of `top`: a
/// full-family census demand (Kira's `select_mandatory_recursively` region).
pub fn census_targets<const N: usize>(
    router: &dyn SectorRouter<N>,
    arity: usize,
    top: &[bool; N],
    max_dots: u8,
    max_numerators: u8,
) -> Vec<[i16; N]> {
    let top_bits = super::router::sector_bits(top);
    let mut targets = Vec::new();
    // Zero sectors vanish; routed sectors are covered by their
    // representatives, which need not be subsectors of `top`.
    let representative = |bits: u64| {
        let sector: [bool; N] = std::array::from_fn(|slot| bits >> slot & 1 == 1);
        router
            .representative(&sector)
            .map(|rep| super::router::sector_bits(&rep))
    };
    let mut sectors: Vec<u64> = down_closure_representatives([top_bits], representative)
        .into_iter()
        .collect();
    sectors.sort_unstable_by_key(|&bits| (std::cmp::Reverse(bits.count_ones()), bits));
    for bits in sectors {
        let lines = bits.count_ones() as i16;
        let census = SeedBox {
            r: lines + i16::from(max_dots),
            s: i16::from(max_numerators),
            d: i16::from(max_dots),
        };
        targets.extend(shell_seeds::<N>(bits as u32, arity, census, None));
    }
    targets
}

/// Representative sectors of the down-closure of `tops` through routes:
/// every nonzero subsector of a top contributes its representative, and a
/// representative reached through a routed subsector is itself a top (its
/// subsectors need not be subsectors of the original tops). `representative`
/// maps a sector id to its representative's id, `None` for a zero sector.
pub fn down_closure_representatives(
    tops: impl IntoIterator<Item = u64>,
    mut representative: impl FnMut(u64) -> Option<u64>,
) -> BTreeSet<u64> {
    let mut visited = BTreeSet::new();
    let mut expanded = BTreeSet::new();
    let mut stack: Vec<u64> = tops.into_iter().collect();
    while let Some(top) = stack.pop() {
        if top == 0 || !expanded.insert(top) {
            continue;
        }
        for bits in subsets(top) {
            if bits == 0 {
                continue;
            }
            match representative(bits) {
                None => {}
                Some(rep) if rep == bits => {
                    visited.insert(bits);
                }
                Some(rep) => {
                    visited.insert(rep);
                    if !expanded.contains(&rep) {
                        stack.push(rep);
                    }
                }
            }
        }
    }
    visited
}

/// Every subset of `bits`, including `0` and `bits`.
fn subsets(bits: u64) -> impl Iterator<Item = u64> {
    let mut next = Some(bits);
    std::iter::from_fn(move || {
        let current = next?;
        next = (current != 0).then(|| (current - 1) & bits);
        Some(current)
    })
}

struct Selection<const N: usize> {
    masters: Vec<[i16; N]>,
    first: usize,
    second: usize,
    closure_columns: usize,
    closure_pivots: usize,
    candidates: usize,
    selected: usize,
    selected_hash: String,
}

impl<const N: usize> BoxLaportaSession<N> {
    pub fn new(
        family: Arc<IntegralFamily>,
        cuts: &CutConstraint,
        router: Arc<dyn SectorRouter<N>>,
        options: BoxOptions,
    ) -> Result<Self, SolverError> {
        let arity = family.denominator_count();
        if !crate::fits_storage(arity, N) {
            return Err(SolverError::InvalidInput(format!(
                "sector-box storage {N} cannot hold {arity} denominators"
            )));
        }
        if arity > 63 {
            return Err(SolverError::InvalidInput(
                "sector-box sector ids need at most 63 denominators".into(),
            ));
        }
        if cuts.arity() != arity || cuts.required_active().active_count() != 0 {
            return Err(SolverError::InvalidInput(
                "the sector-box engine does not support cut constraints yet".into(),
            ));
        }
        if options.tubes.is_some() && !matches!(options.seeding, Seeding::Staircase { .. }) {
            return Err(SolverError::InvalidInput(
                "numerator tubes require staircase seeding".into(),
            ));
        }
        if options.tubes.is_some_and(|tubes| tubes.min_rank == 0) {
            return Err(SolverError::InvalidInput(
                "numerator tubes need min_rank >= 1".into(),
            ));
        }
        if matches!(options.seeding, Seeding::Staircase { .. })
            && options.insertion == super::Insertion::TopDownLevels
        {
            return Err(SolverError::InvalidInput(
                "staircase seeding uses bottom-up insertion".into(),
            ));
        }
        if options.settle_on_expected
            && (options.expected_counts.is_none()
                || !matches!(options.seeding, Seeding::Staircase { .. }))
        {
            return Err(SolverError::InvalidInput(
                "settle_on_expected needs staircase seeding and expected_counts".into(),
            ));
        }
        let truncate = options.truncate;
        let sources = SourceSystem::<N>::from_family_with_capacity(&family, false)?;
        let order = Arc::new(BoxOrder::with_slot_priority(
            &options.order,
            arity,
            &[],
            &options.slot_priority,
        )?);
        let factory = ZpRowFactory::new(
            &sources,
            arity,
            order.clone(),
            router.clone(),
            options.sample.prime,
            options.sample.seed,
            &[],
        )?;
        let replay = ZpRowFactory::new(
            &sources,
            arity,
            order.clone(),
            router.clone(),
            options.sample.prime,
            replay_seed(options.sample.seed),
            &[],
        )?;
        let modular = GlobalDiscovery::new(order.clone(), factory.field().clone());
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(options.workers.max(1))
            .build()
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let counter = if options.lp_gate {
            Some(
                MasterCounter::try_new(&family, MasterCountOptions::default())
                    .map_err(|error| SolverError::InvalidInput(error.to_string()))?,
            )
        } else {
            None
        };
        Ok(Self {
            family,
            router,
            options,
            arity,
            sources,
            order,
            preferred: Vec::new(),
            factory,
            replay,
            modular,
            seeds: Vec::new(),
            boxes: BTreeMap::new(),
            tube_seeds: BTreeMap::new(),
            tube_ids: std::collections::HashSet::new(),
            subtubes: std::sync::Mutex::new(HashMap::new()),
            growth: BTreeMap::new(),
            demand: BTreeSet::new(),
            zero_demand: BTreeSet::new(),
            preferred_zero: BTreeSet::new(),
            visited: BTreeSet::new(),
            truncate,
            truncate_steps: 0,
            generated_terms: 0,
            masters: Vec::new(),
            counter,
            lp: HashMap::new(),
            stats: BoxStats::default(),
            pool,
            started: Instant::now(),
            progress: None,
            selection: None,
        })
    }

    /// Receive one human-readable line per phase (seeding, elimination,
    /// selection, gates). Purely diagnostic; results do not depend on it.
    pub fn set_progress(&mut self, progress: impl FnMut(&str) + Send + 'static) {
        self.progress = Some(Box::new(progress));
    }

    fn note(&mut self, message: impl FnOnce() -> String) {
        if let Some(progress) = self.progress.as_mut() {
            let line = message();
            progress(&line);
        }
    }

    pub fn family(&self) -> &Arc<IntegralFamily> {
        &self.family
    }

    pub fn masters(&self) -> &[[i16; N]] {
        &self.masters
    }

    pub fn stats(&self) -> &BoxStats {
        &self.stats
    }

    /// Hulls of the seeded regions by Kira sector id.
    pub fn boxes(&self) -> impl Iterator<Item = (u64, SeedBox)> + '_ {
        self.boxes
            .iter()
            .filter_map(|(&bits, region)| Some((bits, region.hull()?)))
    }

    /// Seeded regions by Kira sector id.
    pub fn regions(&self) -> impl Iterator<Item = (u64, &SeedRegion)> + '_ {
        self.boxes.iter().map(|(&bits, region)| (bits, region))
    }

    /// The truncation parameter `l` in force.
    pub fn truncate(&self) -> Option<i16> {
        self.truncate
    }

    /// Modular reduced row of `target` at the first sample: `(integral,
    /// coefficient)` with `target = sum coefficient * integral` over the
    /// current system's non-pivot columns. Diagnostic only (ModularOnly).
    pub fn modular_reduction(
        &mut self,
        target: &[i16; N],
    ) -> Result<Vec<([i16; N], u64)>, SolverError> {
        use symbolica::domains::finite_field::FiniteFieldCore;
        self.check_powers(target)?;
        let target = integral(*target)?;
        self.modular.register(vec![target])?;
        let discovery = &self.modular.discovery;
        let column = discovery.column_index(&target).expect("registered");
        let reached = closure(discovery, &[column]);
        let rows = super::select::reduced_rows(discovery, &reached, &[column]);
        let field = discovery.field();
        Ok(rows[0]
            .iter()
            .map(|(column, value)| {
                (
                    powers(&discovery.columns()[*column]),
                    field.from_element(value),
                )
            })
            .collect())
    }

    fn check_powers(&self, values: &[i16; N]) -> Result<(), SolverError> {
        if values[self.arity..].iter().any(|&value| value != 0) {
            return Err(SolverError::InvalidInput(
                "sector-box integrals must be zero on storage padding".into(),
            ));
        }
        integral(*values).map(|_| ())
    }

    /// Add targets and preferred masters; seed, eliminate and select until
    /// the masters are stable. Earlier rows and seeds are kept. With Kira
    /// boxes this is one round (the boxes are fixed); growth comes with
    /// demand and staircase seeding.
    ///
    /// Preferred integrals of non-representative sectors are replaced by
    /// their single-term route onto the representative sector (an error if
    /// the route has several terms); preferred integrals of zero sectors
    /// are reported as unneeded (`zero_demand`), and preferred integrals that
    /// do not end up as masters are listed in
    /// [`ModularReport::preferred_not_masters`].
    ///
    /// A new preferred set rebuilds the whole system from the recorded seeds
    /// (the column order changes), so callers should pass all preferred
    /// integrals in one call. An unlucky sample (`UnluckyBoxSample`) is
    /// returned to the caller, who resamples with another `Sample::seed`.
    pub fn extend(
        &mut self,
        targets: &[[i16; N]],
        preferred: &[[i16; N]],
    ) -> Result<ModularReport<N>, SolverError> {
        let started = Instant::now();
        for values in targets.iter().chain(preferred) {
            self.check_powers(values)?;
        }
        let mut preferred_reps = Vec::with_capacity(preferred.len());
        for values in preferred {
            match self.preferred_representative(values)? {
                Some(representative) => preferred_reps.push(representative),
                None => {
                    self.preferred_zero.insert(*values);
                    self.zero_demand.insert(*values);
                }
            }
        }
        let mut new_preferred: Vec<[i16; N]> = self.preferred.clone();
        new_preferred.extend_from_slice(&preferred_reps);
        new_preferred.sort_unstable();
        new_preferred.dedup();
        if new_preferred != self.preferred {
            self.reorder(new_preferred)?;
        }
        for values in targets.iter().chain(&preferred_reps) {
            // Integrals of non-representative sectors are demanded through
            // their exact routes onto representative sectors.
            let keys = self.factory.route_keys(values)?;
            if keys.is_empty() {
                self.zero_demand.insert(*values);
            }
            self.demand.extend(keys);
        }
        let mut rounds = 0;
        let mut growth_log = Vec::new();
        let mut monitor = BTreeMap::new();
        // The in-box residual monitor compares with symmetry-free LP counts,
        // so it is only meaningful without symmetry rows.
        let monitored = self.router.is_identity();
        let rows_before = self.stats.rows;
        let mut selection;
        loop {
            rounds += 1;
            self.seed_round(&format!("round{rounds}"), (0, 0, 0))?;
            selection = self.select()?;
            let reports = self.sector_reports(&selection.masters)?;
            for report in reports.iter().filter(|_| monitored) {
                if let Some(lp) = report.lp
                    && report.in_box_residuals < lp
                {
                    monitor
                        .entry(report.id)
                        .or_insert((report.in_box_residuals, lp));
                }
            }
            let grown = self.grow_round(&selection.masters, &reports)?;
            let masters = selection.masters.len();
            let grown_count = grown.len();
            let truncate = self.truncate;
            self.note(|| {
                format!("round {rounds}: {masters} masters, grow {grown_count} sectors (l = {truncate:?})")
            });
            if grown.is_empty() {
                break;
            }
            growth_log.push(grown.iter().map(|&bits| bits as u32).collect());
        }
        let before = selection.masters.clone();
        // Re-check only when this call seeded anything: an extend that
        // reuses the existing system keeps the earlier stability verdict.
        let seeded = self.stats.rows > rows_before;
        let stable = match self.options.stability {
            Some(delta) if !self.boxes.is_empty() && seeded => {
                self.enlarge(delta)?;
                selection = self.select()?;
                Some(selection.masters == before)
            }
            _ => None,
        };
        self.masters = selection.masters.clone();
        let reports = self.sector_reports(&selection.masters)?;
        for report in reports.iter().filter(|_| monitored) {
            if let Some(lp) = report.lp
                && report.in_box_residuals < lp
            {
                monitor
                    .entry(report.id)
                    .or_insert((report.in_box_residuals, lp));
            }
        }
        let pivot_hash = {
            let mut hasher = blake3::Hasher::new();
            let discovery = &self.modular.discovery;
            for (column, _) in discovery.pivot_columns() {
                for power in discovery.columns()[column].powers() {
                    hasher.update(&power.value().to_le_bytes());
                }
            }
            hasher.finalize().to_hex().to_string()
        };
        let master_hash = {
            let mut hasher = blake3::Hasher::new();
            for master in &self.masters {
                for value in master {
                    hasher.update(&value.to_le_bytes());
                }
            }
            hasher.finalize().to_hex().to_string()
        };
        let master_check = self.master_check(&self.masters);
        self.refresh_totals();
        let seeds = reports.iter().map(|report| report.seeds).sum();
        let hull_seeds = reports.iter().map(|report| report.hull_seeds).sum();
        if self.options.master_check == MasterCheckMode::Strict && !master_check.passed() {
            return Err(SolverError::MasterCheck(format!(
                "{} extra masters {:?}, classes below expected {:?}, above expected {:?}",
                master_check.extra.len(),
                master_check.extra,
                master_check.missing,
                master_check.excess
            )));
        }
        let masters: BTreeSet<[i16; N]> = self.masters.iter().copied().collect();
        let preferred_not_masters = self
            .preferred
            .iter()
            .filter(|preferred| !masters.contains(*preferred))
            .copied()
            .collect();
        Ok(ModularReport {
            masters: self.masters.clone(),
            sectors: reports,
            demand: self.demand.len(),
            zero_demand: self.zero_demand.len(),
            preferred_not_masters,
            closure_columns: selection.closure_columns,
            closure_pivots: selection.closure_pivots,
            candidates: selection.candidates,
            selected_rows: selection.selected,
            pivot_hash,
            selected_hash: selection.selected_hash,
            master_hash,
            masters_first_sample: selection.first,
            masters_second_sample: selection.second,
            rounds,
            growth: growth_log,
            stable,
            masters_before_stability: before,
            monitor_violations: monitor
                .into_iter()
                .map(|(id, (residuals, lp))| (id, residuals, lp))
                .collect(),
            master_check,
            truncate: self.truncate,
            seeds,
            hull_seeds,
            seconds: started.elapsed().as_secs_f64(),
        })
    }

    /// The representative-sector integral standing for a preferred
    /// integral: itself, its single-term route, or `None` in a zero sector.
    fn preferred_representative(&self, values: &[i16; N]) -> Result<Option<[i16; N]>, SolverError> {
        let bits = integral_bits(values);
        match self.factory.sector_state(bits) {
            SectorState::Zero => Ok(None),
            SectorState::Representative => Ok(Some(*values)),
            SectorState::Routed => {
                let terms: Vec<[i16; N]> = self
                    .router
                    .route(values)?
                    .into_iter()
                    .filter(|(key, coefficient)| {
                        !coefficient.numerator.is_zero()
                            && self.factory.sector_state(integral_bits(key)) != SectorState::Zero
                    })
                    .map(|(key, _)| key)
                    .collect();
                match terms.as_slice() {
                    [image] if integral_bits(image).count_ones() == bits.count_ones() => {
                        Ok(Some(*image))
                    }
                    _ => Err(SolverError::InvalidInput(format!(
                        "preferred master {values:?} of a non-representative sector routes to {} integrals, not one of its representative sector",
                        terms.len()
                    ))),
                }
            }
        }
    }

    /// A new preferred set changes the column order: rebuild the modular
    /// system from the recorded seeds (no seed is generated twice).
    fn reorder(&mut self, preferred: Vec<[i16; N]>) -> Result<(), SolverError> {
        let preferred_integrals = preferred
            .iter()
            .map(|values| integral(*values))
            .collect::<Result<Vec<_>, _>>()?;
        self.order = Arc::new(BoxOrder::with_slot_priority(
            &self.options.order,
            self.arity,
            &preferred_integrals,
            &self.options.slot_priority,
        )?);
        self.preferred = preferred;
        self.factory = ZpRowFactory::new(
            &self.sources,
            self.arity,
            self.order.clone(),
            self.router.clone(),
            self.options.sample.prime,
            self.options.sample.seed,
            &[],
        )?;
        self.replay = ZpRowFactory::new(
            &self.sources,
            self.arity,
            self.order.clone(),
            self.router.clone(),
            self.options.sample.prime,
            replay_seed(self.options.sample.seed),
            &[],
        )?;
        self.modular = GlobalDiscovery::rebuilt(
            self.order.clone(),
            self.factory.field().clone(),
            &mut self.modular,
        )?;
        // The rebuilt system replaces the old one: size totals describe the
        // current system (and feed the row limit), so they restart here.
        // Work timings stay cumulative.
        self.stats.seeds = 0;
        self.stats.rows = 0;
        self.stats.automorphism_rows = 0;
        self.stats.empty_rows = 0;
        self.generated_terms = 0;
        self.stats.terms = 0;
        if self.seeds.is_empty() {
            return Ok(());
        }
        // Same schedule as seeding: one bottom-up batch, or levels top-down.
        let mut by_level: BTreeMap<std::cmp::Reverse<u32>, Vec<(u32, [i16; N])>> = BTreeMap::new();
        for (id, seed) in self.seeds.iter().enumerate() {
            by_level
                .entry(std::cmp::Reverse(integral_bits(seed).count_ones()))
                .or_default()
                .push((id as u32, *seed));
        }
        if self.options.insertion == super::Insertion::TopDownLevels {
            for (level, seeds) in by_level {
                self.insert_level(&format!("reorder/level{}", level.0), &seeds)?;
            }
        } else {
            let seeds: Vec<(u32, [i16; N])> = by_level.into_values().flatten().collect();
            self.insert_level("reorder/bottom-up", &seeds)?;
        }
        Ok(())
    }

    /// Representative sectors of the down-closure (through routes) of the
    /// sectors of `demand` and of every seeded sector, by Kira id.
    fn closure_of(&self, demand: &BTreeSet<[i16; N]>) -> BTreeSet<u64> {
        self.closure_with(demand, &self.boxes)
    }

    /// [`Self::closure_of`] for given seeded regions.
    fn closure_with(
        &self,
        demand: &BTreeSet<[i16; N]>,
        boxes: &BTreeMap<u64, SeedRegion>,
    ) -> BTreeSet<u64> {
        let mut tops: BTreeSet<u64> = demand.iter().map(integral_bits).collect();
        tops.extend(boxes.keys().copied());
        let factory = &self.factory;
        down_closure_representatives(tops, |bits| factory.representative_bits(bits))
    }

    /// Every representative sector of the seeded down-closure carries a
    /// nonempty region (the router seeding invariant): routes can only
    /// reach representatives of this closure, so no column is left in an
    /// unseeded sector.
    fn check_seeded(&self) -> Result<(), SolverError> {
        for &bits in &self.visited {
            if !self
                .boxes
                .get(&bits)
                .is_some_and(|region| !region.is_empty())
            {
                return Err(SolverError::SeedingInvariant(format!(
                    "representative sector {bits} of the seeded down-closure has no seeds"
                )));
            }
        }
        Ok(())
    }

    /// Merge a wanted region into a seeded one: the union for staircase
    /// seeding, the hull (one box) otherwise.
    fn merge_region(&self, old: Option<&SeedRegion>, wanted: SeedRegion) -> SeedRegion {
        if matches!(self.options.seeding, Seeding::Staircase { .. }) {
            return old.map_or(wanted.clone(), |old| old.union(&wanted));
        }
        let hull = match (old.and_then(SeedRegion::hull), wanted.hull()) {
            (Some(old), Some(wanted)) => old.hull(&wanted),
            (Some(single), None) | (None, Some(single)) => single,
            (None, None) => return SeedRegion::new(),
        };
        SeedRegion::from_box(hull)
    }

    /// Kira 3 Eq. 9 cap on inherited numerator rank for a sector with
    /// `lines` lines and explicit rank `explicit` (0 when none).
    fn truncation_cap(truncate: Option<i16>, lines: i16, explicit: i16) -> Option<i16> {
        truncate.map(|l| 1.max(lines - l + 1).max(explicit))
    }

    fn demand_columns(&mut self) -> Result<Vec<usize>, SolverError> {
        let demand: Vec<Integral<N>> = self
            .demand
            .iter()
            .map(|values| integral(*values).expect("checked demand"))
            .collect();
        self.modular.register(demand.clone())?;
        let discovery = &self.modular.discovery;
        Ok(demand
            .iter()
            .map(|integral| {
                discovery
                    .column_index(integral)
                    .expect("demand was registered")
            })
            .collect())
    }

    /// One seeding pass: plan regions level by level, highest first. With
    /// bottom-up insertion all new rows go in as one batch at the end.
    /// `delta` enlarges the explicit (demand) boxes, as in a rerun with
    /// larger `(r, s, d)` settings.
    fn seed_round(&mut self, label: &str, delta: (u8, u8, u8)) -> Result<(), SolverError> {
        self.visited = self.closure_of(&self.demand);
        let sectors = self.visited.clone();
        let Some(max_lines) = sectors.iter().map(|bits| bits.count_ones()).max() else {
            return Ok(());
        };
        let top_down = self.options.insertion == super::Insertion::TopDownLevels;
        let demand = self.demand.clone();
        let mut pending = Vec::new();
        for lines in (1..=max_lines).rev() {
            let level: Vec<u64> = sectors
                .iter()
                .copied()
                .filter(|bits| bits.count_ones() == lines)
                .collect();
            if level.is_empty() {
                continue;
            }
            let desired_started = Instant::now();
            let boxes = self.boxes.clone();
            let desired =
                self.desired_regions(&level, top_down, &boxes, &demand, self.truncate, delta)?;
            let desired_seconds = desired_started.elapsed().as_secs_f64();
            let mut seeds = Vec::new();
            for (bits, wanted) in desired {
                let old = self.boxes.get(&bits).cloned();
                let new = self.merge_region(old.as_ref(), wanted);
                if old.as_ref() == Some(&new) {
                    continue;
                }
                let shell = region_shell_seeds::<N>(bits as u32, self.arity, &new, old.as_ref());
                for seed in shell {
                    // A tube seed the region now covers is already seeded.
                    if self
                        .tube_seeds
                        .get(&bits)
                        .is_some_and(|known| known.contains(&seed))
                    {
                        continue;
                    }
                    let id = self.push_seed(seed, false)?;
                    seeds.push((id, seed));
                }
                self.boxes.insert(bits, new);
            }
            if self.options.tubes.is_some() {
                for &bits in &level {
                    let fresh = self.fresh_tube_seeds(
                        bits,
                        self.boxes.get(&bits),
                        self.tube_seeds.get(&bits),
                        &demand,
                        delta,
                    )?;
                    for seed in fresh {
                        self.tube_seeds.entry(bits).or_default().insert(seed);
                        let id = self.push_seed(seed, true)?;
                        seeds.push((id, seed));
                    }
                }
            }
            let planned = seeds.len();
            self.note(|| {
                format!("{label}/level{lines}: {planned} new seeds planned ({desired_seconds:.2}s)")
            });
            if top_down {
                if !seeds.is_empty() {
                    self.insert_level(&format!("{label}/level{lines}"), &seeds)?;
                }
            } else {
                pending.extend(seeds);
            }
        }
        if !pending.is_empty() {
            self.insert_level(&format!("{label}/bottom-up"), &pending)?;
        }
        self.check_seeded()
    }

    /// Wanted regions of one level, before merging with seeded regions.
    /// `boxes` are the regions seeded so far (higher levels of this round
    /// included); `truncate` is the Kira 3 `l` in force; `delta` enlarges the
    /// explicit boxes of box and staircase seeding (demand seeding ignores
    /// it). The demand and staircase arms are in `seeding.rs`.
    fn desired_regions(
        &mut self,
        level: &[u64],
        use_closure: bool,
        boxes: &BTreeMap<u64, SeedRegion>,
        demand: &BTreeSet<[i16; N]>,
        truncate: Option<i16>,
        delta: (u8, u8, u8),
    ) -> Result<Vec<(u64, SeedRegion)>, SolverError> {
        let (delta_r, delta_s, delta_d) =
            (i16::from(delta.0), i16::from(delta.1), i16::from(delta.2));
        let lines_here = level[0].count_ones();
        let wanted: BTreeSet<u64> = level.iter().copied().collect();
        match self.options.seeding {
            Seeding::KiraBox { r, s, d } => {
                // Sectors holding demand are explicit (Kira's `reduce`
                // sectors); the others are truncated subsectors.
                let explicit: BTreeSet<u64> = demand
                    .iter()
                    .map(integral_bits)
                    .filter(|bits| wanted.contains(bits))
                    .collect();
                Ok(level
                    .iter()
                    .map(|&bits| {
                        let s = i16::from(s) + delta_s;
                        let s = if explicit.contains(&bits) {
                            s
                        } else {
                            Self::truncation_cap(truncate, lines_here as i16, 0)
                                .map_or(s, |cap| s.min(cap))
                        };
                        (
                            bits,
                            SeedRegion::from_box(SeedBox {
                                r: i16::from(r) + delta_r,
                                s,
                                d: i16::from(d) + delta_d,
                            }),
                        )
                    })
                    .collect())
            }
            Seeding::Demand {
                extra_r, extra_s, ..
            } => self.demand_regions(
                level,
                use_closure,
                boxes,
                demand,
                truncate,
                extra_r,
                extra_s,
            ),
            Seeding::Staircase { s_pad, d_pad, .. } => {
                self.staircase_regions(level, boxes, demand, truncate, delta, s_pad, d_pad)
            }
        }
    }

    /// Append one seed; tube seeds are recorded so that they get no
    /// automorphism rows.
    fn push_seed(&mut self, seed: [i16; N], tube: bool) -> Result<u32, SolverError> {
        let id = u32::try_from(self.seeds.len()).map_err(|_| SolverError::Limit {
            kind: LimitKind::Rows,
            estimate: self.seeds.len() as u64,
            cap: u32::MAX as u64,
        })?;
        self.seeds.push(seed);
        if tube {
            self.tube_ids.insert(id);
        }
        Ok(id)
    }

    /// Generate the rows of `seeds` in parallel and insert them as one batch.
    fn insert_level(&mut self, phase: &str, seeds: &[(u32, [i16; N])]) -> Result<(), SolverError> {
        let limits = self.options.limits;
        let estimate = seeds.len() * self.factory.source_count();
        limit(LimitKind::Rows, self.stats.rows + estimate, limits.max_rows)?;
        if let Some(wall) = limits.wall {
            limit(
                LimitKind::Wall,
                self.started.elapsed().as_secs() as usize,
                wall.as_secs() as usize,
            )?;
        }
        let generation = Instant::now();
        let factory = &self.factory;
        let tube_ids = &self.tube_ids;
        let (rows, empty) = self
            .pool
            .install(|| factory.rows_for(seeds, &|id| !tube_ids.contains(&id)))?;
        let generation_seconds = generation.elapsed().as_secs_f64();
        self.check_automorphism_closure(&rows)?;
        self.check_row_sectors(&rows)?;
        let terms = rows.iter().map(|row| row.terms.len()).sum::<usize>();
        self.generated_terms += terms;
        let automorphism_rows = rows
            .iter()
            .filter(|row| self.factory.is_automorphism_source(row.provenance.source))
            .count();
        let elimination = Instant::now();
        let outcome = self.modular.insert_batch(rows)?;
        let seconds = elimination.elapsed().as_secs_f64();
        let discovery = &self.modular.discovery;
        self.stats.seeds += seeds.len();
        self.stats.rows += outcome.rows;
        self.stats.terms = self.generated_terms;
        self.stats.automorphism_rows += automorphism_rows;
        self.stats.empty_rows += empty;
        self.stats.generation_seconds += generation_seconds;
        self.stats.elimination_seconds += seconds;
        let phase = PhaseStats {
            phase: phase.to_string(),
            seeds: seeds.len(),
            rows: outcome.rows,
            terms,
            automorphism_rows,
            columns: discovery.columns().len(),
            rank: discovery.basis_len(),
            u_nnz: discovery.u_nnz(),
            l_entries: discovery.l_entries(),
            selected: 0,
            generation_seconds,
            seconds,
            bytes: self.modular.bytes(),
        };
        let line = format!(
            "{}: seeds {} rows {} ({} automorphism, +{} empty) gen {:.2}s elim {:.2}s | cols {} rank {} u_nnz {} l {} bytes {}",
            phase.phase,
            phase.seeds,
            phase.rows,
            phase.automorphism_rows,
            empty,
            phase.generation_seconds,
            phase.seconds,
            phase.columns,
            phase.rank,
            phase.u_nnz,
            phase.l_entries,
            phase.bytes
        );
        self.note(|| line);
        limit(LimitKind::Columns, phase.columns, limits.max_columns)?;
        limit(LimitKind::UNonzeros, phase.u_nnz, limits.max_u_nnz)?;
        limit(LimitKind::LEntries, phase.l_entries, limits.max_l_entries)?;
        limit(LimitKind::Bytes, phase.bytes, limits.max_bytes)?;
        self.stats.phases.push(phase);
        Ok(())
    }

    /// Every term of a generated row lies in a representative sector of the
    /// seeded down-closure (routes never leave it).
    fn check_row_sectors(&self, rows: &[super::rows::ZpRow<N>]) -> Result<(), SolverError> {
        let dense = self.arity <= 20;
        let mut mask = Vec::new();
        if dense {
            mask = vec![false; 1usize << self.arity];
            for &bits in &self.visited {
                mask[bits as usize] = true;
            }
        }
        for row in rows {
            for term in &row.terms {
                let bits = integral_bits(&powers(&term.integral));
                let inside = if dense {
                    mask[bits as usize]
                } else {
                    self.visited.contains(&bits)
                };
                if !inside {
                    return Err(SolverError::SeedingInvariant(format!(
                        "row of seed {:?} reaches sector {bits} outside the seeded down-closure",
                        self.seeds[row.provenance.seed as usize]
                    )));
                }
            }
        }
        Ok(())
    }

    /// Seed boxes must be closed under automorphism images: every term of
    /// an automorphism row in the seed's own sector lies in that sector's
    /// seeded box (other terms are pinched into subsectors).
    fn check_automorphism_closure(
        &self,
        rows: &[super::rows::ZpRow<N>],
    ) -> Result<(), SolverError> {
        for row in rows {
            if !self.factory.is_automorphism_source(row.provenance.source) {
                continue;
            }
            let seed = &self.seeds[row.provenance.seed as usize];
            let bits = integral_bits(seed);
            let seed_box = self.boxes.get(&bits).ok_or_else(|| {
                SolverError::SeedingInvariant(format!("automorphism row of unseeded sector {bits}"))
            })?;
            for term in &row.terms {
                let values = powers(&term.integral);
                if integral_bits(&values) != bits {
                    continue;
                }
                let (lines, dots, numerators) = box_coordinates(&values);
                if !seed_box.contains(lines, dots, numerators) {
                    return Err(SolverError::InvalidInput(format!(
                        "seed box {seed_box:?} of sector {bits} is not closed under the automorphism image {values:?} of {seed:?}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Closure, two-sample master test and row selection.
    fn select(&mut self) -> Result<Selection<N>, SolverError> {
        let started = Instant::now();
        let demand = self.demand_columns()?;
        let discovery = &self.modular.discovery;
        let reached = closure(discovery, &demand);
        let closure_seconds = started.elapsed().as_secs_f64();
        let first = nonzero_masters(discovery, &reached, &demand);
        let backsub_seconds = started.elapsed().as_secs_f64() - closure_seconds;
        let roots: Vec<usize> = reached.pivots.iter().map(|&(_, row)| row).collect();
        let selected = discovery.trace_union(&roots);
        let trace_seconds = started.elapsed().as_secs_f64() - closure_seconds - backsub_seconds;
        limit(
            LimitKind::Selected,
            selected.len(),
            self.options.limits.max_selected,
        )?;
        let mut hasher = blake3::Hasher::new();
        let mut rows = Vec::with_capacity(selected.len());
        let mut expected = Vec::with_capacity(selected.len());
        let mut provenance_rows = Vec::with_capacity(selected.len());
        for &row in &selected {
            let provenance = self.modular.provenance[row];
            let seed = &self.seeds[provenance.seed as usize];
            for value in seed {
                hasher.update(&value.to_le_bytes());
            }
            hasher.update(&provenance.source.to_le_bytes());
            rows.push(self.replay.row(seed, provenance.source)?);
            provenance_rows.push((*seed, provenance.source));
            expected.push(discovery.columns()[discovery.u_row_columns(row)[0] as usize]);
        }
        let replay = Replay::new(
            self.order.clone(),
            self.replay.field().clone(),
            &rows,
            &expected,
        )?;
        let candidates: BTreeSet<Integral<N>> = reached
            .candidates
            .iter()
            .map(|&column| discovery.columns()[column])
            .collect();
        // Demanded candidates are masters by themselves at every sample.
        let mut replay_demand = Vec::new();
        let mut second: BTreeSet<Integral<N>> = BTreeSet::new();
        for &column in &demand {
            let integral = discovery.columns()[column];
            if candidates.contains(&integral) {
                second.insert(integral);
            } else {
                let index =
                    replay
                        .column_index(&integral)
                        .ok_or(SolverError::UnluckyBoxSample {
                            phase: super::BoxPhase::Replay,
                        })?;
                replay_demand.push(index);
            }
        }
        let replay_closure = closure(&replay, &replay_demand);
        for column in nonzero_masters(&replay, &replay_closure, &replay_demand) {
            second.insert(replay.column(column));
        }
        if !second.is_subset(&candidates) {
            return Err(SolverError::UnluckyBoxSample {
                phase: super::BoxPhase::MasterTest,
            });
        }
        let mut masters: Vec<Integral<N>> = first
            .iter()
            .map(|&column| discovery.columns()[column])
            .collect();
        let first_count = masters.len();
        let second_count = second.len();
        masters.extend(second.iter().copied());
        let order = self.order.clone();
        masters.sort_unstable_by(|left, right| order.compare(left, right));
        masters.dedup();
        let selected_hash = hasher.finalize().to_hex().to_string();
        let master_hash = {
            let mut hasher = blake3::Hasher::new();
            for master in &masters {
                for value in powers(master) {
                    hasher.update(&value.to_le_bytes());
                }
            }
            hasher.finalize().to_hex().to_string()
        };
        let demand_integrals: Vec<Integral<N>> = demand
            .iter()
            .map(|&column| discovery.columns()[column])
            .collect();
        self.selection = Some(SelectionState {
            rows: provenance_rows,
            pivots: expected,
            masters: masters.clone(),
            demand: demand_integrals,
            selected_hash: selected_hash.clone(),
            master_hash,
        });
        let seconds = started.elapsed().as_secs_f64();
        self.stats.selection_seconds += seconds;
        let line = format!(
            "select: closure {} cols / {} pivots / {} candidates ({closure_seconds:.2}s), back-sub {backsub_seconds:.2}s, trace {} rows ({trace_seconds:.2}s), masters {} | {} (total {seconds:.2}s)",
            reached.columns.len(),
            reached.pivots.len(),
            reached.candidates.len(),
            selected.len(),
            first_count,
            second_count
        );
        self.note(|| line);
        self.stats.phases.push(PhaseStats {
            phase: "select".into(),
            columns: reached.columns.len(),
            rank: reached.pivots.len(),
            selected: selected.len(),
            seconds,
            ..Default::default()
        });
        Ok(Selection {
            masters: masters.iter().map(powers).collect(),
            first: first_count,
            second: second_count,
            closure_columns: reached.columns.len(),
            closure_pivots: reached.pivots.len(),
            candidates: reached.candidates.len(),
            selected: selected.len(),
            selected_hash,
        })
    }

    fn lp_count(&mut self, bits: u64) -> Result<Option<MasterCount>, SolverError> {
        let Some(counter) = self.counter.as_mut() else {
            return Ok(None);
        };
        if let Some(count) = self.lp.get(&bits) {
            return Ok(Some(count.clone()));
        }
        let started = Instant::now();
        let mask = Mask::try_new((0..self.arity).map(|slot| bits >> slot & 1 == 1))
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let count = counter
            .count(&mask)
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        self.stats.lp_seconds += started.elapsed().as_secs_f64();
        self.lp.insert(bits, count.clone());
        Ok(Some(count))
    }

    fn sector_reports(&mut self, masters: &[[i16; N]]) -> Result<Vec<SectorReport>, SolverError> {
        let sectors = self.visited.clone();
        let mut per_sector: BTreeMap<u64, usize> = BTreeMap::new();
        for master in masters {
            *per_sector.entry(integral_bits(master)).or_default() += 1;
        }
        let mut residuals: BTreeMap<u64, usize> = BTreeMap::new();
        {
            let discovery = &self.modular.discovery;
            for (column, integral) in discovery.columns().iter().enumerate() {
                if discovery.pivot_row(column).is_some() {
                    continue;
                }
                let values = powers(integral);
                let bits = integral_bits(&values);
                let Some(seed_box) = self.boxes.get(&bits) else {
                    continue;
                };
                let (lines, dots, numerators) = box_coordinates(&values);
                if seed_box.contains(lines, dots, numerators) {
                    *residuals.entry(bits).or_default() += 1;
                }
            }
        }
        let mut reports = Vec::with_capacity(sectors.len());
        for bits in sectors.iter().copied().chain(per_sector.keys().copied()) {
            if reports
                .iter()
                .any(|report: &SectorReport| report.id as u64 == bits)
            {
                continue;
            }
            let count = self.lp_count(bits)?;
            let (lp, no_verdict) = match count {
                Some(MasterCount::Counted(k)) => (Some(k), false),
                Some(MasterCount::Zero) => (Some(0), false),
                Some(MasterCount::NoVerdict { .. }) => (None, true),
                None => (None, false),
            };
            let region = self.boxes.get(&bits);
            let hull = region.and_then(SeedRegion::hull);
            let tube_seeds = self.tube_seeds.get(&bits).map_or(0, |tube| {
                tube.iter()
                    .filter(|seed| {
                        let (lines, dots, numerators) = box_coordinates(*seed);
                        !region.is_some_and(|region| region.contains(lines, dots, numerators))
                    })
                    .count()
            });
            let seeds = tube_seeds
                + region.map_or(0, |region| {
                    region_shell_count(bits as u32, self.arity, region, None)
                });
            // The hull also covers the full boxes of tube targets.
            let mut full = region.cloned().unwrap_or_default();
            for seed_box in self.tube_pass_down(bits, &self.demand, (0, 0, 0)) {
                full.insert(seed_box);
            }
            let hull_seeds = full.hull().map_or(0, |hull| {
                region_shell_count(bits as u32, self.arity, &SeedRegion::from_box(hull), None)
            });
            reports.push(SectorReport {
                id: bits as u32,
                lines: bits.count_ones() as usize,
                seed_box: hull,
                seed_region: region.map_or_else(Vec::new, |region| region.boxes().to_vec()),
                seeds,
                hull_seeds,
                tube_seeds,
                masters: per_sector.get(&bits).copied().unwrap_or(0),
                lp,
                lp_no_verdict: no_verdict,
                in_box_residuals: residuals.get(&bits).copied().unwrap_or(0),
                growth: self.growth.get(&bits).map_or(0, |&(r, s)| r.max(s)),
                expected: self
                    .options
                    .expected_counts
                    .as_ref()
                    .and_then(|counts| counts.get(&bits).copied()),
            });
        }
        reports.sort_unstable_by_key(|report| report.id);
        let lp_seconds = self.stats.lp_seconds;
        self.note(|| {
            format!(
                "reports: {} sectors (LP so far {lp_seconds:.2}s)",
                reports.len()
            )
        });
        Ok(reports)
    }

    fn refresh_totals(&mut self) {
        let discovery = &self.modular.discovery;
        self.stats.columns = discovery.columns().len();
        self.stats.rank = discovery.basis_len();
        self.stats.u_nnz = discovery.u_nnz();
        self.stats.l_entries = discovery.l_entries();
        self.stats.column_batches = discovery.column_batches();
    }
}
