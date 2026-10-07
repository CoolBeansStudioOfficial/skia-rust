#!/bin/bash
# Usage: queue.sh "branch:pr[:agent]" ... — lands PRs strictly in order; stops at the first failure.
T=${ORCH_DIR:-$(cd "$(dirname "$0")" && pwd)}
for item in "$@"; do
  IFS=: read -r br pr agent <<<"$item"
  echo "=== landing #$pr ($br)"
  if ! bash $T/land.sh "$br" "$pr" "$agent"; then
    echo "QUEUE STOPPED at #$pr"
    exit 1
  fi
done
echo "QUEUE DONE"
