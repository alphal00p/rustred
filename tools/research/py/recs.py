import sys, json, collections
fn=sys.argv[1]; maxn=int(sys.argv[2])
kinds=collections.Counter(); sec=collections.Counter(); cnt=collections.Counter()
succ=collections.Counter(); ev=collections.Counter()
secs={'Apply':[], 'Route':[]}
shape=collections.Counter()
owners_sec=collections.Counter()
n=0
with open(fn,'rb') as f:
    for line in f:
        n+=1
        if n>maxn: break
        d=json.loads(line)
        k=d.get('record_kind'); kinds[k]+=1
        ph=d.get('phase')
        lo=d.get('lower'); up=d.get('upper'); pb=d.get('power_bounds') or {}
        zl = all(x==0 for x in lo) if lo else None
        infu = sum(1 for x in up if x is None) if up else None
        shape[(ph, k, zl, 'allinf' if infu==len(up) else ('noinf' if infu==0 else 'mixed'), pb.get('max_positive_power') is None)] += 1
        if k=='native_inspection':
            s=d.get('seconds') or 0.0
            sec[ph]+=s; cnt[ph]+=1
            st=d.get('stats') or {}
            succ[ph]+=st.get('successors',0) or 0
            ev[ph]+=d.get('accepted_events',0) or 0
            secs[ph].append(s)
            owners_sec[(ph,d.get('owner'))]+=s
print('lines',n-1 if n>maxn else n)
print('kinds',dict(kinds))
for ph in cnt:
    L=sorted(secs[ph]); tot=sum(L)
    def q(p): return L[min(len(L)-1,int(p*len(L)))]
    heavy=sum(x for x in L if x>=10); mid=sum(x for x in L if 1<=x<10)
    print(ph,'n',cnt[ph],'sec',round(tot,1),'mean %.4f'%(tot/len(L)),'p50 %.4f p90 %.4f p99 %.3f max %.1f'%(q(.5),q(.9),q(.99),L[-1]),
          'share>=10s %.1f%% share1-10s %.1f%%'%(100*heavy/tot if tot else 0,100*mid/tot if tot else 0),'succ/insp %.1f'%(succ[ph]/cnt[ph]),'events/insp %.1f'%(ev[ph]/cnt[ph]))
print('top owners by seconds', [(k,round(v,1)) for k,v in owners_sec.most_common(8)])
print('shapes (phase,kind,zero_lower,upper,Aunbounded):')
for k,v in shape.most_common(14): print(' ',k,v)
