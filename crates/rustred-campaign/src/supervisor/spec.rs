//! The campaign spec subset used by the supervisor now (`rustred.campaign.spec.v1`, plan 3.3.1; interfaces/supervisor.md
//! section 13). Unknown fields are refused: a misspelt limit must not silently disable it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CAMPAIGN_SPEC_SCHEMA: &str = "rustred.campaign.spec.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignSpec {
    pub schema: String,
    pub campaign: String,
    #[serde(default = "d_kind")]
    pub kind: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dirs: Dirs,
    #[serde(default)]
    pub programs: BTreeMap<String, Program>,
    #[serde(default)]
    pub licence: Licence,
    #[serde(default)]
    pub resources: Resources,
    #[serde(default)]
    pub progress: ProgressCfg,
    #[serde(default)]
    pub synthetic: Option<Synthetic>,
    /// Existing material the campaign starts from (tables, hand rules, evidence), by path + digest (R41 P-1): the
    /// pre-flight and every start verify each one and refuse otherwise; they are registered as ledger inputs.
    #[serde(default)]
    pub inputs: Vec<SpecInput>,
    /// `campaign` (default) or `optimise` (user ruling U-13: boot from an existing campaign result and write the new
    /// result beside it; the work source decides the work, the supervisor passes the mode through).
    #[serde(default = "d_mode")]
    pub mode: String,
    /// The campaign directory an `optimise` run boots from (read-only for this run: never written).
    #[serde(default)]
    pub source_campaign_dir: Option<PathBuf>,
}

fn d_mode() -> String {
    "campaign".into()
}

fn d_kind() -> String {
    "test".into()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dirs {
    #[serde(default)]
    pub campaign_dir: Option<PathBuf>,
    /// Non-snapshotted root for run directories (default: the campaign directory).
    #[serde(default)]
    pub work_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub path: PathBuf,
    /// Optional pinned digest (BLAKE3 hex); pre-flight refuses a mismatch.
    #[serde(default)]
    pub blake3: Option<String>,
    #[serde(default)]
    pub optional: bool,
}

/// One input of the spec (generic: the work source gives the role its meaning).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecInput {
    /// Ledger input id (`[A-Za-z0-9._:-]`).
    pub id: String,
    /// e.g. `table`, `family`, `evidence`, `box`.
    pub role: String,
    pub path: PathBuf,
    /// BLAKE3 hex of the file.
    pub blake3: String,
}

impl SpecInput {
    /// Ok((bytes)) when the file exists with the stated digest.
    pub fn check(&self) -> Result<u64, String> {
        match crate::atomic::blake3_file(&self.path) {
            Ok((bytes, d)) if d == self.blake3 => Ok(bytes),
            Ok((_, d)) => Err(format!(
                "input {} ({}): blake3 {d} differs from the spec's {}",
                self.id,
                self.path.display(),
                self.blake3
            )),
            Err(e) => Err(format!(
                "input {} missing: {} ({e})",
                self.id,
                self.path.display()
            )),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Licence {
    /// NAME of the licence variable passed through to children (its value is never read into logs).
    #[serde(default)]
    pub env: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// "plain" (CPU list or count) or "site" (site-policy JSON file, process-layer.md section 7).
    #[serde(default = "d_policy")]
    pub policy: String,
    #[serde(default)]
    pub cpus: Option<String>,
    #[serde(default)]
    pub site_policy: Option<PathBuf>,
    /// Cores the campaign may hold (default: what the policy allows).
    #[serde(default)]
    pub max_cores: Option<usize>,
    #[serde(default = "d_max_jobs")]
    pub max_jobs: usize,
    /// Sum of the reserved caps of running jobs (bytes or "30G").
    #[serde(default = "d_mem_total")]
    pub mem_total: ByteSize,
    #[serde(default = "d_floor")]
    pub host_floor: ByteSize,
    #[serde(default = "d_disk_floor")]
    pub disk_floor: ByteSize,
    /// Grace given to running jobs on the first stop request (plus 5 s before the group kill).
    #[serde(default = "d_grace")]
    pub grace_seconds: u64,
    /// Existence of any = no launch (agents' test campaigns: WS/ops/PAUSE and the implementer's; the user's run: none).
    #[serde(default)]
    pub pause_files: Vec<PathBuf>,
    #[serde(default = "d_sampler_ms")]
    pub sampler_ms: u64,
    /// Heartbeat timeout (default 3 x heartbeat_s + 5 s, contract 6.2).
    #[serde(default)]
    pub heartbeat_timeout_s: Option<u64>,
    /// No `job_started` event this long after spawn = unresponsive (contract 6.2: 60 s).
    #[serde(default = "d_start_timeout")]
    pub start_timeout_s: u64,
    /// Allow running on a kernel without pidfd (signals then use the documented weaker path). Default false: refused.
    #[serde(default)]
    pub allow_no_pidfd: bool,
}

fn d_policy() -> String {
    "plain".into()
}
fn d_max_jobs() -> usize {
    8
}
fn d_mem_total() -> ByteSize {
    ByteSize(30_000_000_000)
}
fn d_floor() -> ByteSize {
    ByteSize(crate::admission::DEFAULT_FLOOR_BYTES)
}
fn d_disk_floor() -> ByteSize {
    ByteSize(20_000_000_000)
}
fn d_grace() -> u64 {
    120
}
fn d_sampler_ms() -> u64 {
    1000
}
fn d_start_timeout() -> u64 {
    60
}

impl Default for Resources {
    fn default() -> Self {
        serde_json::from_str("{}").expect("defaults")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressCfg {
    #[serde(default = "d_cadence")]
    pub cadence_seconds: u64,
}

fn d_cadence() -> u64 {
    2
}

impl Default for ProgressCfg {
    fn default() -> Self {
        ProgressCfg {
            cadence_seconds: d_cadence(),
        }
    }
}

/// The synthetic campaign of the `ScriptedWorkSource` (interfaces/supervisor.md section 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Synthetic {
    pub items: u32,
    #[serde(default = "d_nodes")]
    pub nodes: u32,
    #[serde(default = "d_rungs")]
    pub rungs: u8,
    #[serde(default)]
    pub seed: u64,
    /// Range of the case duration (ms) of a fake job.
    #[serde(default = "d_case_ms")]
    pub case_ms: [u64; 2],
    #[serde(default = "d_p_norule")]
    pub p_norule: f64,
    #[serde(default = "d_p_mem")]
    pub p_mem: f64,
    #[serde(default = "d_p_crash")]
    pub p_crash: f64,
    /// Probability that an item depends on one or two earlier items.
    #[serde(default = "d_deps")]
    pub deps: f64,
    /// Memory cap of a fake job (MiB); a mem_excess job allocates twice this much.
    #[serde(default = "d_mem_cap_mb")]
    pub mem_cap_mb: u64,
    #[serde(default = "d_hb")]
    pub heartbeat_s: u32,
    #[serde(default = "d_child_grace")]
    pub stop_grace_s: u32,
    /// Reaction of every fake child to SIGTERM (tests: `ignore` = only the group kill ends it).
    #[serde(default)]
    pub on_sigterm: rustred_campaign_contract::fake::OnSigterm,
    /// Every rule job forks a helper that ESCAPES its group (setsid, no PDEATHSIG): a contract violation that the
    /// marker sweep after the job end must clean up.
    #[serde(default)]
    pub escape_helpers: bool,
    /// Items whose rung-0 job stops its heartbeat and hangs (the heartbeat timeout must kill it).
    #[serde(default)]
    pub hang_items: Vec<u32>,
}

fn d_nodes() -> u32 {
    3
}
fn d_rungs() -> u8 {
    3
}
fn d_case_ms() -> [u64; 2] {
    [100, 400]
}
fn d_p_norule() -> f64 {
    0.25
}
fn d_p_mem() -> f64 {
    0.1
}
fn d_p_crash() -> f64 {
    0.1
}
fn d_deps() -> f64 {
    0.3
}
fn d_mem_cap_mb() -> u64 {
    48
}
fn d_hb() -> u32 {
    1
}
fn d_child_grace() -> u32 {
    5
}

/// Bytes, from a number or a string "30G" / "512M" / "1.5T" (decimal units, as the plan writes them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(pub u64);

impl ByteSize {
    pub fn parse(s: &str) -> Result<ByteSize, String> {
        let s = s.trim();
        let (num, mult) = match s.chars().last() {
            Some('K' | 'k') => (&s[..s.len() - 1], 1e3),
            Some('M' | 'm') => (&s[..s.len() - 1], 1e6),
            Some('G' | 'g') => (&s[..s.len() - 1], 1e9),
            Some('T' | 't') => (&s[..s.len() - 1], 1e12),
            _ => (s, 1.0),
        };
        let v: f64 = num
            .trim()
            .parse()
            .map_err(|_| format!("bad byte size {s:?}"))?;
        if !(v.is_finite() && v >= 0.0) {
            return Err(format!("bad byte size {s:?}"));
        }
        Ok(ByteSize((v * mult).round() as u64))
    }
}

impl Serialize for ByteSize {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match v {
            serde_json::Value::Number(n) => n.as_u64().map(ByteSize).ok_or_else(|| {
                serde::de::Error::custom("byte size must be a non-negative integer")
            }),
            serde_json::Value::String(s) => ByteSize::parse(&s).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("byte size: number or string")),
        }
    }
}

impl CampaignSpec {
    /// Parses and validates; returns the spec and the BLAKE3 of its bytes.
    pub fn read(path: &Path) -> Result<(CampaignSpec, String), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let spec: CampaignSpec =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        spec.validate()?;
        Ok((spec, blake3::hash(&bytes).to_hex().to_string()))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CAMPAIGN_SPEC_SCHEMA {
            return Err(format!(
                "schema {:?}, expected {CAMPAIGN_SPEC_SCHEMA}",
                self.schema
            ));
        }
        if self.campaign.is_empty()
            || self.campaign.len() > 96
            || !self
                .campaign
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(format!(
                "campaign id {:?}: 1-96 of [A-Za-z0-9._-]",
                self.campaign
            ));
        }
        match self.resources.policy.as_str() {
            "plain" => {
                if self.resources.cpus.is_none() {
                    return Err("resources.cpus is required with policy plain".into());
                }
            }
            "site" => {
                if self.resources.site_policy.is_none() {
                    return Err("resources.site_policy is required with policy site".into());
                }
            }
            p => return Err(format!("resources.policy {p:?}: plain or site")),
        }
        if self.resources.max_jobs == 0 {
            return Err("resources.max_jobs is 0".into());
        }
        if let Some(s) = &self.synthetic {
            if s.items == 0 || s.nodes == 0 || s.rungs == 0 || s.rungs > 9 {
                return Err("synthetic: items, nodes > 0 and 1 <= rungs <= 9".into());
            }
            if s.case_ms[0] > s.case_ms[1] {
                return Err("synthetic.case_ms: [min, max]".into());
            }
            for p in [s.p_norule, s.p_mem, s.p_crash, s.deps] {
                if !(0.0..=1.0).contains(&p) {
                    return Err("synthetic probabilities must lie in [0, 1]".into());
                }
            }
            if s.p_norule + s.p_mem + s.p_crash > 1.0 {
                return Err("synthetic: p_norule + p_mem + p_crash > 1".into());
            }
            if !self.programs.contains_key("rr_fake_child") {
                return Err("a synthetic campaign needs programs.rr_fake_child".into());
            }
        } else {
            return Err(
                "only synthetic campaigns are supported by this version (real adapters come with the integration lane)"
                    .into(),
            );
        }
        for i in &self.inputs {
            if i.id.is_empty()
                || i.id.starts_with("bin:")
                || !i
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
            {
                return Err(format!("input id {:?}: [A-Za-z0-9._:-], not bin:*", i.id));
            }
            if !rustred_campaign_contract::is_blake3_hex(&i.blake3) {
                return Err(format!("input {}: blake3 must be 64 lower-case hex", i.id));
            }
        }
        match self.mode.as_str() {
            "campaign" => {}
            "optimise" => {
                if self.source_campaign_dir.is_none() {
                    return Err("mode optimise needs source_campaign_dir".into());
                }
            }
            m => return Err(format!("mode {m:?}: campaign or optimise")),
        }
        if let Some(e) = &self.licence.env
            && (e.is_empty() || e.contains('='))
        {
            return Err("licence.env must be a variable name".into());
        }
        Ok(())
    }

    /// Refuses a campaign directory that is (inside) the source campaign of an optimise run (U-13).
    pub fn check_dirs(&self, cdir: &Path) -> Result<(), String> {
        if let Some(src) = &self.source_campaign_dir {
            let c = std::fs::canonicalize(cdir).unwrap_or_else(|_| cdir.to_path_buf());
            let s = std::fs::canonicalize(src).unwrap_or_else(|_| src.clone());
            if c.starts_with(&s) || s.starts_with(&c) {
                return Err(format!(
                    "campaign directory {} overlaps the source campaign {} of this optimise run: the source is never written",
                    c.display(),
                    s.display()
                ));
            }
        }
        Ok(())
    }

    /// The heartbeat timeout for a job with heartbeat `hb` seconds.
    pub fn heartbeat_timeout(&self, hb: u32) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.resources
                .heartbeat_timeout_s
                .unwrap_or(3 * hb as u64 + 5),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_sizes_and_defaults() {
        assert_eq!(ByteSize::parse("30G").unwrap().0, 30_000_000_000);
        assert_eq!(ByteSize::parse("512M").unwrap().0, 512_000_000);
        assert_eq!(ByteSize::parse("1.5T").unwrap().0, 1_500_000_000_000);
        assert_eq!(ByteSize::parse("17").unwrap().0, 17);
        assert!(ByteSize::parse("x").is_err());
        let r = Resources::default();
        assert_eq!(r.host_floor.0, 50_000_000_000);
        assert_eq!(r.grace_seconds, 120);
        let s: CampaignSpec = serde_json::from_str(
            r#"{"schema":"rustred.campaign.spec.v1","campaign":"t","programs":{"rr_fake_child":{"path":"/x"}},
                "resources":{"cpus":"0-3","mem_total":"4G"},"synthetic":{"items":3}}"#,
        )
        .unwrap();
        s.validate().unwrap();
        assert_eq!(s.resources.mem_total.0, 4_000_000_000);
        assert!(
            serde_json::from_str::<CampaignSpec>(r#"{"schema":"x","campaign":"t","bogus":1}"#)
                .is_err()
        );
    }
}
