import json,sys
r=json.load(open(sys.argv[1]))
def show(d,prefix='',depth=0):
    for k,v in d.items():
        if isinstance(v,dict) and depth<4:
            show(v,prefix+k+'.',depth+1)
        elif isinstance(v,list):
            print(prefix+k,'= list',len(v), (v[:5] if len(v)<200 and all(not isinstance(x,(dict,list)) for x in v) else ''))
        else:
            print(prefix+k,'=',v)
show(r)
