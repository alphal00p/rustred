use super::*;

/// Explicit caller arithmetic policy. Defaults lift resource quotas while
/// retaining Symbolica's native exponent and checked representation limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalDimensionRecurrenceOptions {
    pub exact_algebra: ExactAlgebraLimits,
}

impl Default for LocalDimensionRecurrenceOptions {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::unbounded_resources(),
        }
    }
}

#[derive(Default)]
pub struct LocalDimensionRecurrenceControl<'a> {
    pub cancelled: Option<&'a dyn Fn() -> bool>,
    pub progress: Option<&'a dyn Fn(LocalDimensionRecurrenceProgress)>,
}

impl LocalDimensionRecurrenceControl<'_> {
    pub(super) fn checkpoint(&self) -> Result<(), Error> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
    pub(super) fn report(&self, progress: LocalDimensionRecurrenceProgress) {
        if let Some(observer) = self.progress {
            observer(progress);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalDimensionRecurrenceProgress {
    pub input_keys: usize,
    pub processed_keys: usize,
    pub nominated_directions: usize,
    pub supported_geometries: usize,
    pub regenerated_source_rows: usize,
    pub equations: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalDimensionRecurrenceStatistics {
    pub input_keys: usize,
    pub nominated_directions: usize,
    pub supported_geometries: usize,
    pub degenerate_grams: usize,
    pub skipped_geometries: usize,
    pub weighted_source_rows: usize,
    pub equations: usize,
    pub output_terms: usize,
    pub inventory_keys: usize,
    pub skipped_reasons: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalDimensionRecurrenceError {
    InvalidInput(String),
    UnsupportedGeometry(String),
    Algebra(String),
    Binary(String),
    Cancelled,
    Overflow(&'static str),
    IdentityMismatch,
}

impl std::fmt::Display for LocalDimensionRecurrenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(detail) => write!(formatter, "local dimension input: {detail}"),
            Self::UnsupportedGeometry(detail) => {
                write!(formatter, "unsupported local dimension geometry: {detail}")
            }
            Self::Algebra(detail) => write!(formatter, "local dimension exact algebra: {detail}"),
            Self::Binary(detail) => write!(formatter, "local dimension capsule: {detail}"),
            Self::Cancelled => {
                formatter.write_str("local dimensional recurrence preparation cancelled")
            }
            Self::Overflow(resource) => write!(formatter, "local dimension {resource} overflow"),
            Self::IdentityMismatch => formatter.write_str(
                "local dimension pair differs from its regenerated ordinary-source identity",
            ),
        }
    }
}
impl std::error::Error for LocalDimensionRecurrenceError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WeightedSource {
    pub ordinal: usize,
    pub seed: IntegralKey,
    pub weight: Coefficient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Witness {
    pub source: IntegralKey,
    pub direction: Vec<Coefficient>,
    pub spectator_rank: usize,
    pub contributions: Vec<WeightedSource>,
    pub equation: TerminalEquation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkippedWitness {
    pub source: IntegralKey,
    pub direction: Vec<Coefficient>,
    pub reason: String,
}

/// Sealed finite fixed-d consequences of reviewed local d±2 pairs. No public
/// constructor accepts claimed source weights or claimed exact equations.
pub struct LocalDimensionRecurrencePlan {
    pub(super) family: Arc<IntegralFamily>,
    pub(super) input: BTreeSet<IntegralKey>,
    pub(super) inventory: BTreeSet<IntegralKey>,
    pub(super) options: LocalDimensionRecurrenceOptions,
    pub(super) witnesses: Vec<Witness>,
    pub(super) skips: Vec<SkippedWitness>,
    pub(super) equations: BTreeMap<IntegralKey, Vec<TerminalEquation>>,
    pub(super) statistics: LocalDimensionRecurrenceStatistics,
}

impl LocalDimensionRecurrencePlan {
    pub fn options(&self) -> LocalDimensionRecurrenceOptions {
        self.options
    }
    pub fn family_fingerprint(&self) -> &str {
        self.family.fingerprint()
    }
    pub fn input_keys(&self) -> &BTreeSet<IntegralKey> {
        &self.input
    }
    pub fn inventory(&self) -> &BTreeSet<IntegralKey> {
        &self.inventory
    }
    pub fn equations_for(&self, key: &IntegralKey) -> &[TerminalEquation] {
        self.equations.get(key).map_or(&[], Vec::as_slice)
    }
    pub fn statistics(&self) -> &LocalDimensionRecurrenceStatistics {
        &self.statistics
    }
}
