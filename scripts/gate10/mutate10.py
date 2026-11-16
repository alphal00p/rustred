# gate10 mutate10.py MUT[+MUT] : apply mutation(s) of round 4/5's new checks to the SOURCE COPY S/mut (never the worktree)
import sys
M='/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/mut/crates/rustred-campaign/src/'
muts={
 # M1 fsx: every stat error is "absent" (the std collapse back)
 'M1': ('fsx.rs', '        Err(e) => Presence::Unknown(e),', '        Err(_e) => Presence::Absent,', 2),
 # M2 fsx: a listing entry error is skipped (an incomplete listing reads complete)
 'M2': ('fsx.rs', '            Err(e) => return Listing::Failed(e),\n        };\n        let file_type', '            Err(_) => continue,\n        };\n        let file_type', 1),
 # M3 G9-B2 revert: one task listing, none live caught = NoneAlive
 'M3': ('resources/mod.rs', '    Err(ProcError::Unreadable(format!(\n        "pid {pid}: the kernel counts', '    if counted < u64::MAX { return Ok(LiveThread::NoneAlive); }\n    Err(ProcError::Unreadable(format!(\n        "pid {pid}: the kernel counts', 1),
 # M4 the final sweep's vanished rule off
 'M4': ('supervisor/mod.rs', '        if vanished.is_empty() {\n            return Ok(());\n        }', '        if true || vanished.is_empty() {\n            return Ok(());\n        }', 1),
 # M5 the own-tree (waitid ECHILD) proof always "no descendant"
 'M5': ('supervisor/mod.rs', '                Ok(crate::launch::OwnChildren::Exist { zombie }) => {\n                    let listed', '                Ok(crate::launch::OwnChildren::Exist { zombie: _ }) if true => return Ok(true),\n                Ok(crate::launch::OwnChildren::Exist { zombie }) => {\n                    let listed', 1),
 # M6 M9-1 revert: unknown namespace skips the scope backstop silently
 'M6': ('supervisor/mod.rs', '        if scan_allowed && let Err(e) = &ns {', '        if scan_allowed && false && let Err(e) = &ns {', 1),
 # M7 token scan: an unreadable environ is "not a holder" (dropped)
 'M7': ('lock.rs', '            None => out.possible.push((p, last)),', '            None => { let _ = last; }', 1),
 # M8 pause flag that cannot be checked = not paused
 'M8': ('cpu_policy.rs', '            crate::fsx::Presence::Unknown(e) => Some(format!(\n                "pause flag {} cannot be checked ({e}): counted as present",', '            crate::fsx::Presence::Unknown(_) if true => None,\n            crate::fsx::Presence::Unknown(e) => Some(format!(\n                "pause flag {} cannot be checked ({e}): counted as present",', 1),
 # M9 ledger create: a FAILED listing of the ledger directory is taken as empty
 'M9': ('ledger.rs', '            crate::fsx::Listing::Failed(e) => return Err(io_err(&dir, "list")(e)),\n        }\n        let manifest = Manifest {', '            crate::fsx::Listing::Failed(_e) => {}\n        }\n        let manifest = Manifest {', 1),
 # M10 manifest that cannot be checked = "no manifest" (G9-B1's first line)
 'M10': ('ledger.rs', '        crate::fsx::Presence::Unknown(e) => Err(io_err(&p, "stat manifest")(e)),', '        crate::fsx::Presence::Unknown(_e) => Ok(false),', 1),
}
for n in sys.argv[1].split('+'):
    f, old, new, cnt = muts[n]
    p = M + f
    s = open(p).read()
    assert s.count(old) == cnt, (n, s.count(old))
    open(p, 'w').write(s.replace(old, new))
    print('applied', n, f)
