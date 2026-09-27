#!/usr/bin/env bash
snap() { awk '/^cpu[0-9]+ /{id=substr($1,4); idle=$5+$6; tot=0; for(i=2;i<=NF;i++) tot+=$i; print id, idle, tot}' /proc/stat; }
snap > /tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/perfskeptic/s1
timeout 3 tail -f /dev/null
snap > /tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/perfskeptic/s2
paste -d' ' /tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/perfskeptic/s1 /tmp/claude-1125/-common-dev-rustred/7dfabea8-fff6-436f-854b-2fed20c422f3/scratchpad/perfskeptic/s2 | awk '{busy=1-($5-$2)/($6-$3); r=($1<28)?"0-27":($1<128)?"28-127":($1<200)?"128-199":($1<256)?"200-255":"smt"; s[r]+=busy; n[r]++} END{for(k in s) printf "%s busy_cpus=%.1f of %d\n",k,s[k],n[k]}'
