#!/usr/bin/env python3
"""gate10 (a)(1): an INDEPENDENT candidate list of the "unreadable / incomplete taken as absent" class, built from the
sources (not from audit/fs_sites.tsv), over the production code of rustred-campaign, rustred-campaign-core and
rustred-campaign-contract (src/ only; #[cfg(test)] modules and src/bin/rr_fake_child* excluded).

usage: python3 -I sites10.py WORKTREE OUT.tsv
Kinds (a line may match several):
  E existence / stat:   .exists() .try_exists( .is_file() .is_dir() .is_symlink() metadata( symlink_metadata( fs::exists(
                        .is_present() libc::stat libc::lstat libc::access faccessat statx
  L listing:            read_dir( walkdir glob( ; flatten/filter_map(ok) within 3 lines after a listing
  R read / open:        fs::read( read_to_string( File::open( OpenOptions read_link( canonicalize( read_to_end( .lines()
                        BufReader libc::open libc::openat libc::read( fsx::read fsx::open fsx::presence fsx::list
  V environment:        env::var( var_os(
  C external command:   Command::new( .output() .status() .spawn()
  D destructive:        remove_file( remove_dir( remove_dir_all( rename( set_len( File::create( truncate(true) fs::write(
                        fs::copy( create(true) libc::unlink libc::rmdir libc::rename renameat
  K signal:             libc::kill killpg pidfd_send_signal cgroup.kill tgkill
  W wait:               waitid waitpid wait4 .wait() .try_wait()
  X collapse in the same statement (with any of the above): .ok() .unwrap_or( .unwrap_or_default() .unwrap_or_else(
                        let _ = if let Ok( let Ok( .is_ok() .is_err() Err(_) => Err(_) | _ => .flatten() map_or(false
                        is_ok_and( unwrap_or(false) unwrap_or(true) .ok()?
"""
import os
import re
import sys

PAT = {
    'E': r'\.exists\(\)|\.try_exists\(|\.is_file\(\)|\.is_dir\(\)|\.is_symlink\(\)|metadata\(|fs::exists\(|'
         r'\.is_present\(\)|libc::stat\b|libc::lstat\b|libc::access\b|faccessat|statx',
    'L': r'read_dir\(|walkdir|\bglob\(',
    'R': r'fs::read\(|read_to_string\(|File::open\(|OpenOptions|read_link\(|canonicalize\(|read_to_end\(|\.lines\(\)|'
         r'BufReader|libc::open\b|libc::openat\b|libc::read\(|fsx::(read|open|presence|presence_lstat|list|list_required|read_to_string)\(',
    'V': r'env::var\(|var_os\(',
    'C': r'Command::new\(|\.output\(\)|\.status\(\)|\.spawn\(\)',
    'D': r'remove_file\(|remove_dir\(|remove_dir_all\(|rename\(|set_len\(|File::create\(|truncate\(true\)|fs::write\(|'
         r'fs::copy\(|create\(true\)|libc::unlink|libc::rmdir|libc::rename|renameat',
    'K': r'libc::kill\b|killpg|pidfd_send_signal|cgroup\.kill|tgkill',
    'W': r'waitid|waitpid|wait4|\.wait\(\)|\.try_wait\(\)',
}
PAT = {k: re.compile(v) for k, v in PAT.items()}
DROP = re.compile(r'\.ok\(\)|\.unwrap_or\(|\.unwrap_or_default\(\)|\.unwrap_or_else\(|let _ =|if let Ok\(|let Ok\(|'
                  r'\.is_ok\(\)|\.is_err\(\)|Err\(_\) =>|Err\(_\) \||\b_ =>|\.flatten\(\)|map_or\(false|is_ok_and\(|'
                  r'unwrap_or\(false\)|unwrap_or\(true\)')
CRATES = ['rustred-campaign', 'rustred-campaign-core', 'rustred-campaign-contract']


def code_lines(text):
    """(lineno, line, in_test) with comment-only lines marked; test modules detected by #[cfg(test)] + mod."""
    lines = text.split('\n')
    out = []
    depth = 0
    test_depth = None
    pending_cfg_test = False
    for i, l in enumerate(lines):
        s = l.strip()
        if s.startswith('#[cfg(test)]'):
            pending_cfg_test = True
        in_test = test_depth is not None
        if pending_cfg_test and re.match(r'(pub(\(\w+\))?\s+)?mod\s+\w+', s) and test_depth is None:
            test_depth = depth
            in_test = True
            pending_cfg_test = False
        elif pending_cfg_test and s and not s.startswith('#') and not s.startswith('//'):
            # cfg(test) on a single item (fn / use / impl): that item is test-only; treat its line as test
            in_test = True
            if '{' in s and not s.rstrip().endswith(';'):
                test_depth = depth if test_depth is None else test_depth
            pending_cfg_test = False
        # strip strings roughly for brace counting
        t = re.sub(r'"(\\.|[^"\\])*"', '""', l)
        t = re.sub(r"'(\\.|[^'\\])'", "''", t)
        t = t.split('//')[0]
        out.append((i + 1, l, in_test or (test_depth is not None)))
        depth += t.count('{') - t.count('}')
        if test_depth is not None and depth <= test_depth and ('}' in t):
            test_depth = None
    return out


def statement(lines, k):
    s = ''
    for _, x, _ in lines[k:k + 8]:
        s += x.strip() + ' '
        e = x.rstrip()
        if e.endswith(';') or e.endswith('{') or e.endswith(','):
            break
    return s


def main():
    wt, outp = sys.argv[1], sys.argv[2]
    rows = []
    for c in CRATES:
        src = os.path.join(wt, 'crates', c, 'src')
        for root, dirs, fs in os.walk(src):
            dirs.sort()
            for f in sorted(fs):
                if not f.endswith('.rs'):
                    continue
                p = os.path.join(root, f)
                rel = os.path.relpath(p, os.path.join(wt, 'crates'))
                if '/src/bin/rr_fake_child' in '/' + rel:
                    continue
                text = open(p, encoding='utf-8', errors='replace').read()
                lines = code_lines(text)
                for k, (n, l, in_test) in enumerate(lines):
                    s = l.strip()
                    if in_test or s.startswith('//') or s.startswith('///') or s.startswith('//!'):
                        continue
                    kinds = [kk for kk, rx in PAT.items() if rx.search(l)]
                    if not kinds:
                        continue
                    st = statement(lines, k)
                    if DROP.search(st):
                        kinds.append('X')
                    rows.append((rel, n, ''.join(kinds), s[:200].replace('\t', ' ')))
    with open(outp, 'w') as o:
        o.write('# gate10 sites10.py: file\tline\tkinds\tcode\n')
        for r in rows:
            o.write('%s\t%d\t%s\t%s\n' % r)
    from collections import Counter
    byc = Counter(r[0].split('/')[0] for r in rows)
    byk = Counter(k for r in rows for k in r[2])
    print('rows', len(rows), dict(byc), dict(byk))


if __name__ == '__main__':
    main()
