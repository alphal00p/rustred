import pickle,collections,heapq,math,json
res=pickle.load(open('recscan.pkl','rb'))
def merge_list(dst,src):
    for i,v in enumerate(src):
        if i<3 and isinstance(v,float) is False and False: pass
kinds=collections.Counter(); phase={}; sech={}; evh={}; idbin=collections.defaultdict(collections.Counter); owner={}; rank={}; deleg=collections.Counter(); light={}; maxpos=collections.Counter(); rbb=collections.defaultdict(collections.Counter)
top=[]
def addv(D,k,v,maxidx=()):
    if k not in D: D[k]=list(v); return
    for i,x in enumerate(v):
        if i in maxidx: D[k][i]=max(D[k][i],x)
        else: D[k][i]+=x
for r in res:
    kinds.update(r['kinds'])
    for k,v in r['phase'].items(): addv(phase,k,v)
    for k,v in r['sechist'].items(): addv(sech,k,v)
    for k,v in r['evhist'].items(): addv(evh,k,v)
    for k,v in r['idbin'].items():
        for kk,vv in v.items():
            if kk in ('rkmax','mpmax'): idbin[k][kk]=max(idbin[k][kk],vv)
            else: idbin[k][kk]+=vv
    for k,v in r['owner'].items(): addv(owner,k,v,maxidx=(3,))
    for k,v in r['rank'].items(): addv(rank,k,v)
    deleg.update(r['deleg'])
    for k,v in r['light'].items(): addv(light,k,v)
    maxpos.update(r['maxpos'])
    for k,v in r['ranks_by_bin'].items(): rbb[k].update(v)
    top.extend(r['top'])
top.sort(reverse=True)
pickle.dump(dict(kinds=kinds,phase=phase,sech=sech,evh=evh,idbin=dict(idbin),owner=owner,rank=rank,deleg=deleg,light=light,maxpos=maxpos,rbb=dict(rbb),top=top[:5000]),open('recmerged.pkl','wb'))
print('kinds',dict(kinds))
print('phase [n,sec,events,ops,termvisits]')
for k,v in phase.items(): print(' ',k,v, 'mean sec %.3g ms'%(1e3*v[1]/v[0]), 'ev/insp %.1f'%(v[2]/v[0]), 'us/ev %.2f'%(1e6*v[1]/max(1,v[2])))
print('light (<16384 ev) [n,sec,ev,ops,tv]')
for k,v in light.items(): print(' ',k,v,'us/ev %.2f'%(1e6*v[1]/max(1,v[2])),'us/op %.3f'%(1e6*v[1]/max(1,v[3])))
print('deleg',dict(deleg))
tot_sec=sum(v[1] for v in phase.values()); tot_n=sum(v[0] for v in phase.values()); tot_ev=sum(v[2] for v in phase.values())
print('TOTAL native n %d sec %.0f (%.1f core-h) events %d'%(tot_n,tot_sec,tot_sec/3600,tot_ev))
# seconds histogram cumulative
print('seconds histogram (all phases): decade bin, n, sec share, cum sec share from top, events share')
agg=collections.defaultdict(lambda:[0,0.0,0,0])
for (ph,b),v in sech.items():
    for i in range(4): agg[b][i]+=v[i]
cum=0
for b in sorted(agg,reverse=True):
    v=agg[b]; cum+=v[1]
    print('  %8.2g s  n=%9d  sec=%9.0f (%.3f)  cum_from_top=%.3f  ev=%d'%(10**(b/10),v[0],v[1],v[1]/tot_sec,cum/tot_sec,v[2]))
