# mutate.py ROOT NAME : apply one mutation of procfix7's new checks to the source copy ROOT (exactly one occurrence each)
import sys
root, name = sys.argv[1], sys.argv[2]
C = root + "/crates/rustred-campaign/src/"
M = {
 "M1_lineage_targets": (C+"resources/lineage.rs",
   "                    if rules.targets.contains(&y) {",
   "                    if false && rules.targets.contains(&y) {"),
 "M2_claim_refusal": (C+"supervisor/workdir.rs",
   "            Ok(s) => {\n                return Err(format!(\n                    \"the work directory",
   "            Ok(_s) if true => return Ok(ClaimOutcome::Ours),\n            Ok(s) => {\n                return Err(format!(\n                    \"the work directory"),
 "M3_m4a_evidence": (C+"supervisor/mod.rs",
   "        return evidence().map(|e| format!(\"{SIGXFSZ_WHY} while {e}\"));",
   "        return Some(evidence().map(|e| format!(\"{SIGXFSZ_WHY} while {e}\")).unwrap_or_else(|| SIGXFSZ_WHY.to_string()));"),
 "M4_mx_check": (C+"supervisor/failpoint.rs",
   "    pub fn check(v: &str) -> Result<(), String> {\n",
   "    pub fn check(v: &str) -> Result<(), String> {\n        if v.len() < usize::MAX { return Ok(()); }\n"),
 "M5_settle_identity": (C+"supervisor/mod.rs",
   "        if workdir::run_dir_owner(&layout.root, &self.ctx.campaign_key)\n            != workdir::RunDirOwner::Ours\n        {\n            let still",
   "        if false && workdir::run_dir_owner(&layout.root, &self.ctx.campaign_key)\n            != workdir::RunDirOwner::Ours\n        {\n            let still"),
}
f, old, new = M[name]
s = open(f).read()
n = s.count(old)
assert n == 1, f"{name}: {n} occurrences in {f}"
open(f, "w").write(s.replace(old, new))
print(f"applied {name} to {f}")
