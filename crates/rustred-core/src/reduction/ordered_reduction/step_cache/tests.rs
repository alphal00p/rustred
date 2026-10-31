use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use symbolica::domains::integer::Integer;

use super::codec::{NodeStage, Reader, Writer, decode_node, encode_node, owner_sector};
use super::factor::FactorBasis;
use super::*;
use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::reduction::{
    OrderedKeyStage, OrderedPhaseEvent, OrderedWeightArithmetic, Reducer, ReductionLimits,
};

fn key(powers: impl IntoIterator<Item = i64>) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

/// A fresh scratch directory under the test temporary directory.
pub(crate) struct Scratch(pub PathBuf);
impl Scratch {
    pub(crate) fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustred-step-cache-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Complete scheduler state at one phase boundary: every pending identity
/// with its exact weight in pop order, the terminal output and the guards.
pub(crate) fn phase_snapshot(event: &OrderedPhaseEvent<'_>) -> Vec<String> {
    let mut lines = vec![format!(
        "{:?} {} processed={} emitted={} merged={} cancelled={}",
        event.boundary,
        event.phase,
        event.progress.processed_keys,
        event.progress.emitted_terms,
        event.progress.merged_terms,
        event.progress.cancelled_terms
    )];
    for entry in event.frontier() {
        let stage = match entry.stage {
            OrderedKeyStage::Integral => "integral".to_owned(),
            OrderedKeyStage::Route => "route".to_owned(),
            OrderedKeyStage::Apply { owner_sector } => format!("apply {owner_sector:?}"),
        };
        lines.push(format!(
            "frontier {} {stage} {:?} {}",
            entry.phase,
            entry.integral.powers(),
            entry.weight.expect("exact weight").to_expression()
        ));
    }
    for (key, weight) in event.terminals() {
        lines.push(format!(
            "terminal {:?} {}",
            key.powers(),
            weight.expect("exact weight").to_expression()
        ));
    }
    for guard in event.nonzero_conditions() {
        lines.push(format!("guard {}", guard.to_expression()));
    }
    lines
}

#[test]
fn integers_and_polynomials_round_trip_in_canonical_form() {
    let values = [
        Integer::from(0i64),
        Integer::from(-1i64),
        Integer::from(i64::MAX),
        Integer::from(i64::MIN),
        Integer::from(i128::MAX),
        Integer::from(i128::MIN),
        Integer::from(i128::from(i64::MIN) - 1),
        Integer::from(u128::MAX) * Integer::from(u128::MAX),
        -(Integer::from(u128::MAX) * Integer::from(7i64)),
    ];
    let mut writer = Writer::default();
    for value in &values {
        writer.integer(value);
    }
    let mut reader = Reader::new(&writer.bytes);
    for value in &values {
        let decoded = reader.integer().unwrap();
        // Symbolica's enum equality distinguishes representations.
        assert_eq!(&decoded, value);
        assert_eq!(
            std::mem::discriminant(&decoded),
            std::mem::discriminant(value)
        );
    }
    assert!(reader.is_empty());
    // A large-form encoding of a small value is not canonical.
    let mut writer = Writer::default();
    writer.u8(1);
    writer.u8(0);
    writer.len(1);
    writer.u8(5);
    assert!(Reader::new(&writer.bytes).integer().is_err());

    let context = CoefficientContext::try_new(["d"]).unwrap();
    let coefficient =
        context.coefficient_fixture("(123456789012345678901234567890*d^3-7*d+2)/(4*d^2-9)");
    let mut writer = Writer::default();
    writer.polynomial(&coefficient.numerator);
    writer.polynomial(&coefficient.denominator);
    let mut reader = Reader::new(&writer.bytes);
    let template = &context.template().numerator;
    let decoded = Coefficient {
        numerator: reader.polynomial(template).unwrap(),
        denominator: reader.polynomial(template).unwrap(),
    };
    assert_eq!(decoded, coefficient);
    context
        .validate_with_limits(&decoded, ExactAlgebraLimits::default())
        .unwrap();
}

#[test]
fn identities_round_trip_and_wide_identities_are_not_recorded() {
    let integral = key([3, -2, 0, 1]);
    for (stage, expected) in [
        (OrderedKeyStage::Integral, NodeStage::Integral),
        (OrderedKeyStage::Route, NodeStage::Route),
        (
            OrderedKeyStage::Apply {
                owner_sector: &[true, true, false, true],
            },
            NodeStage::Apply(0b1011),
        ),
    ] {
        let mut bytes = vec![7u8];
        assert!(encode_node(stage, &integral, &mut bytes));
        assert_eq!(
            decode_node(&bytes[1..]).unwrap(),
            (expected, integral.clone())
        );
    }
    assert_eq!(owner_sector::<4>(0b1011), Some([true, true, false, true]));
    assert_eq!(owner_sector::<3>(0b1011), None);
    let mut bytes = vec![1u8, 2];
    assert!(!encode_node(
        OrderedKeyStage::Route,
        &key([200, 1]),
        &mut bytes
    ));
    assert_eq!(bytes, [1, 2]);
    assert!(!encode_node(
        OrderedKeyStage::Apply {
            owner_sector: &[true; 65]
        },
        &key([1]),
        &mut bytes
    ));
}

#[test]
fn denominator_factorizations_multiply_back_and_share_the_basis() {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let mut basis = FactorBasis::default();
    let first = context.coefficient_fixture("1/(-12*(d-4)^2*(2*d-3)*(d+1))");
    let second = context.coefficient_fixture("(d-1)/(6*(d-4)*(3-2*d)^3)");
    let template = &context.template().numerator;
    let a = basis.factor(&first.denominator).unwrap();
    let b = basis.factor(&second.denominator).unwrap();
    assert_eq!(basis.len(), 3);
    assert_eq!(basis.expand(&a, template).unwrap(), first.denominator);
    assert_eq!(basis.expand(&b, template).unwrap(), second.denominator);
    for &(factor, _) in a.factors.iter().chain(b.factors.iter()) {
        let factor = basis.get(factor).unwrap();
        assert!(!factor.is_constant());
        assert!(!factor.lcoeff().is_negative());
    }
    let constant = context.coefficient_fixture("5/6");
    let c = basis.factor(&constant.denominator).unwrap();
    assert!(c.factors.is_empty());
    assert_eq!(c.content, Integer::from(6i64));
}

fn closed_binding(artifact: &ClosedArtifact, digest: u8) -> OrderedStepCacheBinding {
    OrderedStepCacheBinding::new(
        OrderedStepCacheAdapter::ClosedArtifact,
        [digest; 32],
        artifact.family_fingerprint(),
        artifact.coefficient_context(),
        None,
    )
}

type ClosedRun = (crate::reduction::OrderedArtifactReduction, Vec<Vec<String>>);

fn closed_run(
    artifact: &ClosedArtifact,
    inputs: &[(IntegralKey, Coefficient)],
    cache: Option<&OrderedStepCache>,
    arithmetic: OrderedWeightArithmetic,
) -> ClosedRun {
    let mut snapshots = Vec::new();
    let reduced = Reducer::reduce_artifact_linear_combination_ordered_with_step_cache(
        artifact,
        inputs.iter().cloned(),
        ReductionLimits::default(),
        arithmetic,
        cache,
        &mut |_, _| Ok(()),
        Some(&mut |event| {
            if arithmetic == OrderedWeightArithmetic::Exact {
                snapshots.push(phase_snapshot(event));
            }
            Ok(())
        }),
    )
    .unwrap();
    (reduced, snapshots)
}

fn sunset_inputs(artifact: &ClosedArtifact) -> Vec<(IntegralKey, Coefficient)> {
    let context = artifact.coefficient_context();
    vec![
        (key([2, 2, 1]), context.coefficient_fixture("1/(d-3)")),
        (key([3, 1, 1]), context.coefficient_fixture("d")),
        (key([-1, 2, 1]), context.integer(2)),
        (
            key([-2, 2, 3]),
            context.coefficient_fixture("(d-2)/(2*d-5)"),
        ),
        (key([0, 2, 2]), context.integer(-3)),
    ]
}

fn assert_same_run(expected: &ClosedRun, actual: &ClosedRun) {
    assert_eq!(actual.0.terms, expected.0.terms);
    assert_eq!(actual.0.nonzero_conditions, expected.0.nonzero_conditions);
    let mut progress = actual.0.progress;
    progress.step_cache_hits = 0;
    progress.step_cache_misses = 0;
    assert_eq!(progress, expected.0.progress);
    assert_eq!(actual.1, expected.1);
}

#[test]
fn closed_cached_runs_reproduce_every_phase_and_recompute_factorizations() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let inputs = sunset_inputs(&artifact);
    let exact = OrderedWeightArithmetic::Exact;
    let uncached = closed_run(&artifact, &inputs, None, exact);
    assert!(!uncached.1.is_empty());
    let cache = OrderedStepCache::in_memory(closed_binding(&artifact, 1), Default::default());
    let cold = closed_run(&artifact, &inputs, Some(&cache), exact);
    assert_same_run(&uncached, &cold);
    assert_eq!(cold.0.progress.step_cache_hits, 0);
    assert_eq!(
        cold.0.progress.step_cache_misses,
        uncached.0.progress.processed_keys
    );
    let warm = closed_run(&artifact, &inputs, Some(&cache), exact);
    assert_same_run(&uncached, &warm);
    let statistics = cache.statistics();
    let opaque: u64 = statistics
        .shards
        .iter()
        .map(|shard| shard.opaque_hits)
        .sum();
    // Product factorizations are recomputed; every other step is replayed.
    assert!(opaque > 0);
    assert!(warm.0.progress.step_cache_hits > 0);
    assert_eq!(
        warm.0.progress.step_cache_hits + opaque as usize,
        uncached.0.progress.processed_keys
    );
    assert_eq!(statistics.hits, warm.0.progress.step_cache_hits as u64);
    assert!(warm.0.work.rule_applications() < uncached.0.work.rule_applications());
    // Modular weights consume the same records.
    for cross_check_exact in [false, true] {
        let common = crate::reduction::ModularCommon {
            threads: 1,
            cross_check_exact,
            ..Default::default()
        };
        let sampled = OrderedWeightArithmetic::ModularSampled(
            common,
            crate::reduction::SampledOptions {
                batch_points: 4,
                ..Default::default()
            },
        );
        let polynomial = OrderedWeightArithmetic::ModularPolynomial(
            common,
            crate::reduction::PolynomialOptions::default(),
        );
        for modular in [sampled, polynomial] {
            let replayed = closed_run(&artifact, &inputs, Some(&cache), modular);
            assert_eq!(replayed.0.terms, uncached.0.terms);
            assert_eq!(replayed.0.nonzero_conditions, uncached.0.nonzero_conditions);
        }
    }
}

/// Insert every record of `source` into `target`, shards and records in
/// reverse order, as a concurrent population could interleave them.
fn copy_records_reversed(source: &OrderedStepCache, target: &OrderedStepCache) {
    let shards: Vec<_> = source.shards.read().unwrap().values().cloned().collect();
    for shard in shards.iter().rev() {
        let inner = shard.inner.read().unwrap();
        let data = &inner.data;
        let mut keys: Vec<_> = data.records.keys().copied().collect();
        keys.sort_unstable_by(|a, b| data.nodes[*b as usize].cmp(&data.nodes[*a as usize]));
        for key in keys {
            let record = &data.records[&key];
            let pair = |&(node, coefficient): &(u32, u32)| {
                (
                    &*data.nodes[node as usize],
                    &*data.coefficients[coefficient as usize],
                )
            };
            let guards: Vec<Coefficient> = record
                .guards
                .iter()
                .map(|&guard| (*data.coefficients[guard as usize]).clone())
                .collect();
            target
                .insert(
                    shard.id,
                    StepRecordInput {
                        key: &data.nodes[key as usize],
                        terms: record.terms.iter().map(pair).collect(),
                        terminals: record.terminals.iter().map(pair).collect(),
                        guards: &guards,
                        opaque: record.opaque,
                    },
                )
                .unwrap();
        }
    }
}

#[test]
fn saved_shards_are_deterministic_lazy_and_reproduce_warm_runs() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let inputs = sunset_inputs(&artifact);
    let exact = OrderedWeightArithmetic::Exact;
    let uncached = closed_run(&artifact, &inputs, None, exact);
    let first = Scratch::new("first");
    let second = Scratch::new("second");
    // Two caches populated in different orders write identical files.
    let a =
        OrderedStepCache::open(&first.0, closed_binding(&artifact, 2), Default::default()).unwrap();
    closed_run(&artifact, &inputs, Some(&a), exact);
    let report = a.save().unwrap();
    assert!(report.shards_written > 0);
    assert_eq!(report.records, a.statistics().records);
    let b = OrderedStepCache::open(&second.0, closed_binding(&artifact, 2), Default::default())
        .unwrap();
    copy_records_reversed(&a, &b);
    b.save().unwrap();
    let listing = |root: &PathBuf| {
        let mut files: Vec<_> = std::fs::read_dir(root.join(SHARD_DIRECTORY))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (
                    path.file_name().unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };
    assert_eq!(listing(&first.0), listing(&second.0));
    assert_eq!(
        std::fs::read(first.0.join(MANIFEST_FILE)).unwrap(),
        std::fs::read(second.0.join(MANIFEST_FILE)).unwrap()
    );
    // A reopened cache loads shards on first use and replays them.
    let reopened =
        OrderedStepCache::open(&first.0, closed_binding(&artifact, 2), Default::default()).unwrap();
    assert!(
        reopened
            .statistics()
            .shards
            .iter()
            .all(|shard| shard.state == OrderedStepShardState::OnDisk)
    );
    let warm = closed_run(&artifact, &inputs, Some(&reopened), exact);
    assert_same_run(&uncached, &warm);
    assert!(warm.0.progress.step_cache_hits > 0);
    let statistics = reopened.statistics();
    assert_eq!(statistics.inserts, 0);
    assert!(statistics.loaded_shards > 0);
    // Saving an unchanged cache rewrites nothing.
    assert_eq!(reopened.save().unwrap().shards_written, 0);
}

#[test]
fn mismatched_or_corrupt_caches_are_rejected_and_never_used() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let inputs = sunset_inputs(&artifact);
    let exact = OrderedWeightArithmetic::Exact;
    let scratch = Scratch::new("corrupt");
    let cache =
        OrderedStepCache::open(&scratch.0, closed_binding(&artifact, 3), Default::default())
            .unwrap();
    closed_run(&artifact, &inputs, Some(&cache), exact);
    cache.save().unwrap();
    // Another program digest: the manifest stamp differs.
    assert!(matches!(
        OrderedStepCache::open(&scratch.0, closed_binding(&artifact, 4), Default::default()),
        Err(ReductionError::OrderedStepCache { .. })
    ));
    // Another program: the binding check rejects the attachment.
    let tadpole = derive_one_loop_unit_mass_tadpole().unwrap();
    assert!(matches!(
        Reducer::reduce_artifact_linear_combination_ordered_with_step_cache(
            &tadpole,
            [(key([2]), tadpole.coefficient_context().one())],
            ReductionLimits::default(),
            exact,
            Some(&cache),
            &mut |_, _| Ok(()),
            None,
        ),
        Err(ReductionError::OrderedStepCache { .. })
    ));
    // Flip one payload byte of one shard: loading it fails loudly.
    let shard = std::fs::read_dir(scratch.0.join(SHARD_DIRECTORY))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = std::fs::read(&shard).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&shard, bytes).unwrap();
    let reopened =
        OrderedStepCache::open(&scratch.0, closed_binding(&artifact, 3), Default::default())
            .unwrap();
    let failed = Reducer::reduce_artifact_linear_combination_ordered_with_step_cache(
        &artifact,
        inputs.iter().cloned(),
        ReductionLimits::default(),
        exact,
        Some(&reopened),
        &mut |_, _| Ok(()),
        None,
    );
    assert!(matches!(
        failed,
        Err(ReductionError::OrderedStepCache { .. })
    ));
    assert_eq!(reopened.statistics().rejected_shards, 1);
}

#[test]
fn budget_and_read_only_caches_skip_records_without_changing_results() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let inputs = sunset_inputs(&artifact);
    let exact = OrderedWeightArithmetic::Exact;
    let uncached = closed_run(&artifact, &inputs, None, exact);
    let tight = OrderedStepCache::in_memory(
        closed_binding(&artifact, 5),
        OrderedStepCacheOptions {
            max_resident_bytes: Some(2_000),
            ..Default::default()
        },
    );
    for _ in 0..2 {
        assert_same_run(
            &uncached,
            &closed_run(&artifact, &inputs, Some(&tight), exact),
        );
    }
    let statistics = tight.statistics();
    assert!(statistics.skipped_inserts > 0);
    assert!(statistics.resident_bytes <= 2_000);
    let read_only = OrderedStepCache::in_memory(
        closed_binding(&artifact, 5),
        OrderedStepCacheOptions {
            populate: false,
            ..Default::default()
        },
    );
    assert_same_run(
        &uncached,
        &closed_run(&artifact, &inputs, Some(&read_only), exact),
    );
    assert_eq!(read_only.statistics().records, 0);
}

#[test]
fn records_keep_guard_order_and_terminal_coefficients() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let inputs = vec![
        (key([5]), context.coefficient_fixture("1/(d-7)")),
        (key([2]), context.coefficient_fixture("(d-4)/4")),
    ];
    let cache = OrderedStepCache::in_memory(closed_binding(&artifact, 6), Default::default());
    let exact = OrderedWeightArithmetic::Exact;
    let uncached = closed_run(&artifact, &inputs, None, exact);
    closed_run(&artifact, &inputs, Some(&cache), exact);
    let warm = closed_run(&artifact, &inputs, Some(&cache), exact);
    assert_same_run(&uncached, &warm);
    assert_eq!(
        warm.0.progress.step_cache_hits,
        uncached.0.progress.processed_keys
    );
    let terminals: BTreeMap<_, _> = warm.0.terms.clone();
    assert!(!terminals.is_empty());
    let _ = Arc::clone(&cache);
}

/// Digest of every record of `cache`, independent of its stamp: each shard
/// id with its record count and deterministic payload, in shard order.
pub(crate) fn records_digest(cache: &OrderedStepCache) -> String {
    let shards: Vec<_> = cache.shards.read().unwrap().values().cloned().collect();
    let mut hasher = blake3::Hasher::new();
    for shard in shards {
        let mut inner = shard.inner.write().unwrap();
        let (bytes, records) = cache.encode_shard(shard.id, &mut inner.data).unwrap();
        hasher.update(format!("{} {records}\n", shard.id).as_bytes());
        hasher.update(&bytes[SHARD_HEADER..]);
    }
    hasher.finalize().to_hex().to_string()
}

/// Pin the records of a fixture to the step semantics version. A changed
/// record means a checked step changed: bump [`ORDERED_STEP_SEMANTICS`] and
/// update the golden digest with it. (Saved caches are invalidated by the
/// source digest in every stamp regardless; the version names the change.)
pub(crate) fn assert_golden_records(fixture: &str, digest: &str, golden: &str) {
    assert_eq!(
        (ORDERED_STEP_SEMANTICS, digest),
        ("rustred-ordered-steps-2026-10-31", golden),
        "the recorded Ordered steps of fixture {fixture} changed: bump ORDERED_STEP_SEMANTICS \
         and update the golden digest"
    );
}

#[test]
fn closed_fixture_records_match_the_step_semantics_golden() {
    let sunset = derive_two_loop_unit_mass_sunset().unwrap();
    let cache = OrderedStepCache::in_memory(closed_binding(&sunset, 0), Default::default());
    closed_run(
        &sunset,
        &sunset_inputs(&sunset),
        Some(&cache),
        OrderedWeightArithmetic::Exact,
    );
    assert_golden_records(
        "closed sunset",
        &records_digest(&cache),
        "71f86f3459ec5f306da95eedbb8440150fca5ec703ff49b306866ac081ef3c68",
    );
}

#[test]
fn stamps_bind_the_engine_sources_and_symbolica_build() {
    assert_eq!(ORDERED_STEP_SOURCES.len(), 64);
    assert!(
        ORDERED_STEP_SOURCES
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    let engine = ordered_step_engine();
    assert!(engine.starts_with(&format!(
        "rustred {} sources {ORDERED_STEP_SOURCES} symbolica ",
        env!("CARGO_PKG_VERSION")
    )));
    assert_eq!(
        symbolica_identity("symbolica-v3.0.1-110-g9cd57678"),
        "git-9cd5767"
    );
    assert_eq!(symbolica_identity("symbolica-9cd5767"), "git-9cd5767");
    assert_eq!(symbolica_identity("symbolica-3.0.1"), "3.0.1");
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let binding = closed_binding(&artifact, 8);
    let description = binding.description();
    assert!(description.contains(&format!("\nengine {engine}\n")));
    assert!(description.ends_with("\nterminals none\n"));
    // A cache recorded by an engine built from other sources (or against
    // another Symbolica) is rejected, whatever ORDERED_STEP_SEMANTICS says.
    let scratch = Scratch::new("engine");
    let cache = OrderedStepCache::open(&scratch.0, binding.clone(), Default::default()).unwrap();
    closed_run(
        &artifact,
        &sunset_inputs(&artifact),
        Some(&cache),
        OrderedWeightArithmetic::Exact,
    );
    cache.save().unwrap();
    let manifest = scratch.0.join(MANIFEST_FILE);
    let current = std::fs::read_to_string(&manifest).unwrap();
    for other_engine in [
        engine.replace(ORDERED_STEP_SOURCES, &"0".repeat(64)),
        format!(
            "{} symbolica git-0000000",
            engine.split(" symbolica ").next().unwrap()
        ),
    ] {
        let other = binding.describe(&other_engine);
        assert_ne!(other, description);
        let stamp = hex(blake3::hash(other.as_bytes()).as_bytes());
        let written =
            current
                .replacen(&description, &other, 1)
                .replacen(&hex(&binding.stamp()), &stamp, 1);
        std::fs::write(&manifest, written).unwrap();
        assert!(matches!(
            OrderedStepCache::open(&scratch.0, binding.clone(), Default::default()),
            Err(ReductionError::OrderedStepCache { .. })
        ));
    }
    std::fs::write(&manifest, current).unwrap();
    assert!(OrderedStepCache::open(&scratch.0, binding, Default::default()).is_ok());
}
