#!/bin/bash
# Usage: merge-when-green.sh <pr-number> [expected-check-count]
# Waits until the PR has at least N checks, all completed, then squash-merges
# only if every check succeeded. Never merges on zero/partial checks.
set -u
export PATH="/c/Program Files/GitHub CLI:$PATH"
PR=$1
WANT=${2:-8}
SHA=${3:-}
cd "${INTEGRATE:-.}"
for i in $(seq 1 540); do
  if [ -n "$SHA" ] && [ "$(gh pr view "$PR" --json headRefOid -q .headRefOid)" != "$SHA" ]; then sleep 20; continue; fi
  STATE=$(gh pr view "$PR" --json statusCheckRollup -q \
    '[.statusCheckRollup[] | (.conclusion // .status)] | "\(length) \(map(select(. == "SUCCESS")) | length) \(map(select(. == "IN_PROGRESS" or . == "QUEUED" or . == "PENDING" or . == "" or . == null)) | length)"')
  read -r TOTAL OK PENDING <<<"$STATE"
  if [ "$TOTAL" -ge "$WANT" ] && [ "$PENDING" -eq 0 ]; then
    if [ "$OK" -eq "$TOTAL" ]; then
      if gh pr merge "$PR" --squash ${SHA:+--match-head-commit "$SHA"}; then echo "#$PR merged ($OK/$TOTAL checks green)"; exit 0; fi
      echo "#$PR NOT merged: gh pr merge failed"; exit 1
    fi
    echo "#$PR NOT merged: $OK/$TOTAL checks succeeded"
    gh pr checks "$PR" 2>&1 | grep -v pass
    exit 1
  fi
  sleep 20
done
echo "#$PR: timed out waiting for checks ($STATE)"
exit 2
