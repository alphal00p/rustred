import json,sys
r=json.load(open(sys.argv[1]))
p=r['parallel']
wall=p['coordinator_duty']['coordinator_elapsed_seconds']
busy=sum(p['slot_busy_seconds']); bp=sum(p['slot_backpressure_seconds']); idle=sum(p['slot_idle_seconds'])
print('wall',wall,'busy',busy/wall,'backpressure',bp/wall,'computing',(busy-bp)/wall,'idle',idle/wall)
d=p['coordinator_duty']
keys=['ordered_commit_seconds','preparation_seconds','dispatch_seconds','poll_seconds','progress_json_seconds','publication_seconds','closure_refresh_seconds','checkpoint_seconds','ready_service_seconds','wait_seconds']
s=0
for k in keys:
    print(k, round(d[k]/wall*100,2)); s+=d[k]
print('sum',round(s/wall*100,2))
ap=p['admission_preparation']
print('commit us/record',ap['ordered_commit_wall_seconds']/ap['prepared_batch_records']*1e6)
print('prep us/batch',ap['preparation_wall_seconds']/ap['parallel_batches']*1e6)
print('records/batch',ap['prepared_batch_records']/ap['parallel_batches'])
print('spec fwd/request',ap['speculative_containment_checks']/ap['speculative_admission_requests'])
print('spec rev/request',ap['speculative_reverse_checks']/ap['speculative_admission_requests'])
print('fwd bit reject share',ap['speculative_forward_bit_rejections']/ap['speculative_containment_checks'])
print('rev bit reject share',ap['speculative_reverse_bit_rejections']/ap['speculative_reverse_checks'])
cp=p['containment_prefilter']
print('coord fwd callbacks',cp['forward_callbacks'],'rev',cp['reverse_callbacks'])
print('requests/s',ap['speculative_admission_requests']/wall)
print('native per s',r['completed_nodes']/wall, 'successors per native', r['successors']/r['completed_nodes'])
print('new domains per native', r['scheduled_nodes']/r['completed_nodes'])
print('events per native', r['events']/r['completed_nodes'])
print('edges per domain', r['descendant_closure']['dependency_edges']/r['scheduled_nodes'])
print('edges per native', r['descendant_closure']['dependency_edges']/r['completed_nodes'])
