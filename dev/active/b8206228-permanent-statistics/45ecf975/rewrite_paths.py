import re
EX=(b'dev/active/55c3e433-aff3ct-ldpc-gaps/',b'dev/active/02b8137c/',b'dev/active/1a379447-zen3-cpu-performance/bench-window/')
def drop(f):
    if f==b'.jit' or f.startswith(b'.jit/'): return True
    if not f.startswith((b'dev/bench_results/',b'dev/active/')) or f.startswith(EX): return False
    p=f.split(b'/');b=p[-1].lower()
    if any(x in p[:-1] for x in (b'inputs',b'source-inputs',b'test-logs')): return True
    if b.endswith((b'.log',b'.err')) and not re.search(rb'execution|journal',b): return True
    return b.endswith((b'.zst',b'.data')) or b'perf.data' in b
