{"kind": "streams-input", "label": "g7-streams-w100", "admissions": 54831235, "exact": 327901, "orthant": 95031, "contained": 53531016, "new": 877287, "refused": 0, "engine_forward_checks_contained": 49768209210, "engine_forward_checks_new": 4207220585, "engine_maintenance": 45432849875, "jobs_records": 273150, "snapshot_ids": 74156033, "post_snapshot_new_ids": 877287}
{"kind": "streams-layer-requests", "label": "g7-streams-w100", "layer_hits": 5987195, "misses": 877284, "stale_hits_post_snapshot_container": 222357, "stale_share_of_layer_hits": 0.037139}

| engine class | layout | set | n | found | candidates / request | p50 | p99 | CPU µs / request | CPU ns / tested |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| engine-hit | l0-stored | hit-minid | 167,927 | 1.000 | 2,361 | 214 | 30,914 | 201.3 | 85.26 |
| engine-hit | l0-stored | hit-firstfound | 167,927 | 1.000 | 1,148 | 79 | 16,319 | 82.0 | 71.39 |
| engine-hit | soa-id | hit-minid | 167,927 | 1.000 | 2,129 | 213 | 27,510 | 27.4 | 12.86 |
| engine-hit | soa-id | hit-firstfound | 167,927 | 1.000 | 920 | 64 | 13,269 | 8.2 | 8.89 |
| engine-hit | soa-pattern | hit-minid | 167,927 | 1.000 | 6,712 | 1,765 | 55,268 | 61.2 | 9.12 |
| engine-hit | soa-pattern | hit-firstfound | 167,927 | 1.000 | 584 | 64 | 7,360 | 6.1 | 10.44 |
| engine-miss | l0-stored | miss | 25,679 | 0.000 | 4,483 | 1,572 | 41,849 | 336.9 | 75.16 |
| engine-miss | l0-stored | reverse | 25,679 | 0.000 | 12,272 | 6,109 | 84,051 | 835.5 | 68.08 |
| engine-miss | soa-id | miss | 25,679 | 0.000 | 3,826 | 1,310 | 36,417 | 38.5 | 10.05 |
| engine-miss | soa-id | reverse | 25,679 | 0.000 | 9,939 | 4,169 | 78,812 | 70.5 | 7.09 |
| engine-miss | soa-pattern | miss | 25,679 | 0.000 | 2,585 | 1,102 | 19,373 | 26.1 | 10.10 |
| engine-miss | soa-pattern | reverse | 25,679 | 0.000 | 2,884 | 1,312 | 23,832 | 38.8 | 13.47 |
