import sys, struct, array, collections
fn=sys.argv[1]
with open(fn,'rb') as f:
    hdr=f.read(32); _,_,_,_,_,count,first=struct.unpack('<4s4sHHIQQ',hdr)
    a=array.array('I'); a.frombytes(f.read(count*8))
tgt=a[1::2]
runmax=0; bins=collections.Counter(); new=0
for t in tgt:
    if t>runmax:
        runmax=t; new+=1; continue
    lag=runmax-t
    if lag<256: bins['<256']+=1
    elif lag<4096: bins['<4k']+=1
    elif lag<65536: bins['<64k']+=1
    elif lag<1048576: bins['<1M']+=1
    else: bins['>=1M']+=1
tot=len(tgt)
print('edges',tot,'new-max edges (fresh admissions proxy)',new,'%.1f%%'%(100*new/tot))
for k in ['<256','<4k','<64k','<1M','>=1M']: print(k, bins[k], '%.2f%%'%(100*bins[k]/tot))
