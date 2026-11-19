# mutate8.py FILE ID : apply mutation ID to FILE (exactly one occurrence of the pattern, else exit 2)
import sys
f, mid = sys.argv[1], sys.argv[2]
M = {
 "M1": ('''            Err(e) => {
                self.recovery_read_failed("the ledger (recovery pass)", &e);
                return;
            }''', '''            Err(_e) => RecoveryFacts::default(),'''),
 "M2": ('''                min_ticks = Some(0);''', '''                let _ = 0;'''),
 "M3": ('''        let limit = Duration::from_secs(self.opts.spec.resources.unproven_end_limit_s);''',
        '''        let limit = Duration::from_secs(self.opts.spec.resources.unproven_end_limit_s);
        if limit.as_secs() < u64::MAX { return; }'''),
 "M4": ('''Ok(Some((c, sess))) if c == self.ctx.campaign_key && known.contains(&sess) => {''', '''Ok(Some((_c, _sess))) => {'''),
 "M5": ('''    if m.permissions().mode() & 0o077 != 0 {''', '''    if m.permissions().mode() & 0o077 == 0o1000 {'''),
 "M6": ('''                return Err(msg);
            }
        };
        if hidden.is_empty() {''', '''                let _ = msg;
                return Ok(());
            }
        };
        if hidden.is_empty() {'''),
}
a, b = M[mid]
s = open(f).read()
n = s.count(a)
if n != 1:
    print(f"{mid}: pattern occurs {n} times"); sys.exit(2)
open(f, "w").write(s.replace(a, b))
print(f"{mid}: applied to {f}")
