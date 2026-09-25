//! One portable saved-owner selection for all root jobs. These are the original
//! native programs, with identical routes/terminal keys; no authority upgrade.
use super::{
    CliError, bad,
    checkpoint::{self, FileIdentity, Snapshot},
    config::Config,
    io_error, plan,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

pub(super) fn prepare(config: Config, directory: &Path) -> Result<Snapshot, CliError> {
    // Bind the reads to source identities before parsing. In particular the
    // byte-for-byte original queries and the partition must share one version.
    let mut source_files = vec![
        FileIdentity::record(&config.manifest)?,
        FileIdentity::record(&config.queries)?,
    ];
    let mut selection: Value = checkpoint::read_json(&config.manifest, 16 * 1024 * 1024)?;
    let queries: Value = checkpoint::read_json(&config.queries, 64 * 1024 * 1024)?;
    let shards = plan::partition(&selection, &queries, config.shards)?;
    if config.jobs > shards.len() {
        return Err(bad(
            "jobs exceeds the number of pending owner jobs; reduce jobs or explicitly group fewer slots",
        ));
    }
    let artifact = directory.join("artifact");
    fs::create_dir(&artifact).map_err(io_error)?;
    fs::create_dir(artifact.join("owners")).map_err(io_error)?;
    fs::create_dir(directory.join("jobs")).map_err(io_error)?;
    fs::create_dir(directory.join("bin")).map_err(io_error)?;
    let mut files = vec![];
    let mut copied = BTreeMap::<String, String>::new();
    for owner in selection["owners"]
        .as_array_mut()
        .ok_or_else(|| bad("owners must be an array"))?
    {
        let source = config.owner_base.join(
            owner["path"]
                .as_str()
                .ok_or_else(|| bad("owner path must be a string"))?,
        );
        let identity = FileIdentity::record(&source)?;
        if owner["bytes"].as_u64() != Some(identity.bytes) {
            return Err(bad(format!(
                "declared owner size differs: {}",
                source.display()
            )));
        }
        let relative = if let Some(relative) = copied.get(&identity.blake3) {
            relative.clone()
        } else {
            let relative = format!("owners/{}.rrbin", identity.blake3);
            let mut frozen = checkpoint::copy_frozen(&source, &artifact.join(&relative), false)?;
            frozen.path = Path::new("artifact").join(&relative);
            files.push(frozen);
            copied.insert(identity.blake3.clone(), relative.clone());
            relative
        };
        source_files.push(identity);
        owner["path"] = json!(relative);
    }
    checkpoint::write_json(&artifact.join("selection.json"), &selection, false)?;
    let copied_queries =
        checkpoint::copy_frozen(&config.queries, &artifact.join("queries.json"), false)?;
    if copied_queries.bytes != source_files[1].bytes
        || copied_queries.blake3 != source_files[1].blake3
    {
        return Err(bad("queries changed between planning and freezing"));
    }
    checkpoint::write_json(&directory.join("config.json"), &config, false)?;
    for relative in [
        "artifact/selection.json",
        "artifact/queries.json",
        "config.json",
    ] {
        let path = directory.join(relative);
        checkpoint::immutable(&path, false)?;
        let mut identity = FileIdentity::record(&path)?;
        identity.path = relative.into();
        files.push(identity);
    }
    for shard in &shards {
        let relative = format!("jobs/{:04}/queries.json", shard.id);
        fs::create_dir(directory.join(format!("jobs/{:04}", shard.id))).map_err(io_error)?;
        let path = directory.join(&relative);
        checkpoint::write_json(&path, &plan::shard_queries(&queries, shard), false)?;
        checkpoint::immutable(&path, false)?;
        let mut identity = FileIdentity::record(&path)?;
        identity.path = relative.into();
        files.push(identity);
    }
    let mut executable = checkpoint::copy_frozen(
        &std::env::current_exe().map_err(io_error)?,
        &directory.join("bin/rustred"),
        true,
    )?;
    executable.path = "bin/rustred".into();
    files.push(executable);
    // A mutable source racing the read/copy above is rejected before launching.
    for source in &source_files {
        source.verify(Path::new(""))?;
    }
    for relative in ["artifact/owners", "artifact", "jobs", "bin"] {
        std::fs::File::open(directory.join(relative))
            .and_then(|directory| directory.sync_all())
            .map_err(io_error)?;
    }
    let root_count = shards.iter().map(|s| s.owners.len()).sum();
    let query_count = shards.iter().map(|s| s.query_indices.len()).sum();
    let snapshot = Snapshot {
        schema: "rustred.independent-root-snapshot.v1".into(),
        config,
        shards,
        files,
        source_files,
        root_count,
        query_count,
    };
    checkpoint::write_json(&directory.join("snapshot.json"), &snapshot, false)?;
    checkpoint::immutable(&directory.join("snapshot.json"), false)?;
    Ok(snapshot)
}

pub(super) fn complete(
    directory: &Path,
    snapshot: &Snapshot,
    receipts: Vec<Value>,
) -> Result<(), CliError> {
    if receipts.len() != snapshot.shards.len() {
        return Err(bad("combined artifact requires every shard receipt"));
    }
    for receipt in &receipts {
        checkpoint::validate_completion(&receipt["native_completion"])?;
    }
    let completion = json!({"schema":"rustred.independent-root-completion.v1","state":"completed",
        "all_scheduled_domains_resolved":true,"all_shards_completed":true,"starting_owner_count":snapshot.root_count,
        "starting_query_count":snapshot.query_count,"shards":receipts,"selection":"selection.json","queries":"queries.json",
        "rules":"original saved native owner programs; one payload per content digest","owner_manifest_changes":"relative payload paths only",
        "shared_descendant_coverage":false,"family_closure_claim":false,"ibp_generation":false});
    checkpoint::write_json(
        &directory.join("artifact/completion.json"),
        &completion,
        true,
    )
}
