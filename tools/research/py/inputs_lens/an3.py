import collections
own=[l.rstrip('\n').split('\t') for l in open('owners.tsv')]
O={int(o[0]):o for o in own}
guard={i for i in O if O[i][9]=='1'}
adj=collections.defaultdict(set); w=collections.Counter()
kinds=collections.Counter()
for line in open('v2g7/transitions.tsv').readlines()[1:]:
    s,t,c=map(int,line.split())
    adj[s].add(t); w[(s,t)]+=c
    ks='A' if s<100000 else ('X' if s<200000 else 'R'); kt='A' if t<100000 else ('X' if t<200000 else 'R')
    kinds[(ks,kt)]+=c
print('edge kinds (A=apply owner, R=route sector):',dict(kinds))
# owner-level: reachability from each owner (Apply node) to other owners
def reach(src):
    seen={src}; st=[src]
    while st:
        v=st.pop()
        for u in adj[v]:
            if u not in seen: seen.add(u); st.append(u)
    return seen
R={o:reach(o) for o in O}
own_reach={o:{x for x in R[o] if x<100000} for o in O}
print('owner -> #owners reachable, reaches guard?, which guards')
hyb=[]
for o in sorted(O):
    g=sorted(own_reach[o]&guard - {o})
    x=O[o]
    print(o,x[1],x[2],'t=',x[3],'v2A=',x[6],'intA=',x[8],'guard' if o in guard else '', '| reach owners',len(own_reach[o]),'| guards reached',g)
    if o not in guard and not g: hyb.append(o)
print('owners that reach no guard owner (excluding guard owners):',len(hyb),hyb)
print('of those, bounded in v2:',[o for o in hyb if O[o][6]!='-1'])
# direct owner->owner edge structure through route nodes: which sources' Route nodes feed guard Apply
feed=collections.Counter()
for (s,t),c in w.items():
    if t in guard and s>=200000:
        feed[(s-200000)]+=c
print('route sectors feeding guard owners:',len(feed))
# sector t of route masks feeding guard owners
tcount=collections.Counter(bin(m).count('1') for m in feed)
print('t of feeding sectors', sorted(tcount.items()))
# self-owner edges, apply->apply direct
aa=sum(c for (s,t),c in w.items() if s<100000 and t<100000)
print('apply->apply edges',aa, 'same owner', sum(c for (s,t),c in w.items() if s<100000 and s==t))
