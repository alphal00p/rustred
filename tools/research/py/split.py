import json,sys,collections
ss=collections.Counter(); st=collections.Counter(); un=collections.Counter(); cu=collections.Counter(); co=collections.Counter()
for line in open(sys.argv[1],'rb'):
    r=json.loads(line)
    if r.get('record_kind')!='native_inspection' or r.get('phase')!='Apply': continue
    o=r['owner']; s=r['stats']; k='hot' if o=='011101110111000' else 'other'
    ss[k]+=s.get('same_support_successors',0); st[k]+=s.get('strict_subsupport_successors',0)
    un[k]+=s.get('unsupported_support_successors',0); cu[k]+=s.get('conditional_unsupported_support_successors',0); co[k]+=s.get('conditional_successors',0)
for k in ss: print(k,'same',ss[k],'strict_subset',st[k],'same_frac',round(ss[k]/(ss[k]+st[k]),3),'unsupported',un[k],'cond_unsupported',cu[k],'conditional',co[k])
