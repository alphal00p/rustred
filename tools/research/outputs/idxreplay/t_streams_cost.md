{"kind": "streams-input", "label": "g7-streams-w100-v4", "admissions": 54831235, "exact": 327901, "orthant": 95031, "contained": 53531016, "new": 877287, "refused": 0, "engine_forward_checks_contained": 49768209210, "engine_forward_checks_new": 4207220585, "engine_maintenance": 45432849875, "jobs_records": 273150, "snapshot_ids": 74156033, "post_snapshot_new_ids": 877287}
{"kind": "streams-layer-requests", "label": "g7-streams-w100-v4", "layer_hits": 5987195, "misses": 877284, "stale_hits_post_snapshot_container": 222357, "stale_share_of_layer_hits": 0.037139}

| engine class | layout | set | n | found | candidates / request | p50 | p99 | CPU µs / request | CPU ns / tested |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| engine-hit | l0-stored | hit-minid | 168,021 | 1.000 | 2,322 | 217 | 30,557 | 175.3 | 75.51 |
| engine-hit | l0-stored | hit-firstfound | 168,021 | 1.000 | 1,134 | 80 | 15,935 | 75.0 | 66.14 |
| engine-hit | soa-id | hit-minid | 168,021 | 1.000 | 2,096 | 215 | 27,135 | 23.3 | 11.12 |
| engine-hit | soa-id | hit-firstfound | 168,021 | 1.000 | 910 | 64 | 13,090 | 7.6 | 8.33 |
| engine-hit | soa-pattern | hit-minid | 168,021 | 1.000 | 6,720 | 1,764 | 54,572 | 57.6 | 8.57 |
| engine-hit | soa-pattern | hit-firstfound | 168,021 | 1.000 | 580 | 64 | 7,252 | 5.4 | 9.31 |
| engine-miss | l0-stored | miss | 25,526 | 0.000 | 4,585 | 1,617 | 42,301 | 323.7 | 70.60 |
| engine-miss | l0-stored | reverse | 25,526 | 0.000 | 12,330 | 6,132 | 85,724 | 718.9 | 58.31 |
| engine-miss | soa-id | miss | 25,526 | 0.000 | 3,902 | 1,356 | 36,357 | 33.8 | 8.67 |
| engine-miss | soa-id | reverse | 25,526 | 0.000 | 9,999 | 4,176 | 78,692 | 65.4 | 6.54 |
| engine-miss | soa-pattern | miss | 25,526 | 0.000 | 2,626 | 1,120 | 19,176 | 26.3 | 10.02 |
| engine-miss | soa-pattern | reverse | 25,526 | 0.000 | 2,938 | 1,322 | 23,987 | 34.9 | 11.87 |

layer-hit requests at MRU k=16: 6,831,228 (container age = commit watermark minus container ID)
| lag L < (new IDs) | 64 | 1,024 | 4,096 | 16,384 | 65,536 | 262,144 | 1,048,576 | 4,194,304 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stale share of layer-hit requests | 0.59% | 1.51% | 1.89% | 2.24% | 2.60% | 3.11% | 3.50% | 4.15% |
