# Keeping Perforizon in step with Horizon

Perforizon is Horizon plus a set of optimizations. When Horizon's `main` gets new commits, the
`upstream-sync` workflow merges them into Perforizon's `main` by itself, but only if the merge is clean and every
check passes. Nobody has to be involved in that case. It never builds, tags or publishes a release.

Files: `.github/workflows/upstream-sync.yml` (the schedule), `tools/ci/upstream_sync.sh` (all the logic, also
runnable by hand).

## What a run does

1. Adds the `upstream` remote (`https://github.com/horizon-rust/horizon.git`) if missing and fetches `upstream/main`.
2. If `main` already contains `upstream/main`: stops quietly.
3. Creates `sync/upstream-<short sha>` from `main` and runs `git merge --no-commit upstream/main`.
4. **Conflicts:** the merge is aborted, nothing is pushed, and one issue titled `Upstream sync blocked: conflicts`
   (label `upstream-sync`, created if missing) lists the conflicted files and the upstream range. An existing open
   issue is updated instead of opening a second one.
5. **Clean:** `GITHUB_TOKEN` cannot push changes to `.github/workflows`, so our `.github/workflows` is restored
   and workflow files upstream added are removed before the merge commit
   (`Merge upstream Horizon main (<sha>) into Perforizon`). Upstream workflow changes are therefore never taken;
   port one by hand if you want it.
6. Runs the checks (the commands come from `ci.yml` and `tools/pr_ready.py`; warnings are denied there too):

   | Check | Command |
   | --- | --- |
   | rustfmt | `cargo fmt --all -- --check` |
   | clippy | `cargo clippy -p horizon-client -p horizon-client-lib -p horizon-client-render -p horizon-protocol -p horizon-world --all-targets --locked -- -D warnings` |
   | tests | `cargo test --locked -p horizon-client-render -p horizon-client -p horizon-client-lib -p horizon-protocol --lib -- --skip <SKIP_TESTS>` |
   | build | `cargo build --locked --profile fast -p horizon-client` |
   | public tree | `python3 tools/check_public_tree.py` |
   | worldgen identical | `crates/horizon-world/src/worldgen` differs from `upstream/main` only by `decoration_order.rs` |
   | launcher files | `launcher/src/palette.rs` and `launcher/src/tui.rs` exist |
   | launcher build | `cargo build --locked` in `launcher/` |

7. **All green:** `main` is fast-forwarded to the merge commit (a plain push, no force).
   **A check failed:** the branch `sync/upstream-<sha>` is pushed and a PR `Upstream sync: checks failed` is opened
   with the tail of the failing logs. `main` is not touched. An older such PR for a previous upstream sha is closed
   as superseded.
8. The result table goes to the job summary (and `$GITHUB_STEP_SUMMARY`).

A sync that already failed is not redone every hour: while `sync/upstream-<sha>` exists on the fork the run stops
early. A newer upstream commit gets a new branch.

## Knobs

- **Schedule:** `cron: '17 * * * *'` in the workflow (hourly). Runs never overlap (`concurrency: upstream-sync`).
- **Dry run:** `gh workflow run upstream-sync -f dry_run=true`, or locally `bash tools/ci/upstream_sync.sh --no-push`.
  It merges and checks on a local `sync/upstream-<sha>` branch and pushes nothing. Local runs need Rust and, on
  Linux, `libasound2-dev libudev-dev` (plus GPUI's headers for the launcher build).
- **Force:** `gh workflow run upstream-sync -f force=true` redoes a sync whose branch already exists (force-pushes
  the branch and refreshes the PR). Use it after fixing a runner problem.
- **Known-failing tests:** the `SKIP_TESTS` array at the top of the script. Currently
  `lan::io_pool::tests::a_close_ends_a_disconnect_the_client_does_not_take` (fails on Windows and on upstream too).
  Add the full test path of any other test that fails for reasons unrelated to Perforizon.
- **Allowed worldgen differences:** `WORLDGEN_ALLOWED` in the script. World generation must stay bit-identical
  to Horizon, so the list is one file.
- **Pause it:** disable the workflow in the Actions tab.
- `SYNC_SKIP_CHECKS=1` skips the cargo and tree checks. It exists only to test the script; do not set it in CI.

If branch protection blocks the push to `main`, the push is refused and the run reports `push-refused` (it never force-pushes `main`): allow `github-actions` to
bypass it, or merge by hand. A push is also refused when `main` moved while the checks ran; the next run redoes the sync.
PR creation by the workflow needs Settings > Actions > General > "Allow GitHub Actions to create and approve pull requests".

## Blocked sync: resolving by hand

```sh
git checkout main && git pull origin main
git remote add upstream https://github.com/horizon-rust/horizon.git   # once
git fetch upstream
git checkout -b sync/manual main
git merge upstream/main                      # stops at the conflicts
git status                                   # "both modified" files
# edit the files; keep the Perforizon optimization and take Horizon's new code around it
git checkout HEAD -- .github/workflows       # keep our workflows
git add -A && git commit                     # "Merge upstream Horizon main into Perforizon"
python3 tools/pr_ready.py --quick            # fmt, clippy, tests
git diff --stat upstream/main HEAD -- crates/horizon-world/src/worldgen   # only decoration_order.rs
git push origin sync/manual:main             # or open a PR
```

Close the `Upstream sync blocked: conflicts` issue afterwards. For a `checks failed` PR, check out its branch, fix,
push, and merge the PR.

## What it cannot do

- **No benchmark.** GitHub's runners have no GPU, so the sync proves that the merge compiles, lints and passes the
  unit tests, not that it is still fast. After a sync, compare on a machine with a GPU, same settings, runs one after
  the other (method in [perforizon-benchmarks.md](perforizon-benchmarks.md)):

  ```sh
  git worktree add ../horizon-upstream upstream/main
  cargo build --profile fast -p horizon-client                                  # in each tree
  target/fast/horizon-client --offscreen --benchmark standard --render-distance 32
  ```

  Run it in the upstream tree and in Perforizon, three runs each, at render distance 16 and 32; the reports land in
  `<game dir>/benchmarks/`. Compare average fps and the 1% and 0.1% lows per scene.
- **No parity harnesses.** They need the player's own Minecraft install (`.local/`), which runners do not have.
- **No release.** Releases are never automatic: a merged sync only updates `main`. Tagging and uploading a release
  stays a manual step ([perforizon-install.md](perforizon-install.md)).
- **No semantic merge.** A clean textual merge can still change behaviour or speed. Compile, lint and unit-test
  failures are caught; a silent slowdown is not, hence the local benchmark above.
