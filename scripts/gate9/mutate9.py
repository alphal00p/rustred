# gate9 mutate9.py MUT : apply ONE mutation of round 3/4's new checks to the SOURCE COPY S/mut (never the worktree)
import sys
M='/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9/mut/crates/rustred-campaign/src/'
muts={
 'M1': ('resources/procfs.rs', '        _ => ProcError::Unreadable(format!("{what}: {e}")),', '        _ => ProcError::Gone,'),
 'M2': ('resources/lineage.rs', '            return LinkRead::Unreadable(format!("stat of {}: {w}", x.0));', '            let _ = w; return LinkRead::Gone;'),
 'M3': ('launch/sweep.rs', '        for (pid, start) in pending {', '        for (pid, start) in pending.into_iter().take(0) {'),
 'M4': ('resources/lineage.rs', '        if self.time_ns_unknown {\n            return true;\n        }', '        if false {\n            return true;\n        }'),
 'M4b': ('supervisor/mod.rs', '&& (l.time_ns_unknown || (l.time_ns.is_some() && l.time_ns != now))', '&& (l.time_ns.is_some() && l.time_ns != now)'),
 'M5': ('resources/procfs.rs', '            if in_window {', '            if in_window && false {'),
 'M6': ('lock.rs', '        Err(ProcError::Unreadable(w)) => Liveness::Unknown(w),', '        Err(ProcError::Unreadable(w)) => { let _ = w; Liveness::Dead(DeadReason::PidAbsent) }'),
}
names = sys.argv[1].split('+')
for n in names:
    f, old, new = muts[n]
    p = M + f
    s = open(p).read()
    assert s.count(old) == 1, (n, s.count(old))
    open(p, 'w').write(s.replace(old, new))
    print('applied', n, f)
