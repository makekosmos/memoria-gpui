#!/usr/bin/env bash
# Verify every file under the Vue Memoria src/ is covered by PARITY.md.
# Usage: scripts/check-parity-coverage.sh [path-to-memoria-src] [path-to-parity-md]
set -euo pipefail
SRC=${1:-/workspace/wt/memoria-kos-147/src}
PARITY=${2:-PARITY.md}

missing=0
while IFS= read -r f; do
  rel=${f#"$SRC"/}
  if ! grep -qF "src/$rel" "$PARITY"; then
    echo "NOT COVERED: src/$rel"
    missing=1
  fi
done < <(find "$SRC" -type f | sort)

if [ "$missing" -eq 0 ]; then
  echo "parity coverage: all $(find "$SRC" -type f | wc -l) files covered"
else
  exit 1
fi
