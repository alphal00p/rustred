use super::*;

pub(super) fn save(options: &MasterReductionOptions, report: &Value) -> Result<(), AppError> {
    write_json(&options.directory.join("import-state.json"), report)
}
pub(super) fn archive(
    options: &MasterReductionOptions,
    name: &str,
    bytes: &[u8],
) -> Result<Value, AppError> {
    let relative = format!("proof-import/{name}");
    let path = storage::safe_child(&options.directory, &relative)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(io)?;
    write_file_atomically(&path, bytes, true).map_err(io)?;
    Ok(
        json!({"path":relative,"blake3":blake3::hash(bytes).to_hex().to_string(),"bytes":bytes.len()}),
    )
}
pub(super) fn archived(
    options: &MasterReductionOptions,
    descriptor: &Value,
) -> Result<Vec<u8>, AppError> {
    let name = descriptor["path"]
        .as_str()
        .ok_or_else(|| AppError::input("import archive path missing"))?;
    let hash = descriptor["blake3"]
        .as_str()
        .ok_or_else(|| AppError::input("import archive digest missing"))?;
    let bytes = recipe::checked(&storage::safe_child(&options.directory, name)?, hash)?;
    if descriptor["bytes"].as_u64() != Some(bytes.len() as u64) {
        return Err(AppError::input("import archive size mismatch"));
    }
    Ok(bytes)
}
pub(super) fn open(
    source: &Path,
    path: &Path,
    options: &MasterReductionOptions,
) -> Result<Value, AppError> {
    let bytes = storage::read_bounded(path, profile::native_io_limits(None).max_program_bytes)?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    recipe::Recipe::parse(&bytes)?;
    let source_absolute = std::path::absolute(source).map_err(io)?;
    let latest = options.directory.join("import-state.json");
    if options.resume {
        let mut report = storage::read_json_with_policy(&latest, options.finite_policy.as_ref())?;
        if report["proof_import"]["schema"] != IMPORT_SCHEMA
            || report["proof_import"]["recipe"]["blake3"].as_str() != Some(hash.as_str())
        {
            return Err(AppError::input(
                "proof-import resume source or recipe differs",
            ));
        }
        if archived(options, &report["proof_import"]["recipe"])? != bytes {
            return Err(AppError::input("archived proof recipe changed"));
        }
        let source_file = if source.is_dir() {
            source.join("artifact.json")
        } else {
            source.to_path_buf()
        };
        if source_file.exists() {
            if let Some(expected) = report["proof_import"]["source_manifest_blake3"].as_str() {
                if storage::digest_file(&source_file)? != expected {
                    return Err(AppError::input(
                        "resume source publication content differs from the archived source",
                    ));
                }
            }
        }
        report["status"] = json!("running");
        return Ok(report);
    }
    if latest.exists() || options.directory.join("latest.json").exists() {
        return Err(AppError::input(
            "import output already exists; use --resume",
        ));
    }
    let recipe = archive(options, "recipe.json", &bytes)?;
    let report = json!({"schema":SCHEMA,"operation":"publish","status":"running","import_complete":false,
        "checkpoint":{"generation":0},"proof_import":{"schema":IMPORT_SCHEMA,"stage":"preparing",
        "recipe":recipe,"recipe_base":std::path::absolute(path).map_err(io)?.parent(),
        "source_current_path":source_absolute,"new_ordinary_source_rows":0,
        "authority":"finite exact momentum-change identities replayed at import and supplied candidate formulas; no independent ordinary-IBP certification"}});
    save(options, &report)?;
    Ok(report)
}

pub(super) fn checkpoint(
    options: &MasterReductionOptions,
    session: &TerminalRelationSession,
    report: &mut Value,
) -> Result<(), AppError> {
    let bytes = session
        .to_native_bytes(profile::native_io_limits(None))
        .map_err(io)?;
    // State bytes are content-addressed before the atomic control pointer.
    let file = format!("work-{}.rrbin", blake3::hash(&bytes).to_hex());
    report["proof_import"]["work_state"] = archive(options, &file, &bytes)?;
    report["proof_import"]["completed_assistance_rows"] =
        json!(session.statistics().completed_assistance_rows);
    report["proof_import"]["pending_assistance_rows"] =
        json!(session.statistics().pending_assistance_rows);
    report["proof_import"]["pending_assistance_keys"] =
        json!(session.statistics().pending_assistance_keys);
    report["checkpoint"]["generation"] = json!(
        report["checkpoint"]["generation"]
            .as_u64()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| AppError::input("import checkpoint generation cannot be represented"))?
    );
    save(options, report)
}
