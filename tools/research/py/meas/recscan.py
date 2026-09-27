import json,sys,os,re,math,heapq,collections
from multiprocessing import Pool
D='/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/checkpoints/main'
files=[os.path.join(D,f) for f in sorted(os.listdir(D)) if f.startswith('records-')]
CH=256*1024*1024
tasks=[]
for f in files:
    sz=os.path.getsize(f)
    s=0
    while s<sz:
        tasks.append((f,s,min(sz,s+CH))); s+=CH
kind_re=re.compile(rb'"record_kind":"([a-z_]+)"')
phase_re=re.compile(rb'"phase":"([A-Za-z]+)"')
id_re=re.compile(rb'"id":(\d+)')
status_re=re.compile(rb'"responsibility_status":"([a-z_]+)"')
def lb(x):
    return int(math.floor(math.log10(x)*10)) if x>0 else -999
def work(t):
    f,s,e=t
    agg={'kinds':collections.Counter(),'phase':collections.defaultdict(lambda:[0,0.0,0,0,0]),
         'sechist':collections.defaultdict(lambda:[0,0.0,0,0]),'evhist':collections.defaultdict(lambda:[0,0.0,0,0]),
         'idbin':collections.defaultdict(lambda:collections.Counter()),'owner':collections.defaultdict(lambda:[0,0.0,0,0.0,0]),
         'rank':collections.defaultdict(lambda:[0,0.0,0]),'deleg':collections.Counter(),'top':[],'light':collections.defaultdict(lambda:[0,0.0,0,0,0]),
         'maxpos':collections.Counter(),'ranks_by_bin':collections.defaultdict(lambda:collections.Counter())}
    top=[]
    with open(f,'rb') as fh:
        fh.seek(s)
        if s>0: fh.readline()
        while fh.tell()<=e:
            line=fh.readline()
            if not line: break
            m=kind_re.search(line)
            k=m.group(1).decode() if m else 'none'
            agg['kinds'][k]+=1
            if k=='native_inspection':
                try: d=json.loads(line)
                except Exception: agg['kinds']['bad']+=1; continue
                sec=d.get('seconds') or 0.0; st=d.get('stats') or {}
                ev=st.get('events',0) or 0; ph=d.get('phase'); own=d.get('owner'); rk=d.get('rank'); i=d['id']
                ops=st.get('native_operations',0) or 0; tv=st.get('term_visits',0) or 0
                pb=d.get('power_bounds') or {}
                p=agg['phase'][ph]; p[0]+=1; p[1]+=sec; p[2]+=ev; p[3]+=ops; p[4]+=tv
                h=agg['sechist'][(ph,lb(sec))]; h[0]+=1; h[1]+=sec; h[2]+=ev; h[3]+=ops
                eb=int(math.log2(ev)) if ev>0 else -1
                h=agg['evhist'][(ph,eb)]; h[0]+=1; h[1]+=sec; h[2]+=ev; h[3]+=ops
                b=i//1000000
                c=agg['idbin'][b]; c['n_'+ph]+=1; c['sec_'+ph]+=sec; c['ev_'+ph]+=ev; c['ops_'+ph]+=ops
                if rk is not None: c['rkmax']=max(c['rkmax'],rk); c['rksum_'+ph]+=rk
                mp=pb.get('max_positive_power')
                if mp is not None: c['mpmax']=max(c['mpmax'],mp)
                agg['ranks_by_bin'][(b,ph)][rk]+=1
                o=agg['owner'][(ph,own)]; o[0]+=1; o[1]+=sec; o[2]+=ev; o[3]=max(o[3],sec); o[4]+=ops
                r=agg['rank'][(ph,rk)]; r[0]+=1; r[1]+=sec; r[2]+=ev
                agg['maxpos'][(ph,mp)]+=1
                if ev<16384:
                    L=agg['light'][ph]; L[0]+=1; L[1]+=sec; L[2]+=ev; L[3]+=ops; L[4]+=tv
                up=d.get("upper") or []; lo=d.get("lower") or []
                wid=sum((u-l) for u,l in zip(up,lo) if u is not None and l is not None); nun=sum(1 for u in up if u is None)
                item=(sec,i,ev,own,ph,rk,ops,tv,mp,wid,nun)
                if len(top)<3000: heapq.heappush(top,item)
                elif sec>top[0][0]: heapq.heapreplace(top,item)
            elif k=='delegated_not_inspected':
                pm=phase_re.search(line); im=id_re.search(line); sm=status_re.search(line)
                ph=pm.group(1).decode() if pm else '?'
                agg['deleg'][(ph, sm.group(1).decode() if sm else '?')]+=1
                if im:
                    agg['idbin'][int(im.group(1))//1000000]['deleg_'+ph]+=1
    agg['top']=top
    # convert defaultdicts
    return {k:(dict(v) if isinstance(v,(dict,collections.Counter)) else v) for k,v in agg.items()}
if __name__=='__main__':
    with Pool(int(sys.argv[1]) if len(sys.argv)>1 else 24) as pool:
        res=pool.map(work,tasks,chunksize=1)
    import pickle
    pickle.dump(res,open('recscan.pkl','wb'))
    print(len(tasks),'tasks done')
