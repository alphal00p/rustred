import sys
res = '/common/dev/rustred/TMP/w0/intel/RESULTS.md'
s = open(res).read()
d = '/common/dev/rustred/TMP/w0/intel/replay/'
def sel(path, keep=None):
    lines = open(d + path).read().splitlines()
    if keep:
        lines = [l for l in lines if l.startswith('| trace') or l.startswith('|---') or keep(l)]
    return '\n'.join(lines)
rep = {
    'DYNAMIC_TABLE': sel('t_dyn.md'),
    'PIPELINE_TABLE': sel('t_pipe.md', lambda l: ('| all |' in l and any(f'| {k} |' in l for k in (1, 16, 64))) or ('5f-w18' in l and '| 16 |' in l)),
    'STATIC_TABLE': sel('t_static.md'),
    'THIN_TABLE': sel('t_thin.md'),
    'THR18_TABLE': sel('t_thr18.md'),
    'LAG_TABLE': sel('t_lag.md'),
}
for k, v in rep.items():
    if k in s:
        s = s.replace(k, v)
open(res, 'w').write(s)
print('filled', [k for k in rep])
