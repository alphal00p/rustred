import re,sys,os
root=sys.argv[1]; notes=sys.argv[2]
files=[f"supervisor/{x}.rs" for x in ("mod","orphans","workdir","failpoint","identity")]
files+=["launch/"+f for f in sorted(os.listdir(root+"/launch")) if f.endswith(".rs")]
files+=["resources/"+f for f in sorted(os.listdir(root+"/resources")) if f.endswith(".rs")]
files+=["lock.rs","ledger.rs","atomic.rs"]
pat=re.compile(r"let _ =|\.ok\(\)|unwrap_or_default\(\)|unwrap_or\(|unwrap_or_else\(|if let Ok\(|let Ok\(|\.flatten\(\)|filter_map\(\s*\|?[a-z_]*\|?\s*[a-z_.]*\.ok\(\)|Result::ok|Err\(_\)\s*=>|\.is_ok\(\)|\.is_ok_and\(|\.exists\(\)|\.is_err\(\)")
tab=open(notes).read()
tabcodes=set()
for l in tab.splitlines():
    if l.startswith("| src/"):
        parts=l.split("|")
        if len(parts)>4: tabcodes.add((parts[1].strip().split(":")[0], re.sub(r"\s+","",parts[3].strip().strip("`"))[:60]))
out=[]
for f in files:
    p=os.path.join(root,f)
    if not os.path.exists(p): continue
    L=open(p).read().split("\n"); intest=False; depth=0
    for i,l in enumerate(L,1):
        s=l.strip()
        if s.startswith("#[cfg(test)]"): intest=True
        if intest: continue
        if s.startswith("//"): continue
        if pat.search(l):
            code=re.sub(r"\s+","",s)[:60]
            key=("src/"+f,code)
            hit=any(k[0]=="src/"+f and (k[1][:40] in code or code[:40] in k[1]) for k in tabcodes)
            out.append((f,i,"IN_TABLE" if hit else "NOT_IN_TABLE",s[:150]))
for f,i,h,s in out:
    print(f"{h}\t{f}:{i}\t{s}")
