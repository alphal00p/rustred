//! Bounded adjacent control files. This never opens or changes native Work.
use super::policy::ControlLimits;
use crate::{AppError, application::atomic_file};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

pub(super) fn read(path: &Path, limit: usize) -> Result<Vec<u8>, AppError> {
    let metadata = fs::symlink_metadata(path).map_err(|e| AppError::execution(e.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::input(format!(
            "control file is not regular: {}",
            path.display()
        )));
    }
    if metadata.len() > limit as u64 {
        return Err(AppError::limit(
            "finite control file exceeds byte allowance",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| AppError::execution(e.to_string()))?
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::execution(e.to_string()))?;
    if bytes.len() > limit {
        return Err(AppError::limit("finite control file grew beyond allowance"));
    }
    Ok(bytes)
}

pub(super) fn directory(path: &Path) -> Result<(), AppError> {
    let metadata = fs::symlink_metadata(path).map_err(|e| AppError::execution(e.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::input(format!(
            "finite namespace is not a real directory: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Read-only accounting shared by execution and caller-bounded inspection.
pub(super) fn validate_inventory(
    root: &Path,
    limits: ControlLimits,
) -> Result<(u64, usize), AppError> {
    let (mut bytes, mut files) = (0u64, 0usize);
    let mut count = |path: &Path| -> Result<(), AppError> {
        let metadata =
            fs::symlink_metadata(path).map_err(|e| AppError::execution(e.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(AppError::input("nonregular finite control object"));
        }
        bytes = bytes
            .checked_add(metadata.len())
            .ok_or_else(|| AppError::limit("control byte overflow"))?;
        files = files
            .checked_add(1)
            .ok_or_else(|| AppError::limit("control file overflow"))?;
        if bytes > limits.max_bytes || files > limits.max_files {
            return Err(AppError::limit("existing finite controls exceed grant"));
        }
        Ok(())
    };
    for entry in fs::read_dir(root).map_err(|e| AppError::execution(e.to_string()))? {
        let entry = entry.map_err(|e| AppError::execution(e.to_string()))?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|e| AppError::execution(e.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(AppError::input("symlink in finite control namespace"));
        }
        if metadata.is_dir() {
            if entry.file_name() == "store" {
                continue; // Separate native quota/lease.
            }
            if entry.file_name() != "sources" {
                return Err(AppError::input("unknown finite control directory"));
            }
            for file in
                fs::read_dir(entry.path()).map_err(|e| AppError::execution(e.to_string()))?
            {
                let file = file.map_err(|e| AppError::execution(e.to_string()))?;
                count(&file.path())?;
            }
        } else {
            count(&entry.path())?;
        }
    }
    Ok((bytes, files))
}

/// Fixed lock ordering: this control lease is acquired before StoreWriter.
/// Dropping it releases all paths, including cancellation/error paths.
pub(super) struct ControlFiles {
    pub(super) root: PathBuf,
    _lease: File,
    limits: ControlLimits,
    bytes: u64,
    files: usize,
}
impl ControlFiles {
    pub(super) fn open(root: &Path, create: bool, limits: ControlLimits) -> Result<Self, AppError> {
        if limits.max_bytes == 0 || limits.max_files == 0 {
            return Err(AppError::input("finite control grants must be positive"));
        }
        if create {
            // No implicit recovery of an old namespace whose CURRENT vanished.
            fs::create_dir(root)
                .map_err(|e| AppError::execution(format!("fresh finite namespace: {e}")))?;
            fs::create_dir(root.join("sources")).map_err(|e| AppError::execution(e.to_string()))?;
        }
        directory(root)?;
        directory(&root.join("sources"))?;
        let lock_path = root.join("control.lock");
        if !create {
            let metadata =
                fs::symlink_metadata(&lock_path).map_err(|e| AppError::execution(e.to_string()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(AppError::input("finite control lock is not regular"));
            }
        }
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(create)
            .open(&lock_path)
            .map_err(|e| AppError::execution(e.to_string()))?;
        lease
            .try_lock()
            .map_err(|e| AppError::execution(format!("finite lifecycle already active: {e}")))?;
        let root = root
            .canonicalize()
            .map_err(|e| AppError::execution(e.to_string()))?;
        let (bytes, files) = validate_inventory(&root, limits)?;
        Ok(Self {
            root,
            _lease: lease,
            limits,
            bytes,
            files,
        })
    }
    pub(super) fn write(
        &mut self,
        name: &Path,
        bytes: &[u8],
        replace: bool,
    ) -> Result<(), AppError> {
        if name.is_absolute()
            || name
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(AppError::input("invalid private control object name"));
        }
        let path = self.root.join(name);
        let old = match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(AppError::input("control replacement is not regular"));
                }
                if !replace {
                    if read(&path, bytes.len())? == bytes {
                        return Ok(());
                    }
                    return Err(AppError::input("immutable finite source/control changed"));
                }
                metadata.len()
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(AppError::execution(error.to_string())),
        };
        let exists = path.exists();
        let slots = if replace { 1 } else { 2 }; // Staging plus immutable install link.
        let reserve = (bytes.len() as u64)
            .checked_mul(if replace { 1 } else { 2 })
            .and_then(|n| self.bytes.checked_add(n));
        if reserve.is_none_or(|n| n > self.limits.max_bytes)
            || self
                .files
                .checked_add(slots)
                .is_none_or(|n| n > self.limits.max_files)
        {
            return Err(AppError::limit("finite control atomic write allowance"));
        }
        atomic_file::write_file_atomically(&path, bytes, replace).map_err(AppError::execution)?;
        self.bytes = self.bytes - old + bytes.len() as u64;
        self.files += usize::from(!exists);
        Ok(())
    }
}
