import json,sys
def load(path):
    d=json.load(open(path))
    p=d['parallel']; c=p['coordinator_duty']
    n=d['processed_nodes']
    ck=d.get('checkpoint') or {}
    return dict(n=n, pub=c['publication_seconds'], ckpt=c['checkpoint_seconds'], commit=c['ordered_commit_seconds'],
                coord=c['coordinator_elapsed_seconds'], trav=d['traversal_seconds'], save=ck.get('save_seconds'))
for path in sys.argv[1:]:
    r=load(path)
    per=lambda x: 1e6*x/r['n']
    print(f"{path}: n={r['n']} trav={r['trav']:.2f} pub={r['pub']:.3f}s ({per(r['pub']):.1f}us/rec) ckpt={r['ckpt']:.3f}s ({per(r['ckpt']):.1f}us/rec) pub+ckpt={per(r['pub']+r['ckpt']):.1f}us/rec commit={r['commit']:.3f} last_save={r['save']} commits/s={r['n']/r['trav']:.0f}")
