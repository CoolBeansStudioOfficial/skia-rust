#!/bin/bash
# Usage: land-here.sh <repo-dir> <branch> <title> <body-file>
# Gate (fmt/clippy/debug+release tests/verify), push, open PR (REST), merge when green. Reports once.
set -u
D=$1; BR=$2; TITLE=$3; BODY=$4
R=repos/CoolBeansStudioOfficial/skia-rust
cd "$D" || exit 1
export RUSTFLAGS="-D warnings"
cargo fmt --all --check || { echo "GATE FAIL: fmt"; exit 5; }
cargo clippy -q --workspace --all-targets -- -D warnings > /tmp/lh-clippy.log 2>&1 || { grep -E "^(error|warning)" -A8 /tmp/lh-clippy.log | head -40; echo "GATE FAIL: clippy"; exit 5; }
cargo test -q --workspace > /tmp/lh-test.log 2>&1 || { grep -E "^---- |panicked|left:|right:|^error" -A3 /tmp/lh-test.log | head -60; echo "GATE FAIL: debug tests"; exit 5; }
cargo test -q --release --workspace > /tmp/lh-rtest.log 2>&1 || { grep -E "^---- |panicked|left:|right:|^error" -A3 /tmp/lh-rtest.log | head -60; echo "GATE FAIL: release tests"; exit 5; }
cargo xtask inventory verify > /tmp/lh-verify.log 2>&1; grep -q "unit tests and manifest agree" /tmp/lh-verify.log && grep -q "GMs and manifest agree" /tmp/lh-verify.log || { tail -15 /tmp/lh-verify.log; echo "GATE FAIL: verify"; exit 5; }
git push -q -f -u origin "$BR" 2>&1 | grep -v "^remote:"
SHA=$(git rev-parse HEAD)
PR=$(gh api "$R/pulls?head=CoolBeansStudioOfficial:$BR&state=open" --jq '.[0].number // empty')
[ -z "$PR" ] && PR=$(gh api -X POST $R/pulls -f title="$TITLE" -f head="$BR" -f base=main -F body=@"$BODY" --jq .number)
echo "PR #$PR at $SHA"
sleep 30
bash "$(dirname "$0")/merge-when-green.sh" "$PR" 12 "$SHA"
