rows=[]
for l in open('hb.tsv'):
    v=l.rstrip('\n').split('\t')
    def f(x):
        try: return float(x)
        except: return None
    rows.append([f(x) for x in v])
rows=[r for r in rows if all(r[i] is not None for i in range(0,23))]
rows.sort(key=lambda r:r[0])
# pick samples at each hour boundary
import bisect
ts=[r[0] for r in rows]
def at(t):
    i=bisect.bisect_left(ts,t); i=min(i,len(rows)-1); return rows[i]
print("win_h  ev/s   compl/h  domains(M) disc(M) pend(M) commit_us/rec prep_us/batch rec/batch specchk/req rev/req coordfwd/req  coordbusy% compl_us_coord/compl ev/compl newdom/compl edges(M) rss(GB)")
H=3600
prev=at(300)
for h in list(range(1,19))+[18.8]:
    cur=at(h*H)
    dt=cur[0]-prev[0]
    dev=cur[1]-prev[1]; dcomp=cur[2]-prev[2]; 
    dprep=cur[6]-prev[6]; dcommit=cur[7]-prev[7]; dbat=cur[8]-prev[8]; drec=cur[9]-prev[9]; dreq=cur[10]-prev[10]; dchk=cur[11]-prev[11]; drev=cur[12]-prev[12]
    dfwd=cur[14]-prev[14]
    dcoord=cur[16]-prev[16]
    busy=(dprep+dcommit+(cur[17]-prev[17])+(cur[18]-prev[18])+(cur[19]-prev[19])+(cur[20]-prev[20])+(cur[21]-prev[21])+(cur[22]-prev[22]))
    ddisc=cur[4]-prev[4]
    print(f"{prev[0]/H:4.1f}-{cur[0]/H:4.1f} {dev/dt:7.0f} {dcomp/dt*3600/1e6:6.2f} {cur[3]/1e6:7.1f} {cur[4]/1e6:6.1f} {cur[5]/1e6:6.1f} {dcommit/drec*1e6 if drec else 0:8.2f} {dprep/dbat*1e6 if dbat else 0:9.0f} {drec/dbat if dbat else 0:7.1f} {dchk/dreq if dreq else 0:8.0f} {drev/dreq if dreq else 0:7.1f} {dfwd/drec if drec else 0:7.1f} {busy/dcoord*100 if dcoord else 0:6.1f} {dcoord/dcomp*1e6 if dcomp else 0:8.0f} {dev/dcomp if dcomp else 0:7.0f} {ddisc/dcomp if dcomp else 0:5.2f} {cur[25]/1e6 if cur[25] else 0:7.0f} {cur[24]/1e9 if cur[24] else 0:5.0f}")
    prev=cur
