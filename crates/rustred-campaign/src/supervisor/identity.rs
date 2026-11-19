//! The campaign id of the marker (gate re-audit C-1; interfaces/supervisor.md section 8). It is the ONLY proof that
//! a process belongs to this campaign, so it must be (a) valid for the launcher's charset, (b) unique among live
//! campaigns and not reproducible by copying a campaign directory, (c) recorded in the ledger.
//!
//! id = `<spec campaign, at most 40 chars>.<16 hex of BLAKE3(nonce | st_dev | st_ino of CDIR)>`, where the nonce is
//! 128 random bits created once in `CDIR/CAMPAIGN_NONCE`. The same directory gives the same id at every start (recovery
//! works); a copy or a restore elsewhere has another inode, hence another id: it can neither adopt nor kill the
//! original's jobs.

use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub const NONCE_FILE: &str = "CAMPAIGN_NONCE";
/// Ledger input id under which the marker id is recorded (`path` = the id).
pub const ID_INPUT: &str = "campaign:marker_id";
pub const ID_ROLE: &str = "campaign_id";

/// The nonce of `cdir`, created (128 random bits, hex) if absent.
pub fn nonce(cdir: &Path) -> Result<String, String> {
    let p = cdir.join(NONCE_FILE);
    match std::fs::read_to_string(&p) {
        Ok(s) => {
            let s = s.trim().to_string();
            if s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                Ok(s)
            } else {
                Err(format!("{} is malformed", p.display()))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut b = [0u8; 16];
            std::fs::File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut b))
                .map_err(|e| format!("/dev/urandom: {e}"))?;
            let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
            match crate::atomic::create_new_atomic(&p, format!("{hex}\n").as_bytes()) {
                Ok(()) => Ok(hex),
                // Lost a race with another creator: read theirs.
                Err(crate::atomic::AtomicError::Exists { .. }) => nonce(cdir),
                Err(e) => Err(format!("{}: {e}", p.display())),
            }
        }
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// The marker campaign id of `cdir` (validated for the launcher's charset).
pub fn campaign_marker_id(cdir: &Path, campaign: &str) -> Result<String, String> {
    let n = nonce(cdir)?;
    let md = crate::fsx::presence("supervisor.dirs", cdir)
        .metadata()
        .map_err(|e| format!("{}: {e}", cdir.display()))?;
    let h = blake3::hash(format!("{n}|{}|{}", md.dev(), md.ino()).as_bytes()).to_hex();
    let prefix: String = campaign
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let id = format!("{prefix}.{}", &h[..16]);
    crate::resources::validate_campaign_id(&id).map_err(|e| e.to_string())?;
    Ok(id)
}

/// The id recorded in a ledger state, if any.
pub fn recorded_id(st: &crate::state::CampaignState) -> Option<String> {
    st.inputs.get(ID_INPUT).map(|i| i.path.clone())
}

/// The ledger event recording `id`.
pub fn id_event(id: &str) -> crate::ledger::Event {
    crate::ledger::Event::InputRegistered {
        input: ID_INPUT.into(),
        role: ID_ROLE.into(),
        path: id.into(),
        bytes: 0,
        blake3: blake3::hash(id.as_bytes()).to_hex().to_string(),
    }
}
