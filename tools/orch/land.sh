#!/bin/bash
# Usage: land.sh <branch> <pr-number> [agent-worktree-id]
# Rebases <branch> onto origin/main in the integration worktree, auto-resolving
# module lists (lib.rs / mod.rs) and API_MAPPING.md (union), runs fmt/clippy/
# tests/verify, force-pushes, then merges via merge-when-green.sh.
set -u
# Paths (override via env): ORCH_DIR = this directory; INTEGRATE = a dedicated git worktree used
# only for landing; REPO = main checkout (agent worktrees live under $REPO/.claude/worktrees).
ORCH_DIR=${ORCH_DIR:-$(cd "$(dirname "$0")" && pwd)}
INTEGRATE=${INTEGRATE:-$(cd "$ORCH_DIR/../.." && pwd)/../skia-rust-integrate}
REPO=${REPO:-$(cd "$ORCH_DIR/../.." && pwd)}
BR=$1; PR=$2; AGENT=${3:-}
# One land at a time: the integration worktree is shared.
LOCK=${TMPDIR:-/tmp}/skia-rust-land.lock
if ! mkdir "$LOCK" 2>/dev/null; then echo "LOCKED: another land is running ($(cat $LOCK/pr 2>/dev/null))"; exit 6; fi
echo "#$PR" > "$LOCK/pr"
trap 'rm -rf "$LOCK"' EXIT
export PATH="$HOME/.cargo/bin:/c/Program Files/GitHub CLI:$PATH"
cd "$INTEGRATE" || exit 1
git rebase --abort 2>/dev/null
git fetch -q origin && git checkout -q -B "land/$BR" "origin/$BR" || exit 1
if ! git rebase origin/main >/dev/null 2>&1; then
  while true; do
    for f in $(git diff --name-only --diff-filter=U); do
      case $f in
        tests/src/unit/mod.rs) python3 $ORCH_DIR/resolve_mods.py "$f" --cfg-test ;;
        */lib.rs|*/mod.rs) python3 $ORCH_DIR/resolve_mods.py "$f" ;;
        docs/*.md|docs/design/*.md|*/Cargo.toml|Cargo.toml) python3 $ORCH_DIR/resolve_union.py "$f" ;;
        inventory/manifest.toml) python3 $ORCH_DIR/resolve_manifest.py "$f" ;;
        tests/src/unit/*_test.rs|crates/*/src/*/tests*.rs|crates/*/src/tests*.rs) python3 $ORCH_DIR/resolve_rust_union.py "$f" || exit 3 ;;
        *) echo "MANUAL CONFLICT: $f"; exit 3 ;;
      esac
    done
    python - <<'EOF'
import re
for p in ['tests/src/unit/mod.rs','crates/skia-rust-core/src/lib.rs']:
    try:
        s=open(p,encoding='utf-8').read()
    except FileNotFoundError:
        continue
    s=re.sub(r'^(//!.*\n)\n*', r'\1\n', s, count=1, flags=re.M)
    open(p,'w',encoding='utf-8',newline='\n').write(s)
EOF
    git add -A
    if GIT_EDITOR=true git rebase --continue >/dev/null 2>&1; then break; fi
    [ -z "$(git diff --name-only --diff-filter=U)" ] && { echo "rebase stuck"; exit 4; }
  done
fi
cargo fmt --all --check || { echo "fmt failed"; exit 5; }
cargo clippy -q --workspace --all-targets -- -D warnings > /tmp/land-clippy.log 2>&1 || { grep -E "^(error|warning)" -A6 /tmp/land-clippy.log | head -30; echo "clippy failed"; exit 5; }
cargo test -q --workspace > /tmp/land-test.log 2>&1 || { grep -E "^error|FAILED|panicked" -A4 /tmp/land-test.log | head -30; echo "tests failed"; exit 5; }
cargo xtask inventory verify | tail -1 | grep -q agree || { cargo xtask inventory verify | tail -5; exit 5; }
git push -q --force-with-lease="$BR" origin "land/$BR:$BR" 2>&1 | grep -v "^remote:"
sleep 25
bash $ORCH_DIR/merge-when-green.sh "$PR" 12 "$(git rev-parse HEAD)"
RC=$?
# Only drop the agent's worktree once its PR is merged, so a failed PR can go back to it.
if [ $RC -eq 0 ] && [ -n "$AGENT" ]; then
  git -C "$REPO" worktree remove --force .claude/worktrees/agent-$AGENT 2>/dev/null
fi
git fetch -q origin; git checkout -q --detach origin/main
exit $RC
