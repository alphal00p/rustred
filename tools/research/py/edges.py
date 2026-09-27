import numpy as np, json, sys
p='TMP/v2-checkpoint-copy-gen3/edges-00000000000000000003.bin'
raw=np.memmap(p,dtype=np.uint8,mode='r')
hdr=bytes(raw[:32]); print(hdr[:8], int.from_bytes(hdr[16:24],'little'), int.from_bytes(hdr[24:32],'little'))
e=np.memmap(p,dtype=np.uint32,mode='r',offset=32).reshape(-1,2)
n=e.shape[0]; print('edges',n)
tgt=e[:,1]; src=e[:,0]
for lim in (67,183,1000,100000):
    print('target <',lim, float((tgt<lim).sum())/n)
cnt=np.bincount(tgt)
nz=cnt[cnt>0]
print('distinct targets',nz.size,'nodes',cnt.size)
s=np.sort(nz)[::-1]
cs=np.cumsum(s)/n
for k in (1,10,67,100,1000,10000,100000,1000000):
    if k<=s.size: print('top',k,'targets share',float(cs[k-1]))
print('top10 targets', np.argsort(cnt)[::-1][:20].tolist(), s[:20].tolist())
# self edges / src->tgt direction
print('tgt>src frac', float((tgt>src).sum())/n)
