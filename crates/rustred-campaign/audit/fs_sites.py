#!/usr/bin/env python3
"""procfix7 round 5 (gate9 M9-2): the MECHANICAL candidate list of the filesystem / process-read class over EVERY .rs file
of the crate rustred-campaign (src/, tests/, examples/, src/bin/: nothing excluded).

Run from anywhere:   python3 -I crates/rustred-campaign/audit/fs_sites.py [--check | --scan FILE...]
Without flags it writes crates/rustred-campaign/audit/fs_sites.tsv (one line per candidate) and prints the counts;
--check writes nothing and exits 1 when a PRODUCTION candidate has no review (the suite runs it: procfix9_tripwire;
the next auditor re-runs it and diffs fs_sites.tsv).

Kinds (one line per match; a line can match several kinds):
  E  std existence / stat forms: .exists() .try_exists( .is_file() .is_dir() .is_symlink() fs::metadata(
     fs::symlink_metadata( .metadata() .symlink_metadata()  (also on Metadata / FileType receivers: listed, the
     receiver text is shown; on a Path these forms are compile-time warnings by clippy.toml disallowed-methods)
  L  listing forms: read_dir( ; .flatten() / filter_map(Result::ok) / filter_map(|e| e.ok()) within 3 lines after one
  R  reads: fs::read( fs::read_to_string( File::open( .read_to_end( .read_to_string(&mut
  D  destructive: remove_file( remove_dir( remove_dir_all( fs::rename( rename_noreplace( .set_len( File::create(
     .truncate(true) write_atomic( write_json_atomic( create_new_atomic( fs::write( fs::copy( archive_control(
  X  an error-dropping form (.ok() .unwrap_or( .unwrap_or_default() .unwrap_or_else( `let _ =` `if let Ok(` .is_ok()
     .is_err() `Err(_) =>` `let Ok(`; round 6: .is_ok_and( .is_err_and( `Err(..) =>` .map_or( .map_or_else() matches!( in the same statement as an fs / io / proc / clock / env call
  F  the typed helpers: crate::fsx::{presence, presence_lstat, list, list_required, read, read_to_string, open} with
     their site label (the arms are matched explicitly: Unknown / Failed never collapse)
  O  opens (OpenOptions, create(true), create_new(true), libc::open); V environment reads; C external commands;
     W waits (waitid / waitpid / wait / try_wait); K signals (kill / killpg / pidfd_send_signal / cgroup.kill)
Scope: prod (production code of src/), wrapper (src/fsx.rs, src/resources/procfs.rs: the two modules that own the rule),
       testdouble (src/bin/rr_fake_child*), test (tests/, examples/, #[cfg(test)] modules).
Review (production only): an automatic verdict where the code itself shows it (F arms, `?` / map_err propagation, a
Metadata / FileType receiver), else the entry of audit/fs_sites_review.tsv keyed by (file, code text); missing =
UNREVIEWED.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)

PAT = {
    'E': re.compile(r'\.exists\(\)|\.try_exists\(|\.is_file\(\)|\.is_dir\(\)|\.is_symlink\(\)|fs::metadata\(|'
                    r'fs::symlink_metadata\(|\.metadata\(\)|\.symlink_metadata\(\)|'
                    # procfix7 round 6 (gate10 m10-4): UFCS / fn-pointer forms and fs::exists
                    r'\bPath::(exists|try_exists|is_file|is_dir|is_symlink|metadata|symlink_metadata|read_dir|'
                    r'canonicalize|read_link)\b|\bPathBuf::(exists|is_file|is_dir|metadata)\b|fs::exists\(|'
                    r'DirEntry::(metadata|file_type)\b'),
    'L': re.compile(r'read_dir\('),
    'R': re.compile(r'fs::read\(|fs::read_to_string\(|File::open\(|\.read_to_end\(|\.read_to_string\(&mut'),
    'D': re.compile(r'remove_file\(|remove_dir\(|remove_dir_all\(|fs::rename\(|rename_noreplace\(|\.set_len\(|'
                    r'File::create\(|\.truncate\(true\)|write_atomic\(|write_json_atomic\(|create_new_atomic\(|'
                    r'fs::write\(|fs::copy\(|archive_control\('),
    'F': re.compile(r'fsx::(presence|presence_lstat|list|list_required|read|read_to_string|open|canonicalize|canonical)\('),
    # procfix7 round 6 (gate10's site list: kinds the round-5 table did not count)
    'O': re.compile(r'OpenOptions::new\(\)|\.create\(true\)|\.create_new\(true\)|libc::open\('),
    'V': re.compile(r'env::var\(|env::var_os\('),
    'C': re.compile(r'Command::new\('),
    'W': re.compile(r'libc::waitid\(|libc::waitpid\(|\.try_wait\(\)|\.wait\(\)'),
    'K': re.compile(r'libc::kill\(|libc::killpg\(|SYS_pidfd_send_signal|pidfd_send_signal\(|signal_with_pidfd\(|'
                    r'signal_pinned_group|cgroup\.kill|kill_path\('),
}
DROP = re.compile(r'\.ok\(\)|\.unwrap_or\(|\.unwrap_or_default\(\)|\.unwrap_or_else\(|let _ =|if let Ok\(|\.is_ok\(\)|'
                  r'\.is_err\(\)|Err\(_\) =>|let Ok\(|\.is_ok_and\(|\.is_err_and\(|Err\(\.\.\) =>|\.map_or\(|'
                  r'\.map_or_else\(|matches!\(')
IOCALL = re.compile(r'fs::|File::|fsx::|read_dir|metadata\(|OpenOptions|procfs::|read_stat\(|environ|waitid|'
                    r'clock_gettime|env::var|Command::new|\.read\(|\.write\(|libc::read|libc::write|'
                    r'canonicalize\(|read_link\(|file_type\(\)|\.open\(')
FLATTEN = re.compile(r'\.flatten\(\)|filter_map\(\s*Result::ok\s*\)|filter_map\(\|\w+\|\s*\w+\.ok\(\)\)')
DEF = re.compile(r'\bfn (write_atomic|write_json_atomic|create_new_atomic|rename_noreplace|archive_control|'
                 r'remove_stale_temporaries|presence|presence_lstat|list|list_required|read|read_to_string|open)\b')
META_RECV = re.compile(r'(\b(m|md|meta|metadata|a|b|st|ft|t|e\.file_type|file_type\(\))|file_type\(\)|'
                       r'Presence::\w+|\bp\.metadata\(\)\?|\.file_type)\s*$')


def files():
    out = []
    for root, dirs, fs in os.walk(CRATE):
        dirs[:] = sorted(d for d in dirs if d not in ('target', 'audit'))
        for f in sorted(fs):
            if f.endswith('.rs'):
                out.append(os.path.join(root, f))
    return sorted(out)


def scope_of(rel, in_test_mod):
    if rel.startswith('tests/') or rel.startswith('examples/') or in_test_mod:
        return 'test'
    if rel in ('src/fsx.rs', 'src/resources/procfs.rs'):
        return 'wrapper'
    if rel.startswith('src/bin/rr_fake_child'):
        return 'testdouble'
    return 'prod'


def statement(lines, k):
    s = ''
    for x in lines[k:k + 8]:
        s += x.strip() + ' '
        e = x.rstrip()
        if e.endswith(';') or e.endswith('{') or e.endswith(','):
            break
    return s


def norm(code):
    return re.sub(r'\s+', ' ', code.strip())


def load_review():
    rv = {}
    p = os.path.join(HERE, 'fs_sites_review.tsv')
    if os.path.exists(p):
        for line in open(p, encoding='utf-8'):
            if not line.strip() or line.startswith('#'):
                continue
            f, kind, code, verdict = line.rstrip('\n').split('\t', 3)
            rv[(f, kind, norm(code))] = verdict
    return rv


def auto_verdict(kind, rel, lines, k, stmt, code):
    if kind == 'F':
        win = ' '.join(x.strip() for x in lines[k:k + 16])
        if '.metadata()' in win[:240] or '.required(' in win[:240] or 'list_required' in code:
            return 'auto: Unknown / Absent / Failed -> Err (propagated)'
        if re.search(r'Presence::Unknown|Listing::Failed|Read::Failed', win):
            return 'auto: explicit Unknown / Failed arm'
        if re.search(r'fsx::canonical(ize)?\(', code) and ('?' in stmt or 'map_err' in stmt) and not DROP.search(stmt):
            return 'auto: error propagated (? / map_err)'
        return None
    if kind == 'X' and re.search(r'fsx::Read::Content\(\w+\) =>', code):
        return 'auto: a parse default inside a typed Content arm (the read itself is matched explicitly)'
    if kind == 'E' and re.search(r'fsx::presence(_lstat)?\([^)]*\)\s*\.metadata\(\)', stmt):
        return 'auto: Presence::metadata() (Unknown / Absent -> Err, propagated)'
    if kind == 'E' and re.search(r'\b(f|file|self\.file)\.metadata\(\)', code):
        return 'auto: fstat of a descriptor this code holds (no path probe)'
    if kind == 'E':
        m = re.search(r'(.{0,40})\.(is_file|is_dir|is_symlink|metadata|exists)\(\)', code)
        if m and META_RECV.search(m.group(1)):
            return 'auto: on a Metadata / FileType / Presence (not a path probe)'
    if kind in ('R', 'D', 'E', 'L', 'O', 'C', 'W') and ('?' in stmt or 'map_err' in stmt) and not DROP.search(stmt):
        return 'auto: error propagated (? / map_err)'
    if kind == 'O' and 'create_new(true)' in stmt:
        return 'auto: no-clobber create (O_EXCL)'
    if kind == 'K' and rel in ('src/launch/guard.rs',):
        return 'auto: the guard (identity checklist on a pidfd opened first; intents; gate F review)'
    return None


def main():
    review = load_review()
    rows = []
    # --scan FILE...: the given files as PRODUCTION code, no review file (tests/procfix9_tripwire.rs runs it over
    # audit/evasion_forms.rs: every known evasion form must be a candidate)
    scan = sys.argv[sys.argv.index('--scan') + 1:] if '--scan' in sys.argv else None
    for path in (scan if scan is not None else files()):
        rel = os.path.relpath(path, CRATE) if scan is None else 'src/' + os.path.basename(path)
        if scan is not None:
            review = {}
        lines = open(path, encoding='utf-8').read().split('\n')
        in_test = False
        for k, l in enumerate(lines):
            t = l.strip()
            if (t.startswith('mod ') and t.endswith('{')
                    and any('cfg(test)' in x for x in lines[max(0, k - 3):k])):
                in_test = True
            if t.startswith('//'):
                continue
            stmt = statement(lines, k)
            kinds = []
            for kind, pat in PAT.items():
                if pat.search(l) and not (kind in ('D', 'F', 'R') and DEF.search(l)):
                    kinds.append(kind)
            if 'L' in kinds:
                pass
            elif FLATTEN.search(l) and any('read_dir(' in x for x in lines[max(0, k - 3):k + 1]):
                kinds.append('L')
            if DROP.search(stmt) and IOCALL.search(stmt) and DROP.search(l + ' ' + (lines[k + 1] if k + 1 < len(lines) else '')):
                kinds.append('X')
            if not kinds:
                continue
            sc = scope_of(rel, in_test)
            for kind in kinds:
                verdict = ''
                if sc == 'prod':
                    verdict = (review.get((rel, kind, norm(l))) or review.get((rel, '*', norm(l)))
                               or auto_verdict(kind, rel, lines, k, stmt, l) or 'UNREVIEWED')
                elif sc == 'wrapper':
                    verdict = 'wrapper (owns the rule: Absent only on ENOENT / Gone only on ENOENT/ESRCH)'
                m = re.search(r'fsx::\w+\(\s*"([^"]+)"', stmt) if kind == 'F' else None
                rows.append((kind, rel, k + 1, sc, m.group(1) if m else '', verdict, norm(l)[:200]))
    if scan is not None:
        for r in rows:
            print('\t'.join(str(x) for x in (r[0], r[2], r[5], r[6])))
        return
    out = os.path.join(HERE, 'fs_sites.tsv') if '--check' not in sys.argv else os.devnull
    with open(out, 'w', encoding='utf-8') as f:
        f.write('# generated by audit/fs_sites.py (procfix7 round 5, gate9 M9-2): kind\tfile\tline\tscope\tsite\treview\tcode\n')
        for r in rows:
            f.write('\t'.join(str(x) for x in r) + '\n')
    from collections import Counter
    c = Counter((r[0], r[3]) for r in rows)
    print('candidates:', len(rows), 'in', len(files()), '.rs files')
    for kind in 'EFLRDXOVCWK':
        print(' ', kind, ' '.join(f'{s}={c[(kind, s)]}' for s in ('prod', 'wrapper', 'testdouble', 'test')))
    unrev = [r for r in rows if r[3] == 'prod' and r[5] == 'UNREVIEWED']
    print('production candidates UNREVIEWED:', len(unrev))
    for r in unrev[:400]:
        print('  ', r[0], f'{r[1]}:{r[2]}', r[6][:150])
    if '--check' in sys.argv and unrev:
        sys.exit(1)


if __name__ == '__main__':
    main()
