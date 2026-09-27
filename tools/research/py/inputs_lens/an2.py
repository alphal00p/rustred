import csv, collections
own=[l.rstrip('\n').split('\t') for l in open('owners.tsv')]
O={int(o[0]):o for o in own}
rows=list(csv.reader(open('v2g7/census.tsv'),delimiter='\t'))[1:]
per=collections.defaultdict(collections.Counter)
rank_excess=collections.defaultdict(collections.Counter)
aex=collections.defaultdict(collections.Counter)
dcls=collections.Counter()
for r in rows:
    phase,owner,rt,rank,ahi,dcl,dlo,inv2,inint,st,closed,c=r
    c=int(c)
    if phase!='0': continue
    o=int(owner)
    if o==255:
        per[255]['n']+=c; continue
    p=per[o]
    p['n']+=c
    p['st'+st]+=c
    if inint=='1': p['inint']+=c; p['inint_st'+st]+=c
    rh=int(O[o][5]); ah=int(O[o][6])
    rk=int(rank)
    rank_excess[O[o][2]][(rk-rh) if rk!=255 else 'inf']+=c
    if ah>=0:
        a=int(ahi)
        aex[O[o][2]][(a-ah) if a!=255 else 'inf']+=c
    else:
        aex[O[o][2]]['helperA=inf,dom_a_hi='+('inf' if ahi=='255' else 'fin')]+=c
    dcls[(O[o][2],dcl,st)]+=c
print('owner mask class t V4 Rh Ah Ah_int guard | apply insp open sealed | in_interim_helper(open)')
tot=collections.Counter()
for o in sorted(per):
    if o==255: print('non-owner apply',per[o]); continue
    p=per[o]; x=O[o]
    print(o,x[1],x[2],x[3],x[4],x[5],x[6],x[8],x[9],'|',p['n'],p['st1'],p['st0'],p['st2'],'|',p['inint'],p['inint_st0'])
    grp=('guard' if x[9]=='1' else ('boundedv2_only' if x[6]!='-1' and x[8]=='-1' else ('unbounded_both' if x[6]=='-1' else 'bounded_both')))
    tot[(grp,'n')]+=p['n']; tot[(grp,'open')]+=p['st0']; tot[(grp,'insp')]+=p['st1']; tot[(grp,'inint')]+=p['inint']; tot[(grp,'owners')]+=1
for k in sorted(tot): print(k,tot[k])
print('rank excess over v2 helper rank, by class')
for k in rank_excess: print(k, sorted(rank_excess[k].items(), key=lambda kv: (str(type(kv[0])),kv[0])))
print('A excess over v2 helper A, by class')
for k in aex: print(k, sorted(aex[k].items(), key=lambda kv: (str(type(kv[0])),kv[0])))
print('D class of apply domains (0: D<9 entirely, 1: meets 9-10, 2: >10)')
for k in sorted(dcls): print(k,dcls[k])
