# gate11 mutate11.py MUT : apply ONE mutation of round 6's new checks to the SOURCE COPY S/mut (never the worktree)
import sys
M='/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11/mut/crates/rustred-campaign/src/'
muts={
 # M1 /proc root listing sentinels (this process, pid 1) off
 'M1': [('resources/procfs.rs', '        if self.is_system() {\n            let me = std::process::id();\n            for s in [me, 1] {', '        if false && self.is_system() {\n            let me = std::process::id();\n            for s in [me, 1] {', 1)],
 # M2 the completeness walk's check (reached + alive + older than the listing + not listed = Unreadable) off
 'M2': [('resources/procfs.rs', '        for &w in reached.iter().chain(known_alive.iter()) {', '        for &w in reached.iter().chain(known_alive.iter()).take(0) {', 1)],
 # M3 task listing: leader sentinel and the atomic Threads: count proof off
 'M3': [('resources/procfs.rs', '        if !v.contains(&pid) {\n            return match self.stat(pid) {', '        if false && !v.contains(&pid) {\n            return match self.stat(pid) {', 1),
        ('resources/procfs.rs', '            && (v.len() as u32) < a\n', '            && (v.len() as u32) < a\n            && false\n', 1)],
 # M4 sweep: a scanned marked process gone / not ours at its stop is NOT recorded as vanished (M10-1 detail reverted)
 'M4': [('launch/sweep.rs', '                Err(Verdict::NotOurs(_)) => {\n                    self.vanished.insert(k, p);', '                Err(Verdict::NotOurs(_)) => {\n                    let _ = (k, p);', 1),
        ('launch/sweep.rs', '                Verdict::NotOurs(_) => {\n                    self.vanished.insert(k, p);', '                Verdict::NotOurs(_) => {\n                    let _ = (k, p);', 1)],
 # M5 the vanished rule's resolution not atomic-only: every open vanished pid resolved at a proven final sweep
 'M5': [('supervisor/mod.rs', '            if atomic && self.launched_here.contains(attempt) {', '            if true {', 1)],
 # M6 the seeds (ledger-recorded pids of this boot, open vanished pids) never checked by their own pid
 'M6': [('launch/sweep.rs', '    for &(pid, start) in seeds {\n        w.suspects.entry(pid).or_insert(start);', '    for &(pid, start) in seeds.iter().take(0) {\n        w.suspects.entry(pid).or_insert(start);', 1)],
 # M7 the scope refusal: marker_only accepted whatever the namespace (also on the host)
 'M7': [('supervisor/mod.rs', '                (Some("marker_only"), Ok(true)) => log.line(', '                (Some("marker_only"), _) => log.line(', 1)],
 # M8 kill_recorded_scope: "empty" from the non-recursive listing alone (m10-3 reverted at the recovery backstop)
 'M8': [('launch/cgroup.rs', '        if procs.is_empty() && !populated(&dir)? {', '        if procs.is_empty() {', 1)],
 # M9 m10-9 reverted: not a child subreaper -> the end is claimed anyway
 'M9': [('supervisor/mod.rs', '                if !atomic {\n                    self.end_unproven.push(', '                if false && !atomic {\n                    self.end_unproven.push(', 1)],
 # M10 m10-5 reverted: no bound on consecutive failed polls
 'M10': [('supervisor/mod.rs', '                    if ch.poll_failures == MAX_POLL_FAILURES {', '                    if false && ch.poll_failures == MAX_POLL_FAILURES {', 1)],
}
n = sys.argv[1]
for f, old, new, cnt in muts[n]:
    p = M + f
    s = open(p).read()
    assert s.count(old) == cnt, (n, f, s.count(old))
    open(p, 'w').write(s.replace(old, new))
print('applied', n)
