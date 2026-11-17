"""gate14: single-part reverts of the round-10 proof fix (uncommitted; restored with git checkout -- <file>).
usage: python3 -I -B mutate14.py F1|F2 <hunt.rs>"""
import sys
mode, p = sys.argv[1], sys.argv[2]
s = open(p).read()
def rep(a, b):
    global s
    assert s.count(a) == 1, ("not unique / absent", a[:80])
    s = s.replace(a, b)
if mode == "F1":   # fixed point: ended entries / dead candidates are no change; stability on the live listing only
    rep("let ended = self.handoff_suspects(&t);\n                        if !ended.is_empty() {\n                            changed = true;",
        "let ended: Vec<u32> = Vec::new(); // GATE14 MUTATION F1\n                        if !ended.is_empty() {\n                            changed = true;")
    rep("                    Class::Dead => {\n                        if !self.killed_member(pid, None) {",
        "                    Class::Dead => {\n                        if false && !self.killed_member(pid, None) { // GATE14 MUTATION F1")
    rep("                        if self.handoff_suspects(&t).is_empty()\n",
        "                        if true // GATE14 MUTATION F1\n")
elif mode == "F2": # proof pass: ended entries, dead listed children, a gone walked child, a new dead child: ignored
    rep("let t0 = self.snapshot_tops(true)?;\n                let ended = self.handoff_suspects(&t0);",
        "let t0 = self.snapshot_tops(true)?;\n                let ended: Vec<u32> = Vec::new(); // GATE14 MUTATION F2")
    rep("if !self.killed_member(pid, t0.live.get(&pid).copied()) {",
        "if false && !self.killed_member(pid, t0.live.get(&pid).copied()) { // GATE14 MUTATION F2")
    rep("let t1 = self.snapshot_tops(true)?;\n                let ended = self.handoff_suspects(&t1);",
        "let t1 = self.snapshot_tops(true)?;\n                let ended: Vec<u32> = Vec::new(); // GATE14 MUTATION F2")
    rep("if t1.live.get(pid) != t0.live.get(pid) {", "if false && t1.live.get(pid) != t0.live.get(pid) { // GATE14 MUTATION F2")
    rep("""                        Class::Dead => {
                            return Err(format!(
                                "child {pid} appeared and ended during the pass (hand-off?)"
                            ));
                        }""", "                        Class::Dead => {} // GATE14 MUTATION F2")
else:
    raise SystemExit("mode?")
open(p, "w").write(s)
print("mutated", mode)
