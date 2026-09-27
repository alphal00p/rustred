"""Build docs/research/fable51_w0_intel_2026-09-27.md from TMP/w0/intel/RESULTS.md (body) plus a summary."""
import sys
res = open('/common/dev/rustred/TMP/w0/intel/RESULTS.md').read()
body = res[res.index('## Binaries, fixtures, host'):]
head = open('/common/dev/rustred/TMP/w0/intel/docs_note_head.md').read()
out = sys.argv[1]
open(out, 'w').write(head.rstrip('\n') + '\n\n' + body)
print('wrote', out)
