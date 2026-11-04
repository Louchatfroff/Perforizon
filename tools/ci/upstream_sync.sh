#!/usr/bin/env bash
# Merge Horizon's main into Perforizon's main when it is safe. Docs: docs/perforizon-sync.md.
#
#   bash tools/ci/upstream_sync.sh --no-push     # dry run: merge and check on a local sync/ branch, push nothing
#   bash tools/ci/upstream_sync.sh               # what the upstream-sync workflow runs
#
# Outcomes: nothing new -> exit 0 quietly; conflicts -> one issue "Upstream sync blocked: conflicts";
# a failed check -> branch sync/upstream-<sha> + PR "Upstream sync: checks failed", main untouched;
# everything green -> main is fast-forwarded to the merge commit.
# Env: SYNC_FORCE=1 (redo a sync whose branch already exists), SYNC_SKIP_CHECKS=1 (tests of this script only).
set -euo pipefail

PUSH=1
for arg in "$@"; do
  case "$arg" in
    --no-push) PUSH=0 ;;
    *) echo "usage: $0 [--no-push]" >&2; exit 2 ;;
  esac
done

UPSTREAM_URL="${UPSTREAM_URL:-https://github.com/horizon-rust/horizon.git}"
BASE=main
# Tests known to fail on this fork's runners and upstream: they do not block a sync. Format: full test path.
SKIP_TESTS=(
  "lan::io_pool::tests::a_close_ends_a_disconnect_the_client_does_not_take"
)
# World generation must stay bit-identical to Horizon: only these files under worldgen may differ from upstream.
WORLDGEN_DIR=crates/horizon-world/src/worldgen
WORLDGEN_ALLOWED=("$WORLDGEN_DIR/decoration_order.rs")
# Files the launcher's look needs.
LAUNCHER_FILES=(launcher/src/palette.rs launcher/src/tui.rs)

cd "$(git rev-parse --show-toplevel)"
LOGS="$(mktemp -d)"
PY=python3; command -v python3 >/dev/null 2>&1 || PY=python
TRAILER="Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"

say() { echo "upstream-sync: $*"; }
summary() { if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then printf '%s\n' "$*" >> "$GITHUB_STEP_SUMMARY"; fi; }
output() { if [ -n "${GITHUB_OUTPUT:-}" ]; then echo "$1=$2" >> "$GITHUB_OUTPUT"; fi; }
have_gh() { [ "$PUSH" = 1 ] && command -v gh >/dev/null 2>&1; }

git config user.name >/dev/null 2>&1 || git config user.name "github-actions[bot]"
git config user.email >/dev/null 2>&1 || git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

git remote get-url upstream >/dev/null 2>&1 || git remote add upstream "$UPSTREAM_URL"
git fetch --quiet upstream main
if [ "$PUSH" = 1 ]; then
  git fetch --quiet origin "$BASE"
  git checkout --quiet -B "$BASE" "origin/$BASE"
fi

if git merge-base --is-ancestor upstream/main "$BASE"; then
  say "up to date with upstream, nothing to do"
  output result up-to-date
  exit 0
fi

SHORT="$(git rev-parse --short=9 upstream/main)"
BRANCH="sync/upstream-$SHORT"
RANGE="$(git merge-base "$BASE" upstream/main | cut -c1-9)..$SHORT ($(git rev-list --count "$BASE"..upstream/main) commits)"

if [ "$PUSH" = 1 ] && [ "${SYNC_FORCE:-0}" != 1 ] && git ls-remote --exit-code --heads origin "$BRANCH" >/dev/null 2>&1; then
  say "$BRANCH already exists on origin (failed earlier, PR open); use force to redo"
  output result already-reported
  exit 0
fi

git checkout --quiet -B "$BRANCH" "$BASE"
# Conflicts that are the same every time are settled here: README.md stays ours, language files are merged key by key
# (our removals and additions kept, upstream's new keys taken), and files the terminal-style launcher deleted on purpose
# stay deleted. Anything else is a real conflict.
auto_resolve() {
  local f
  if git diff --name-only --diff-filter=U | grep -qx README.md; then
    git checkout --ours -- README.md && git add README.md
  fi
  local langs
  langs="$(git diff --name-only --diff-filter=U -- 'launcher/lang/*.json' || true)"
  if [ -n "$langs" ]; then
    # shellcheck disable=SC2086
    "$PY" tools/ci/merge_lang.py $langs && git add $langs
  fi
  while read -r status path; do
    [ "$status" = DU ] || continue
    case "$path" in
      launcher/src/glass*|launcher/src/partner.rs|launcher/src/themes*|launcher/src/frame.rs|launcher/src/sky.rs)
        git rm -q -- "$path" ;;
    esac
  done < <(git status --porcelain | awk '{print $1" "$2}')
}

if ! git merge --no-commit --no-ff upstream/main >"$LOGS/merge.log" 2>&1; then
  auto_resolve || true
  if [ -n "$(git diff --name-only --diff-filter=U || true)" ]; then
  FILES="$(git diff --name-only --diff-filter=U || true)"
  git merge --abort 2>/dev/null || git reset --hard --quiet
  git checkout --quiet "$BASE"
  git branch -D "$BRANCH" >/dev/null
  say "merge of $RANGE conflicts:"; echo "$FILES"
  [ -n "$FILES" ] || FILES="(none listed; git said: $(tail -n 3 "$LOGS/merge.log"))"
  BODY="$(printf 'Merging upstream Horizon `%s` into `%s` conflicts. Nothing was pushed to main.\n\nUpstream range: `%s`\n\nConflicted files:\n\n```\n%s\n```\n\nResolve by hand: see docs/perforizon-sync.md (section "Blocked sync"). Close this issue once resolved.' "$SHORT" "$BASE" "$RANGE" "$FILES")"
  if have_gh; then
    gh label create upstream-sync --color ededed --description "Automatic upstream Horizon sync" >/dev/null 2>&1 || true
    NUM="$(gh issue list --label upstream-sync --state open --search 'in:title "Upstream sync blocked: conflicts"' --json number --jq '.[0].number // empty')"
    if [ -n "$NUM" ]; then
      gh issue view "$NUM" --json body --jq .body | grep -q "$SHORT" || gh issue edit "$NUM" --body "$BODY" >/dev/null
      say "issue #$NUM updated"
    else
      gh issue create --title "Upstream sync blocked: conflicts" --label upstream-sync --body "$BODY" >/dev/null \n        || gh issue create --title "Upstream sync blocked: conflicts" --body "$BODY" >/dev/null \n        || say "could not open the issue"
      say "issue opened"
    fi
  fi
  summary "### Upstream sync blocked: conflicts"; summary "$BODY"
  output result conflicts
  exit 0
  fi
fi

# GITHUB_TOKEN cannot push changes to .github/workflows: keep ours, drop what upstream added there.
git checkout HEAD -- .github/workflows
for f in $(git diff --cached --name-only --diff-filter=A -- .github/workflows); do git rm -qf -- "$f"; done
git commit --quiet -m "Merge upstream Horizon main ($SHORT) into Perforizon" -m "$TRAILER"
MERGE_SHA="$(git rev-parse HEAD)"

# ---- checks -----------------------------------------------------------------------------------------------------
FAILED=()
RESULTS=()
check() { # name, command...
  local name="$1"; shift
  local log="$LOGS/${name// /_}.log"
  if [ "${SYNC_SKIP_CHECKS:-0}" = 1 ]; then RESULTS+=("| $name | skipped |"); return; fi
  say "check: $name"
  if "$@" >"$log" 2>&1; then
    RESULTS+=("| $name | pass |")
  else
    RESULTS+=("| $name | **FAIL** |"); FAILED+=("$name")
    { echo "----- $name (last 40 lines) -----"; tail -n 40 "$log"; } >> "$LOGS/failures.txt"
  fi
}

PKGS=(-p horizon-client -p horizon-client-lib -p horizon-client-render -p horizon-protocol -p horizon-world)
TEST_PKGS=(-p horizon-client-render -p horizon-client -p horizon-client-lib -p horizon-protocol)

c_fmt()    { cargo fmt --all -- --check; }
# ci.yml denies warnings (clippy ... -D warnings), so do we.
c_clippy() { cargo clippy "${PKGS[@]}" --all-targets --locked -- -D warnings; }
c_test() {
  local skips=() t
  for t in "${SKIP_TESTS[@]}"; do skips+=(--skip "$t"); done
  cargo test --locked "${TEST_PKGS[@]}" --lib -- "${skips[@]}"
}
c_build()  { cargo build --locked --profile fast -p horizon-client; }
c_tree()   { "$PY" tools/check_public_tree.py; }
c_worldgen() {
  local changed bad=0 f a ok
  changed="$(git diff --name-only upstream/main HEAD -- "$WORLDGEN_DIR")"
  for f in $changed; do
    ok=0; for a in "${WORLDGEN_ALLOWED[@]}"; do [ "$f" = "$a" ] && ok=1; done
    if [ "$ok" = 0 ]; then echo "worldgen differs from upstream: $f"; bad=1; fi
  done
  return $bad
}
c_launcher_files() {
  local f bad=0
  for f in "${LAUNCHER_FILES[@]}"; do if [ ! -f "$f" ]; then echo "missing $f"; bad=1; fi; done
  return $bad
}
c_launcher_build() { (cd launcher && cargo build --locked); }

check "rustfmt" c_fmt
check "clippy" c_clippy
check "tests" c_test
check "build fast" c_build
check "public tree" c_tree
check "worldgen identical" c_worldgen
check "launcher files" c_launcher_files
check "launcher build" c_launcher_build

ROWS="$(printf '%s\n' "${RESULTS[@]}")"
TABLE="$(printf '| Check | Result |\n| --- | --- |\n%s' "$ROWS")"

if [ "${#FAILED[@]}" = 0 ]; then
  if [ "$PUSH" = 1 ]; then
    # Plain push, never forced: if main moved while the checks ran, git refuses and the next run redoes the sync.
    if ! git push origin "$MERGE_SHA:refs/heads/$BASE"; then
      say "push to $BASE refused (main moved or is protected); nothing changed, the next run retries"
      git checkout --quiet "$BASE"; git branch -D "$BRANCH" >/dev/null
      summary "### Upstream sync: push to $BASE refused ($RANGE)"; summary "$TABLE"
      output result push-refused
      exit 0
    fi
    git checkout --quiet -B "$BASE" "$MERGE_SHA"
    git branch -D "$BRANCH" >/dev/null
    say "main fast-forwarded to $MERGE_SHA"
  else
    say "dry run: all green, would push $MERGE_SHA to $BASE (kept on local branch $BRANCH)"
  fi
  summary "### Upstream sync: merged $RANGE"; summary "$TABLE"
  echo "$TABLE"
  output result merged
  exit 0
fi

say "failed: ${FAILED[*]}"
echo "$TABLE"
BODY="$(printf 'Merging upstream Horizon `%s` into `%s` is clean but these checks failed: **%s**. Main was not touched.\n\n%s\n\n```\n%s\n```\n\nFix on this branch, then merge. See docs/perforizon-sync.md.\n\nOpened by the upstream-sync workflow.' "$SHORT" "$BASE" "${FAILED[*]}" "$TABLE" "$(tail -c 12000 "$LOGS/failures.txt")")"
if have_gh; then
  git push --force origin "$BRANCH"
  OLD="$(gh pr list --state open --search 'in:title "Upstream sync: checks failed"' --json number,headRefName --jq ".[] | select(.headRefName != \"$BRANCH\" and (.headRefName|startswith(\"sync/upstream-\"))) | .number")"
  for n in $OLD; do gh pr close "$n" --comment "Superseded by $BRANCH." >/dev/null || true; done
  NUM="$(gh pr list --state open --head "$BRANCH" --json number --jq '.[0].number // empty')"
  if [ -n "$NUM" ]; then
    gh pr edit "$NUM" --body "$BODY" >/dev/null
  else
    gh pr create --base "$BASE" --head "$BRANCH" --title "Upstream sync: checks failed" --body "$BODY" >/dev/null || say "could not open the PR"
  fi
elif [ "$PUSH" = 0 ]; then
  say "dry run: would push $BRANCH and open a PR; branch kept locally"
  echo "$BODY"
fi
summary "### Upstream sync: checks failed ($RANGE)"; summary "$TABLE"
output result checks-failed
exit 0
