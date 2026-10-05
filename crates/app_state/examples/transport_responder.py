import sys

out = sys.stdout.buffer
payload = b"x" * 10_000_000
for line in sys.stdin.buffer:
    n = int(line)
    out.write(memoryview(payload)[:n])
    out.flush()
