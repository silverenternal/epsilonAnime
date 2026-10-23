#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
status=0
while IFS= read -r -d '' file; do
  lines=$(wc -l < "$file" | tr -d ' ')
  if (( lines > 600 )); then
    echo "FAIL $file: $lines lines"
    status=1
  fi
done < <(find src crates py/backend py/tests py/vendor build.rs \
  -type f \( -name '*.rs' -o -name '*.py' \) -print0; \
  find scripts -type f -name '*.sh' -print0)
if (( status == 0 )); then
  echo "OK: all code files are <= 600 lines"
fi
exit "$status"
