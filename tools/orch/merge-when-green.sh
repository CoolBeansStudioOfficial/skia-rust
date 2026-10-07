#!/bin/bash
# REST-only (GraphQL is blocked in cloud sessions). Usage: merge-when-green.sh <pr> <n-checks> <sha>
# Waits for >= n check runs on <sha>, all complete; squash-merges only if all succeeded. Reports once.
set -u
R=repos/CoolBeansStudioOfficial/skia-rust
PR=$1; WANT=$2; SHA=$3
for i in $(seq 1 540); do
  HEAD=$(gh api $R/pulls/$PR --jq .head.sha 2>/dev/null)
  [ "$HEAD" != "$SHA" ] && { echo "#$PR head moved to $HEAD (expected $SHA)"; exit 3; }
  read -r TOTAL OK PENDING <<<"$(gh api "$R/commits/$SHA/check-runs?per_page=100" --jq '[.check_runs[]] | "\(length) \(map(select(.conclusion=="success"))|length) \(map(select(.status!="completed"))|length)"' 2>/dev/null)"
  TOTAL=${TOTAL:-0}; PENDING=${PENDING:-1}
  if [ "$TOTAL" -ge "$WANT" ] && [ "$PENDING" -eq 0 ]; then
    if [ "$OK" -eq "$TOTAL" ]; then
      TITLE=$(gh api $R/pulls/$PR --jq .title)
      if gh api -X PUT $R/pulls/$PR/merge -f merge_method=squash -f sha="$SHA" -f commit_title="$TITLE (#$PR)" >/dev/null; then echo "#$PR merged ($OK/$TOTAL green)"; exit 0; fi
      echo "#$PR NOT merged: merge API failed"; exit 1
    fi
    echo "#$PR NOT merged: $OK/$TOTAL succeeded"
    gh api "$R/commits/$SHA/check-runs?per_page=100" --jq '.check_runs[] | select(.conclusion!="success") | "  \(.name): \(.conclusion)"'
    exit 1
  fi
  sleep 20
done
echo "#$PR timed out"; exit 2
