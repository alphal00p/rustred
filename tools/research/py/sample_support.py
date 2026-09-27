import json, os, collections, sys
f='TMP/v2-checkpoint-copy-gen3/records-00000000000000000003.jsonl'
size=os.path.getsize(f)
agg=collections.Counter(); n=0; kinds=collections.Counter()
ex_unsup=[]
with open(f,'rb') as fh:
    for k in range(40):
        fh.seek(int(size*k/40)); 
        if k: fh.readline()
        for _ in range(25000):
            line=fh.readline()
            if not line: break
            r=json.loads(line); n+=1
            kinds[(r.get('record_kind'),r.get('phase'))]+=1
            s=r.get('stats') or {}
            for key in ('successors','same_support_successors','strict_subsupport_successors','unsupported_support_successors','conditional_unsupported_support_successors','problems','events'):
                if key in s: agg[key]+=s[key]
            if s.get('unsupported_support_successors',0)>0 and len(ex_unsup)<3:
                ex_unsup.append((r['id'],r['owner'],r['phase'],s['unsupported_support_successors'],s.get('successors')))
            if r.get('frontiers'): agg['records_with_frontiers']+=1
            if r.get('error'): agg['records_with_error']+=1
print('records sampled',n); print(kinds); print(dict(agg)); print(ex_unsup)
