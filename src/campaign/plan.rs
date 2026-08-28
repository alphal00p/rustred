//! Deterministic, topology-neutral planning for multi-root campaigns.
//!
//! V1 owns compact ingress, family, job, and strict proper-subsector metadata.
//! It does not generate relations, construct a reducer, track execution, or
//! claim that any job is solved/closed. Readiness is a pure projection over an
//! externally supplied, dependency-closed completion prefix.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::{IntegralFamily, IntegralOrderingPolicy, SectorFoundationError, SectorMask};

pub const CAMPAIGN_PLAN_V1_SCHEMA: &str = "rustred.campaign-plan.v1";
const DEFAULT_MAX_ROOT_ID_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignPlanLimits {
    pub max_roots: usize,
    pub max_families: usize,
    pub max_jobs: usize,
    pub max_dependency_edges: usize,
    pub max_dependency_witness_positions: usize,
    pub max_root_id_bytes: usize,
    pub max_total_root_id_bytes: usize,
    pub max_family_identity_bytes: usize,
    pub max_total_family_identity_bytes: usize,
}

impl Default for CampaignPlanLimits {
    fn default() -> Self {
        Self {
            max_roots: 1_000_000,
            max_families: 1_000_000,
            max_jobs: 16_000_000,
            max_dependency_edges: 64_000_000,
            max_dependency_witness_positions: 256_000_000,
            max_root_id_bytes: DEFAULT_MAX_ROOT_ID_BYTES,
            max_total_root_id_bytes: 256 * 1024 * 1024,
            max_family_identity_bytes: portable_limit(1024u128 * 1024 * 1024),
            max_total_family_identity_bytes: portable_limit(16u128 * 1024 * 1024 * 1024),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CampaignPlanStats {
    roots: usize,
    families: usize,
    jobs: usize,
    dependency_edges: usize,
    dependency_witness_positions: usize,
    total_root_id_bytes: usize,
    total_family_identity_bytes: usize,
}

impl CampaignPlanStats {
    pub const fn roots(self) -> usize {
        self.roots
    }

    pub const fn families(self) -> usize {
        self.families
    }

    pub const fn jobs(self) -> usize {
        self.jobs
    }

    pub const fn dependency_edges(self) -> usize {
        self.dependency_edges
    }

    pub const fn dependency_witness_positions(self) -> usize {
        self.dependency_witness_positions
    }

    pub const fn total_root_id_bytes(self) -> usize {
        self.total_root_id_bytes
    }

    pub const fn total_family_identity_bytes(self) -> usize {
        self.total_family_identity_bytes
    }
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CampaignRootId(Arc<String>);

impl CampaignRootId {
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, CampaignPlanError> {
        Self::try_new_with_limit(value, DEFAULT_MAX_ROOT_ID_BYTES)
    }

    pub fn try_new_with_limit(
        value: impl AsRef<str>,
        max_bytes: usize,
    ) -> Result<Self, CampaignPlanError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(CampaignPlanError::EmptyRootId);
        }
        check_limit("campaign root identifier bytes", value.len(), max_bytes)?;
        Ok(Self(Arc::new(try_copy_string(
            value,
            "campaign root identifier",
        )?)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn shares_owner_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for CampaignRootId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignRootId")
            .field("encoded_bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CampaignRootId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Opaque exact-representation identity. There is intentionally no public
/// constructor from arbitrary bytes: the owner is shared from an authenticated
/// [`IntegralFamily`] fingerprint.
#[derive(Clone)]
pub struct CampaignFamilyId(Arc<String>);

impl CampaignFamilyId {
    fn try_from_family(
        family: &IntegralFamily,
        max_bytes: usize,
    ) -> Result<Self, CampaignPlanError> {
        check_limit(
            "campaign family identity bytes",
            family.fingerprint_ref().len(),
            max_bytes,
        )?;
        Ok(Self(Arc::clone(family.fingerprint_owner())))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn encoded_bytes(&self) -> usize {
        self.0.len()
    }

    fn shares_owner_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for CampaignFamilyId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

impl Eq for CampaignFamilyId {}

impl PartialOrd for CampaignFamilyId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CampaignFamilyId {
    fn cmp(&self, other: &Self) -> Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            Ordering::Equal
        } else {
            self.0.cmp(&other.0)
        }
    }
}

impl fmt::Debug for CampaignFamilyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignFamilyId")
            .field("encoded_bytes", &self.encoded_bytes())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct CampaignJobKey {
    family: CampaignFamilyId,
    sector: Arc<SectorMask>,
    ordering: IntegralOrderingPolicy,
}

impl PartialEq for CampaignJobKey {
    fn eq(&self, other: &Self) -> bool {
        self.family == other.family
            && (Arc::ptr_eq(&self.sector, &other.sector) || self.sector == other.sector)
            && self.ordering == other.ordering
    }
}

impl Eq for CampaignJobKey {}

impl PartialOrd for CampaignJobKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CampaignJobKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.family
            .cmp(&other.family)
            .then_with(|| {
                if Arc::ptr_eq(&self.sector, &other.sector) {
                    Ordering::Equal
                } else {
                    self.sector.cmp(&other.sector)
                }
            })
            .then_with(|| self.ordering.cmp(&other.ordering))
    }
}

impl CampaignJobKey {
    fn new(family: CampaignFamilyId, sector: SectorMask, ordering: IntegralOrderingPolicy) -> Self {
        Self {
            family,
            sector: Arc::new(sector),
            ordering,
        }
    }

    pub const fn family_id(&self) -> &CampaignFamilyId {
        &self.family
    }

    pub fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub const fn ordering(&self) -> IntegralOrderingPolicy {
        self.ordering
    }

    fn shares_owners_with(&self, other: &Self) -> bool {
        self.family.shares_owner_with(&other.family) && Arc::ptr_eq(&self.sector, &other.sector)
    }
}

impl fmt::Debug for CampaignJobKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignJobKey")
            .field("family_identity_bytes", &self.family.encoded_bytes())
            .field("sector", &self.sector())
            .field("ordering", &self.ordering)
            .finish()
    }
}

#[derive(Clone)]
pub struct CampaignRootSpec {
    id: CampaignRootId,
    family: Arc<IntegralFamily>,
    sector: SectorMask,
}

impl CampaignRootSpec {
    pub fn new(id: CampaignRootId, family: Arc<IntegralFamily>, sector: SectorMask) -> Self {
        Self { id, family, sector }
    }

    pub fn try_new(
        id: impl AsRef<str>,
        family: Arc<IntegralFamily>,
        sector: SectorMask,
    ) -> Result<Self, CampaignPlanError> {
        Ok(Self::new(CampaignRootId::try_new(id)?, family, sector))
    }

    pub const fn id(&self) -> &CampaignRootId {
        &self.id
    }

    pub const fn family(&self) -> &Arc<IntegralFamily> {
        &self.family
    }

    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
}

impl fmt::Debug for CampaignRootSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignRootSpec")
            .field("id", &self.id)
            .field("family_name_bytes", &self.family.name().len())
            .field(
                "family_identity_bytes",
                &self.family.fingerprint_ref().len(),
            )
            .field("sector", &self.sector)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignRootRecord {
    id: CampaignRootId,
    job: CampaignJobKey,
}

impl CampaignRootRecord {
    pub const fn id(&self) -> &CampaignRootId {
        &self.id
    }

    pub const fn job(&self) -> &CampaignJobKey {
        &self.job
    }
}

#[derive(Clone)]
pub struct CampaignFamilyRecord {
    id: CampaignFamilyId,
    family: Arc<IntegralFamily>,
}

impl CampaignFamilyRecord {
    pub const fn id(&self) -> &CampaignFamilyId {
        &self.id
    }

    pub const fn family(&self) -> &Arc<IntegralFamily> {
        &self.family
    }
}

impl fmt::Debug for CampaignFamilyRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignFamilyRecord")
            .field("id", &self.id)
            .field("name_bytes", &self.family.name().len())
            .finish()
    }
}

impl PartialEq for CampaignFamilyRecord {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for CampaignFamilyRecord {}

/// Replayable proof that one job sector is a strict proper subsector of
/// another. Positions are denominator ordinals active in the parent and
/// inactive in the child.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProperSubsectorWitness {
    contracted_positions: Arc<[usize]>,
}

impl ProperSubsectorWitness {
    fn try_contracted_count(
        parent: &CampaignJobKey,
        child: &CampaignJobKey,
    ) -> Result<usize, CampaignPlanError> {
        if parent.family != child.family || parent.ordering != child.ordering {
            return Err(CampaignPlanError::DependencyWitnessMismatch {
                detail: "proper-subsector endpoints do not share family and ordering",
            });
        }
        if parent.sector().arity() != child.sector().arity() {
            return Err(CampaignPlanError::WrongDependencyArity {
                expected: parent.sector().arity(),
                actual: child.sector().arity(),
            });
        }
        let mut contracted_count = 0usize;
        for (&parent_active, &child_active) in parent
            .sector()
            .active_bits()
            .iter()
            .zip(child.sector().active_bits())
        {
            match (parent_active, child_active) {
                (false, true) => {
                    return Err(CampaignPlanError::NonDescendingDependency {
                        parent: parent.clone(),
                        child: child.sector().clone(),
                    });
                }
                (true, false) => {
                    contracted_count =
                        checked_add("proper-subsector witness positions", contracted_count, 1)?;
                }
                _ => {}
            }
        }
        if contracted_count == 0 {
            return Err(CampaignPlanError::NonDescendingDependency {
                parent: parent.clone(),
                child: child.sector().clone(),
            });
        }
        Ok(contracted_count)
    }

    fn try_new_with_count(
        parent: &CampaignJobKey,
        child: &CampaignJobKey,
        contracted_count: usize,
    ) -> Result<Self, CampaignPlanError> {
        let mut contracted_positions = Vec::new();
        contracted_positions
            .try_reserve_exact(contracted_count)
            .map_err(|_| CampaignPlanError::AllocationFailure {
                resource: "proper-subsector witness positions",
                requested: contracted_count,
            })?;
        for (position, (&parent_active, &child_active)) in parent
            .sector()
            .active_bits()
            .iter()
            .zip(child.sector().active_bits())
            .enumerate()
        {
            if parent_active && !child_active {
                contracted_positions.push(position);
            }
        }
        if contracted_positions.len() != contracted_count {
            return Err(CampaignPlanError::DependencyWitnessMismatch {
                detail: "proper-subsector endpoints changed after witness preflight",
            });
        }
        Ok(Self {
            contracted_positions: Arc::from(contracted_positions),
        })
    }

    pub fn contracted_positions(&self) -> &[usize] {
        &self.contracted_positions
    }

    pub fn replay(
        &self,
        parent: &CampaignJobKey,
        child: &CampaignJobKey,
    ) -> Result<(), CampaignPlanError> {
        if parent.family != child.family || parent.ordering != child.ordering {
            return Err(CampaignPlanError::DependencyWitnessMismatch {
                detail: "proper-subsector endpoints do not share family and ordering",
            });
        }
        if parent.sector().arity() != child.sector().arity() {
            return Err(CampaignPlanError::WrongDependencyArity {
                expected: parent.sector().arity(),
                actual: child.sector().arity(),
            });
        }
        let mut witness = self.contracted_positions.iter().copied();
        let mut expected = witness.next();
        for (position, (&parent_active, &child_active)) in parent
            .sector()
            .active_bits()
            .iter()
            .zip(child.sector().active_bits())
            .enumerate()
        {
            match (parent_active, child_active) {
                (false, true) => {
                    return Err(CampaignPlanError::DependencyWitnessMismatch {
                        detail: "child activates a parent-inactive denominator",
                    });
                }
                (true, false) if expected == Some(position) => expected = witness.next(),
                (true, false) => {
                    return Err(CampaignPlanError::DependencyWitnessMismatch {
                        detail: "witness omits or misorders a contracted denominator",
                    });
                }
                _ if expected == Some(position) => {
                    return Err(CampaignPlanError::DependencyWitnessMismatch {
                        detail: "witness names an uncontracted denominator",
                    });
                }
                _ => {}
            }
        }
        if self.contracted_positions.is_empty() || expected.is_some() {
            return Err(CampaignPlanError::DependencyWitnessMismatch {
                detail: "proper-subsector witness is empty or has trailing positions",
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlannedCampaignJob {
    dependencies: BTreeMap<CampaignJobKey, ProperSubsectorWitness>,
}

impl PlannedCampaignJob {
    pub const fn dependencies(&self) -> &BTreeMap<CampaignJobKey, ProperSubsectorWitness> {
        &self.dependencies
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignRootInsertion {
    Added {
        job: CampaignJobKey,
        family_added: bool,
        job_added: bool,
    },
    AlreadyPresent {
        job: CampaignJobKey,
    },
}

impl CampaignRootInsertion {
    pub const fn job(&self) -> &CampaignJobKey {
        match self {
            Self::Added { job, .. } | Self::AlreadyPresent { job } => job,
        }
    }

    pub const fn was_added(&self) -> bool {
        matches!(self, Self::Added { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignDependencyInsertion {
    Added {
        child: CampaignJobKey,
        job_added: bool,
    },
    AlreadyPresent {
        child: CampaignJobKey,
    },
}

impl CampaignDependencyInsertion {
    pub const fn child(&self) -> &CampaignJobKey {
        match self {
            Self::Added { child, .. } | Self::AlreadyPresent { child } => child,
        }
    }

    pub const fn was_added(&self) -> bool {
        matches!(self, Self::Added { .. })
    }
}

#[derive(Clone)]
pub struct CampaignPlan {
    schema: &'static str,
    ordering: IntegralOrderingPolicy,
    roots: BTreeMap<CampaignRootId, CampaignRootRecord>,
    families: BTreeMap<CampaignFamilyId, CampaignFamilyRecord>,
    jobs: BTreeMap<CampaignJobKey, PlannedCampaignJob>,
    limits: CampaignPlanLimits,
    stats: CampaignPlanStats,
}

impl fmt::Debug for CampaignPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CampaignPlan")
            .field("schema", &self.schema)
            .field("ordering", &self.ordering)
            .field("stats", &self.stats)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// Mathematical plan equality deliberately excludes resource limits and
/// derived statistics. It also has no execution revision or progress state.
impl PartialEq for CampaignPlan {
    fn eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.ordering == other.ordering
            && self.roots == other.roots
            && self.families == other.families
            && self.jobs == other.jobs
    }
}

impl Eq for CampaignPlan {}

impl CampaignPlan {
    pub fn compile(
        roots: impl IntoIterator<Item = CampaignRootSpec>,
        ordering: IntegralOrderingPolicy,
        limits: CampaignPlanLimits,
    ) -> Result<Self, CampaignPlanError> {
        let mut plan = Self {
            schema: CAMPAIGN_PLAN_V1_SCHEMA,
            ordering,
            roots: BTreeMap::new(),
            families: BTreeMap::new(),
            jobs: BTreeMap::new(),
            limits,
            stats: CampaignPlanStats::default(),
        };
        for root in roots {
            plan.try_insert_root(root)?;
        }
        if plan.roots.is_empty() {
            return Err(CampaignPlanError::NoRoots);
        }
        plan.verify()?;
        Ok(plan)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn ordering(&self) -> IntegralOrderingPolicy {
        self.ordering
    }

    pub const fn limits(&self) -> CampaignPlanLimits {
        self.limits
    }

    pub const fn stats(&self) -> CampaignPlanStats {
        self.stats
    }

    pub const fn roots(&self) -> &BTreeMap<CampaignRootId, CampaignRootRecord> {
        &self.roots
    }

    pub const fn families(&self) -> &BTreeMap<CampaignFamilyId, CampaignFamilyRecord> {
        &self.families
    }

    pub const fn jobs(&self) -> &BTreeMap<CampaignJobKey, PlannedCampaignJob> {
        &self.jobs
    }

    pub fn root(&self, id: &CampaignRootId) -> Option<&CampaignRootRecord> {
        self.roots.get(id)
    }

    pub fn family(&self, id: &CampaignFamilyId) -> Option<&CampaignFamilyRecord> {
        self.families.get(id)
    }

    pub fn job(&self, key: &CampaignJobKey) -> Option<&PlannedCampaignJob> {
        self.jobs.get(key)
    }

    pub fn intrinsic_jobs(&self) -> impl ExactSizeIterator<Item = &CampaignJobKey> {
        self.jobs.keys()
    }

    pub fn try_insert_root(
        &mut self,
        root: CampaignRootSpec,
    ) -> Result<CampaignRootInsertion, CampaignPlanError> {
        if root.sector.arity() != root.family.denominator_count() {
            return Err(CampaignPlanError::WrongSectorArity {
                root: root.id,
                expected: root.family.denominator_count(),
                actual: root.sector.arity(),
            });
        }
        check_limit(
            "campaign root identifier bytes",
            root.id.as_str().len(),
            self.limits.max_root_id_bytes,
        )?;
        let family_id =
            CampaignFamilyId::try_from_family(&root.family, self.limits.max_family_identity_bytes)?;
        let (family_id, family_added) =
            if let Some((canonical_id, existing)) = self.families.get_key_value(&family_id) {
                if existing.family.limits() != root.family.limits() {
                    return Err(CampaignPlanError::FamilyResourcePolicyConflict {
                        family: canonical_id.clone(),
                    });
                }
                (canonical_id.clone(), false)
            } else {
                (family_id, true)
            };
        let candidate_job = CampaignJobKey::new(family_id.clone(), root.sector, self.ordering);
        let (job, job_added) =
            if let Some((canonical_job, _)) = self.jobs.get_key_value(&candidate_job) {
                (canonical_job.clone(), false)
            } else {
                (candidate_job, true)
            };
        if let Some(existing) = self.roots.get(&root.id) {
            return if existing.job == job {
                Ok(CampaignRootInsertion::AlreadyPresent { job })
            } else {
                Err(CampaignPlanError::RootConflict { root: root.id })
            };
        }

        let next_roots = checked_add("campaign roots", self.stats.roots, 1)?;
        check_limit("campaign roots", next_roots, self.limits.max_roots)?;
        let next_root_bytes = checked_add(
            "campaign root identifier bytes",
            self.stats.total_root_id_bytes,
            root.id.as_str().len(),
        )?;
        check_limit(
            "campaign root identifier bytes",
            next_root_bytes,
            self.limits.max_total_root_id_bytes,
        )?;
        let next_families = checked_add(
            "campaign families",
            self.stats.families,
            usize::from(family_added),
        )?;
        check_limit("campaign families", next_families, self.limits.max_families)?;
        let next_family_bytes = if family_added {
            checked_add(
                "campaign family identity bytes",
                self.stats.total_family_identity_bytes,
                family_id.encoded_bytes(),
            )?
        } else {
            self.stats.total_family_identity_bytes
        };
        check_limit(
            "campaign family identity bytes",
            next_family_bytes,
            self.limits.max_total_family_identity_bytes,
        )?;
        let next_jobs = checked_add("campaign jobs", self.stats.jobs, usize::from(job_added))?;
        check_limit("campaign jobs", next_jobs, self.limits.max_jobs)?;

        if family_added {
            self.families.insert(
                family_id.clone(),
                CampaignFamilyRecord {
                    id: family_id,
                    family: root.family,
                },
            );
        }
        if job_added {
            self.jobs.insert(job.clone(), PlannedCampaignJob::default());
        }
        self.roots.insert(
            root.id.clone(),
            CampaignRootRecord {
                id: root.id,
                job: job.clone(),
            },
        );
        self.stats.roots = next_roots;
        self.stats.families = next_families;
        self.stats.jobs = next_jobs;
        self.stats.total_root_id_bytes = next_root_bytes;
        self.stats.total_family_identity_bytes = next_family_bytes;
        Ok(CampaignRootInsertion::Added {
            job,
            family_added,
            job_added,
        })
    }

    pub fn try_add_strict_subsector_dependency(
        &mut self,
        parent: &CampaignJobKey,
        child_sector: SectorMask,
    ) -> Result<CampaignDependencyInsertion, CampaignPlanError> {
        let Some((canonical_parent, _)) = self.jobs.get_key_value(parent) else {
            return Err(CampaignPlanError::UnknownJob {
                job: parent.clone(),
            });
        };
        let parent = canonical_parent.clone();
        if child_sector.arity() != parent.sector().arity() {
            return Err(CampaignPlanError::WrongDependencyArity {
                expected: parent.sector().arity(),
                actual: child_sector.arity(),
            });
        }
        let candidate_child =
            CampaignJobKey::new(parent.family.clone(), child_sector, parent.ordering);
        let (child, job_added) =
            if let Some((canonical_child, _)) = self.jobs.get_key_value(&candidate_child) {
                (canonical_child.clone(), false)
            } else {
                (candidate_child, true)
            };
        let contracted_count = ProperSubsectorWitness::try_contracted_count(&parent, &child)?;
        if let Some(existing) = self
            .jobs
            .get(&parent)
            .and_then(|job| job.dependencies.get(&child))
        {
            existing.replay(&parent, &child)?;
            if existing.contracted_positions().len() != contracted_count {
                return Err(CampaignPlanError::DependencyWitnessConflict { parent, child });
            }
            return Ok(CampaignDependencyInsertion::AlreadyPresent { child });
        }

        let next_jobs = checked_add("campaign jobs", self.stats.jobs, usize::from(job_added))?;
        check_limit("campaign jobs", next_jobs, self.limits.max_jobs)?;
        let next_edges = checked_add("campaign dependency edges", self.stats.dependency_edges, 1)?;
        check_limit(
            "campaign dependency edges",
            next_edges,
            self.limits.max_dependency_edges,
        )?;
        let next_witness_positions = checked_add(
            "campaign dependency witness positions",
            self.stats.dependency_witness_positions,
            contracted_count,
        )?;
        check_limit(
            "campaign dependency witness positions",
            next_witness_positions,
            self.limits.max_dependency_witness_positions,
        )?;
        let witness =
            ProperSubsectorWitness::try_new_with_count(&parent, &child, contracted_count)?;

        if job_added {
            self.jobs
                .insert(child.clone(), PlannedCampaignJob::default());
        }
        self.jobs
            .get_mut(&parent)
            .expect("parent was authenticated before mutation")
            .dependencies
            .insert(child.clone(), witness);
        self.stats.jobs = next_jobs;
        self.stats.dependency_edges = next_edges;
        self.stats.dependency_witness_positions = next_witness_positions;
        Ok(CampaignDependencyInsertion::Added { child, job_added })
    }

    /// Return the deterministic Kahn-ready closure antichain for a known,
    /// dependency-closed completion prefix. No heavyweight task owner is
    /// materialized.
    pub fn try_ready_job_antichain(
        &self,
        completed: &BTreeSet<CampaignJobKey>,
    ) -> Result<BTreeSet<CampaignJobKey>, CampaignPlanError> {
        for key in completed {
            let Some(job) = self.jobs.get(key) else {
                return Err(CampaignPlanError::UnknownCompletedJob { job: key.clone() });
            };
            if let Some(missing) = job
                .dependencies
                .keys()
                .find(|dependency| !completed.contains(*dependency))
            {
                return Err(CampaignPlanError::CompletionPrefixNotClosed {
                    job: key.clone(),
                    missing_dependency: missing.clone(),
                });
            }
        }
        Ok(self
            .jobs
            .iter()
            .filter(|(key, job)| {
                !completed.contains(*key)
                    && job
                        .dependencies
                        .keys()
                        .all(|dependency| completed.contains(dependency))
            })
            .map(|(key, _)| key.clone())
            .collect())
    }

    pub fn verify(&self) -> Result<(), CampaignPlanError> {
        if self.schema != CAMPAIGN_PLAN_V1_SCHEMA {
            return Err(CampaignPlanError::SchemaMismatch);
        }
        for (id, family) in &self.families {
            check_limit(
                "campaign family identity bytes",
                id.encoded_bytes(),
                self.limits.max_family_identity_bytes,
            )?;
            if id.as_str() != family.family.fingerprint_ref()
                || id != &family.id
                || !id.shares_owner_with(&family.id)
                || !Arc::ptr_eq(&id.0, family.family.fingerprint_owner())
            {
                return Err(CampaignPlanError::FamilyIdentityMismatch);
            }
        }
        for (id, root) in &self.roots {
            check_limit(
                "campaign root identifier bytes",
                id.as_str().len(),
                self.limits.max_root_id_bytes,
            )?;
            let Some((canonical_job, _)) = self.jobs.get_key_value(&root.job) else {
                return Err(CampaignPlanError::IngressBindingMismatch { root: id.clone() });
            };
            if id != &root.id
                || !id.shares_owner_with(&root.id)
                || !canonical_job.shares_owners_with(&root.job)
            {
                return Err(CampaignPlanError::IngressBindingMismatch { root: id.clone() });
            }
        }
        for (key, job) in &self.jobs {
            if key.ordering != self.ordering {
                return Err(CampaignPlanError::JobOrderingMismatch { job: key.clone() });
            }
            let Some((canonical_family_id, family)) = self.families.get_key_value(&key.family)
            else {
                return Err(CampaignPlanError::JobFamilyMissing { job: key.clone() });
            };
            if !canonical_family_id.shares_owner_with(&key.family) {
                return Err(CampaignPlanError::CanonicalOwnerMismatch {
                    detail: "campaign job retains a noncanonical family identity owner",
                });
            }
            if key.sector().arity() != family.family.denominator_count() {
                return Err(CampaignPlanError::WrongDependencyArity {
                    expected: family.family.denominator_count(),
                    actual: key.sector().arity(),
                });
            }
            for (dependency, witness) in &job.dependencies {
                let Some((canonical_dependency, _)) = self.jobs.get_key_value(dependency) else {
                    return Err(CampaignPlanError::UnknownJob {
                        job: dependency.clone(),
                    });
                };
                if !canonical_dependency.shares_owners_with(dependency) {
                    return Err(CampaignPlanError::CanonicalOwnerMismatch {
                        detail: "campaign dependency retains a noncanonical job-key owner",
                    });
                }
                witness.replay(key, dependency)?;
            }
        }
        let replayed = self.recompute_stats()?;
        if replayed != self.stats {
            return Err(CampaignPlanError::StatsMismatch {
                expected: self.stats,
                actual: replayed,
            });
        }
        self.check_stats_limits(replayed)
    }

    fn recompute_stats(&self) -> Result<CampaignPlanStats, CampaignPlanError> {
        let mut stats = CampaignPlanStats {
            roots: self.roots.len(),
            families: self.families.len(),
            jobs: self.jobs.len(),
            ..CampaignPlanStats::default()
        };
        for id in self.roots.keys() {
            stats.total_root_id_bytes = checked_add(
                "campaign root identifier bytes",
                stats.total_root_id_bytes,
                id.as_str().len(),
            )?;
        }
        for id in self.families.keys() {
            stats.total_family_identity_bytes = checked_add(
                "campaign family identity bytes",
                stats.total_family_identity_bytes,
                id.encoded_bytes(),
            )?;
        }
        for job in self.jobs.values() {
            stats.dependency_edges = checked_add(
                "campaign dependency edges",
                stats.dependency_edges,
                job.dependencies.len(),
            )?;
            for witness in job.dependencies.values() {
                stats.dependency_witness_positions = checked_add(
                    "campaign dependency witness positions",
                    stats.dependency_witness_positions,
                    witness.contracted_positions().len(),
                )?;
            }
        }
        Ok(stats)
    }

    fn check_stats_limits(&self, stats: CampaignPlanStats) -> Result<(), CampaignPlanError> {
        check_limit("campaign roots", stats.roots, self.limits.max_roots)?;
        check_limit(
            "campaign families",
            stats.families,
            self.limits.max_families,
        )?;
        check_limit("campaign jobs", stats.jobs, self.limits.max_jobs)?;
        check_limit(
            "campaign dependency edges",
            stats.dependency_edges,
            self.limits.max_dependency_edges,
        )?;
        check_limit(
            "campaign dependency witness positions",
            stats.dependency_witness_positions,
            self.limits.max_dependency_witness_positions,
        )?;
        check_limit(
            "campaign root identifier bytes",
            stats.total_root_id_bytes,
            self.limits.max_total_root_id_bytes,
        )?;
        check_limit(
            "campaign family identity bytes",
            stats.total_family_identity_bytes,
            self.limits.max_total_family_identity_bytes,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignPlanError {
    NoRoots,
    EmptyRootId,
    SchemaMismatch,
    WrongSectorArity {
        root: CampaignRootId,
        expected: usize,
        actual: usize,
    },
    RootConflict {
        root: CampaignRootId,
    },
    FamilyResourcePolicyConflict {
        family: CampaignFamilyId,
    },
    UnknownJob {
        job: CampaignJobKey,
    },
    UnknownCompletedJob {
        job: CampaignJobKey,
    },
    WrongDependencyArity {
        expected: usize,
        actual: usize,
    },
    NonDescendingDependency {
        parent: CampaignJobKey,
        child: SectorMask,
    },
    DependencyWitnessConflict {
        parent: CampaignJobKey,
        child: CampaignJobKey,
    },
    DependencyWitnessMismatch {
        detail: &'static str,
    },
    CompletionPrefixNotClosed {
        job: CampaignJobKey,
        missing_dependency: CampaignJobKey,
    },
    FamilyIdentityMismatch,
    IngressBindingMismatch {
        root: CampaignRootId,
    },
    JobFamilyMissing {
        job: CampaignJobKey,
    },
    JobOrderingMismatch {
        job: CampaignJobKey,
    },
    CanonicalOwnerMismatch {
        detail: &'static str,
    },
    StatsMismatch {
        expected: CampaignPlanStats,
        actual: CampaignPlanStats,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Sector(SectorFoundationError),
}

impl fmt::Display for CampaignPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRoots => formatter.write_str("a campaign needs at least one root"),
            Self::EmptyRootId => formatter.write_str("a campaign root identifier cannot be empty"),
            Self::SchemaMismatch => formatter.write_str("campaign plan schema mismatch"),
            Self::WrongSectorArity {
                root,
                expected,
                actual,
            } => write!(
                formatter,
                "campaign root {root} has sector arity {actual}, expected {expected}"
            ),
            Self::RootConflict { root } => write!(
                formatter,
                "campaign root identifier {root} was repeated with a different exact job"
            ),
            Self::FamilyResourcePolicyConflict { family } => write!(
                formatter,
                "campaign family with {} identity bytes was repeated with a different retained resource policy",
                family.encoded_bytes()
            ),
            Self::UnknownJob { job } => write!(
                formatter,
                "campaign job for sector {} is not present in this plan",
                job.sector()
            ),
            Self::UnknownCompletedJob { job } => write!(
                formatter,
                "completed campaign prefix contains unknown sector {}",
                job.sector()
            ),
            Self::WrongDependencyArity { expected, actual } => write!(
                formatter,
                "campaign dependency sector has arity {actual}, expected {expected}"
            ),
            Self::NonDescendingDependency { parent, child } => write!(
                formatter,
                "campaign dependency {} -> {child} is not a strict proper subsector",
                parent.sector()
            ),
            Self::DependencyWitnessConflict { parent, child } => write!(
                formatter,
                "campaign dependency {} -> {} has conflicting witnesses",
                parent.sector(),
                child.sector()
            ),
            Self::DependencyWitnessMismatch { detail } => {
                write!(
                    formatter,
                    "proper-subsector witness replay failed: {detail}"
                )
            }
            Self::CompletionPrefixNotClosed {
                job,
                missing_dependency,
            } => write!(
                formatter,
                "completed sector {} is missing completed dependency {}",
                job.sector(),
                missing_dependency.sector()
            ),
            Self::FamilyIdentityMismatch => {
                formatter.write_str("campaign family identity binding mismatch")
            }
            Self::IngressBindingMismatch { root } => {
                write!(
                    formatter,
                    "campaign ingress binding mismatch for root {root}"
                )
            }
            Self::JobFamilyMissing { job } => write!(
                formatter,
                "campaign job for sector {} has no family owner",
                job.sector()
            ),
            Self::JobOrderingMismatch { job } => write!(
                formatter,
                "campaign job for sector {} uses an ordering other than the plan ordering",
                job.sector()
            ),
            Self::CanonicalOwnerMismatch { detail } => {
                write!(formatter, "campaign canonical owner mismatch: {detail}")
            }
            Self::StatsMismatch { expected, actual } => write!(
                formatter,
                "campaign plan statistics mismatch: retained {expected:?}, replayed {actual:?}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::Sector(error) => write!(formatter, "campaign sector validation failed: {error}"),
        }
    }
}

impl std::error::Error for CampaignPlanError {}

impl From<SectorFoundationError> for CampaignPlanError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

fn portable_limit(preferred: u128) -> usize {
    usize::try_from(preferred).unwrap_or(usize::MAX)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CampaignPlanError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(CampaignPlanError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CampaignPlanError> {
    left.checked_add(right)
        .ok_or(CampaignPlanError::ResourceCountOverflow { resource })
}

fn try_copy_string(value: &str, resource: &'static str) -> Result<String, CampaignPlanError> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| CampaignPlanError::AllocationFailure {
            resource,
            requested: value.len(),
        })?;
    output.push_str(value);
    Ok(output)
}
