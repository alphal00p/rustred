import csv, collections
own=[l.rstrip('\n').split('\t') for l in open('owners.tsv')]
O={int(o[0]):o for o in own}
guard={i for i in O if O[i][9]=='1'}
def Lset(path):
    adj=collections.defaultdict(set)
    for line in open(path).readlines()[1:]:
        s,t,c=map(int,line.split()); adj[s].add(t)
    L=[]
    for o in O:
        if o in guard: continue
        seen={o}; st=[o]
        while st:
            v=st.pop()
            for u in adj[v]:
                if u not in seen: seen.add(u); st.append(u)
        if not (seen & guard): L.append(o)
    return sorted(L)
L3=Lset('v2g3/transitions.tsv'); L7=Lset('v2g7/transitions.tsv')
print('L from gen3:',len(L3),'from gen7:',len(L7),'equal:',L3==L7, 'diff', set(L3)^set(L7))
L=set(L7)
def grp(o): return 'L' if o in L else ('G' if o in guard else 'U')
for g in ('v2g3','v2g6','v2g7'):
    rows=list(csv.reader(open(g+'/census.tsv'),delimiter='\t'))[1:]
    c=collections.Counter()
    for r in rows:
        n=int(r[-1]); st={'0':'open','1':'insp','2':'sealed'}[r[9]]
        if r[0]=='0': c[('A'+grp(int(r[1])),st)]+=n
        else: c[('R',st)]+=n
    print(g, {k:v for k,v in sorted(c.items())})
