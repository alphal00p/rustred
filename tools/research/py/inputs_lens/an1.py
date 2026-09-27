import csv, collections
own=[l.rstrip('\n').split('\t') for l in open('owners.tsv')]
cls={int(o[0]):o[2] for o in own}; tt={int(o[0]):int(o[3]) for o in own}; guard={int(o[0]):o[9]=='1' for o in own}
rows=list(csv.reader(open('v2g7/census.tsv'),delimiter='\t'))[1:]
tot=collections.Counter()
by=collections.Counter()
ST={'0':'open','1':'inspected','2':'sealed_only'}
for r in rows:
    phase,owner,rt,rank,ahi,dcl,dlo,inv2,inint,st,closed,c=r
    c=int(c)
    tot[(phase,ST[st])]+=c
    if phase=='0':
        o=int(owner)
        k=cls.get(o,'nonowner')
        by[('apply',k,ST[st])]+=c
        by[('apply_in_v2_helper',inv2,ST[st])]+=c
        by[('apply_in_int_helper',inint,ST[st])]+=c
        by[('apply_guard',guard.get(o),ST[st])]+=c
    else:
        by[('route_t',int(rt),ST[st])]+=c
print('totals',sorted(tot.items()))
for k in sorted(by, key=str): print(k, by[k])
