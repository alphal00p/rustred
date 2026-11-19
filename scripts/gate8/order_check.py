#!/usr/bin/env python3
# order_check.py STRACE_FILE... -- CDIR
# gate5 item 1a. For every signal syscall ISSUED by a supervisor thread group (tgid whose execve was "rr_campaign run"):
#   pidfd_send_signal(fd<pid:P>, SIG, ...), kill(P|-PGID, SIG), and write(fd<.../cgroup.kill>, "1")
# there must be an intent line (kind "intent", same pid / pgid / 0 for cgroup, same signal, same target) written to
# CDIR/log/signal-intents.jsonl by a write() that COMPLETED, followed by an fdatasync/fsync of that file that COMPLETED
# with 0 and whose ENTRY came after the write's completion, all before the ENTRY of the signal syscall (strace output
# order = order of ptrace stops). Each intent covers one send (multiset). Signal 0 probes are listed, not required.
# Also: every successful send has a ledger signal_sent (pid, signal) record (procfix5's end-state check), and every
# intent without a completion line has exactly one POSSIBLY SENT record.
# strace flags expected: -f -ttt -q --decode-fds=path,pidfd -s 1000000 -e trace=write,fdatasync,fsync,pidfd_send_signal,
#   pidfd_open,kill,tgkill,tkill,execve,clone,clone3,fork,vfork -e signal=none
import sys, re, json, glob, collections

args = sys.argv[1:]
cut = args.index('--')
files, cdir = args[:cut], args[cut + 1]
INTENTS = cdir.rstrip('/') + '/log/signal-intents.jsonl'
SIG = {'SIGKILL': 9, 'SIGSTOP': 19, 'SIGTERM': 15, 'SIGCONT': 18, 'SIGINT': 2, 'SIGHUP': 1, 'SIGQUIT': 3,
       'SIGUSR1': 10, 'SIGUSR2': 12, '0': 0}


def cunescape(s):
    out = bytearray()
    i = 0
    while i < len(s):
        c = s[i]
        if c != '\\':
            out += c.encode('utf-8', 'surrogateescape')
            i += 1
            continue
        n = s[i + 1]
        if n in 'nrtvfab\\"\'':
            out.append({'n': 10, 'r': 13, 't': 9, 'v': 11, 'f': 12, 'a': 7, 'b': 8, '\\': 92, '"': 34, "'": 39}[n])
            i += 2
        elif n == 'x':
            out.append(int(s[i + 2:i + 4], 16))
            i += 4
        elif n.isdigit():
            j = i + 1
            while j < len(s) and j < i + 4 and s[j].isdigit():
                j += 1
            out.append(int(s[i + 1:j], 8) & 255)
            i = j
        else:
            out.append(ord(n))
            i += 2
    return bytes(out)


def fdpath(arg):
    m = re.match(r'^\s*(-?\d+)<(.*)>\s*$', arg)
    return (int(m.group(1)), m.group(2)) if m else (None, None)


def send_key(call, a, g=None):
    parts = [x.strip() for x in a.split(',')]
    try:
        if call == 'pidfd_send_signal':
            pm = re.search(r'<pid:(\d+)>', parts[0])
            pid = int(pm.group(1)) if pm else pidfd_map.get((g, int(re.match(r'\s*(\d+)', parts[0]).group(1))), -1)
            return (pid, SIG.get(parts[1], parts[1]), 'process'), call
        if call == 'kill':
            pid = int(parts[0])
            return (abs(pid), SIG.get(parts[1], parts[1]), 'group' if pid < 0 else 'process'), call
    except Exception:
        pass
    return None, call


total = collections.Counter()
miss = []
pidfd_map = {}
ok_box = [0]
probes = 0
checked_at_entry = {}
issued_ok = collections.Counter()  # successful sends (pid, sig) for the ledger check
durable = collections.Counter()    # (pid, sig, target) -> count of durable, unconsumed intents
written = []                       # list of (pid, sig, target, file_index, line_no_completed)
other_signals = []
for fi, f in enumerate(files):
    tg = {}
    role = {}
    pending = {}      # tid -> (entry text, entry line no)
    first = None
    fsync_entry = {}  # tid -> line no of fdatasync entry (unfinished)
    lineno = 0
    for raw in open(f, errors='surrogateescape'):
        lineno += 1
        line = raw.rstrip('\n')
        m = re.match(r'^(\d+)\s+(\d+\.\d+)\s+(.*)$', line)
        if not m:
            continue
        tid, rest = int(m.group(1)), m.group(3)
        if first is None:
            first = tid
        entry_line = lineno
        if rest.endswith('<unfinished ...>'):
            body = rest[:-len('<unfinished ...>')].rstrip()
            pending[tid] = (body, lineno)
            # entries that matter at ENTRY time: signal syscalls and fdatasync entries
            g = tg.setdefault(tid, tid)
            sm = re.match(r'^(pidfd_send_signal|kill|tgkill|tkill|fdatasync|fsync|write)\(', body)
            if sm and role.get(g) == 'sup':
                name = sm.group(1)
                if name in ('fdatasync', 'fsync'):
                    fsync_entry[tid] = lineno
                elif name == 'write':
                    pass
                else:
                    # check the ordering at ENTRY time (the args are complete in the entry text)
                    key, desc = send_key(name, body[len(name) + 1:], g)
                    if key is not None:
                        checked_at_entry[tid] = key
                        if key[1] != 0:
                            total[name] += 1
                            if durable[key] > 0:
                                durable[key] -= 1
                                ok_box[0] += 1
                            else:
                                miss.append((f, lineno, name, key, 'at entry'))
            continue
        r = re.match(r'^<\.\.\. (\w+) resumed>(.*)$', rest)
        if r:
            body, eline = pending.pop(tid, (r.group(1) + '(', lineno))
            rest = body + r.group(2)
            entry_line = eline
        g = tg.setdefault(tid, tid)
        c = re.match(r'^(clone3?|fork|vfork)\((.*)\)\s+=\s+(\d+)', rest)
        if c:
            child = int(c.group(3))
            if 'CLONE_THREAD' in c.group(2):
                tg[child] = g
            else:
                tg[child] = child
                role[child] = role.get(g, 'other')
            continue
        e = re.match(r'^execve\("([^"]*)", \[(.*?)\]', rest)
        if e and re.search(r'\)\s+=\s+0\s*$', rest):
            role[g] = 'sup' if (e.group(1).endswith('rr_campaign') and '"run"' in e.group(2)) else 'other'
            continue
        if role.get(g) != 'sup':
            continue
        w = re.match(r'^write\((\d+<[^>]*>), "(.*)"(\.\.\.)?, (\d+)\)\s+=\s+(-?\d+)', rest)
        if w:
            fd, path = fdpath(w.group(1))
            ret = int(w.group(5))
            if path and path.endswith('/log/signal-intents.jsonl') and ret > 0:
                data = cunescape(w.group(2))[:ret]
                for ln in data.split(b'\n'):
                    try:
                        o = json.loads(ln)
                    except Exception:
                        continue
                    if o.get('kind') != 'intent':
                        continue
                    written.append([o.get('pid'), o.get('signal'), o.get('target'), fi, lineno, False, o.get('id')])
            elif path and path.endswith('/cgroup.kill'):
                total['cgroup.kill'] += 1
                key = (0, 9, 'cgroup')
                if durable[key] > 0:
                    durable[key] -= 1
                    ok_box[0] += 1
                else:
                    miss.append((f, entry_line, 'cgroup.kill write without durable intent', path))
            continue
        s = re.match(r'^(fdatasync|fsync)\((\d+<[^>]*>)\)\s+=\s+(-?\d+)', rest)
        if s:
            fd, path = fdpath(s.group(2))
            # the entry position of THIS call: the unfinished entry line if it was split, else this line
            ent = entry_line
            fsync_entry.pop(tid, None)
            if path and path.endswith('/log/signal-intents.jsonl') and int(s.group(3)) == 0:
                for wr in written:
                    if not wr[5] and (wr[3] < fi or (wr[3] == fi and wr[4] < ent)):
                        wr[5] = True
                        durable[(wr[0], wr[1], wr[2])] += 1
            continue
        po = re.match(r'^pidfd_open\((\d+), [^)]*\)\s+=\s+(\d+)', rest)
        if po:
            pidfd_map[(g, int(po.group(2)))] = int(po.group(1))
            continue
        k = re.match(r'^(pidfd_send_signal|kill|tgkill|tkill)\((.*)\)\s+=\s+(-?\d+)(.*)$', rest)
        if not k:
            continue
        call, a, ret = k.group(1), k.group(2), int(k.group(3))
        parts = [x.strip() for x in a.split(',')]
        if call == 'pidfd_send_signal':
            pm = re.search(r'<pid:(\d+)>', parts[0])
            pid = int(pm.group(1)) if pm else pidfd_map.get((g, int(re.match(r'\s*(\d+)', parts[0]).group(1))), -1)
            sig = SIG.get(parts[1], parts[1])
            target = 'process'
        elif call == 'kill':
            pid = int(parts[0])
            sig = SIG.get(parts[1], parts[1])
            target = 'group' if pid < 0 else 'process'
            pid = abs(pid)
        else:
            other_signals.append((f, lineno, rest))
            continue
        if sig == 0:
            probes += 1
            checked_at_entry.pop(tid, None)
            continue
        key = (pid, sig, target)
        if checked_at_entry.pop(tid, None) is None:
            total[call] += 1
            if durable[key] > 0:
                durable[key] -= 1
                ok_box[0] += 1
            else:
                miss.append((f, entry_line, call, pid, sig, target, ret))
        if ret == 0:
            issued_ok[(pid, sig)] += 1

rec = collections.Counter()
possibly = 0
recovered_delivered = 0
for lf in sorted(glob.glob(cdir + '/ledger/*.jsonl')):
    for l in open(lf):
        try:
            ev = json.loads(l)['event']
        except Exception:
            continue
        if ev.get('kind') == 'signal_sent':
            b = ev['body']
            rec[(b['pid'], b['signal'])] += 1
            if 'POSSIBLY SENT' in b.get('by', ''):
                possibly += 1
            if 'delivered, not recorded' in b.get('by', ''):
                recovered_delivered += 1
ledger_miss = {k: v - rec[k] for k, v in issued_ok.items() if rec[k] < v}
# intents of the log without completion
nodone = 0
try:
    ids, done = [], set()
    for l in open(INTENTS, errors='replace'):
        try:
            o = json.loads(l)
        except Exception:
            continue
        if o.get('kind') == 'intent':
            ids.append(o['id'])
        elif o.get('kind') == 'done':
            done.add(o['id'])
    nodone = sum(1 for i in ids if i not in done)
except FileNotFoundError:
    pass
print(f"ORDER: signal syscalls issued by supervisors: {sum(total.values())} ({dict(total)}); with a durable intent "
      f"written before: {ok_box[0]}; WITHOUT: {len(miss)}; sig-0 probes {probes}; thread-directed {len(other_signals)}")
for x in miss[:20]:
    print('  MISS', x)
print(f"LEDGER: successful sends {sum(issued_ok.values())}; ledger signal_sent {sum(rec.values())}; missing "
      f"{sum(ledger_miss.values())}; POSSIBLY SENT records {possibly}; recovered delivered {recovered_delivered}; "
      f"intents without completion in the log {nodone}")
for k, v in sorted(ledger_miss.items())[:20]:
    print('  LEDGER-MISSING', k, 'x', v)
sys.exit(1 if (miss or ledger_miss) else 0)
