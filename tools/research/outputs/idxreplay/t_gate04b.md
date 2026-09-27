| cost setting | design | admission ms / native at 74M | share at 74M | share at 1G, alpha 0.44 / 0.65 / 0.69 / 0.73 / 0.82 |
|---|---|---:|---:|---|
| 1-thread costs | pipeline, SoA-pattern first-found | 0.351 | 6.3% | 16.8% / 25.8% / 27.8% / 29.9% / 35.0% |
| 1-thread costs | pipeline, SoA-id first-found | 0.528 | 9.2% | 23.6% / 34.7% / 37.0% / 39.5% / 45.1% |
| 1-thread costs | pipeline, SoA-id min-ID | 0.922 | 15.0% | 35.3% / 48.4% / 51.0% / 53.6% / 59.3% |
| 1-thread costs | pipeline, SoA-pattern min-ID | 1.657 | 24.0% | 49.6% / 62.9% / 65.3% / 67.6% / 72.5% |
| 1-thread costs | pipeline, L0 first-found | 5.246 | 50.0% | 75.8% / 84.4% / 85.7% / 87.0% / 89.4% |
| 1-thread costs | today's engine (counters x L0 cost) | 17.155 | 76.6% | 91.1% / 94.7% / 95.2% / 95.6% / 96.5% |
| per-set factors 48 vs 1 thread | pipeline, SoA-pattern first-found | 0.670 | 11.3% | 28.3% / 40.4% / 42.9% / 45.4% / 51.3% |
| per-set factors 48 vs 1 thread | pipeline, SoA-id first-found | 0.992 | 15.9% | 37.0% / 50.3% / 52.9% / 55.4% / 61.1% |
| per-set factors 48 vs 1 thread | pipeline, SoA-id min-ID | 1.890 | 26.5% | 52.9% / 66.0% / 68.3% / 70.5% / 75.1% |
| per-set factors 48 vs 1 thread | pipeline, SoA-pattern min-ID | 3.243 | 38.2% | 65.9% / 77.0% / 78.8% / 80.4% / 83.9% |
| per-set factors 48 vs 1 thread | pipeline, L0 first-found | 8.379 | 61.5% | 83.4% / 89.6% / 90.6% / 91.4% / 93.1% |
| per-set factors 48 vs 1 thread | today's engine (counters x L0 cost) | 26.544 | 83.5% | 94.1% / 96.5% / 96.8% / 97.1% / 97.7% |
| per-set factors 90 vs 1 thread | pipeline, SoA-pattern first-found | 0.842 | 13.8% | 33.2% / 46.1% / 48.7% / 51.3% / 57.0% |
| per-set factors 90 vs 1 thread | pipeline, SoA-id first-found | 1.278 | 19.6% | 43.1% / 56.7% / 59.2% / 61.7% / 67.0% |
| per-set factors 90 vs 1 thread | pipeline, SoA-id min-ID | 2.616 | 33.3% | 60.9% / 72.9% / 74.9% / 76.8% / 80.7% |
| per-set factors 90 vs 1 thread | pipeline, SoA-pattern min-ID | 4.708 | 47.3% | 73.8% / 82.9% / 84.4% / 85.7% / 88.3% |
| per-set factors 90 vs 1 thread | pipeline, L0 first-found | 10.004 | 65.6% | 85.7% / 91.2% / 92.0% / 92.7% / 94.1% |
| per-set factors 90 vs 1 thread | today's engine (counters x L0 cost) | 31.303 | 85.7% | 94.9% / 97.0% / 97.3% / 97.6% / 98.1% |
| scalar factor 2.4 | pipeline, SoA-pattern first-found | 0.816 | 13.5% | 32.5% / 45.3% / 47.9% / 50.5% / 56.3% |
| scalar factor 2.4 | pipeline, SoA-id first-found | 1.240 | 19.1% | 42.4% / 55.9% / 58.4% / 60.9% / 66.3% |
| scalar factor 2.4 | pipeline, SoA-id min-ID | 2.185 | 29.4% | 56.6% / 69.2% / 71.4% / 73.4% / 77.7% |
| scalar factor 2.4 | pipeline, SoA-pattern min-ID | 3.949 | 43.0% | 70.2% / 80.3% / 81.9% / 83.4% / 86.4% |
| scalar factor 2.4 | pipeline, L0 first-found | 12.563 | 70.6% | 88.3% / 92.9% / 93.5% / 94.1% / 95.3% |
| scalar factor 2.4 | today's engine (counters x L0 cost) | 41.171 | 88.7% | 96.1% / 97.7% / 97.9% / 98.1% / 98.5% |
| scalar factor 2.6 | pipeline, SoA-pattern first-found | 0.882 | 14.4% | 34.2% / 47.3% / 49.9% / 52.4% / 58.2% |
| scalar factor 2.6 | pipeline, SoA-id first-found | 1.342 | 20.4% | 44.3% / 57.9% / 60.4% / 62.8% / 68.1% |
| scalar factor 2.6 | pipeline, SoA-id min-ID | 2.365 | 31.1% | 58.5% / 70.9% / 73.0% / 75.0% / 79.1% |
| scalar factor 2.6 | pipeline, SoA-pattern min-ID | 4.276 | 44.9% | 71.9% / 81.5% / 83.0% / 84.4% / 87.3% |
| scalar factor 2.6 | pipeline, L0 first-found | 13.608 | 72.2% | 89.1% / 93.4% / 94.0% / 94.5% / 95.6% |
| scalar factor 2.6 | today's engine (counters x L0 cost) | 44.602 | 89.5% | 96.4% / 97.9% / 98.1% / 98.3% / 98.6% |
| 90-thread factors, native x3.75 (gate 0.3) | pipeline, SoA-pattern first-found | 0.842 | 4.1% | 11.7% / 18.6% / 20.2% / 21.9% / 26.2% |
| 90-thread factors, native x3.75 (gate 0.3) | pipeline, SoA-id first-found | 1.278 | 6.1% | 16.8% / 25.8% / 27.9% / 30.0% / 35.1% |
| 90-thread factors, native x3.75 (gate 0.3) | pipeline, SoA-id min-ID | 2.616 | 11.8% | 29.4% / 41.8% / 44.3% / 46.9% / 52.8% |
| 90-thread factors, native x3.75 (gate 0.3) | pipeline, SoA-pattern min-ID | 4.708 | 19.3% | 42.9% / 56.4% / 59.0% / 61.5% / 66.8% |
| 90-thread factors, native x3.75 (gate 0.3) | pipeline, L0 first-found | 10.004 | 33.7% | 61.5% / 73.4% / 75.4% / 77.3% / 81.1% |
| 90-thread factors, native x3.75 (gate 0.3) | today's engine (counters x L0 cost) | 31.303 | 61.4% | 83.3% / 89.6% / 90.6% / 91.4% / 93.1% |
| 1 thread, MRU k=64 | pipeline, SoA-pattern first-found | 0.335 | 6.0% | 16.2% / 24.8% / 26.8% / 28.8% / 33.8% |
| 1 thread, MRU k=64 | pipeline, SoA-id first-found | 0.505 | 8.8% | 22.8% / 33.6% / 35.9% / 38.4% / 44.0% |
| 1 thread, MRU k=64 | pipeline, SoA-id min-ID | 0.850 | 14.0% | 33.4% / 46.3% / 48.9% / 51.5% / 57.3% |
| 1 thread, MRU k=64 | pipeline, SoA-pattern min-ID | 1.479 | 22.0% | 46.8% / 60.2% / 62.7% / 65.1% / 70.2% |
| 1 thread, MRU k=64 | pipeline, L0 first-found | 5.015 | 48.9% | 75.0% / 83.8% / 85.2% / 86.4% / 89.0% |
| 1 thread, MRU k=64 | today's engine (counters x L0 cost) | 17.155 | 76.6% | 91.1% / 94.7% / 95.2% / 95.6% / 96.5% |
| 1 thread, cheap tier 300 ns | pipeline, SoA-pattern first-found | 0.391 | 6.9% | 17.4% / 26.2% / 28.2% / 30.3% / 35.3% |
| 1 thread, cheap tier 300 ns | pipeline, SoA-id first-found | 0.568 | 9.8% | 24.0% / 35.0% / 37.3% / 39.7% / 45.4% |
| 1 thread, cheap tier 300 ns | pipeline, SoA-id min-ID | 0.961 | 15.5% | 35.6% / 48.6% / 51.2% / 53.7% / 59.4% |
| 1 thread, cheap tier 300 ns | pipeline, SoA-pattern min-ID | 1.696 | 24.5% | 49.8% / 63.1% / 65.4% / 67.7% / 72.6% |
| 1 thread, cheap tier 300 ns | pipeline, L0 first-found | 5.286 | 50.2% | 75.9% / 84.4% / 85.7% / 87.0% / 89.4% |
| 1 thread, cheap tier 300 ns | today's engine (counters x L0 cost) | 17.155 | 76.6% | 91.1% / 94.7% / 95.2% / 95.6% / 96.5% |
| 90-thread factors, cheap 300 ns | pipeline, SoA-pattern first-found | 0.881 | 14.4% | 33.5% / 46.3% / 48.9% / 51.4% / 57.2% |
| 90-thread factors, cheap 300 ns | pipeline, SoA-id first-found | 1.318 | 20.1% | 43.4% / 56.8% / 59.3% / 61.8% / 67.1% |
| 90-thread factors, cheap 300 ns | pipeline, SoA-id min-ID | 2.656 | 33.6% | 61.1% / 73.0% / 75.0% / 76.9% / 80.8% |
| 90-thread factors, cheap 300 ns | pipeline, SoA-pattern min-ID | 4.747 | 47.5% | 73.8% / 83.0% / 84.4% / 85.7% / 88.3% |
| 90-thread factors, cheap 300 ns | pipeline, L0 first-found | 10.044 | 65.7% | 85.7% / 91.2% / 92.0% / 92.7% / 94.2% |
| 90-thread factors, cheap 300 ns | today's engine (counters x L0 cost) | 31.303 | 85.7% | 94.9% / 97.0% / 97.3% / 97.6% / 98.1% |
