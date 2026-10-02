#!/usr/bin/env bash
# Prints ONE `::error` annotation holding every failed test of a cargo/nextest log with its
# panic message. GitHub shows 10 annotations per step but one message may hold ~64 KB, and
# the job log itself needs admin rights to download.
# Usage: annotate-failures.sh <log> <title>
log=$1
title=$2
plain=$(sed 's/\x1b\[[0-9;]*m//g' "$log" | tr -d '\r')

fails=$(printf '%s\n' "$plain" | grep -E '^\s+(FAIL|SIGABRT|SIGSEGV|SIGTERM|TIMEOUT|LEAK) \[' | sed 's/^\s*//' | sort -u)
[ -n "$fails" ] || fails=$(printf '%s\n' "$plain" | grep -E '^test .* \.\.\. FAILED$' | sort -u)

# The panic line and what follows it up to the blank line or git's `note:`, once per distinct block.
panics=$(printf '%s\n' "$plain" | awk '
  function flush() { if (block != "" && !(block in seen)) { seen[block] = 1; print block; print "" } block = "" }
  /panicked at/ { flush(); block = $0; inside = 1; next }
  inside && (/^[[:space:]]*$/ || /^[[:space:]]*note: run with/ || /^[[:space:]]*(PASS|FAIL|stack backtrace)/) { inside = 0; flush(); next }
  inside { block = block "\n" $0 }
  END { flush() }')

message=$fails
[ -z "$panics" ] || message="$message"$'\n\n'"$panics"
[ -n "$fails$panics" ] || message=$(printf '%s\n' "$plain" | tail -n 40)

printf '::error title=%s::' "$title"
printf '%s' "$message" | sed ':a;N;$!ba;s/%/%25/g;s/\r/%0D/g;s/\n/%0A/g' | head -c 60000 | sed 's/%0\{0,1\}$//'
printf '\n'
